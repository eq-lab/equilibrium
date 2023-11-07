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

//! # Equilibrium EQ-to-Q Swap

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(warnings)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

use codec::{Decode, Encode};
use core::ops::{Div, Sub};
use eq_primitives::asset::{EQ, Q};
use eq_primitives::balance::{BalanceGetter, EqCurrency, WithdrawReason};
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::vestings::EqVestingSchedule;
use eq_primitives::{SignedBalance, Vesting};
use eq_utils::eq_ensure;
use frame_support::pallet_prelude::DispatchResult;
use frame_support::traits::{ExistenceRequirement, Get, IsSubType, WithdrawReasons};
use frame_support::transactional;
use frame_support::weights::Weight;
use scale_info::TypeInfo;
use sp_runtime::traits::{AtLeast32BitUnsigned, CheckedAdd, DispatchInfoOf, SignedExtension, Zero};
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction,
};
use sp_runtime::{ArithmeticError, FixedPointNumber, FixedPointOperand, Percent};
use sp_std::convert::{TryFrom, TryInto};
use sp_std::fmt::Debug;
use sp_std::marker::PhantomData;
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use eq_primitives::Vesting;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Numerical representation of stored balances
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + FixedPointOperand
            + TryFrom<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>;
        /// Origin for setting configuration
        type SetEqSwapConfigurationOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        // Used for managing vestings
        type Vesting: Vesting<Self::AccountId>
            + EqVestingSchedule<Self::Balance, Self::AccountId, Moment = Self::BlockNumber>;
        /// Used for managing balances and currencies
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>
            + BalanceGetter<Self::AccountId, Self::Balance>;
        /// Returns vesting account
        type VestingAccountId: Get<Self::AccountId>;
        /// Returns Q holder account
        type QHolderAccountId: Get<Self::AccountId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// Stores EQ-to-Q swap configuration
    #[pallet::storage]
    pub type EqSwapConfiguration<T: Config> =
        StorageValue<_, SwapConfiguration<T::Balance, T::BlockNumber>, ValueQuery>;

    /// Stores Q amount transferred to users
    #[pallet::storage]
    pub type QReceivedAmounts<T: Config> =
        StorageMap<_, Identity, T::AccountId, T::Balance, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Transfer event. Included values are:
        /// - from `AccountId`
        /// - requested amount
        /// - Q received amount
        /// - Q vested amount
        /// \[from, amount_1, amount_2, amount_3\]
        EqToQSwap(T::AccountId, T::Balance, T::Balance, T::Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Swaps are disabled
        SwapsAreDisabled,
        /// Configuration is invalid
        InvalidConfiguration,
        /// Available balance is not enough to perform swap
        NotEnoughBalance,
        /// Specified amount is too small to perform swap
        AmountTooSmall,
        /// Q amount exceeded for a given account.
        QAmountExceeded,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut configuration = EqSwapConfiguration::<T>::get();

            if configuration.enabled && configuration.vesting_starting_block.le(&now) {
                configuration.enabled = false;
                EqSwapConfiguration::<T>::put(configuration);

                return T::DbWeight::get().reads_writes(1, 1);
            }

            T::DbWeight::get().reads(1)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_config(
            origin: OriginFor<T>,
            mb_enabled: Option<bool>,
            mb_min_amount: Option<T::Balance>,
            mb_max_q_amount: Option<T::Balance>,
            mb_eq_to_q_ratio: Option<u128>,
            mb_vesting_share: Option<Percent>,
            mb_vesting_starting_block: Option<T::BlockNumber>,
            mb_vesting_duration_blocks: Option<T::Balance>,
        ) -> DispatchResultWithPostInfo {
            T::SetEqSwapConfigurationOrigin::ensure_origin(origin)?;

            Self::do_set_config(
                mb_enabled,
                mb_min_amount,
                mb_max_q_amount,
                mb_eq_to_q_ratio,
                mb_vesting_share,
                mb_vesting_starting_block,
                mb_vesting_duration_blocks,
            )?;

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight((T::WeightInfo::swap_eq_to_q(), DispatchClass::Normal, Pays::No))]
        #[transactional]
        pub fn swap_eq_to_q(
            origin: OriginFor<T>,
            amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let caller = ensure_signed(origin)?;
            let configuration = EqSwapConfiguration::<T>::get();

            Self::ensure_valid_amount(&configuration, &amount)?;
            Self::ensure_eq_swap_enabled(&configuration)?;
            Self::do_swap_eq_to_q(&caller, &amount, &configuration)?;

            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    fn ensure_eq_swap_enabled(
        configuration: &SwapConfiguration<T::Balance, T::BlockNumber>,
    ) -> DispatchResult {
        eq_ensure!(
            configuration.enabled,
            Error::<T>::SwapsAreDisabled,
            target: "eq_to_q_swap",
            "{}:{}. EQ swap is not allowed.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn ensure_valid_amount(
        configuration: &SwapConfiguration<T::Balance, T::BlockNumber>,
        amount: &T::Balance,
    ) -> DispatchResult {
        eq_ensure!(
            amount.ge(&configuration.min_amount),
            Error::<T>::AmountTooSmall,
            target: "eq_to_q_swap",
            "{}:{}. Specified amount is too small to perform swap.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn ensure_q_amount_not_exceeded(
        configuration: &SwapConfiguration<T::Balance, T::BlockNumber>,
        q_received: &T::Balance,
    ) -> DispatchResult {
        eq_ensure!(
            q_received.le(&configuration.max_q_amount),
            Error::<T>::QAmountExceeded,
            target: "eq_to_q_swap",
            "{}:{}. Q amount exceeded for a given account.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn ensure_enough_balance(
        balance: &SignedBalance<T::Balance>,
        amount: &T::Balance,
    ) -> DispatchResult {
        let remaining_balance = balance
            .sub_balance(amount)
            .ok_or(ArithmeticError::Underflow)?;

        eq_ensure!(
            remaining_balance.is_positive(),
            Error::<T>::NotEnoughBalance,
            target: "eq_to_q_swap",
            "{}:{}. Available balance is not enough to perform swap.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn do_set_config(
        mb_enabled: Option<bool>,
        mb_min_amount: Option<T::Balance>,
        max_q_amount: Option<T::Balance>,
        mb_eq_to_q_ratio: Option<u128>,
        mb_vesting_share: Option<Percent>,
        mb_vesting_starting_block: Option<T::BlockNumber>,
        mb_vesting_duration_blocks: Option<T::Balance>,
    ) -> DispatchResult {
        let mut configuration = EqSwapConfiguration::<T>::get();

        if let Some(mb_enabled) = mb_enabled {
            configuration.enabled = mb_enabled;
        }

        if let Some(mb_min_amount) = mb_min_amount {
            configuration.min_amount = mb_min_amount;
        }

        if let Some(max_q_amount) = max_q_amount {
            configuration.max_q_amount = max_q_amount;
        }

        if let Some(mb_eq_to_q_ratio) = mb_eq_to_q_ratio {
            configuration.eq_to_q_ratio = mb_eq_to_q_ratio;
        }

        if let Some(mb_vesting_share) = mb_vesting_share {
            configuration.vesting_share = mb_vesting_share;
        }

        if let Some(mb_vesting_starting_block) = mb_vesting_starting_block {
            configuration.vesting_starting_block = mb_vesting_starting_block;
        }

        if let Some(mb_vesting_duration_blocks) = mb_vesting_duration_blocks {
            configuration.vesting_duration_blocks = mb_vesting_duration_blocks;
        }

        let is_valid_configuration = !configuration.enabled
            || configuration.min_amount.gt(&T::Balance::zero())
                && configuration.max_q_amount.gt(&T::Balance::zero())
                && !configuration.eq_to_q_ratio.is_zero()
                && !configuration.vesting_starting_block.is_zero()
                && !configuration.vesting_duration_blocks.is_zero();

        eq_ensure!(
            is_valid_configuration,
            Error::<T>::InvalidConfiguration,
            target: "eq_to_q_swap",
            "{}:{}. Invalid configuration provided.",
            file!(),
            line!(),
        );

        EqSwapConfiguration::<T>::put(configuration);

        Ok(())
    }

    fn do_swap_eq_to_q(
        who: &T::AccountId,
        amount: &T::Balance,
        configuration: &SwapConfiguration<T::Balance, T::BlockNumber>,
    ) -> DispatchResult {
        let balance = T::EqCurrency::get_balance(who, &EQ);
        Self::ensure_enough_balance(&balance, amount)?;

        let q_total_amount = EqFixedU128::from_inner(configuration.eq_to_q_ratio)
            .checked_mul_int(*amount)
            .ok_or(ArithmeticError::Overflow)?;

        let q_holder_account_id = T::QHolderAccountId::get();
        let mut q_amount = q_total_amount;
        let mut vesting_amount = T::Balance::zero();

        if !configuration.vesting_share.is_zero() {
            let vesting_account_id = T::VestingAccountId::get();
            vesting_amount = configuration.vesting_share.mul_floor(q_total_amount);
            q_amount = q_total_amount.sub(vesting_amount);

            T::EqCurrency::currency_transfer(
                &q_holder_account_id,
                &vesting_account_id,
                Q,
                vesting_amount,
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::SwapEqToQ,
                true,
            )?;

            if T::Vesting::has_vesting_schedule(who.clone()) {
                T::Vesting::update_vesting_schedule(
                    who,
                    vesting_amount,
                    configuration.vesting_duration_blocks,
                )?;
            } else {
                let per_block = configuration
                    .vesting_duration_blocks
                    .lt(&vesting_amount)
                    .then(|| vesting_amount.div(configuration.vesting_duration_blocks))
                    .unwrap_or(vesting_amount.div(vesting_amount));

                T::Vesting::add_vesting_schedule(
                    who,
                    vesting_amount,
                    per_block,
                    configuration.vesting_starting_block,
                )?;
            }
        }

        let q_received = QReceivedAmounts::<T>::get(who)
            .checked_add(&q_amount)
            .ok_or(ArithmeticError::Overflow)?;
        Self::ensure_q_amount_not_exceeded(&configuration, &q_received)?;

        QReceivedAmounts::<T>::insert(who, q_received);

        T::EqCurrency::withdraw(
            who,
            EQ,
            *amount,
            true,
            Some(WithdrawReason::SwapEqToQ),
            WithdrawReasons::empty(),
            ExistenceRequirement::KeepAlive,
        )?;

        T::EqCurrency::currency_transfer(
            &q_holder_account_id,
            who,
            Q,
            q_amount,
            ExistenceRequirement::AllowDeath,
            eq_primitives::TransferReason::SwapEqToQ,
            true,
        )?;

        Self::deposit_event(Event::EqToQSwap(
            who.clone(),
            *amount,
            q_amount,
            vesting_amount,
        ));

        Ok(())
    }
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, scale_info::TypeInfo)]
pub struct CheckEqToQSwap<T: Config + Send + Sync + scale_info::TypeInfo>(PhantomData<T>)
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>;

impl<T: Config + Send + Sync + scale_info::TypeInfo> Debug for CheckEqToQSwap<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "CheckEqToQSwap")
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> Default for CheckEqToQSwap<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> CheckEqToQSwap<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> SignedExtension for CheckEqToQSwap<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    const IDENTIFIER: &'static str = "CheckEqToQSwap";
    type AccountId = T::AccountId;
    type Call = T::RuntimeCall;
    type AdditionalSigned = ();
    type Pre = ();

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        Ok(())
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        self.validate(who, call, info, len)
            .map(|_| Self::Pre::default())
            .map_err(Into::into)
    }

    /// Checks:
    /// - Swap is enabled.
    /// - Available balance is enough to perform swap.
    /// - Swapping amount is greater or equal to the minimum swap amount.
    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> TransactionValidity {
        if let Some(local_call) = call.is_sub_type() {
            if let Call::swap_eq_to_q { amount } = local_call {
                let configuration = EqSwapConfiguration::<T>::get();

                Pallet::<T>::ensure_eq_swap_enabled(&configuration).map_err(|_| {
                    InvalidTransaction::Custom(ValidityError::SwapsAreDisabled.into())
                })?;
                Pallet::<T>::ensure_valid_amount(&configuration, amount).map_err(|_| {
                    InvalidTransaction::Custom(ValidityError::AmountTooSmall.into())
                })?;

                let balance = T::EqCurrency::get_balance(who, &EQ);

                Pallet::<T>::ensure_enough_balance(&balance, amount).map_err(|_| {
                    InvalidTransaction::Custom(ValidityError::NotEnoughBalance.into())
                })?;

                let q_received = QReceivedAmounts::<T>::get(who).checked_add(amount).ok_or(
                    InvalidTransaction::Custom(ValidityError::QAmountExceeded.into()),
                )?;

                Pallet::<T>::ensure_q_amount_not_exceeded(&configuration, &q_received).map_err(
                    |_| InvalidTransaction::Custom(ValidityError::QAmountExceeded.into()),
                )?;
            }
        }

        Ok(ValidTransaction::default())
    }
}

/// Claim validation errors
#[repr(u8)]
pub enum ValidityError {
    /// Swaps are disabled
    SwapsAreDisabled = 1,
    /// Configuration is invalid
    InvalidConfiguration = 2,
    /// Available balance is not enough to perform swap
    NotEnoughBalance = 3,
    /// Specified amount is too small to perform swap
    AmountTooSmall = 4,
    /// Q amount exceeded for a given account.
    QAmountExceeded = 5,
}

impl From<ValidityError> for u8 {
    fn from(err: ValidityError) -> Self {
        err as u8
    }
}

#[derive(Default, Debug, Decode, Encode, PartialEq, TypeInfo)]
pub struct SwapConfiguration<Balance, BlockNumber> {
    pub enabled: bool,
    pub min_amount: Balance,
    pub max_q_amount: Balance,
    pub eq_to_q_ratio: u128,
    pub vesting_share: Percent,
    pub vesting_starting_block: BlockNumber,
    pub vesting_duration_blocks: Balance,
}
