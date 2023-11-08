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

//! # Equilibrium Distribution Pallet
//!
//! This is a template/placeholder pallet.
//! Equilibrium uses it to allocate EQ tokens for different user-groups per our tokenomics: For example Investors, PLO (parachain lease offering),
//! and Liquidity Farming groups are all represented by instances of the Distribution pallet with preset initial balances of EQ tokens.
//!
//! Distribution pallet allows for transfer and vested_transfer of assets from its balance to specified accounts.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(warnings)]

pub mod benchmarking;
mod mock;
mod origin;
mod tests;
pub mod weights;

use core::convert::{TryFrom, TryInto};
use eq_primitives::vestings::EqVestingSchedule;
use eq_utils::eq_ensure;
use frame_support::traits::{ExistenceRequirement, Get};
use frame_support::PalletId;
use frame_system::pallet_prelude::OriginFor;
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned, Zero};

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::origin::EnsureManager;
    use eq_primitives::asset::{Asset, AssetGetter};
    use eq_primitives::balance::EqCurrency;
    use eq_primitives::TransferReason;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::EitherOfDiverse;
    use frame_system::pallet_prelude::*;
    // use frame_system::EnsureRoot;

    pub type EnsureManagerOrManagementOrigin<T, I> =
        EitherOfDiverse<EnsureManager<T, I>, <T as Config<I>>::ManagementOrigin>;

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        /// Pallet's AccountId for balance
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Numerical representation of stored balances
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + TryFrom<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>;
        type ManagementOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Gets vesting account (for vesting transfers).
        type VestingAccountId: Get<Self::AccountId>;
        /// Used to schedule vesting part of a claim.
        type Vesting: EqVestingSchedule<Self::Balance, Self::AccountId, Moment = Self::BlockNumber>;
        /// Used to deal with Native Asset
        type AssetGetter: AssetGetter;
        /// Used for currency-related operations and calculations
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn manager)]
    pub type PalletManager<T: Config<I>, I: 'static = ()> = StorageValue<_, T::AccountId>;

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Transfer funds from pallets account
        ///
        /// The dispatch origin for this call must be _Root_ or _Manager_.
        ///
        /// Parameters:
        ///  - `asset`: The asset that will be transfered;
        ///  - `target`: The account that should be transferred funds;
        ///  - `value`: The amount of `asset` that will be transferred.
        #[pallet::call_index(0)]
        #[pallet::weight((
            T::WeightInfo::transfer(),
            DispatchClass::Normal
        ))]
        pub fn transfer(
            origin: OriginFor<T>,
            asset: Asset,
            target: T::AccountId,
            value: T::Balance,
        ) -> DispatchResultWithPostInfo {
            <EnsureManagerOrManagementOrigin<T, I>>::ensure_origin(origin)?;

            T::EqCurrency::currency_transfer(
                &Self::account_id(),
                &target,
                asset,
                value,
                ExistenceRequirement::AllowDeath,
                TransferReason::Common,
                true,
            )?;

            Ok(().into())
        }

        /// Transfer funds from pallets account with vesting
        ///
        /// The dispatch origin for this call must be _Root_ or _Manager_.
        ///
        /// Parameters:
        ///  - `target`: The account that should be transferred funds.
        ///  - `schedule`: The vesting schedule:
        ///  -  First balance is the total amount that should be held for vesting.
        ///  -  Second balance is how much should be unlocked per block.
        ///  -  The block number is when the vesting should start.
        #[pallet::call_index(1)]
        #[pallet::weight((
            T::WeightInfo::vested_transfer(),
            DispatchClass::Normal,
        ))]
        pub fn vested_transfer(
            origin: OriginFor<T>,
            target: T::AccountId,
            schedule: (T::Balance, T::Balance, T::BlockNumber),
        ) -> DispatchResultWithPostInfo {
            <EnsureManagerOrManagementOrigin<T, I>>::ensure_origin(origin)?;

            eq_ensure!(
                schedule.1 > T::Balance::zero(),
                Error::<T, I>::AmountLow,
                target: "eq_distribuiton",
                "{}:{}. Schedule per block equals zero. Schedule: {:?}.",
                file!(),
                line!(),
                schedule.1
            );
            eq_ensure!(
                T::Vesting::vesting_balance(&target).is_none(),
                Error::<T, I>::ExistingVestingSchedule,
                target: "eq_distribuiton",
                "{}:{}. An existing vesting schedule already exists for account. Who: {:?}.",
                file!(),
                line!(),
                target
            );
            // we need firstly to transfer funds to Vesting account
            T::EqCurrency::currency_transfer(
                &Self::account_id(),
                &T::VestingAccountId::get(),
                T::AssetGetter::get_main_asset(),
                schedule.0,
                ExistenceRequirement::AllowDeath,
                TransferReason::Common,
                true,
            )?;
            // We do not expect error as a result of add_vesting_schedule method
            T::Vesting::add_vesting_schedule(&target, schedule.0, schedule.1, schedule.2)
                .expect("user does not have an existing vesting schedule; q.e.d.");
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// An existing vesting schedule already exists for this account that cannot be clobbered
        ExistingVestingSchedule,

        /// Amount being transferred is too low to create a vesting schedule
        AmountLow,
    }

    // empty genesis, only for adding ref to module's AccountId
    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub empty: (),
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                empty: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |_: &GenesisConfig| {
                use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};
                EqPalletAccountInitializer::<T>::initialize(
                    &T::PalletId::get().into_account_truncating(),
                );
            };
            extra_genesis_builder(self);
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    // helper for module's AccountId
    pub fn account_id() -> T::AccountId {
        T::PalletId::get().into_account_truncating()
    }
}

/// Instance16 to be used for instantiable pallet define with `pallet` macro.
#[cfg(feature = "runtime-benchmarks")]
#[derive(Clone, Copy, PartialEq, Eq, frame_support::RuntimeDebugNoBound)]
pub struct Instance16;

#[cfg(feature = "runtime-benchmarks")]
impl frame_support::traits::Instance for Instance16 {
    const PREFIX: &'static str = "DistriBenchInstance";
    const INDEX: u8 = 0;
}
