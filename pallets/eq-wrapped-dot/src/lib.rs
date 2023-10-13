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

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

pub mod benchmarking;
mod mock;
mod tests;
pub mod weights;
pub use weights::WeightInfo;

use codec::{Codec, Decode, Encode, HasCompact, MaxEncodedLen};
use eq_primitives::{
    asset,
    balance::EqCurrency,
    balance::{DepositReason, WithdrawReason},
    balance_number::EqFixedU128,
    wrapped_dot::EqDotPrice,
    Aggregates, PriceGetter, TransferReason, UserGroup,
};
use eq_utils::{balance_from_xcm, balance_into_xcm, XcmBalance};
use eq_xcm::{
    relay_interface::{
        call::{CallBuilder, StakingWeights},
        config::{RelayRuntime, RelaySystemConfig},
    },
    ParaId,
};
use frame_support::{
    ensure,
    traits::{EnsureOrigin, ExistenceRequirement, Get, WithdrawReasons},
    BoundedVec, PalletId,
};
pub use pallet::*;
use sp_arithmetic::{
    traits::{AtLeast32BitUnsigned, BaseArithmetic, One, Zero},
    FixedI64, FixedPointNumber, FixedPointOperand, Permill,
};
use sp_runtime::{
    traits::{AccountIdConversion, Saturating},
    ArithmeticError, DispatchError, DispatchResult,
};
use sp_staking::EraIndex;
use sp_std::{
    convert::{TryFrom, TryInto},
    fmt::Debug,
    prelude::*,
    vec::Vec,
};
use xcm::latest::prelude::*;

#[derive(
    Clone, Default, Debug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo, MaxEncodedLen,
)]
pub struct StakingBalance<Balance: Default> {
    /// Free DOT balance on parachain account
    pub transferable: Balance,
    /// Amount of tokens in staking
    pub staked: Balance,
}

/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be unlocked.
#[derive(PartialEq, Eq, Clone, Encode, Decode, scale_info::TypeInfo)]
pub struct UnlockChunk<Balance: HasCompact> {
    /// Amount of funds to be unlocked.
    #[codec(compact)]
    pub value: Balance,
    /// Era number at which point it'll be unlocked.
    #[codec(compact)]
    pub era: EraIndex,
}

frame_support::parameter_types! {
    pub MaxUnlockingChunks: u32 = 32;
}

/// The ledger of a (bonded) stash.
#[derive(PartialEq, Eq, Clone, Encode, Decode, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct StakingLedger<T: RelaySystemConfig> {
    /// The stash account whose balance is actually locked and at stake.
    pub stash: T::AccountId,
    /// The total amount of the stash's balance that we are currently accounting for.
    /// It's just `active` plus all the `unlocking` balances.
    #[codec(compact)]
    pub total: T::Balance,
    /// The total amount of the stash's balance that will be at stake in any forthcoming
    /// rounds.
    #[codec(compact)]
    pub active: T::Balance,
    /// Any balance that is becoming free, which may eventually be transferred out of the stash
    /// (assuming it doesn't get slashed first). It is assumed that this will be treated as a first
    /// in, first out queue where the new (higher value) eras get pushed on the back.
    pub unlocking: BoundedVec<UnlockChunk<T::Balance>, MaxUnlockingChunks>,
    /// List of eras for which the stakers behind a validator have claimed rewards. Only updated
    /// for validators.
    pub claimed_rewards: Vec<EraIndex>,
}

impl<T: RelaySystemConfig> StakingLedger<T> {
    fn consolidate_unlocked(&mut self, current_era: EraIndex) -> T::Balance {
        let mut delta = T::Balance::zero();
        self.unlocking.retain(|chunk| {
            if chunk.era > current_era {
                true
            } else {
                delta = delta.saturating_add(chunk.value);
                false
            }
        });
        self.total = self.total.saturating_sub(delta);
        delta
    }
}

impl<T: RelaySystemConfig> StakingLedger<T> {
    /// Initializes the default object using the given `validator`.
    pub fn default_from(stash: T::AccountId) -> Self {
        Self {
            stash,
            total: Zero::zero(),
            active: Zero::zero(),
            unlocking: Default::default(),
            claimed_rewards: vec![],
        }
    }
}

#[derive(
    Copy, Clone, Debug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum WithdrawAmount<Balance> {
    Dot(Balance),
    EqDot(Balance),
}

impl<Balance: Copy + BaseArithmetic + Default> StakingBalance<Balance> {
    fn total(&self) -> Balance {
        self.staked + self.transferable
    }
}

pub const DOT_DECIMALS: u8 = 10;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::weights::WeightInfo;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Numerical representation of stored balances
        type Balance: Member
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Codec
            + Copy
            + Parameter
            + Default
            + Zero
            + Into<eq_primitives::balance::Balance>
            + MaxEncodedLen
            + TryFrom<eq_primitives::balance::Balance>
            + FixedPointOperand;

        type StakingInitializeOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Used to get total supply of EQDOT
        type Aggregates: Aggregates<Self::AccountId, Self::Balance>;

        /// Maximum reservation coefficient that can possibly be observed
        /// If reservation coefficient exceeds that value, `balance_staking` will balance it to `TargetReserve`
        #[pallet::constant]
        type MaxReserve: Get<Permill>;

        /// Reservation coefficient that is preferable, with deviation in range [`MinReserve`, `MaxReserve`]
        #[pallet::constant]
        type TargetReserve: Get<Permill>;

        /// Minimum reservation coefficient that can possibly be observed
        /// If reservation coefficient falls below that value, `balance_staking` will balance it to `TargetReserve`
        #[pallet::constant]
        type MinReserve: Get<Permill>;

        /// Min amount to deposit, by default 5 DOT
        #[pallet::constant]
        type MinDeposit: Get<Self::Balance>;

        /// The Call builder for communicating with RelayChain via XCM messaging.
        type RelayChainCallBuilder: CallBuilder<Self::AccountId, XcmBalance>;

        /// Used for sending XCM
        type XcmRouter: SendXcm;

        #[pallet::constant]
        type ParachainId: Get<ParaId>;

        /// Used to get DOT price
        type PriceGetter: PriceGetter;

        /// Used for currency-related operations and calculations
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;

        /// Fee payed for withdraw, also used in price calculation, by default 0.98940904738
        #[pallet::constant]
        type WithdrawFee: Get<Permill>;

        /// Pallet identifier
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Extrisic weights
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// Copies of `CurrentEra` and `Ledger` storages on relay chain.
    /// Will be updated every block in `on_finalize`.
    #[pallet::storage]
    pub type RelayStakingInfo<T> =
        StorageValue<_, (EraIndex, StakingLedger<RelayRuntime>), OptionQuery>;

    /// Current distribution of DOTs in staking.
    /// Relate to reservation coefficient by formula: RC = transferable / (transferable + staked)
    #[pallet::storage]
    #[pallet::getter(fn current_balance)]
    pub type CurrentBalance<T: Config> = StorageValue<_, StakingBalance<T::Balance>, ValueQuery>;

    /// Withdraw queue, (Beneficiary, DOT amount to withdraw, EQDOT amount to burn)
    #[pallet::storage]
    #[pallet::getter(fn withdraw_queue)]
    pub(super) type WithdrawQueue<T: Config> =
        StorageValue<_, Vec<(T::AccountId, T::Balance, T::Balance)>, ValueQuery>;

    /// Total unlocking sum
    #[pallet::storage]
    #[pallet::getter(fn total_unlocking)]
    pub(super) type TotalUnlocking<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

    /// Last withdraw era
    #[pallet::storage]
    #[pallet::getter(fn last_withdraw_era)]
    pub(super) type LastWithdrawEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

    /// Periodicity of on-initialize functions: clear withdraw queue and rebalance transferable balance
    #[pallet::storage]
    #[pallet::getter(fn routine_periodicity)]
    pub type StakingRoutinePeriodicity<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    // empty genesis, only for adding ref to module's AccountId
    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        #[serde(skip)]
        pub empty: PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};
            EqPalletAccountInitializer::<T>::initialize(
                &T::PalletId::get().into_account_truncating(),
            );

            StakingRoutinePeriodicity::<T>::put(BlockNumberFor::<T>::one());
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Not a price source for an asset
        NotSupportedAsset,
        /// Amount to deposit less than min deposit amount
        InsufficientDeposit,
        /// Amount to withdraw less than min amount to withdraw
        InsufficientWithdraw,
        /// Math error
        MathError,
        /// Xcm call pallet_staking::bond_extra failed
        XcmStakingBondExtraFailed,
        /// Xcm call pallet_staking::unbond failed
        XcmStakingUnbondFailed,
        /// Xcm call pallet_staking::withraw_unbonded failed
        XcmStakingWithdrawUnbondedFailed,
        /// Asset without xcm information
        XcmUnknownAsset,
        /// Error while converting balance to relay chain balance
        XcmBalanceConversionError,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Deposit amount of DOT
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::deposit())]
        pub fn deposit(origin: OriginFor<T>, amount: T::Balance) -> DispatchResultWithPostInfo {
            let account_id = ensure_signed(origin)?;
            ensure!(
                amount >= T::MinDeposit::get(),
                Error::<T>::InsufficientDeposit
            );

            Self::burn_dot_deposit_wrapped_dot(account_id, amount)?;
            CurrentBalance::<T>::mutate(|v| v.transferable += amount);

            Ok(().into())
        }

        /// Withdraw
        /// params:
        /// - amount - amount of DOT/EQDOT to withdraw/burn
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::withdraw())]
        pub fn withdraw(
            origin: OriginFor<T>,
            amount: WithdrawAmount<T::Balance>,
        ) -> DispatchResultWithPostInfo {
            let account_id = ensure_signed(origin)?;

            let (withdraw_amount, burn_amount) = Self::get_withdraw_and_burn_amount(amount)?;
            ensure!(
                withdraw_amount >= T::MinDeposit::get(),
                Error::<T>::InsufficientWithdraw
            );

            let current_balance = CurrentBalance::<T>::get();
            if current_balance.transferable >= withdraw_amount {
                Self::deposit_dot_burn_wrapped_dot(account_id, withdraw_amount, burn_amount)?;
                CurrentBalance::<T>::mutate(|v| v.transferable -= withdraw_amount);
                Ok(().into())
            } else {
                // Recalculate without fee
                let (withdraw_without_fee, burn_without_fee) = match amount {
                    WithdrawAmount::Dot(withdraw_amount) => {
                        let burn_without_fee = T::WithdrawFee::get() * burn_amount;
                        (withdraw_amount, burn_without_fee)
                    }
                    WithdrawAmount::EqDot(burn_amount) => {
                        let withdraw_amount_without_fee =
                            T::WithdrawFee::get().saturating_reciprocal_mul(withdraw_amount);
                        (withdraw_amount_without_fee, burn_amount)
                    }
                };

                Self::send_xcm_unbond(withdraw_without_fee)?;
                WithdrawQueue::<T>::mutate(|queue| {
                    queue.push((account_id.clone(), withdraw_without_fee, burn_without_fee))
                });
                Self::transfer_wrapped_dot_to_pallet(account_id, burn_without_fee)?;

                Ok(Some(T::WeightInfo::withdraw_unbond()).into())
            }
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::withdraw())]
        pub fn initialize(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            transferable: T::Balance,
            bond: T::Balance,
        ) -> DispatchResultWithPostInfo {
            T::StakingInitializeOrigin::ensure_origin(origin)?;
            let sovereign_account: T::AccountId = T::ParachainId::get().into_account_truncating();
            let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
                T::RelayChainCallBuilder::staking_bond(
                    sovereign_account,
                    balance_into_xcm(bond, DOT_DECIMALS)
                        .ok_or(Error::<T>::XcmBalanceConversionError)?,
                    pallet_staking::RewardDestination::Staked,
                ),
                StakingWeights::<T>::bond(),
            );
            let result = send_xcm::<T::XcmRouter>(Parent.into(), xcm_message);

            ensure!(result.is_ok(), Error::<T>::XcmStakingBondExtraFailed);

            T::EqCurrency::deposit_creating(
                &account_id,
                asset::EQDOT,
                bond,
                true,
                Some(DepositReason::Staking),
            )?;

            CurrentBalance::<T>::put(StakingBalance {
                transferable,
                staked: bond,
            });

            Ok(().into())
        }

        ///Set total unlocking. For maintenance purposes
        #[pallet::call_index(3)]
        #[pallet::weight(T::DbWeight::get().writes(7))]
        pub fn set_total_unlocking(
            origin: OriginFor<T>,
            value: T::Balance,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            TotalUnlocking::<T>::put(value);
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            // Errors are returned before `staking_balance` will change,
            // so we can just igrore errors
            if let Some((current_era, ledger)) = RelayStakingInfo::<T>::get() {
                let maybe_relay_unlocking =
                    balance_from_xcm::<T::Balance>(ledger.total - ledger.active, DOT_DECIMALS);
                let maybe_relay_total = balance_from_xcm::<T::Balance>(ledger.total, DOT_DECIMALS);

                let mut staking_balance = CurrentBalance::<T>::get();

                let is_ledger_ready_to_sync = match (maybe_relay_unlocking, maybe_relay_total) {
                    (Some(relay_unlocking), Some(relay_total)) => {
                        relay_unlocking == TotalUnlocking::<T>::get()
                            && relay_total >= staking_balance.staked
                    }
                    _ => false,
                };

                if is_ledger_ready_to_sync {
                    let _ = Self::update_relay_ledger(&mut staking_balance, current_era, ledger);

                    let withdraw_queue_len = if (n % Self::routine_periodicity()).is_zero() {
                        let _ = Self::clear_withdraw_queue(&mut staking_balance);
                        let _ = Self::rebalance_staking(&mut staking_balance);
                        WithdrawQueue::<T>::decode_len().unwrap_or(0)
                    } else {
                        0
                    };

                    CurrentBalance::<T>::put(staking_balance);
                    return T::WeightInfo::on_initialize(withdraw_queue_len as u32)
                        + T::WeightInfo::on_finalize();
                }
            }

            T::WeightInfo::on_finalize()
        }

        fn on_finalize(_n: BlockNumberFor<T>) {
            RelayStakingInfo::<T>::put(Self::fetch_relay_storages());
        }
    }
}

impl<T: Config> Pallet<T> {
    fn update_relay_ledger(
        StakingBalance {
            transferable,
            staked,
        }: &mut StakingBalance<T::Balance>,
        current_era: EraIndex,
        mut ledger: StakingLedger<RelayRuntime>,
    ) -> DispatchResult {
        let last_withdraw_era = Self::last_withdraw_era();
        if current_era > last_withdraw_era {
            let unlocking = ledger.consolidate_unlocked(current_era);
            if unlocking != 0 {
                let unlocking = balance_from_xcm(unlocking, DOT_DECIMALS)
                    .ok_or(Error::<T>::XcmBalanceConversionError)?;
                Self::send_xcm_withdraw_unbond(unlocking)?;

                *transferable = transferable.saturating_add(unlocking);
                *staked = staked.saturating_sub(unlocking);
            }
            LastWithdrawEra::<T>::put(current_era);
        }

        let ledger_total = balance_from_xcm(ledger.total, DOT_DECIMALS)
            .ok_or(Error::<T>::XcmBalanceConversionError)?;
        if ledger_total > *staked {
            *staked = ledger_total;
        }

        Ok(())
    }

    fn clear_withdraw_queue(
        StakingBalance { transferable, .. }: &mut StakingBalance<T::Balance>,
    ) -> DispatchResult {
        let withdraw_queue = WithdrawQueue::<T>::get();
        let mut to_remove_amount = 0;
        let mut total_burnt_eqdot = T::Balance::zero();
        while let Some((beneficiary, withdraw_amount, burn_amount)) =
            withdraw_queue.get(to_remove_amount)
        {
            if *transferable < *withdraw_amount {
                break;
            }
            T::EqCurrency::deposit_creating(
                beneficiary,
                asset::DOT,
                *withdraw_amount,
                true,
                Some(DepositReason::Staking),
            )?;

            total_burnt_eqdot += *burn_amount;
            *transferable -= *withdraw_amount;
            to_remove_amount += 1;
        }
        if !total_burnt_eqdot.is_zero() {
            T::EqCurrency::withdraw(
                &T::PalletId::get().into_account_truncating(),
                asset::EQDOT,
                total_burnt_eqdot,
                false,
                Some(WithdrawReason::Staking),
                WithdrawReasons::empty(),
                ExistenceRequirement::KeepAlive,
            )?;
        }
        WithdrawQueue::<T>::put(&withdraw_queue[to_remove_amount..]);

        Ok(())
    }

    fn rebalance_staking(
        StakingBalance {
            transferable,
            staked,
        }: &mut StakingBalance<T::Balance>,
    ) -> DispatchResult {
        let total = *transferable + *staked;

        if *transferable > T::MaxReserve::get() * total {
            // transferable/total > MaxReserve > TargetReserve
            // No underflow; qed
            let delta = *transferable - T::TargetReserve::get() * total;

            Self::send_xcm_bond_extra(delta)?;
            *transferable = transferable.saturating_sub(delta);
            *staked = staked.saturating_add(delta);
        } else {
            let to_withdraw = WithdrawQueue::<T>::get()
                .iter()
                .fold(T::Balance::zero(), |acc, (_, withdraw_amount, _)| {
                    acc.saturating_add(*withdraw_amount)
                });
            let unlocking_and_transferable =
                *transferable + TotalUnlocking::<T>::get() - to_withdraw;
            let total_without_withdraw = total - to_withdraw; // contains unbonds from previous iterations
            if unlocking_and_transferable < T::MinReserve::get() * total_without_withdraw {
                // TargetReserve > MinReserve > transferable/total
                // No underflow; qed
                let delta =
                    T::TargetReserve::get() * total_without_withdraw - unlocking_and_transferable;

                Self::send_xcm_unbond(delta)?;
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "runtime-benchmarks"))]
    fn fetch_relay_storages() -> (EraIndex, StakingLedger<RelayRuntime>) {
        use eq_xcm::relay_interface::storage::*;

        let ref relay_backend = create_relay_backend().expect("Should create");
        let current_era = get_with::<EraIndex>(relay_backend, known_keys::STAKING_CURRENT_ERA)
            .ok()
            .flatten()
            .unwrap_or_default();
        let sovereign_account: <RelayRuntime as RelaySystemConfig>::AccountId =
            T::ParachainId::get().into_account_truncating();
        let staking_ledger = get_with::<StakingLedger<RelayRuntime>>(
            relay_backend,
            known_keys::staking_ledger_maybe_derivative(sovereign_account.clone(), None),
        )
        .ok()
        .flatten()
        .unwrap_or_else(|| StakingLedger::default_from(sovereign_account));
        (current_era, staking_ledger)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn fetch_relay_storages() -> (EraIndex, StakingLedger<RelayRuntime>) {
        let sovereign_account: <RelayRuntime as RelaySystemConfig>::AccountId =
            T::ParachainId::get().into_account_truncating();
        (1u32, StakingLedger::default_from(sovereign_account))
    }

    fn wrapped_dot_total_supply() -> T::Balance {
        let eqdot_aggregate = T::Aggregates::get_total(UserGroup::Balances, asset::EQDOT);
        eqdot_aggregate.collateral - eqdot_aggregate.debt
    }

    fn send_xcm_bond_extra(value: T::Balance) -> DispatchResult {
        let bond_amount =
            balance_into_xcm(value, DOT_DECIMALS).ok_or(Error::<T>::XcmBalanceConversionError)?;

        let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
            T::RelayChainCallBuilder::staking_bond_extra(bond_amount),
            StakingWeights::<T>::bond_extra(),
        );
        let result = send_xcm::<T::XcmRouter>(Parent.into(), xcm_message);
        ensure!(result.is_ok(), Error::<T>::XcmStakingBondExtraFailed);

        Ok(())
    }

    fn send_xcm_unbond(value: T::Balance) -> DispatchResult {
        // from substrate
        const SPECULATIVE_NUM_SPANS: u32 = 32;

        let unbond_amount =
            balance_into_xcm(value, DOT_DECIMALS).ok_or(Error::<T>::XcmBalanceConversionError)?;

        let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
            T::RelayChainCallBuilder::staking_unbond(unbond_amount),
            StakingWeights::<T>::withdraw_unbonded_kill(SPECULATIVE_NUM_SPANS),
        );
        let result = send_xcm::<T::XcmRouter>(Parent.into(), xcm_message);
        ensure!(result.is_ok(), Error::<T>::XcmStakingUnbondFailed);

        TotalUnlocking::<T>::mutate(|v| *v = v.saturating_add(value));

        Ok(())
    }

    fn send_xcm_withdraw_unbond(unlocking: T::Balance) -> DispatchResult {
        // Hardcoded
        const NUM_SLASHING_SPANS: u32 = 5;

        let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
            T::RelayChainCallBuilder::staking_withdraw_unbonded(NUM_SLASHING_SPANS),
            StakingWeights::<T>::withdraw_unbonded_kill(NUM_SLASHING_SPANS),
        );
        let result = send_xcm::<T::XcmRouter>(Parent.into(), xcm_message);
        ensure!(result.is_ok(), Error::<T>::XcmStakingWithdrawUnbondedFailed);

        TotalUnlocking::<T>::mutate(|v| *v = v.saturating_sub(unlocking));

        Ok(())
    }

    fn transfer_wrapped_dot_to_pallet(
        account_id: T::AccountId,
        wrapped_dot_amount: T::Balance,
    ) -> DispatchResult {
        let pallet_account_id = T::PalletId::get().into_account_truncating();
        T::EqCurrency::currency_transfer(
            &account_id,
            &pallet_account_id,
            asset::EQDOT,
            wrapped_dot_amount,
            ExistenceRequirement::KeepAlive,
            TransferReason::Common,
            true,
        )
    }

    fn calc_mint_wrapped_amount(deposit_amount: T::Balance) -> Result<T::Balance, DispatchError> {
        Self::dot_to_wrapped_dot_ratio()
            .and_then(|dot_wrapped_dot_ratio: EqFixedU128| dot_wrapped_dot_ratio.reciprocal())
            .and_then(|wrapped_dot_to_dot_ratio: EqFixedU128| {
                wrapped_dot_to_dot_ratio.checked_mul_int(deposit_amount)
            })
            .ok_or(Error::<T>::MathError.into())
    }

    fn calc_burn_wrapped_amount(withdraw_amount: T::Balance) -> Result<T::Balance, DispatchError> {
        let price_coeff: EqFixedU128 = Self::get_price_coeff().ok_or(ArithmeticError::Overflow)?;
        let price_coeff_reciprocal = price_coeff
            .reciprocal()
            .ok_or(ArithmeticError::DivisionByZero)?;

        // to_burn = withdraw_amount / price_coeff
        price_coeff_reciprocal
            .checked_mul_int(withdraw_amount)
            .ok_or(ArithmeticError::Overflow.into())
    }

    fn calc_deposit_amount(burn_wrapped_amount: T::Balance) -> Result<T::Balance, DispatchError> {
        let price_coeff: EqFixedU128 = Self::get_price_coeff().ok_or(ArithmeticError::Overflow)?;

        price_coeff
            .checked_mul_int(burn_wrapped_amount)
            .ok_or(ArithmeticError::Overflow.into())
    }

    fn get_withdraw_and_burn_amount(
        withdraw_amount: WithdrawAmount<T::Balance>,
    ) -> Result<(T::Balance, T::Balance), DispatchError> {
        match withdraw_amount {
            WithdrawAmount::Dot(withdraw_amount) => {
                let to_burn_amount = Self::calc_burn_wrapped_amount(withdraw_amount)?;
                Ok((withdraw_amount, to_burn_amount))
            }
            WithdrawAmount::EqDot(to_burn_amount) => {
                let withdraw_amount = Self::calc_deposit_amount(to_burn_amount)?;
                Ok((withdraw_amount, to_burn_amount))
            }
        }
    }

    fn burn_dot_deposit_wrapped_dot(
        account_id: T::AccountId,
        deposit_amount: T::Balance,
    ) -> DispatchResult {
        T::EqCurrency::withdraw(
            &account_id,
            asset::DOT,
            deposit_amount,
            true,
            Some(WithdrawReason::Staking),
            WithdrawReasons::empty(),
            ExistenceRequirement::KeepAlive,
        )?;

        let mint_amount = Self::calc_mint_wrapped_amount(deposit_amount)?;
        T::EqCurrency::deposit_creating(
            &account_id,
            asset::EQDOT,
            mint_amount,
            true,
            Some(DepositReason::Staking),
        )
    }

    fn deposit_dot_burn_wrapped_dot(
        account_id: T::AccountId,
        deposit_amount: T::Balance,
        burn_amount: T::Balance,
    ) -> DispatchResult {
        T::EqCurrency::deposit_creating(
            &account_id,
            asset::DOT,
            deposit_amount,
            true,
            Some(DepositReason::Staking),
        )?;

        T::EqCurrency::withdraw(
            &account_id,
            asset::EQDOT,
            burn_amount,
            true,
            Some(WithdrawReason::Staking),
            WithdrawReasons::empty(),
            ExistenceRequirement::KeepAlive,
        )
    }

    fn dot_to_wrapped_dot_ratio<
        FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>,
    >() -> Option<FixedNumber> {
        let wrapped_dot_supply = Self::wrapped_dot_total_supply();

        FixedNumber::checked_from_rational(CurrentBalance::<T>::get().total(), wrapped_dot_supply)
    }
}

impl<T: Config> EqDotPrice for Pallet<T> {
    fn get_price_coeff<FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>>(
    ) -> Option<FixedNumber> {
        // price will be None for initial/ zero supply state
        let price_coeff: FixedI64 = Self::dot_to_wrapped_dot_ratio()?;

        (price_coeff * T::WithdrawFee::get().into()).try_into().ok()
    }
}
