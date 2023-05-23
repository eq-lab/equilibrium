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

//! # Equilibrium Whitelist Pallet
//!
//! Simple whitelist functionality, work only with the oracle module. Accounts may be added to whitelist / removed from whitelist delisted.
//! There are methods to check if an account is whitelisted and to get the list of all whitelisted accounts.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

pub mod benchmarking;
mod mock;
mod tests;
pub mod weights;

use core::convert::TryInto;
use eq_primitives::{AccountRefCounter, AccountRefCounts};
use sp_std::prelude::*;
pub use weights::WeightInfo;

/// Interface for checking whitelisted accounts
pub trait CheckWhitelisted<AccountId> {
    /// Checks if `account_id` is in whitelist
    fn in_whitelist(account_id: &AccountId) -> bool;
    /// Gets a vector of all whitelisted accounts
    fn accounts() -> Vec<AccountId>;
}

pub trait OnRemove<AccountId> {
    /// External actions after removing
    fn on_remove(who: &AccountId);
}

impl<AccountId> OnRemove<AccountId> for () {
    fn on_remove(_: &AccountId) {}
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type WhitelistManagementOrigin: EnsureOrigin<Self::Origin>;
        /// External actions after removing account from whitelist
        type OnRemove: OnRemove<Self::AccountId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Adds a `who_to_add` account to whitelist. Requires root authorization
        #[pallet::call_index(0)]
        #[pallet::weight((
            T::WeightInfo::add_to_whitelist(),
            DispatchClass::Normal))
        ]
        pub fn add_to_whitelist(
            origin: OriginFor<T>,
            who_to_add: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            T::WhitelistManagementOrigin::ensure_origin(origin)?;

            let mut accounts = WhiteList::<T>::get().unwrap_or_default();
            match accounts.binary_search(&who_to_add) {
                Ok(_) => frame_support::fail!(Error::<T>::AlreadyAdded),
                Err(index) => accounts.insert(index, who_to_add.clone()),
            }

            <WhiteList<T>>::put(accounts);

            // we don't want the whitelisted account to be "killed"
            AccountRefCounter::<T>::inc_ref(&who_to_add);

            Self::deposit_event(Event::AddedToWhitelist(who_to_add));

            Ok(().into())
        }

        /// Removes an account `who_to_remove` from whitelist. Requires sudo authorization
        #[pallet::call_index(1)]
        #[pallet::weight((
            T::WeightInfo::remove_from_whitelist(),
            DispatchClass::Normal
        ))]
        pub fn remove_from_whitelist(
            origin: OriginFor<T>,
            who_to_remove: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            T::WhitelistManagementOrigin::ensure_origin(origin)?;

            let mut accounts = WhiteList::<T>::get().unwrap_or_default();
            match accounts.binary_search(&who_to_remove) {
                Ok(index) => accounts.remove(index),
                Err(_) => frame_support::fail!(Error::<T>::AlreadyRemoved),
            };

            <WhiteList<T>>::put(accounts);
            // The account can be killed now
            AccountRefCounter::<T>::dec_ref(&who_to_remove);

            T::OnRemove::on_remove(&who_to_remove);

            Self::deposit_event(Event::RemovedFromWhitelist(who_to_remove));

            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// `AccountId` was added to the whitelist. \[who\]
        AddedToWhitelist(T::AccountId),
        /// `AccountId` was removed from the whitelist. \[who\]
        RemovedFromWhitelist(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Account was not added to whitelist: already in whitelist
        AlreadyAdded,
        /// Account was not removed from whitelist: not in whitelist
        AlreadyRemoved,
    }
    /// Storage of all whitelisted `AccountId`s
    #[pallet::storage]
    #[pallet::getter(fn whitelists)]
    pub type WhiteList<T: Config> = StorageValue<_, Vec<T::AccountId>>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub whitelist: Vec<T::AccountId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                whitelist: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |config: &GenesisConfig<T>| {
                let mut accounts = Vec::new();
                for &ref who in config.whitelist.iter() {
                    accounts.push(who);
                    AccountRefCounter::<T>::inc_ref(&who);
                }
                accounts.sort();
                WhiteList::<T>::put(accounts);
            };
            extra_genesis_builder(self);
        }
    }
}

impl<T: Config> CheckWhitelisted<T::AccountId> for Pallet<T> {
    fn in_whitelist(account_id: &T::AccountId) -> bool {
        let accounts = WhiteList::<T>::get().unwrap_or_default();
        accounts.binary_search(account_id).is_ok()
    }
    fn accounts() -> Vec<T::AccountId> {
        WhiteList::<T>::get().unwrap_or_default()
    }
}

pub mod migrations {
    use super::*;
    use frame_support::{
        migration::StorageKeyIterator, storage::generator::StorageValue, Blake2_128Concat,
    };

    pub fn migrate<T: Config>() {
        #[allow(deprecated)]
        let old_accounts = <StorageKeyIterator<T::AccountId, bool, Blake2_128Concat>>::new(
            <WhiteList<T>>::module_prefix(),
            <WhiteList<T>>::storage_prefix(),
        )
        .drain();
        let mut accounts: Vec<T::AccountId> = Vec::new();
        for (who, is_in_whitelist) in old_accounts {
            if is_in_whitelist {
                accounts.push(who);
            }
        }
        accounts.sort();
        WhiteList::<T>::put(accounts);
    }
}