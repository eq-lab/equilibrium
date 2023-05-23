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

//! # Equilibrium Bailsman Pallet
//! Equilibrium's Bailsman Pallet is a substrate module that calculates user interest payments.
//! Registered bailsmen receive fees paid by borrowers and secure the system by taking on liquidated collateral and debt of defaulted borrowers.
//! If bailsmen get debt as a result of borrower liquidation they themselves become subject to interest rate fees.

//! Registering as a bailsman requires a minimum amount of assets (in USD terms) to be present on the balance of the bailsman sub-account.
//! There is an offchain worker which unregisters accounts as bailsman (removes from aggregates) if the min. requirement is breached.
//! Users may not unregister as bailsmen when they have debt (negative balances) - e.g. they have to fulfill their obligations to the system.  

//! This pallet contains a separate module (rates) which calculates interest rate on a per borrower basis.
//! It also assesses the risk of insolvency of the entire system and scales borrower interest accordingly (scale calculation inside rates module).

//! Bailsmen pallet accrues fees (basic tokens), liquidated debt (negative balances) and liquidated collateral (positive balances) on its account.
//! The redistribution happens on reinit_bailsman - either via an offchain worker, or when bailsmen perform transfers, deposits, withdrawals e.t.c.

//! Currently bailsman reinit is an expensive transaction since the system has to calculate relative weights of every bailsman account
//! inside the bailsman pool and split balances accrued on pallet’s account accordingly.
//! In further releases we will optimize this by working with aggregates/integrals and not making redistribution on price updates.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

pub mod benchmarking;
mod mock;

#[cfg(test)]
mod tests;
pub mod weights;
extern crate alloc;

// pub use eq_oracle;

use codec::Codec;
#[allow(unused_imports)]
use eq_primitives::balance::{BalanceChecker, EqCurrency};
use sp_runtime::{
    traits::{CheckedAdd, CheckedSub, Saturating},
    ArithmeticError, FixedPointNumber,
};

use codec::{Decode, Encode};
use core::convert::TryInto;
use eq_primitives::{
    asset,
    asset::{Asset, AssetGetter},
    balance::{BalanceGetter, DebtCollateralDiscounted},
    balance_number::EqFixedU128,
    offchain_batcher::{OffchainErr, OffchainResult, ValidatorOffchainBatcher},
    price::PriceGetter,
    signed_balance::SignedBalance,
    subaccount::SubaccountsManager,
    AccountDistribution, Aggregates, BailsmanManager, BalanceChange, Distribution, DistributionId,
    MarginCallManager, MarginState, UserGroup, DISTRIBUTION_ACC,
};
use eq_utils::{
    eq_ensure,
    fixed::{eq_fixedu128_from_balance, fixedi128_from_balance, fixedi128_from_eq_fixedu128},
    vec_map::{SortedVec, VecMap},
};
use frame_support::traits::WithdrawReasons;
use frame_support::weights::{Pays, PostDispatchInfo};
use frame_support::{
    pallet_prelude::InvalidTransaction,
    traits::{ExistenceRequirement, Get, UnixTime},
    PalletId, Parameter,
};
use frame_system as system;
use sp_application_crypto::RuntimeAppPublic;
#[allow(unused_imports)]
use sp_runtime::{
    traits::{
        AccountIdConversion, AtLeast32BitUnsigned, Bounded, MaybeSerializeDeserialize, Member, One,
        Zero,
    },
    DispatchError, DispatchResult, FixedPointOperand,
};
use sp_std::convert::From;
use sp_std::{default::Default, prelude::*};
use system::offchain::{SendTransactionTypes, SubmitTransaction};
pub use weights::WeightInfo;

pub use pallet::*;

const DB_PREFIX: &[u8] = b"eq-bailsman/";

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use eq_primitives::balance::DebtCollateralDiscounted;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + SendTransactionTypes<Call<Self>> {
        /// Pallet's AccountId for balances
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        type AuthorityId: Member + Parameter + RuntimeAppPublic + Ord + MaybeSerializeDeserialize;

        /// Gets currency prices from oracle
        type PriceGetter: PriceGetter;

        /// Timestamp provider
        type UnixTime: UnixTime;

        /// Numerical representation of stored balances
        type Balance: Member
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Codec
            + Copy
            + Parameter
            + Default
            // + sp_std::fmt::Debug
            + From<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>
            + FixedPointOperand;
        /// Gets users balances to calculate fees and check margin calls
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;
        /// Used for currency-related operations and calculations
        type EqCurrency: eq_primitives::balance::EqCurrency<Self::AccountId, Self::Balance>;
        /// Used to work with `TotalAggregates` storing aggregated collateral and debt
        type Aggregates: Aggregates<Self::AccountId, Self::Balance>;
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Minimal sum of the collateral and debt abs values to distribute temp Bailsman balances
        #[pallet::constant]
        type MinTempBalanceUsd: Get<Self::Balance>;
        /// Minimum amount of balances account should have to register as bailsman
        #[pallet::constant]
        type MinimalCollateral: Get<Self::Balance>;
        /// Amount of bailsmen to redistribute per block in offchain
        #[pallet::constant]
        type MaxBailsmenToDistribute: Get<u32>;
        /// Priority for offchain extrinsics
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;
        /// Used to execute batch operations for every `AuthorityId` key in keys storage
        type ValidatorOffchainBatcher: ValidatorOffchainBatcher<
            Self::AuthorityId,
            Self::BlockNumber,
            Self::AccountId,
        >;
        /// Used to deal with Assets
        type AssetGetter: AssetGetter;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        /// Interface for checking current margin
        type MarginCallManager: MarginCallManager<Self::AccountId, Self::Balance>;
        /// Interface for working with subaccounts
        type SubaccountsManager: SubaccountsManager<Self::AccountId>;
        /// Special constant for improving weight in unsigned extrinsics
        #[pallet::constant]
        type QueueLengthWeightConstant: Get<u32>;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(<T as pallet::Config>::WeightInfo::toggle_auto_redistribution())]
        pub fn toggle_auto_redistribution(
            origin: OriginFor<T>,
            enabled: bool,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            AutoRedistributionEnabled::<T>::put(enabled);
            Ok(().into())
        }

        /// Request to redistribute single bailsman sent by offchain worker.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::redistribute_unsigned(request.queue_len + T::QueueLengthWeightConstant::get()))]
        pub fn redistribute_unsigned(
            origin: OriginFor<T>,
            request: DistributionRequest<T::AccountId, T::BlockNumber>,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            let DistributionRequest { bailsman, .. } = request;
            <Self as BailsmanManager<_, _>>::redistribute(&bailsman)?;

            let (_, queue) = DistributionQueue::<T>::get();
            let weight = T::WeightInfo::redistribute_unsigned(queue.len() as u32);
            Ok(PostDispatchInfo {
                actual_weight: Some(weight),
                pays_fee: Pays::Yes,
            })
        }

        /// Operation to redistribute single bailsman manually.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::redistribute(30))]
        pub fn redistribute(origin: OriginFor<T>, who: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            <Self as BailsmanManager<_, _>>::redistribute(&who)?;

            let (_, queue) = DistributionQueue::<T>::get();
            let weight = T::WeightInfo::redistribute(queue.len() as u32);
            Ok(PostDispatchInfo {
                actual_weight: Some(weight),
                pays_fee: Pays::Yes,
            })
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_: BlockNumberFor<T>) -> Weight {
            let temp_balances = Self::get_account_id();
            let distribution_balances = DISTRIBUTION_ACC.into_account_truncating();
            let remaining_bailsmen = BailsmenCount::<T>::get();
            let mut queue_len = 0;
            if remaining_bailsmen != 0 {
                if let Ok(DebtCollateralDiscounted {
                    debt: temp_debt_usd,
                    collateral: temp_collateral_usd,
                    discounted_collateral: _,
                }) = T::BalanceGetter::get_debt_and_collateral(&temp_balances)
                {
                    if temp_collateral_usd.saturating_add(temp_debt_usd)
                        > T::MinTempBalanceUsd::get()
                    {
                        if let Ok(DebtCollateralDiscounted {
                            debt: distribution_debt_usd,
                            collateral: distribution_collateral_usd,
                            discounted_collateral: _,
                        }) = T::BalanceGetter::get_debt_and_collateral(&distribution_balances)
                        {
                            if let Some((total_usd, prices)) = Self::get_total_usd(
                                temp_collateral_usd.saturating_add(distribution_collateral_usd),
                                temp_debt_usd.saturating_add(distribution_debt_usd),
                            )
                            .ok()
                            {
                                let temp_balances_vec =
                                    T::BalanceGetter::iterate_account_balances(&temp_balances);
                                for (&asset, balance) in &temp_balances_vec {
                                    let (source_acc, dest_acc, amount) = match *balance {
                                        SignedBalance::Positive(a) => {
                                            (&temp_balances, &distribution_balances, a)
                                        }
                                        SignedBalance::Negative(a) => {
                                            (&distribution_balances, &temp_balances, a)
                                        }
                                    };

                                    let res = T::EqCurrency::currency_transfer(
                                        source_acc,
                                        dest_acc,
                                        asset,
                                        amount,
                                        ExistenceRequirement::KeepAlive,
                                        eq_primitives::TransferReason::Common,
                                        false,
                                    );

                                    if let Err(err) = res {
                                        log::error!(target: "eq_bails", "Transfer failed. error: {:?}", err);
                                    }
                                }

                                let (curr_distr_id, mut queue) = Self::distribution_queue();
                                queue.push(
                                    curr_distr_id + 1,
                                    Distribution {
                                        total_usd,
                                        remaining_bailsmen,
                                        distribution_balances: temp_balances_vec.into(),
                                        prices: prices.into(),
                                    },
                                );
                                queue_len = queue.len();

                                DistributionQueue::<T>::put((curr_distr_id + 1, queue));
                            }
                        }
                    }
                }
            }

            <T as pallet::Config>::WeightInfo::on_initialize()
                + <T as pallet::Config>::WeightInfo::on_finalize(queue_len as u32)
        }

        fn on_finalize(_: BlockNumberFor<T>) {
            let (curr_distr_id, mut queue) = Self::distribution_queue();

            let prev_len = queue.len();
            queue.retain(|_, distr| distr.remaining_bailsmen != 0);

            if prev_len != queue.len() {
                DistributionQueue::<T>::put((curr_distr_id, queue));
            }
        }

        fn offchain_worker(now: BlockNumberFor<T>) {
            if Self::auto_redistribution_enabled() && sp_io::offchain::is_validator() {
                let lock_res = eq_utils::offchain::accure_lock(DB_PREFIX, || {
                    // doesn't return error anyway, all errors are logged inside `execute_batch`
                    let _ = T::ValidatorOffchainBatcher::execute_batch(
                        now,
                        Self::check_bailsmen_for_single_auth,
                        "eq_bails",
                    );
                });

                match lock_res {
                    eq_utils::offchain::LockedExecResult::Executed => {
                        log::trace!(target: "eq_bails", "offchain_worker:executed");
                    }
                    eq_utils::offchain::LockedExecResult::Locked => {
                        log::trace!(target: "eq_bails", "offchain_worker:locked");
                    }
                }
            }
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Account has insufficient balance to register as bailsman
        CollateralMustBeMoreThanMin,
        /// Cannot register account as bailsman because account is already a bailsman
        AlreadyBailsman,
        /// Cannot unregister bailsman account - account is not bailsman
        NotBailsman,
        /// Prices received from oracle are outdated
        PricesAreOutdated,
        /// Cannot register/unregister or transfer from bailsman: bailsman account should not have negative balances
        BailsmanHasDebt,
        /// Bailsmen cannot have negative total balance
        TotalBailsmenPoolBalanceIsNegative,
        /// Bailsman cannot have debt > collat
        NeedToMcBailsmanFirstly,
        /// Need to distribute temp balances
        TempBalancesNotDistributed,
        /// No basic transfers from / to bailsman temp balances
        TempBalancesTransfer,
        /// Wrong margin for operation performing
        WrongMargin,
        /// Balance convertion error
        Convert,
        /// Price not found for redistribution
        PriceNotFound,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Bailsman subaccount is no longer a bailsman. \[who\]
        UnregisteredBailsman(T::AccountId),
    }

    /// Store total amount of bailsmen
    #[pallet::storage]
    #[pallet::getter(fn bailsmen_count)]
    pub type BailsmenCount<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Store last redistributed id for each bailsman
    #[pallet::storage]
    pub type LastDistribution<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, DistributionId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn distribution_queue)]
    /// Store id for next distribution and distribution queue
    pub type DistributionQueue<T: Config> = StorageValue<
        _,
        (
            DistributionId,
            VecMap<DistributionId, Distribution<T::Balance>>,
        ),
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn auto_redistribution_enabled)]
    pub type AutoRedistributionEnabled<T: Config> = StorageValue<_, bool, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub bailsmen: Vec<T::AccountId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                bailsmen: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};

            AutoRedistributionEnabled::<T>::put(true);

            let temp_balances = Pallet::<T>::get_account_id();
            let distribution_balances = DISTRIBUTION_ACC.into_account_truncating();

            T::Aggregates::set_usergroup(&temp_balances, UserGroup::Bailsmen, true)
                .expect("Overflow in user group");
            T::Aggregates::set_usergroup(&distribution_balances, UserGroup::Bailsmen, true)
                .expect("Overflow in user group");
            for who in &self.bailsmen {
                T::Aggregates::set_usergroup(who, UserGroup::Bailsmen, true)
                    .expect("Overflow in user group");
            }
            BailsmenCount::<T>::put(self.bailsmen.len() as u32);
            EqPalletAccountInitializer::<T>::initialize(&temp_balances);
            EqPalletAccountInitializer::<T>::initialize(&distribution_balances)
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match (source, call) {
                (_, Call::redistribute_unsigned { request, signature }) => {
                    Self::check_unsigned_payload(&request, &signature)?;
                    let queue_len = request.queue_len as u64;
                    let priority = if request.higher_priority {
                        queue_len + T::UnsignedPriority::get() + 1
                    } else {
                        queue_len + T::UnsignedPriority::get()
                    };

                    ValidTransaction::with_tag_prefix("BailsRedistribution")
                        .priority(priority)
                        .and_provides(&request.bailsman)
                        .longevity(5)
                        .propagate(true)
                        .build()
                }
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}

impl<T: Config> BalanceChecker<T::Balance, T::AccountId, T::BalanceGetter, T::SubaccountsManager>
    for Pallet<T>
{
    fn need_to_check_impl(
        who: &T::AccountId,
        _changes: &Vec<(Asset, SignedBalance<T::Balance>)>,
    ) -> bool {
        T::Aggregates::in_usergroup(who, UserGroup::Bailsmen)
    }

    fn can_change_balance_impl(
        who: &T::AccountId,
        changes: &Vec<(Asset, SignedBalance<T::Balance>)>,
        _withdraw_reasons: Option<WithdrawReasons>,
    ) -> Result<(), DispatchError> {
        let self_account_id = Self::get_account_id();
        let distribution_balance = DISTRIBUTION_ACC.into_account_truncating();
        if *who == distribution_balance {
            return Err((Error::<T>::TempBalancesTransfer).into());
        }

        if T::Aggregates::in_usergroup(who, UserGroup::Bailsmen) {
            let temp_balance = T::BalanceGetter::get_debt_and_collateral(&self_account_id)?;
            let min_temp_balance_usd = T::MinTempBalanceUsd::get();

            if temp_balance.debt.saturating_add(temp_balance.collateral) > min_temp_balance_usd {
                return Err((Error::<T>::TempBalancesNotDistributed).into());
            }

            let DebtCollateralDiscounted {
                debt: debt_value,
                collateral: _,
                discounted_collateral,
            } = T::BalanceGetter::get_debt_and_collateral(who)?;

            let should_unreg = Self::should_unreg_bailsman(
                who,
                &changes,
                Some((debt_value, discounted_collateral)),
            )?;

            if should_unreg && !debt_value.is_zero() {
                return Err((Error::<T>::BailsmanHasDebt).into());
            }
        }
        let check_margin_result = Self::check_margin(who, changes)?;
        check_margin_result
            .then(|| ())
            .ok_or_else(|| Error::<T>::WrongMargin.into())
    }
}

impl<T: Config> BailsmanManager<T::AccountId, T::Balance> for Pallet<T> {
    fn register_bailsman(who: &T::AccountId) -> Result<(), DispatchError> {
        let existing = T::Aggregates::in_usergroup(who, UserGroup::Bailsmen);
        eq_ensure!(
            !existing,
            Error::<T>::AlreadyBailsman,
            target: "eq_bailsman",
            "{}:{}. Account is already bailsman. Who: {:?}.",
            file!(),
            line!(),
            who
        );

        let (is_enough, debt_usd, balance_usd, min_collateral) =
            Self::is_enough_to_become_bailsman(who)?;

        eq_ensure!(
            debt_usd.is_zero(),
            Error::<T>::BailsmanHasDebt,
            target: "eq_bailsman",
            "{}:{}. Bailsman cannot have debt. Who: {:?}, debt: {:?}",
            file!(),
            line!(),
            who,
            debt_usd
        );

        eq_ensure!(
            is_enough,
            Error::<T>::CollateralMustBeMoreThanMin,
            target: "eq_bailsman",
            "{}:{}. Total usd balance less or equal to min value. Total usd balance: {:?}, min value: {:?}.",
            file!(), line!(), balance_usd, min_collateral
        );

        T::Aggregates::set_usergroup(who, UserGroup::Bailsmen, true)?;
        BailsmenCount::<T>::mutate(|c| *c += 1);
        LastDistribution::<T>::insert(who, Self::get_current_distribution_id());

        Ok(())
    }

    fn unregister_bailsman(who: &T::AccountId) -> Result<(), DispatchError> {
        Self::do_redistribute(who)?;

        // Ensuring account is a bailsman
        let existing = T::Aggregates::in_usergroup(who, UserGroup::Bailsmen);
        eq_ensure!(
            existing,
            Error::<T>::NotBailsman,
            target: "eq_bailsman",
            "{}:{}. Account not a bailsman. Who: {:?}.",
            file!(),
            line!(),
            who
        );

        // Ensuring account does not have any debt
        for (_, balance) in T::BalanceGetter::iterate_account_balances(who) {
            if let SignedBalance::Negative(_) = balance {
                eq_ensure!(
                    balance.is_zero(),
                    Error::<T>::BailsmanHasDebt,
                    target: "eq_bailsman",
                    "{}:{}. Bailsman account cannot unregister because of debt. Who: {:?}.",
                    file!(),
                    line!(),
                    who
                );
            }
        }

        T::Aggregates::set_usergroup(who, UserGroup::Bailsmen, false)?;
        BailsmenCount::<T>::mutate(|c| *c -= 1);
        LastDistribution::<T>::remove(who);
        Self::deposit_event(Event::UnregisteredBailsman(who.clone()));

        Ok(())
    }

    fn redistribute(who: &T::AccountId) -> Result<u32, DispatchError> {
        Self::ensure_bailsman(who)?;
        Self::do_redistribute(who)
    }

    fn get_account_distribution(
        who: &T::AccountId,
    ) -> Result<AccountDistribution<T::Balance>, DispatchError> {
        Self::get_account_distribution(who)
    }

    fn receive_position(
        who: &T::AccountId,
        is_deleting_position: bool,
    ) -> Result<(), DispatchError> {
        let self_account = Pallet::<T>::get_account_id();
        let debt_collateral_discounted = T::BalanceGetter::get_debt_and_collateral(&who)?;
        let mut borrower_debt_in_usd = debt_collateral_discounted.debt;
        if !is_deleting_position {
            // 1.005 - debt plus penalty 0.5%
            let multiplier = EqFixedU128::one() + T::MarginCallManager::get_critical_margin();
            borrower_debt_in_usd = multiplier.saturating_mul_int(borrower_debt_in_usd);
        }

        let mut account_balances: Vec<_> = T::BalanceGetter::iterate_account_balances(who).into();

        account_balances
            .sort_by(|a, b| T::AssetGetter::priority(a.0).cmp(&T::AssetGetter::priority(b.0)));

        for (asset, balance) in account_balances {
            let (amount, source_acc, dest_acc) = match balance {
                SignedBalance::Positive(balance_inner) => {
                    let mut amount = balance_inner;
                    if !is_deleting_position {
                        let price = T::PriceGetter::get_price::<EqFixedU128>(&asset)?;
                        let discount = T::AssetGetter::collateral_discount(&asset);
                        let borrower_debt = price
                            .reciprocal()
                            .ok_or(ArithmeticError::DivisionByZero)?
                            .checked_mul_int(borrower_debt_in_usd)
                            .ok_or(ArithmeticError::Overflow)?;
                        let transfer_amount = borrower_debt.min(balance_inner);
                        amount = transfer_amount;
                        let amount_in_usd = (price * discount)
                            .checked_mul_int(transfer_amount)
                            .ok_or(ArithmeticError::Overflow)?;
                        borrower_debt_in_usd = borrower_debt_in_usd - amount_in_usd;
                    }

                    (amount, who, &self_account)
                }
                SignedBalance::Negative(balance_inner) => (balance_inner, &self_account, who),
            };

            T::EqCurrency::currency_transfer(
                source_acc,
                dest_acc,
                asset,
                amount,
                ExistenceRequirement::KeepAlive,
                eq_primitives::TransferReason::MarginCall,
                false,
            )?;
        }

        let is_bailsman = T::Aggregates::in_usergroup(who, UserGroup::Bailsmen);
        if is_bailsman {
            Self::unregister_bailsman(who)?;
        }

        Ok(())
    }

    fn should_unreg_bailsman(
        who: &T::AccountId,
        changes: &[(Asset, SignedBalance<T::Balance>)],
        debt_and_discounted_collateral: Option<(T::Balance, T::Balance)>,
    ) -> Result<bool, DispatchError> {
        if changes
            .iter()
            .any(|(_a, c)| matches!(c, SignedBalance::Negative(_)))
        {
            let (mut change_collateral_usd, mut change_debt_usd) =
                (T::Balance::zero(), T::Balance::zero());
            for (asset, change) in changes.iter() {
                match change {
                    SignedBalance::Positive(amount) => {
                        change_collateral_usd = change_collateral_usd
                            + (T::PriceGetter::get_price::<EqFixedU128>(asset)?
                                * T::AssetGetter::collateral_discount(asset))
                            .checked_mul_int(*amount)
                            .ok_or(ArithmeticError::Overflow)?;
                    }
                    SignedBalance::Negative(amount) => {
                        change_debt_usd = change_debt_usd
                            + T::PriceGetter::get_price::<EqFixedU128>(asset)?
                                .checked_mul_int(*amount)
                                .ok_or(ArithmeticError::Overflow)?;
                    }
                };
            }

            let (debt, discounted_collateral) = if debt_and_discounted_collateral.is_some() {
                debt_and_discounted_collateral.unwrap()
            } else {
                let DebtCollateralDiscounted {
                    debt,
                    discounted_collateral,
                    collateral: _,
                } = T::BalanceGetter::get_debt_and_collateral(&who)?;
                (debt, discounted_collateral)
            };

            let min_bailsman_collateral = T::MinimalCollateral::get();

            Ok(discounted_collateral + change_collateral_usd
                < min_bailsman_collateral + debt + change_debt_usd)
        } else {
            Ok(false)
        }
    }

    fn bailsmen_count() -> u32 {
        BailsmenCount::<T>::get()
    }

    fn distribution_queue_len() -> u32 {
        let (_, queue) = DistributionQueue::<T>::get();
        queue.len() as u32
    }
}

impl<T: Config> Pallet<T> {
    /// Inner function that returns Bailsman pallet account id
    pub fn get_account_id() -> T::AccountId {
        let temp_balance = T::PalletId::get();
        temp_balance.into_account_truncating()
    }

    /// Inner function that returns current distribution id
    pub fn get_current_distribution_id() -> DistributionId {
        use frame_support::storage::{generator::StorageValue, unhashed};
        unhashed::get::<DistributionId>(&DistributionQueue::<T>::storage_value_final_key())
            .unwrap_or(0)
    }

    fn ensure_bailsman(who: &T::AccountId) -> DispatchResult {
        eq_ensure!(
            T::Aggregates::in_usergroup(who, UserGroup::Bailsmen),
            Error::<T>::NotBailsman,
            target: "eq_bailsman",
            "{}:{}. Reinit should be called only for bailsman subaccount. AccountId {:?}",
            file!(),
            line!(),
            who
        );

        Ok(())
    }

    /// Apply distributions for all bailsmen
    fn _full_redistribute() -> Result<u32, DispatchError> {
        let mut was_redistributed = 0;
        let self_account = Self::get_account_id();
        let distr_acc = DISTRIBUTION_ACC.into_account_truncating();

        for bailsman_acc in T::Aggregates::iter_account(UserGroup::Bailsmen) {
            if bailsman_acc == self_account || bailsman_acc == distr_acc {
                continue;
            }

            let redistributed = Self::do_redistribute(&bailsman_acc)?;
            was_redistributed = was_redistributed.saturating_add(redistributed);
        }

        Ok(was_redistributed)
    }

    fn get_account_distribution(
        bailsman_acc_id: &T::AccountId,
    ) -> Result<AccountDistribution<T::Balance>, DispatchError> {
        let last_distribution_id = LastDistribution::<T>::get(bailsman_acc_id).unwrap_or(0u32);

        let (current_distribution_id, mut queue) = DistributionQueue::<T>::get();
        if queue
            .last_key()
            .map(|id| *id <= last_distribution_id)
            .unwrap_or(true)
        {
            // Nothing to distribute when queue is empty
            // or last_distr_id in queue less or equal to current distribution id of bailsman
            return Ok(AccountDistribution {
                transfers: Default::default(),
                last_distribution_id,
                current_distribution_id,
                new_queue: queue,
            });
        }

        let mut before_distr_balances = T::BalanceGetter::iterate_account_balances(bailsman_acc_id);

        // We accumulate all transfers in VecMap, then make transfers
        // It should reduce total count of transfers
        let mut transfer_accumulator = VecMap::new();
        for (&id, distribution) in queue.iter_mut() {
            if id <= last_distribution_id {
                continue;
            }

            Self::apply_distribution(
                &mut before_distr_balances,
                distribution,
                &mut transfer_accumulator,
            )?;

            distribution.remaining_bailsmen -= 1;
        }

        let is_last_distribution = queue
            .iter()
            .all(|(_, distr)| distr.remaining_bailsmen.is_zero());

        if is_last_distribution {
            Self::get_rest_from_distribution_account(&mut transfer_accumulator);
        }

        Ok(AccountDistribution {
            transfers: transfer_accumulator,
            last_distribution_id,
            current_distribution_id,
            new_queue: queue,
        })
    }

    /// Apply all distributions from queue for account
    fn do_redistribute(bailsman_acc_id: &T::AccountId) -> Result<u32, DispatchError> {
        let AccountDistribution {
            transfers,
            last_distribution_id,
            current_distribution_id,
            new_queue,
        } = Self::get_account_distribution(&bailsman_acc_id)?;

        if transfers.is_empty() {
            return Ok(0);
        }

        Self::make_transfers(bailsman_acc_id, transfers)?;

        LastDistribution::<T>::insert(bailsman_acc_id, current_distribution_id);
        DistributionQueue::<T>::set((current_distribution_id, new_queue));

        Ok(current_distribution_id - last_distribution_id)
    }

    /// Calculate amount of distribution and save it to accumulator
    fn apply_distribution(
        before_distr_balances: &mut VecMap<Asset, SignedBalance<T::Balance>>,
        distr: &Distribution<T::Balance>,
        transfer_accumulator: &mut VecMap<Asset, SignedBalance<T::Balance>>,
    ) -> DispatchResult {
        let total_usd = fixedi128_from_balance(distr.total_usd).ok_or(ArithmeticError::Overflow)?;

        let (bailsman_debt_usd, bailsman_collat_usd) =
            Self::get_debt_and_collateral(&before_distr_balances, &distr.prices)?;

        let portion = (fixedi128_from_eq_fixedu128(bailsman_collat_usd - bailsman_debt_usd)
            .ok_or(ArithmeticError::Overflow)?)
            / total_usd;

        for (asset, balance) in distr.distribution_balances.iter() {
            let amount =
                SignedBalance::<T::Balance>::map(balance, |b| portion.saturating_mul_int(b));

            before_distr_balances
                .entry(*asset)
                .and_modify(|v| *v += amount.clone())
                .or_insert(amount.clone());

            transfer_accumulator
                .entry(*asset)
                .and_modify(|v| *v += amount.clone())
                .or_insert(amount);
        }

        Ok(())
    }

    /// We want to keep zero balances distribution account in case of all distribution happened
    fn get_rest_from_distribution_account(
        transfer_accumulator: &mut VecMap<Asset, SignedBalance<T::Balance>>,
    ) {
        let distribution_acc = &DISTRIBUTION_ACC.into_account_truncating();
        for (asset, balance) in T::BalanceGetter::iterate_account_balances(&distribution_acc) {
            if let Some(transfer) = transfer_accumulator.get_mut(&asset) {
                let rest = balance - transfer.clone();
                *transfer += rest;
            }
        }
    }

    fn make_transfers(
        bailsman_acc: &T::AccountId,
        transfers: VecMap<Asset, SignedBalance<T::Balance>>,
    ) -> DispatchResult {
        let distribution_acc = &DISTRIBUTION_ACC.into_account_truncating();
        for (asset, signed_balance) in transfers {
            let (amount, source_acc, dest_acc) = match signed_balance {
                SignedBalance::Positive(balance) => (balance, distribution_acc, bailsman_acc),
                SignedBalance::Negative(balance) => (balance, bailsman_acc, distribution_acc),
            };

            T::EqCurrency::currency_transfer(
                source_acc,
                dest_acc,
                asset,
                amount,
                ExistenceRequirement::KeepAlive,
                eq_primitives::TransferReason::BailsmenRedistribution,
                false,
            )?;
        }

        Ok(())
    }

    /// Recalculate account balances with distribution prices
    fn get_debt_and_collateral(
        balances: &VecMap<Asset, SignedBalance<T::Balance>>,
        prices: &SortedVec<(Asset, EqFixedU128)>,
    ) -> Result<(EqFixedU128, EqFixedU128), DispatchError> {
        let mut debt = EqFixedU128::zero();
        let mut collateral = EqFixedU128::zero();

        for (asset, signed_balance) in balances {
            // we can use binary search because prices stored in sorted vec
            let price = if *asset == asset::EQD {
                T::PriceGetter::get_price(&asset)?
            } else {
                let price_index = prices
                    .binary_search_by(|(a, _)| a.cmp(&asset))
                    .map_err(|_| Error::<T>::PriceNotFound)?;
                let (_, price) = prices[price_index];
                price
            };

            let (cur_debt, cur_collateral) = match signed_balance {
                SignedBalance::Negative(value) => (
                    eq_fixedu128_from_balance(*value) * price,
                    EqFixedU128::zero(),
                ),
                SignedBalance::Positive(value) => (
                    EqFixedU128::zero(),
                    eq_fixedu128_from_balance(*value) * price,
                ),
            };

            collateral = collateral + cur_collateral;
            debt = debt + cur_debt;
        }

        eq_ensure!(
            collateral > debt,
            Error::<T>::NeedToMcBailsmanFirstly,
            target: "eq_bailsman",
            "{}:{}. Bailsman cannot have debt > collat.",
            file!(),
            line!()
        );

        Ok((debt, collateral))
    }

    /// Returns total balance in USD of all bailsmen excluding this pallet and prices.
    fn get_total_usd(
        self_collateral_usd: T::Balance,
        self_debt_usd: T::Balance,
    ) -> Result<(T::Balance, VecMap<Asset, EqFixedU128>), DispatchError> {
        let mut total_collateral_usd = T::Balance::zero();
        let mut total_debt_usd = T::Balance::zero();
        let mut prices = VecMap::new();

        for (asset, total) in T::Aggregates::iter_total(UserGroup::Bailsmen) {
            let price = T::PriceGetter::get_price::<EqFixedU128>(&asset)?;

            if asset != eq_primitives::asset::EQD {
                prices.insert(asset, price);
            }

            let additional_collateral = price
                .checked_mul_int(total.collateral.into())
                .map(|b| b.try_into().ok())
                .flatten()
                .ok_or(ArithmeticError::Overflow)?;

            let additional_debt = price
                .checked_mul_int(total.debt.into())
                .map(|b| b.try_into().ok())
                .flatten()
                .ok_or(ArithmeticError::Overflow)?;

            total_collateral_usd = total_collateral_usd
                .checked_add(&additional_collateral)
                .ok_or(ArithmeticError::Overflow)?;

            total_debt_usd = total_debt_usd
                .checked_add(&additional_debt)
                .ok_or(ArithmeticError::Overflow)?;
        }

        Ok((
            (total_collateral_usd + self_debt_usd)
                .checked_sub(&(total_debt_usd + self_collateral_usd))
                .ok_or(ArithmeticError::Underflow)?,
            prices,
        ))
    }

    /// Returns `true` if changes will up margin
    /// or if margin will not be lower than `MarginState::Good`
    fn check_margin(
        owner: &T::AccountId,
        changes: &[(Asset, SignedBalance<T::Balance>)],
    ) -> Result<bool, DispatchError> {
        let balance_changes: Vec<_> = changes
            .into_iter()
            .map(|(asset, change)| BalanceChange {
                change: change.clone(),
                asset: *asset,
            })
            .collect();

        let (margin_state, is_margin_increased) =
            T::MarginCallManager::check_margin_with_change(owner, &balance_changes, &[])?;
        Ok(is_margin_increased || margin_state == MarginState::Good)
    }

    /// Returns (is_enough, debt_value, discounted_collateral_value, min_bailsman_collateral)
    fn is_enough_to_become_bailsman(
        who: &T::AccountId,
    ) -> Result<(bool, T::Balance, T::Balance, T::Balance), sp_runtime::DispatchError> {
        let DebtCollateralDiscounted {
            debt,
            collateral: _,
            discounted_collateral,
        } = T::BalanceGetter::get_debt_and_collateral(&who).map_err(|err| {
            log::error!(
                "{}:{}. Could not fetch account collateral value during \
                            checking for min bailsman collateral. Who: {:?}",
                file!(),
                line!(),
                who
            );
            err
        })?;

        let min_bailsman_collateral = T::MinimalCollateral::get();

        Ok((
            (discounted_collateral > min_bailsman_collateral) && debt.is_zero(),
            debt,
            discounted_collateral,
            min_bailsman_collateral,
        ))
    }

    fn check_unsigned_payload(
        request: &DistributionRequest<T::AccountId, T::BlockNumber>,
        signature: &<T::AuthorityId as RuntimeAppPublic>::Signature,
    ) -> Result<(), InvalidTransaction> {
        const INVALID_VALIDATORS_LEN: u8 = 10;
        const WRONG_QUEUE_LEN: u8 = 11;

        let keys = T::ValidatorOffchainBatcher::authority_keys();
        if keys.len() as u32 != request.val_len {
            return Err(InvalidTransaction::Custom(INVALID_VALIDATORS_LEN));
        }

        let auth_id = keys
            .get(request.auth_idx as usize)
            .ok_or(InvalidTransaction::BadProof)?;

        auth_id
            .verify(&request.encode(), &signature)
            .then(|| ())
            .ok_or(InvalidTransaction::BadProof)?;

        let (_, queue) = DistributionQueue::<T>::get();
        let current_queue_len = queue.len() as u32;

        // Current queue length should be less or equal request's queue length plus constant
        if current_queue_len > request.queue_len + T::QueueLengthWeightConstant::get() {
            return Err(InvalidTransaction::Custom(WRONG_QUEUE_LEN));
        }

        Ok(())
    }

    fn check_bailsmen_for_single_auth(
        auth_idx: u32,
        auth_key: T::AuthorityId,
        block_number: T::BlockNumber,
        val_len: u32,
    ) -> OffchainResult {
        let (curr_distr_id, queue) = DistributionQueue::<T>::get();
        let queue_len = queue.len() as u32;

        let max_bailsmen_to_distribute = T::MaxBailsmenToDistribute::get() as usize;
        let mut bailsmen_ids =
            Vec::<(T::AccountId, u32)>::with_capacity(max_bailsmen_to_distribute + 1);
        for (bailsman, last_distr_id) in LastDistribution::<T>::iter() {
            if last_distr_id >= curr_distr_id {
                continue;
            }

            match bailsmen_ids.binary_search_by_key(&last_distr_id, |(_, distr_id)| *distr_id) {
                Ok(pos) | Err(pos) if pos < max_bailsmen_to_distribute => {
                    bailsmen_ids.insert(pos, (bailsman, last_distr_id));
                    if bailsmen_ids.len() > max_bailsmen_to_distribute {
                        bailsmen_ids.pop();
                    }
                }
                _ => {}
            }
        }

        bailsmen_ids
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| {
                // idx % len = [auth_idx-1, auth_idx, auth_idx+1]
                *idx as u32 % val_len + 1 >= auth_idx && *idx as u32 % val_len <= auth_idx + 1
            })
            .try_for_each(|(idx, (bailsman, last_distr_id))| {
                log::trace!(
                    target: "eq_bailsman",
                    "check_bailsmen_for_single_auth. auth_idx: {:?}, bailsman: {:?}",
                    auth_idx,
                    bailsman,
                );

                Self::submit_redistribute_bailsman(
                    auth_idx,
                    &auth_key,
                    block_number,
                    val_len,
                    bailsman,
                    last_distr_id,
                    curr_distr_id,
                    queue_len,
                    idx as u32 % val_len == auth_idx,
                )
            })?;

        Ok(())
    }

    fn submit_redistribute_bailsman(
        auth_idx: u32,
        auth_key: &T::AuthorityId,
        block_number: T::BlockNumber,
        val_len: u32,
        bailsman: T::AccountId,
        last_distr_id: DistributionId,
        curr_distr_id: DistributionId,
        queue_len: u32,
        higher_priority: bool,
    ) -> OffchainResult {
        let request = DistributionRequest {
            bailsman,
            last_distr_id,
            curr_distr_id,
            queue_len,
            auth_idx,
            val_len,
            block_number,
            higher_priority,
        };
        let signature = auth_key
            .sign(&request.encode())
            .ok_or(OffchainErr::FailedSigning)?;

        let call = Call::redistribute_unsigned { request, signature };
        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
            .map_err(|_| OffchainErr::SubmitTransaction)
    }
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
    type Public = T::AuthorityId;
}

#[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
pub struct DistributionRequest<AccountId, BlockNumber> {
    /// Bailsman account to redistribute
    pub bailsman: AccountId,
    /// Index of last bailsman's distribution that is already redistributed
    pub last_distr_id: DistributionId,
    /// Index of current distribution
    pub curr_distr_id: DistributionId,
    /// An index of the authority on the list of validators
    pub auth_idx: u32,
    /// The length of session validator set
    pub val_len: u32,
    /// Number of a block
    pub block_number: BlockNumber,
    /// Determines whether this request has the higher priority
    pub higher_priority: bool,
    /// Distribution queue length plus constant
    pub queue_len: u32,
}
