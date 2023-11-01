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
use eq_primitives::balance::{BalanceGetter, EqCurrency};
use eq_primitives::Vesting;
use eq_utils::eq_ensure;
use frame_support::pallet_prelude::DispatchResult;
use frame_support::traits::VestingSchedule;
use frame_support::transactional;
use scale_info::TypeInfo;
use sp_runtime::traits::AtLeast32BitUnsigned;
use sp_runtime::Percent;
use sp_std::convert::{TryFrom, TryInto};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
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
            + TryFrom<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>;
        /// Origin for setting configuration
        type SetEqSwapConfigurationOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        // Used for managing vestings
        type VestingManager: Vesting<Self::AccountId>
            + VestingSchedule<Self::AccountId, Moment = Self::BlockNumber>;
        /// Used for managing balances and currencies
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>
            + BalanceGetter<Self::AccountId, Self::Balance>;
    }

    /// Stores EQ-to-Q swap configuration
    #[pallet::storage]
    pub type EqSwapConfiguration<T: Config> = StorageValue<_, SwapConfiguration, ValueQuery>;

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
            mb_vesting_starting_block: Option<u64>,
            mb_vesting_duration_blocks: Option<u64>,
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
        // TBD
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 2))]
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
    fn ensure_eq_swap_enabled(configuration: &SwapConfiguration) -> DispatchResult {
        eq_ensure!(
            !configuration.enabled,
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
        mb_vesting_starting_block: Option<u64>,
        mb_vesting_duration_blocks: Option<u64>,
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
            || configuration.eq_to_q_ratio == 0
            || configuration.vesting_starting_block == 0
            || configuration.vesting_duration_blocks == 0;

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
        _configuration: &SwapConfiguration,
    ) -> DispatchResult {
        // TBD
        Self::deposit_event(Event::EqToQSwap(who.clone(), *amount, *amount, *amount));

        Ok(())
    }
}

#[derive(Default, Decode, Encode, PartialEq, TypeInfo)]
pub struct SwapConfiguration {
    pub enabled: bool,
    pub eq_to_q_ratio: u128,
    pub vesting_share: Percent,
    pub vesting_starting_block: u64,
    pub vesting_duration_blocks: u64,
}
