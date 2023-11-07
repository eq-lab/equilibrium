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

use codec::{Decode, Encode};
use core::ops::{Div, Sub};
use eq_primitives::asset::{EQ, Q};
use eq_primitives::balance::{BalanceGetter, DepositReason, EqCurrency, WithdrawReason};
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::vestings::EqVestingSchedule;
use eq_primitives::Vesting;
use eq_utils::eq_ensure;
use frame_support::pallet_prelude::DispatchResult;
use frame_support::traits::{ExistenceRequirement, Get, WithdrawReasons};
use frame_support::transactional;
use frame_support::weights::Weight;
use scale_info::TypeInfo;
use sp_runtime::traits::{AtLeast32BitUnsigned, Zero};
use sp_runtime::{ArithmeticError, FixedPointNumber, FixedPointOperand, Percent};
use sp_std::convert::{TryFrom, TryInto};

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
    }

    /// Stores EQ-to-Q swap configuration
    #[pallet::storage]
    pub type EqSwapConfiguration<T: Config> =
        StorageValue<_, SwapConfiguration<T::Balance, T::BlockNumber>, ValueQuery>;

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
            mb_eq_to_q_ratio: Option<u128>,
            mb_vesting_share: Option<Percent>,
            mb_vesting_starting_block: Option<T::BlockNumber>,
            mb_vesting_duration_blocks: Option<T::Balance>,
        ) -> DispatchResultWithPostInfo {
            T::SetEqSwapConfigurationOrigin::ensure_origin(origin)?;

            Self::do_set_config(
                mb_enabled,
                mb_eq_to_q_ratio,
                mb_vesting_share,
                mb_vesting_starting_block,
                mb_vesting_duration_blocks,
            )?;

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads(1))]
        #[transactional]
        pub fn swap_eq_to_q(
            origin: OriginFor<T>,
            amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let caller = ensure_signed(origin)?;
            let configuration = EqSwapConfiguration::<T>::get();

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

    fn do_set_config(
        mb_enabled: Option<bool>,
        mb_eq_to_q_ratio: Option<u128>,
        mb_vesting_share: Option<Percent>,
        mb_vesting_starting_block: Option<T::BlockNumber>,
        mb_vesting_duration_blocks: Option<T::Balance>,
    ) -> DispatchResult {
        let mut configuration = EqSwapConfiguration::<T>::get();

        if let Some(mb_enabled) = mb_enabled {
            configuration.enabled = mb_enabled;
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
            || !configuration.eq_to_q_ratio.is_zero()
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
        let remaining_balance = balance
            .sub_balance(amount)
            .ok_or(ArithmeticError::Underflow)?;

        frame_support::ensure!(
            !amount.is_zero() && remaining_balance.is_positive(),
            Error::<T>::NotEnoughBalance
        );

        let q_total_amount = EqFixedU128::from_inner(configuration.eq_to_q_ratio)
            .checked_mul_int(*amount)
            .ok_or(ArithmeticError::Overflow)?;

        let mut q_amount = q_total_amount;
        let mut vesting_amount = T::Balance::zero();

        if !configuration.vesting_share.is_zero() {
            let vesting_account_id = T::VestingAccountId::get();
            vesting_amount = configuration.vesting_share.mul_floor(q_total_amount);
            q_amount = q_total_amount.sub(vesting_amount);

            T::EqCurrency::deposit_creating(
                &vesting_account_id,
                Q,
                q_amount,
                false,
                Some(DepositReason::SwapEqToQ),
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

        let q_holder_account_id = T::QHolderAccountId::get();

        T::EqCurrency::withdraw(
            who,
            EQ,
            *amount,
            false,
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
            false,
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

#[derive(Default, Debug, Decode, Encode, PartialEq, TypeInfo)]
pub struct SwapConfiguration<Balance, BlockNumber> {
    pub enabled: bool,
    pub eq_to_q_ratio: u128,
    pub vesting_share: Percent,
    pub vesting_starting_block: BlockNumber,
    pub vesting_duration_blocks: Balance,
}