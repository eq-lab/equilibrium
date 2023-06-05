// This file is part of Equilibrium.

// Copyright (C) 2023 EQ Lab.
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # Equilibrium Rate Pallet
//!
//! Equilibrium's Rate Pallet is a Substrate module for processing different fee payments and keeping account balances up to date.
//!
//! The rates pallet adds the ability to write off the interest rate from borrower account balances using following 2 functions:
//!
//! reinit - any user can call it, it is also called upon any client action (transfer, deposit, etc.)
//!
//!
//! unsafe_reinit - only validators can call it, all validators perform (off-chain worker) recalculation of the interest rate according to their set of accounts and call unsafe_reinit if accrued interest has become more than the minimum value (MinSurplus setting), validators do not pay fees when they call this function.
//!
//!
//! The function sequence of reinit() is the following:
//!
//! calculate + write-off of the accrued interest.
//! Margincall an account if needed. (write off collateral and debt, currently at 105% minLTV there is an effective penalty of 5%, we want to get rid of it in subsequent updates/releases).
//! Perform an action (including price updates).

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
// #![deny(warnings)]

mod mock;
mod tests;

pub mod benchmarking;
pub mod rate;
mod rate_tests;
pub mod reinit_extension;
pub mod weights;
use eq_primitives::balance_number::EqFixedU128;
use eq_utils::fixed::{fixedi128_from_balance, fixedi128_from_i64f64};
use financial_pallet::FinancialMetrics;
pub use weights::WeightInfo;

use eq_primitives::asset::{Asset, AssetType, EQD};
use eq_primitives::balance::ration;
use eq_primitives::UserGroup;
use eq_primitives::{
    asset::AssetGetter,
    bailsman_redistribute_weight::RedistributeWeightInfo,
    balance::{BalanceGetter, BalanceRemover, DepositReason, EqCurrency, WithdrawReason},
    offchain_batcher::*,
    Aggregates, BailsmanManager, EqBuyout, LendingAssetRemoval, LendingPoolManager,
    MarginCallManager, MarginState, PriceGetter, SignedBalance, UpdateTimeManager,
};
use eq_utils::{
    eq_ensure,
    fixed::{balance_from_fixedi128, eq_fixedu128_from_balance, fixedi128_from_fixedi64},
    ok_or_error,
};
use frame_support::traits::ExistenceRequirement;
use frame_support::{
    codec::{Decode, Encode},
    dispatch::DispatchError,
    traits::{Get, OnKilledAccount, OnNewAccount, OneSessionHandler, UnixTime},
    Parameter,
};
use frame_system as system;
use rate::InterestRateError;
use sp_application_crypto::RuntimeAppPublic;
use sp_core::crypto::KeyTypeId;
use sp_runtime::traits::One;
use sp_runtime::{
    traits::{AccountIdConversion, AtLeast32BitUnsigned, MaybeSerializeDeserialize, Member, Zero},
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        ValidTransaction,
    },
    ArithmeticError, DispatchResult, FixedI128, FixedPointNumber, RuntimeDebug,
};
use sp_std::convert::{TryFrom, TryInto};
use sp_std::prelude::*;
use system::offchain::{SendTransactionTypes, SubmitTransaction};
use system::{ensure_none, ensure_root, ensure_signed};

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"rate");
const DB_PREFIX: &[u8] = b"eq-rate/";

pub type AuthIndex = u32;
pub type OffchainResult<A> = Result<A, OffchainErr>;

/// Module for crypto signatures
pub mod ed25519 {
    pub use super::KEY_TYPE;
    mod app_ed25519 {
        use core::convert::TryFrom;
        use sp_application_crypto::{app_crypto, ed25519};
        app_crypto!(ed25519, super::KEY_TYPE);
    }

    sp_application_crypto::with_pair! {
        pub type AuthorityPair = app_ed25519::Pair;
    }
    pub type AuthoritySignature = app_ed25519::Signature;
    pub type AuthorityId = app_ed25519::Public;
}

/// Request data for offchain signing
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct OperationRequest<AccountId, BlockNumber>
where
    AccountId: PartialEq + Eq + Decode + Encode,
    BlockNumber: Decode + Encode,
{
    pub account: Option<AccountId>,
    /// An index of the authority on the list of validators
    pub authority_index: AuthIndex,
    /// The length of session validator set
    pub validators_len: u32,
    /// Number of a block
    pub block_num: BlockNumber,
    /// Determines whether this request has the higher priority:
    pub higher_priority: bool,
}

/// Request data for offchain signing
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct BalanceRemovalRequest<AccountId, Asset, Balance, BlockNumber>
where
    AccountId: PartialEq + Eq + Decode + Encode,
    Balance: Into<eq_primitives::balance::Balance> + Copy,
    BlockNumber: Decode + Encode,
{
    pub account: AccountId,
    pub asset: Asset,
    pub amount: Balance,
    /// An index of the authority on the list of validators
    pub authority_index: AuthIndex,
    /// The length of session validator set
    pub validators_len: u32,
    /// Number of a block
    pub block_num: BlockNumber,
    /// Determines whether this request has the higher priority:
    pub higher_priority: bool,
}

use crate::rate::InterestRateCalculator;
use eq_primitives::financial_storage::FinancialStorage;
use eq_primitives::BalanceChange;
pub use pallet::*;
use sp_std::fmt::Debug;

#[derive(Debug)]
pub(crate) struct Fee<Balance: Debug + Member + AtLeast32BitUnsigned + Copy + Zero> {
    basic_asset: Asset,
    treasury: Balance,
    bailsman: Balance,
    lender: Vec<(Asset, Balance)>,
}

impl<Balance> Fee<Balance>
where
    Balance: Debug + Member + AtLeast32BitUnsigned + Copy + Zero,
{
    pub fn total_fee(&self) -> Balance {
        self.bailsman
            + self.treasury
            + self
                .lender
                .iter()
                .fold(Balance::zero(), |acc, (_, b)| acc + *b)
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use eq_primitives::subaccount::SubaccountsManager;
    use financial_pallet::Financial;
    use frame_support::dispatch::PostDispatchInfo;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::WithdrawReasons;
    use frame_support::PalletId;
    use frame_system::pallet_prelude::*;
    use sp_arithmetic::Permill;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + SendTransactionTypes<Call<Self>>
        + pallet_session::Config
        + authorship::Config
        + eq_assets::Config
    {
        type AutoReinitToggleOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Timestamp provider
        type UnixTime: UnixTime;
        /// Numerical representation of stored balances
        type Balance: Member
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Parameter
            + Default
            + TryFrom<eq_primitives::balance::Balance>
            //+ From<u128>
            + Into<eq_primitives::balance::Balance>
            + Copy;
        /// Gets information about account balances
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;
        /// Deletes balances from storage while asset removal
        type BalanceRemover: BalanceRemover<Self::AccountId>;
        /// Used to deal with Assets
        type AssetGetter: AssetGetter;
        /// Gets currency prices from oracle
        type PriceGetter: PriceGetter;
        /// Used to work with `TotalAggregates` storing aggregated collateral and debt
        type Aggregates: Aggregates<Self::AccountId, Self::Balance>;
        /// The identifier type for an authority.
        type AuthorityId: Member + Parameter + RuntimeAppPublic + Ord + MaybeSerializeDeserialize;
        /// Used to integrate margin call logic
        type MarginCallManager: MarginCallManager<Self::AccountId, Self::Balance>;
        /// Used to integrate bailsman operations
        type BailsmanManager: BailsmanManager<Self::AccountId, Self::Balance>;
        /// Minimum new debt for system reinit
        #[pallet::constant]
        type MinSurplus: Get<Self::Balance>;
        /// Minimum temp bailsmen balances for Bailsman pallet reinit
        #[pallet::constant]
        type MinTempBailsman: Get<Self::Balance>;
        /// Manager for treasury basic asset exchanging transactions
        type EqBuyout: EqBuyout<Self::AccountId, Self::Balance>;
        /// Gets bailsman module account for margincall and fee transfers
        #[pallet::constant]
        type BailsmanModuleId: Get<frame_support::PalletId>;
        /// Integrates balances operations of `eq-balances` pallet
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        /// Used for subaccounts checks
        type SubaccountsManager: SubaccountsManager<Self::AccountId>;
        /// For unsigned transaction priority calculation
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;
        /// Lower bound for scaling risk model
        #[pallet::constant]
        type RiskLowerBound: Get<FixedI128>;
        /// Upper bound for scaling risk model
        #[pallet::constant]
        type RiskUpperBound: Get<FixedI128>;
        /// Number of standard deviations to consider when stress testing
        #[pallet::constant]
        type RiskNSigma: Get<FixedI128>;
        /// Pricing model scaling factor
        #[pallet::constant]
        type Alpha: Get<FixedI128>;
        /// Interface for Financial pallet calculations (asset volatilities, correlations and etc.)
        type Financial: Financial<Asset = Asset, Price = substrate_fixed::types::I64F64>;
        /// Interface for accessing Financial pallet storage
        type FinancialStorage: FinancialStorage<
            Asset = Asset,
            Price = substrate_fixed::types::I64F64,
        >;
        /// Treasury fee rate
        #[pallet::constant]
        type TreasuryFee: Get<Permill>;
        /// Fee part that stays in Treasury pallet
        #[pallet::constant]
        type WeightFeeTreasury: Get<u32>;
        /// Fee part that goes to validator
        #[pallet::constant]
        type WeightFeeValidator: Get<u32>;
        /// Base bailsman fee
        #[pallet::constant]
        type BaseBailsmanFee: Get<Permill>;
        /// Base lender fee
        #[pallet::constant]
        type BaseLenderFee: Get<Permill>;
        /// Lender part of prime rate
        #[pallet::constant]
        type LenderPart: Get<Permill>;
        /// For transferring fee to treasury
        #[pallet::constant]
        type TreasuryModuleId: Get<PalletId>;
        /// For transferring fee to lending pool
        #[pallet::constant]
        type LendingModuleId: Get<PalletId>;
        /// For notifying LendingPoool about new rewards
        type LendingPoolManager: LendingPoolManager<Self::Balance>;
        /// Used to clear Lenders storage while asset removal
        type LendingAssetRemoval: LendingAssetRemoval<Self::AccountId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        /// Weight information of bailsman redistribution
        type RedistributeWeightInfo: RedistributeWeightInfo;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Request to check account balance for margin call and withdraw fees.
        ///
        /// The dispatch origin for this call must be _None_ (unsigned transaction).

        /// Parameters:
        ///  - `request`: OperationRequest.
        ///  - `_signature`: OperationRequest signature
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::reinit())]
        pub fn reinit(
            origin: OriginFor<T>,
            request: OperationRequest<T::AccountId, T::BlockNumber>,
            // since signature verification is done in `validate_unsigned`
            // we can skip doing it here again.
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;
            eq_ensure!(
                Self::auto_reinit_enabled(),
                Error::<T>::AutoReinitIsDisabled,
                target: "eq_rate",
                "{}:{}. AutoReinit is disabled .",
                file!(),
                line!(),
            );
            eq_ensure!(
                &request.account.is_some(),
                Error::<T>::ValidationError,
                target: "eq_rate",
                "{}:{}. Account is none. Validation not passed",
                file!(),
                line!()
            );

            let account = request.account.unwrap();
            Self::do_reinit(&account)?;

            let weight = if Self::is_bailsman(&account) {
                <T as Config>::WeightInfo::reinit()
                    + <T as Config>::RedistributeWeightInfo::redistribute(30)
            } else {
                <T as Config>::WeightInfo::reinit()
            };

            Ok(PostDispatchInfo {
                actual_weight: Some(weight),
                pays_fee: Pays::Yes,
            })
        }

        /// Request to delete an account and all of it subaccounts
        ///
        /// The dispatch origin for this call must be _None_ (unsigned transaction).
        ///
        /// Parameters:
        ///  - `request`: OperationRequest.
        ///  - `_signature`: OperationRequest signature
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::delete_account())]
        pub fn delete_account(
            origin: OriginFor<T>,
            request: OperationRequest<T::AccountId, T::BlockNumber>,
            // since signature verification is done in `validate_unsigned`
            // we can skip doing it here again.
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            eq_ensure!(
                request.account.is_some(),
                Error::<T>::ValidationError,
                target: "eq_rate",
                "{}:{}. Could not delete account. Account is not submitted",
                file!(),
                line!(),
            );

            T::EqCurrency::delete_account(&request.account.unwrap())?;
            Ok(().into())
        }

        /// Request to deposit asset for account
        ///
        /// The dispatch origin for this call must be _None_ (unsigned transaction).
        ///
        /// Parameters:
        ///  - `request`: OperationRequest.
        ///  - `_signature`: OperationRequest signature
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::delete_account())]
        pub fn deposit(
            origin: OriginFor<T>,
            request: BalanceRemovalRequest<T::AccountId, Asset, T::Balance, T::BlockNumber>,
            // since signature verification is done in `validate_unsigned`
            // we can skip doing it here again.
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            let assets_to_remove = eq_assets::AssetsToRemove::<T>::get().unwrap_or(Vec::new());
            eq_ensure!(
                assets_to_remove.into_iter().find(|&asset| asset == request.asset).is_some(),
                Error::<T>::AssetNotInRemovalQueue,
                target: "eq_rate",
                "{}:{}. Could not deposit asset, which is not in removal queue",
                file!(),
                line!(),
            );

            T::EqCurrency::deposit_into_existing(
                &request.account,
                request.asset,
                request.amount,
                Some(DepositReason::AssetRemoval),
            )?;

            T::BalanceRemover::remove_asset(request.account, &request.asset)?;

            Ok(().into())
        }

        /// Request to burn asset for account
        ///
        /// The dispatch origin for this call must be _None_ (unsigned transaction).
        ///
        /// Parameters:
        ///  - `request`: OperationRequest.
        ///  - `_signature`: OperationRequest signature
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::delete_account())]
        pub fn withdraw(
            origin: OriginFor<T>,
            request: BalanceRemovalRequest<T::AccountId, Asset, T::Balance, T::BlockNumber>,
            // since signature verification is done in `validate_unsigned`
            // we can skip doing it here again.
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            let assets_to_remove = eq_assets::AssetsToRemove::<T>::get().unwrap_or(Vec::new());
            eq_ensure!(
                assets_to_remove.into_iter().find(|&asset| asset == request.asset).is_some(),
                Error::<T>::AssetNotInRemovalQueue,
                target: "eq_rate",
                "{}:{}. Could not withdraw asset, which is not in removal queue",
                file!(),
                line!(),
            );

            T::LendingAssetRemoval::remove_from_lenders(&request.asset, &request.account);

            T::EqCurrency::withdraw(
                &request.account,
                request.asset,
                request.amount,
                true,
                Some(WithdrawReason::AssetRemoval),
                WithdrawReasons::empty(),
                ExistenceRequirement::AllowDeath,
            )?;

            T::BalanceRemover::remove_asset(request.account, &request.asset)?;

            Ok(().into())
        }

        /// Request to delete an account and all of it subaccounts
        /// This function is used by any user and executed by substrate transaction
        ///
        /// The dispatch origin for this call must be `Signed` by the transactor.
        ///
        /// Parameters:
        ///  - `account`: Account that should be checked for deletion.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::delete_account_external())]
        pub fn delete_account_external(
            origin: OriginFor<T>,
            account: <T as system::Config>::AccountId,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_signed(origin)?;

            T::EqCurrency::delete_account(&account)?;
            Ok(().into())
        }

        /// Request to check account balance for margin call and withdraw fees.
        /// This function is used by any user and executed by substrate transaction
        ///
        /// The dispatch origin for this call must be `Signed` by the transactor.
        ///
        /// Parameters:
        ///  - `account`: Account that should be checked for margin call and charged fee.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::reinit_external())]
        pub fn reinit_external(
            origin: OriginFor<T>,
            owner: <T as system::Config>::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            // no need to ckeck reinit is needed, because of signed (with fees) transactions
            Self::do_reinit(&owner)?;
            Ok(().into())
        }

        /// Function used in test builds for time move
        #[pallet::call_index(6)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_now_millis_offset(
            origin: OriginFor<T>,
            offset: u64,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let current_offset = <NowMillisOffset<T>>::get();
            eq_ensure!(
                offset > current_offset,
                Error::<T>::InvalidOffset,
                target: "eq_rate",
                "{}:{}. Offset to set is lower than current. Offset: {:?}, current offset: {:?}.",
                file!(),
                line!(),
                offset,
                current_offset
            );
            <NowMillisOffset<T>>::put(offset);
            log::trace!(target: "eq_rate", "Time offset set to {} seconds", offset / 1000);
            Ok(().into())
        }

        /// Enables or disables offchain workers. `true` to enable offchain worker
        /// operations, `false` to disable them.
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::set_auto_reinit_enabled())]
        pub fn set_auto_reinit_enabled(
            origin: OriginFor<T>,
            enabled: bool,
        ) -> DispatchResultWithPostInfo {
            T::AutoReinitToggleOrigin::ensure_origin(origin)?;
            <AutoReinitEnabled<T>>::put(enabled);
            log::trace!(target: "eq_rate", "Offchain worker status set to {}", enabled);
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Runs after every block.
        fn offchain_worker(now: T::BlockNumber) {
            let worker_allowed = <AutoReinitEnabled<T>>::get();

            // Only send messages if we are a potential validator.
            if sp_io::offchain::is_validator() && worker_allowed {
                let lock_res = eq_utils::offchain::accure_lock(DB_PREFIX, || {
                    // doesn't return error anyway, all errors are logged inside `execute_batch`
                    let _ =
                        Self::execute_batch(now, Self::check_accounts_for_single_auth, "eq-rate");
                });

                match lock_res {
                    eq_utils::offchain::LockedExecResult::Executed => {
                        log::trace!(target: "eq_rate", "eq_rate offchain_worker:executed");
                    }
                    eq_utils::offchain::LockedExecResult::Locked => {
                        log::trace!(target: "eq_rate", "eq_rate offchain_worker:locked");
                    }
                }
            } else {
                log::trace!(
                    target: "eq_rate",
                    "Skipping reinit at {:?}. Not a validator.",
                    now,
                )
            }
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Error used during time offset in test builds
        InvalidOffset,
        /// Auto reinit is disabled
        AutoReinitIsDisabled,
        /// This method is not allowed in production
        MethodNotAllowed,
        /// Prices are outdated
        NoPrices,
        /// Financial parameters are outdated
        NoFinancial,
        /// Some external error while rate calculation
        ExternalError,
        /// Math error in rate calculation
        MathError,
        /// Error in rate calculation
        ValueError,
        /// Validation error
        ValidationError,
        /// Last update in fututure
        LastUpdateInFuture,
        /// Asset is not in removal queue
        AssetNotInRemovalQueue,
    }

    /// Pallet storage for keys
    #[pallet::storage]
    #[pallet::getter(fn keys)]
    pub type Keys<T: Config> = StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

    /// Pallet storage - last update timestamps in seconds for each `AccountId` that has balances
    #[pallet::storage]
    #[pallet::getter(fn last_fee_update)]
    pub type LastFeeUpdate<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

    /// Pallet storage used for time offset in test builds. Disabled by "production" feature.
    #[pallet::storage]
    #[pallet::getter(fn now_millis_offset)]
    pub type NowMillisOffset<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::type_value]
    pub fn DefaultForAutoReinitEnabled() -> bool {
        true
    }

    /// Stores flag for on/off setting for offchain workers (reinits)
    #[pallet::storage]
    #[pallet::getter(fn auto_reinit_enabled)]
    pub type AutoReinitEnabled<T: Config> =
        StorageValue<_, bool, ValueQuery, DefaultForAutoReinitEnabled>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub keys: Vec<T::AuthorityId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                keys: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |config| {
                Pallet::<T>::initialize_keys(&config.keys);
            };
            extra_genesis_builder(self);
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            const INVALID_VALIDATORS_LEN: u8 = 10;
            const CHECK_NOT_PASSED: u8 = 20;

            let check_signature = |validators_len: u32,
                                   authority_index: usize,
                                   encoded_heartbeat: &[u8],
                                   signature: &<T::AuthorityId as RuntimeAppPublic>::Signature|
             -> Result<(), InvalidTransaction> {
                let keys = Keys::<T>::get();
                if keys.len() as u32 != validators_len {
                    return Err(InvalidTransaction::Custom(INVALID_VALIDATORS_LEN));
                }
                let authority_id = match keys.get(authority_index) {
                    Some(id) => id,
                    None => return Err(InvalidTransaction::BadProof),
                };

                // check signature (this is expensive so we do it last).
                let signature_valid = authority_id.verify(&encoded_heartbeat, &signature);

                if !signature_valid {
                    return Err(InvalidTransaction::BadProof);
                }

                Ok(())
            };

            match call {
                Call::reinit { request, signature } => {
                    check_signature(
                        request.validators_len,
                        request.authority_index as usize,
                        &request.encode(),
                        &signature,
                    )?;

                    if !Self::need_to_reinit(request.account.as_ref().unwrap()) {
                        return InvalidTransaction::Custom(CHECK_NOT_PASSED).into();
                    }

                    let priority = if request.higher_priority {
                        T::UnsignedPriority::get() + 1
                    } else {
                        T::UnsignedPriority::get()
                    };
                    ValidTransaction::with_tag_prefix("EqFee")
                        .priority(priority)
                        .and_provides(request.account.clone().unwrap())
                        .longevity(5)
                        .propagate(true)
                        .build()
                }
                Call::delete_account { request, signature } => {
                    check_signature(
                        request.validators_len,
                        request.authority_index as usize,
                        &request.encode(),
                        &signature,
                    )?;
                    // request.account checked in check_signature
                    if !T::EqCurrency::can_be_deleted(request.account.as_ref().unwrap())
                        .unwrap_or(false)
                    {
                        return InvalidTransaction::Custom(CHECK_NOT_PASSED).into();
                    }

                    let priority = if request.higher_priority {
                        T::UnsignedPriority::get() + 1
                    } else {
                        T::UnsignedPriority::get()
                    };
                    ValidTransaction::with_tag_prefix("DelAcc")
                        .priority(priority)
                        .and_provides(request.account.clone().unwrap())
                        .longevity(5)
                        .propagate(true)
                        .build()
                }
                Call::withdraw { request, signature } => {
                    check_signature(
                        request.validators_len,
                        request.authority_index as usize,
                        &request.encode(),
                        &signature,
                    )?;

                    let assets_to_remove =
                        eq_assets::AssetsToRemove::<T>::get().unwrap_or(Vec::new());
                    if assets_to_remove
                        .into_iter()
                        .find(|&asset| asset == request.asset)
                        .is_none()
                    {
                        return InvalidTransaction::Custom(CHECK_NOT_PASSED).into();
                    }

                    let priority = if request.higher_priority {
                        T::UnsignedPriority::get() + 1
                    } else {
                        T::UnsignedPriority::get()
                    };
                    ValidTransaction::with_tag_prefix("Withdraw")
                        .priority(priority)
                        .and_provides(request.account.clone())
                        .longevity(5)
                        .propagate(true)
                        .build()
                }
                Call::deposit { request, signature } => {
                    check_signature(
                        request.validators_len,
                        request.authority_index as usize,
                        &request.encode(),
                        &signature,
                    )?;

                    let assets_to_remove =
                        eq_assets::AssetsToRemove::<T>::get().unwrap_or(Vec::new());
                    if assets_to_remove
                        .into_iter()
                        .find(|&asset| asset == request.asset)
                        .is_none()
                    {
                        return InvalidTransaction::Custom(CHECK_NOT_PASSED).into();
                    }

                    let priority = if request.higher_priority {
                        T::UnsignedPriority::get() + 1
                    } else {
                        T::UnsignedPriority::get()
                    };
                    ValidTransaction::with_tag_prefix("Deposit")
                        .priority(priority)
                        .and_provides(request.account.clone())
                        .longevity(5)
                        .propagate(true)
                        .build()
                }
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}

impl<T: Config> UpdateTimeManager<T::AccountId> for Pallet<T> {
    fn set_last_update(account_id: &T::AccountId) {
        let now = <Self as UnixTime>::now().as_secs();
        <LastFeeUpdate<T>>::insert(account_id, now);
    }

    #[cfg(not(feature = "production"))]
    fn set_last_update_timestamp(account_id: &T::AccountId, timestamp_ms: u64) {
        <LastFeeUpdate<T>>::insert(account_id, timestamp_ms);
    }

    fn remove_last_update(account_id: &T::AccountId) {
        <LastFeeUpdate<T>>::remove(account_id);
    }
}

impl<T: Config> Pallet<T> {
    /// used only in genesis
    fn initialize_keys(keys: &[T::AuthorityId]) {
        if !keys.is_empty() {
            assert!(Keys::<T>::get().is_empty(), "Keys are already initialized!");
            Keys::<T>::put(keys);
        }
    }

    /// -- calls reinit for account that acc_index mod validators_len == authority_index
    /// and need to be reinited (fee is more than MinSurplus or position should be margincalled)
    /// -- calls delete account for account that acc_index mod validators_len == authority_index and
    /// position balance is less than ExistentialDeposit and can be deleted (ref count is zero)
    /// -- calls bailsman reinit if block_number mod validators_len == authority_index
    /// (we don't need several reinits in one block) if bailsman temp balance is more than MinTempBailsman
    /// In theory one node may have more than one suitable key
    fn check_accounts_for_single_auth(
        authority_index: u32,
        key: T::AuthorityId,
        block_number: T::BlockNumber,
        validators_len: u32,
    ) -> OffchainResult<()> {
        // calc fee for acc % len = index
        let bailsman_acc_id: T::AccountId = T::BailsmanModuleId::get().into_account_truncating();
        let distribution_acc_id: T::AccountId =
            eq_primitives::DISTRIBUTION_ACC.into_account_truncating();

        let assets_to_remove = eq_assets::AssetsToRemove::<T>::get().unwrap_or(Vec::new());
        for (index, account_id, balances) in T::BalanceGetter::iterate_balances()
            .into_iter()
            .enumerate()
            .filter_map(|(index, (account_id, balances))| {
                // idx % len = [auth_idx-1, auth_idx, auth_idx+1]
                if (index as u32) % validators_len + 1 >= authority_index
                    && (index as u32) % validators_len <= authority_index + 1
                {
                    Some((index, account_id, balances))
                } else {
                    None
                }
            })
        {
            if account_id != bailsman_acc_id && account_id != distribution_acc_id {
                log::trace!(
                    target: "eq_rate",
                    "eq-rate. check_accounts_for_single_auth: account_id {:?}",
                    account_id
                );
                if Self::need_to_reinit(&account_id) {
                    Self::submit_reinit_unsigned(
                        authority_index,
                        &key,
                        block_number,
                        validators_len,
                        &account_id,
                        (index as u32) % validators_len == authority_index,
                    )?;
                }

                if T::EqCurrency::can_be_deleted(&account_id).unwrap_or(false) {
                    Self::submit_delete_account_unsigned(
                        authority_index,
                        &key,
                        block_number,
                        validators_len,
                        &account_id,
                        (index as u32) % validators_len == authority_index,
                    )?;
                }
            }

            for (asset, balance) in balances.into_iter().filter(|(asset, _)| {
                assets_to_remove
                    .iter()
                    .any(|asset_to_remove| *asset == *asset_to_remove)
            }) {
                let total_debt_is_zero = T::Aggregates::get_total(UserGroup::Balances, asset)
                    .debt
                    .is_zero();
                if balance.is_negative() || (balance.is_positive() && total_debt_is_zero) {
                    Self::submit_balance_removal_unsigned(
                        authority_index,
                        &key,
                        block_number,
                        validators_len,
                        &account_id,
                        asset,
                        balance,
                        false,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn submit_balance_removal_unsigned(
        authority_index: u32,
        key: &T::AuthorityId,
        block_number: T::BlockNumber,
        validators_len: u32,
        account_id: &T::AccountId,
        asset: Asset,
        balance: SignedBalance<<T as Config>::Balance>,
        higher_priority: bool,
    ) -> OffchainResult<()> {
        let request = BalanceRemovalRequest::<T::AccountId, Asset, T::Balance, T::BlockNumber> {
            account: account_id.clone(),
            asset: asset,
            amount: balance.abs(),
            authority_index,
            validators_len,
            block_num: block_number,
            higher_priority,
        };
        let signature = key.sign(&request.encode()).unwrap();
        let debug_sign = signature.clone();
        let debug_data = request.clone();
        let call = if balance.is_positive() {
            Call::withdraw { request, signature }
        } else {
            Call::deposit { request, signature }
        };

        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(|_| {
            log::error!(
                "{}:{}. Submit withdraw error. Signature: {:?}, request account: {:?}, \
                                                            asset: {:?}, amount: {:?}, \
                            authority_index: {:?}, validators_len: {:?}, block_num:{:?}.",
                file!(),
                line!(),
                debug_sign,
                &debug_data.account,
                &debug_data.asset,
                &debug_data.amount,
                &debug_data.authority_index,
                &debug_data.validators_len,
                &debug_data.block_num
            );
            OffchainErr::SubmitTransaction
        })
    }

    fn submit_delete_account_unsigned(
        authority_index: u32,
        key: &T::AuthorityId,
        block_number: T::BlockNumber,
        validators_len: u32,
        account_id: &T::AccountId,
        higher_priority: bool,
    ) -> OffchainResult<()> {
        let call_data = OperationRequest::<T::AccountId, T::BlockNumber> {
            account: Some(account_id.clone()),
            authority_index,
            validators_len,
            block_num: block_number,
            higher_priority,
        };

        let option_signature = key.sign(&call_data.encode());
        let signature = ok_or_error!(
            option_signature,
            OffchainErr::FailedSigning,
            "{}:{}. Couldn't sign acc delete request. Key: {:?}, request account: {:?}, \
                    authority_index: {:?}, validators_len: {:?}, block_num:{:?}.",
            file!(),
            line!(),
            key,
            &call_data.account,
            &call_data.authority_index,
            &call_data.validators_len,
            &call_data.block_num
        )?;
        let debug_data = call_data.clone();
        let debug_sign = signature.clone();
        let call = Call::delete_account {
            request: call_data,
            signature,
        };

        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(|_| {
            log::error!(
                "{}:{}. Submit delete acc error. Signature: {:?}, request account: {:?}, \
                            authority_index: {:?}, validators_len: {:?}, block_num:{:?}.",
                file!(),
                line!(),
                debug_sign,
                &debug_data.account,
                &debug_data.authority_index,
                &debug_data.validators_len,
                &debug_data.block_num
            );
            OffchainErr::SubmitTransaction
        })
    }

    fn submit_reinit_unsigned(
        authority_index: u32,
        key: &T::AuthorityId,
        block_number: T::BlockNumber,
        validators_len: u32,
        account_id: &T::AccountId,
        higher_priority: bool,
    ) -> OffchainResult<()> {
        let reinit_data = OperationRequest::<T::AccountId, T::BlockNumber> {
            account: Some(account_id.clone()),
            authority_index,
            validators_len,
            block_num: block_number,
            higher_priority,
        };

        let option_signature = key.sign(&reinit_data.encode());
        let signature = ok_or_error!(
            option_signature,
            OffchainErr::FailedSigning,
            "{}:{}. Couldn't sign. Key: {:?}, request account: {:?}, authority_index: {:?}, \
                    validators_len: {:?}, block_num:{:?}.",
            file!(),
            line!(),
            key,
            &reinit_data.account,
            &reinit_data.authority_index,
            &reinit_data.validators_len,
            &reinit_data.block_num
        )?;
        let acc = reinit_data.account.clone();
        let index = reinit_data.authority_index.clone();
        let len = reinit_data.validators_len.clone();
        let block = reinit_data.block_num.clone();
        let sign = signature.clone();
        let call = Call::reinit {
            request: reinit_data,
            signature,
        };

        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(|_| {
            log::error!(
                "{}:{}. Submit reinit error. Signature: {:?}, request account: {:?}, \
                            authority_index: {:?}, validators_len: {:?}, block_num:{:?}.",
                file!(),
                line!(),
                sign,
                acc,
                index,
                len,
                block
            );
            OffchainErr::SubmitTransaction
        })
    }

    fn local_rate_authority_keys() -> impl Iterator<Item = (u32, T::AuthorityId)> {
        let authorities = Keys::<T>::get();
        let mut local_keys = T::AuthorityId::all();

        local_keys.sort();

        authorities
            .into_iter()
            .enumerate()
            .filter_map(move |(index, authority)| {
                local_keys
                    .binary_search(&authority)
                    .ok()
                    .map(|location| (index as u32, local_keys[location].clone()))
            })
    }

    fn do_reinit(who: &<T as system::Config>::AccountId) -> Result<(), DispatchError> {
        // technical accounts: bailsman, treasury, lender
        let basic_asset = T::AssetGetter::get_main_asset();
        let bailsman_acc_id: T::AccountId = T::BailsmanModuleId::get().into_account_truncating();
        let distribution_acc_id: T::AccountId =
            eq_primitives::DISTRIBUTION_ACC.into_account_truncating();

        if bailsman_acc_id == *who || distribution_acc_id == *who {
            return Ok(());
        }

        let last_update = <LastFeeUpdate<T>>::get(who);
        #[allow(unused_must_use)]
        if Self::is_bailsman(who) {
            T::BailsmanManager::redistribute(who)?;
        }
        let result = T::MarginCallManager::check_margin(who)?;
        if result == MarginState::SubCritical {
            // do margin call and adios
            let _r = T::MarginCallManager::try_margincall(who)?;
            Self::set_last_update(who);
            return Ok(());
        }

        let mut may_be_interest_rate_err = None;
        match Self::charge_fee(who) {
            Ok(_) => {
                let current_native_asset_balance = T::BalanceGetter::get_balance(who, &basic_asset);

                // eq_buyout and charge_fee fall simultaneously, so we expect Ok
                if let SignedBalance::Negative(negative_current_eq) = current_native_asset_balance {
                    T::EqBuyout::eq_buyout(who, negative_current_eq).expect("eq_buyout failure");
                }

                Self::set_last_update(who);
            }
            Err(err) => {
                if last_update == 0 {
                    // we need to init account
                    Self::set_last_update(who);
                } else {
                    may_be_interest_rate_err = Some(err);
                }
            }
        }
        // try margin call any way
        let _margin_state = T::MarginCallManager::try_margincall(who)?;

        if let Some(error) = may_be_interest_rate_err {
            return Err(error);
        }

        Ok(())
    }

    fn need_to_reinit(account_id: &T::AccountId) -> bool {
        let mut balance_changes = if Self::is_bailsman(account_id) {
            T::BailsmanManager::get_account_distribution(account_id)
                .map(|account_distribution| {
                    account_distribution
                        .transfers
                        .into_iter()
                        .map(|(asset, change)| BalanceChange { asset, change })
                        .collect()
                })
                .unwrap_or(Vec::new())
        } else {
            Vec::new()
        };

        let mut can_reinit =
            match T::MarginCallManager::check_margin_with_change(account_id, &balance_changes, &[])
            {
                Ok((margin_state, _)) => !margin_state.good_position(),
                Err(_) => false, // don't margincall if check_margin returns Err
            };

        if !can_reinit {
            let fee = match Self::calc_fee(account_id) {
                Ok(fee) => fee.total_fee(),
                Err(err) => {
                    match err {
                        InterestRateError::ZeroDebt => {} // skip logging, not an error
                        _ => {
                            log::error!(target:"eq_rate","Calculated interest_fee error {:?}", err);
                        }
                    };

                    T::Balance::zero()
                } // nothing to charge, no need to reinit
            };

            let basic_asset = T::AssetGetter::get_main_asset();

            // position can become bad after charging fee
            balance_changes.push(BalanceChange {
                asset: basic_asset,
                change: SignedBalance::Negative(fee),
            });
            let need_to_mc = match T::MarginCallManager::check_margin_with_change(
                account_id,
                &balance_changes,
                &[],
            ) {
                Ok((result, _)) => !result.good_position(),
                Err(_) => false, // don't margincall if check_margin returns Err
            };
            can_reinit = need_to_mc || fee > T::MinSurplus::get();
        }

        can_reinit
    }

    /// Calculate fee in base asset(EQ/GENS) for treasury, bailsman and lender pools
    fn calc_fee(account_id: &T::AccountId) -> Result<Fee<T::Balance>, InterestRateError> {
        let assets_data = T::AssetGetter::get_assets_data_with_usd();
        let currencies: Vec<_> = assets_data.iter().map(|a| a.id).collect();
        let basic_asset = T::AssetGetter::get_main_asset();
        let rate_calculator = InterestRateCalculator::<Self>::create(account_id, &currencies)?;

        let account_debt_weights = rate_calculator.debt_weights()?;
        let eqd_debt_weight = account_debt_weights
            .iter()
            .zip(&currencies)
            .find(|(_, &asset)| asset == EQD)
            .map(|(&weight, _)| weight)
            .unwrap_or_else(|| FixedI128::zero());

        // Intermediate coefficient
        // Coeff = Debt * (t / T) / native_price
        // Debt - account debt in USD
        // t - elapsed time from last update
        // T - constant, ms in Year
        let coeff = {
            let debt_collateral_discounted = T::BalanceGetter::get_debt_and_collateral(&account_id)
                .map_err(|_| {
                    log::error!(
                        target: "eq_rate",
                        "{}:{}. Could not get debt value for account to calculate fee. Account: {:?}",
                        file!(),
                        line!(),
                        account_id
                    );
                    InterestRateError::ExternalError
                })?;

            let debt = fixedi128_from_balance(debt_collateral_discounted.debt)
                .ok_or(InterestRateError::Overflow)?;

            let elapsed = {
                // elapsed seconds from last reinit
                let last_update = <LastFeeUpdate<T>>::get(account_id.clone());
                let current_time = Self::now().as_secs();

                FixedI128::saturating_from_integer(current_time - last_update)
            };

            //seconds in one year = 365.25*24*60*60 = 31557600
            let seconds_in_year = FixedI128::saturating_from_integer(31557600 as u128);

            let basic_asset_price = T::PriceGetter::get_price(&basic_asset)
                .map(|p| fixedi128_from_fixedi64(p))
                .map_err(|_| {
                    log::error!(target: "eq_rate","{}:{}.", file!(), line!());
                    InterestRateError::NoPrices
                })?;

            let coeff = debt * (elapsed / seconds_in_year) / basic_asset_price;
            coeff
        };

        let prime_rate = {
            // use 2% for unit-tests and with feature="test"
            if cfg!(test) || cfg!(feature = "test") {
                // hardcoded 2% for main logic tests
                FixedI128::saturating_from_rational(2, 100)
            } else {
                rate_calculator.interest_rate()?
            }
        };

        let lender_part = FixedI128::from(T::LenderPart::get());

        let treasury_fee = coeff * FixedI128::from(T::TreasuryFee::get());

        let bailsman_fee = {
            let base = coeff * FixedI128::from(T::BaseBailsmanFee::get());
            let insurance = coeff * (FixedI128::one() - lender_part) * prime_rate;
            let eqd = coeff * eqd_debt_weight * lender_part * prime_rate;

            base + insurance + eqd
        };

        let base_lend_rate = FixedI128::from(T::BaseLenderFee::get());

        let lender_fees = account_debt_weights
            .iter()
            .zip(assets_data)
            .filter(|(_, asset_data)| asset_data.asset_type == AssetType::Physical)
            .map(|(&debt_weight, asset_data)| {
                let fee = balance_from_fixedi128(
                    coeff * debt_weight * (base_lend_rate + lender_part * prime_rate),
                )
                .ok_or(InterestRateError::ValueError)?;
                Ok((asset_data.id, fee))
            })
            .collect::<Result<Vec<(Asset, T::Balance)>, InterestRateError>>()?;

        let fee = Fee::<T::Balance> {
            basic_asset,
            treasury: balance_from_fixedi128(treasury_fee).ok_or_else(|| {
                log::error!(
                    target: "eq_rate",
                    "{}:{}. Conversion of treasury_fee to balance failed. Treasury fee: {:?}, account id: {:?}",
                    file!(),
                    line!(),
                    treasury_fee,
                    account_id
                );
                InterestRateError::Overflow
            })?,
            bailsman: balance_from_fixedi128(bailsman_fee).ok_or_else(|| {
                log::error!(
                    target: "eq_rate",
                    "{}:{}. Conversion of bailsman_fee to balance failed. Treasury fee: {:?}, account id: {:?}",
                    file!(),
                    line!(),
                    bailsman_fee,
                    account_id
                );
                InterestRateError::Overflow
            })?,
            lender: lender_fees
        };

        Ok(fee)
    }

    fn charge_fee(account_id: &T::AccountId) -> DispatchResult {
        let fee = match Self::calc_fee(account_id) {
            Ok(fee) => fee,
            Err(e) => match e {
                InterestRateError::ExternalError => frame_support::fail!(Error::<T>::ExternalError),
                InterestRateError::NoPrices => frame_support::fail!(Error::<T>::NoPrices),
                InterestRateError::NoFinancial => frame_support::fail!(Error::<T>::NoFinancial),
                InterestRateError::MathError => frame_support::fail!(Error::<T>::MathError),
                InterestRateError::ValueError => frame_support::fail!(Error::<T>::ValueError),
                InterestRateError::LastUpdateInFuture => {
                    frame_support::fail!(Error::<T>::LastUpdateInFuture)
                }
                InterestRateError::Overflow => {
                    frame_support::fail!(ArithmeticError::Overflow)
                }
                InterestRateError::ZeroDebt => return Ok(()), // not an error, zero fee, nothing to charge
            },
        };

        Self::charge_treasury_fee(account_id, fee.basic_asset, fee.treasury)?;
        Self::charge_bailsman_fee(account_id, fee.basic_asset, fee.bailsman)?;
        Self::charge_lender_fee(account_id, fee.basic_asset, fee.lender)?;

        Ok(())
    }

    fn charge_treasury_fee(
        account_id: &T::AccountId,
        basic_asset: Asset,
        fee_amount: T::Balance,
    ) -> DispatchResult {
        // treasury module account id
        let treasury_account = T::TreasuryModuleId::get().into_account_truncating();
        //
        if let Some(author) = authorship::Pallet::<T>::author() {
            let (fee_for_account, fee_for_author) = ration(
                fee_amount,
                T::WeightFeeTreasury::get(),
                T::WeightFeeValidator::get(),
            );
            // transferring a fee to the treasury
            T::EqCurrency::currency_transfer(
                account_id,
                &treasury_account,
                basic_asset,
                fee_for_account,
                ExistenceRequirement::KeepAlive,
                eq_primitives::TransferReason::InterestFee,
                false,
            )?;

            // transferring a fee to the author
            T::EqCurrency::currency_transfer(
                account_id,
                &author,
                basic_asset,
                fee_for_author,
                ExistenceRequirement::KeepAlive,
                eq_primitives::TransferReason::InterestFee,
                false,
            )?;
        }

        Ok(())
    }

    fn charge_bailsman_fee(
        account_id: &T::AccountId,
        basic_asset: Asset,
        fee_amount: T::Balance,
    ) -> DispatchResult {
        let bailsman_account = T::BailsmanModuleId::get().into_account_truncating();

        T::EqCurrency::currency_transfer(
            account_id,
            &bailsman_account,
            basic_asset,
            fee_amount,
            ExistenceRequirement::KeepAlive,
            eq_primitives::TransferReason::InterestFee,
            false,
        )
    }

    #[frame_support::transactional]
    fn charge_lender_fee(
        account_id: &T::AccountId,
        basic_asset: Asset,
        fees: Vec<(Asset, T::Balance)>,
    ) -> DispatchResult {
        let lending_pool = T::LendingModuleId::get().into_account_truncating();

        // we charge lender_fee by asset when lending pool has asset
        let mut fee_amount = T::Balance::zero();
        for (asset, amount) in fees {
            match T::LendingPoolManager::add_reward(asset, amount) {
                Err(_) => {
                    log::error!(target: "eq_rate","Lending pool is not initialized. Lender fee wasn't charged from account {:?} fee {:?}", account_id, fee_amount);
                }
                _ => {
                    fee_amount += amount;
                }
            };
        }

        T::EqCurrency::currency_transfer(
            account_id,
            &lending_pool,
            basic_asset,
            fee_amount,
            ExistenceRequirement::KeepAlive,
            eq_primitives::TransferReason::InterestFee,
            false,
        )?;

        Ok(())
    }

    fn is_bailsman(account_id: &T::AccountId) -> bool {
        T::Aggregates::in_usergroup(account_id, UserGroup::Bailsmen)
    }
}

impl<T: Config> OnKilledAccount<T::AccountId> for Pallet<T> {
    fn on_killed_account(who: &T::AccountId) {
        Self::remove_last_update(who);
    }
}

/// Sets timestamp of last update on account creation
impl<T: Config> OnNewAccount<T::AccountId> for Pallet<T> {
    fn on_new_account(who: &T::AccountId) {
        Self::set_last_update(who);
    }
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
    type Public = T::AuthorityId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
    type Key = T::AuthorityId;

    fn on_genesis_session<'a, I: 'a>(validators: I)
    where
        I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
    {
        let keys = validators.map(|x| x.1).collect::<Vec<_>>();
        Self::initialize_keys(&keys);
    }

    fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, _queued_validators: I)
    where
        I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
    {
        Keys::<T>::put(validators.map(|x| x.1).collect::<Vec<_>>());
    }

    fn on_disabled(_i: u32) {
        // ignored because no validators disabling functionality for now
    }
}

impl<T: Config> UnixTime for Pallet<T> {
    fn now() -> core::time::Duration {
        let now = T::UnixTime::now();
        if !cfg!(feature = "production") {
            let offset = <NowMillisOffset<T>>::get();
            let duration = core::time::Duration::from_millis(offset);
            now + duration
        } else {
            now
        }
    }
}

impl<T: Config> ValidatorOffchainBatcher<T::AuthorityId, T::BlockNumber, T::AccountId>
    for Pallet<T>
{
    fn authority_keys() -> Vec<T::AuthorityId> {
        Keys::<T>::get()
    }

    fn local_authority_keys() -> Vec<(u32, T::AuthorityId)> {
        Self::local_rate_authority_keys().collect()
    }

    fn get_validators_len() -> u32 {
        <pallet_session::Pallet<T>>::validators().len() as u32
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn set_local_authority_keys(keys: Vec<T::AuthorityId>) {
        Keys::<T>::put(keys);
    }
}

impl<T: Config> rate::InterestRateDataSource for Pallet<T> {
    type AccountId = T::AccountId;
    type Price = <<T as Config>::FinancialStorage as FinancialStorage>::Price;

    fn get_settings() -> rate::InterestRateSettings {
        rate::InterestRateSettings::new(
            T::RiskLowerBound::get(),
            T::RiskUpperBound::get(),
            T::RiskNSigma::get(),
            T::Alpha::get(),
        )
    }

    fn get_price(asset: Asset) -> Result<EqFixedU128, sp_runtime::DispatchError> {
        T::PriceGetter::get_price(&asset)
    }

    fn get_fin_metrics() -> Option<FinancialMetrics<Asset, Self::Price>> {
        T::FinancialStorage::get_metrics()
    }

    fn get_covariance(
        c1: Asset,
        c2: Asset,
        metrics: &FinancialMetrics<Asset, Self::Price>,
    ) -> Option<FixedI128> {
        let assets_amount = metrics.assets.len();
        if assets_amount == 0 {
            log::trace!(target: "eq_rate","get_covariance ERROR: assets_amount is zero",);
            return None;
        };

        let c1_index = metrics.assets.iter().position(|&curr| curr == c1)?;
        let c2_index = metrics.assets.iter().position(|&curr| curr == c2)?;
        let covariance_index = c1_index * assets_amount + c2_index;
        if covariance_index + 1 > metrics.covariances.len() {
            log::trace!(
                target: "eq_rate",
                "get_covariance ERROR: covariance_index ({}) + 1 > metrics.covariances.len() ({}) \
                currencies: {:?} {:?}",
                covariance_index,
                metrics.covariances.len(),
                c1,
                c2
            );
            return None;
        };

        let result = metrics.covariances[covariance_index];
        Some(fixedi128_from_i64f64(result))
    }

    fn get_bailsmen_total_balance(asset: Asset) -> rate::TotalBalance {
        let totals = T::Aggregates::get_total(UserGroup::Bailsmen, asset);

        rate::TotalBalance::new(
            eq_fixedu128_from_balance(totals.collateral),
            eq_fixedu128_from_balance(totals.debt),
        )
    }

    fn get_borrowers_balance(asset: Asset) -> rate::TotalBalance {
        let totals = T::Aggregates::get_total(UserGroup::Borrowers, asset);

        rate::TotalBalance::new(
            eq_fixedu128_from_balance(totals.collateral),
            eq_fixedu128_from_balance(totals.debt),
        )
    }

    fn get_balance(account_id: &T::AccountId, asset: Asset) -> rate::TotalBalance {
        let balance = T::BalanceGetter::get_balance(account_id, &asset);

        match balance {
            SignedBalance::Positive(value) => {
                rate::TotalBalance::new(eq_fixedu128_from_balance(value), EqFixedU128::zero())
            }
            SignedBalance::Negative(value) => {
                rate::TotalBalance::new(EqFixedU128::zero(), eq_fixedu128_from_balance(value))
            }
        }
    }

    fn get_discount(asset: Asset) -> EqFixedU128 {
        T::AssetGetter::collateral_discount(&asset)
    }
}
