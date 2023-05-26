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

pub mod weights;

pub use weights::PalletWeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::WeightInfo;
    use frame_system::{pallet_prelude::*, KeyValue};
    use sp_std::{convert::TryInto, vec::Vec};

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: PalletWeightInfo;
        /// Set storage calls per block
        #[pallet::constant]
        type MigrationsPerBlock: Get<u16>;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Adds new migration, if there is no migration in storage
        #[pallet::call_index(0)]
        #[pallet::weight((
            T::WeightInfo::set_migration(),
            DispatchClass::Operational))
        ]
        pub fn set_migration(
            origin: OriginFor<T>,
            migration: Vec<KeyValue>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            ensure!(!Migration::<T>::exists(), Error::<T>::MigrationIsInProgress);

            let migration_len: u16 = migration
                .len()
                .try_into()
                .map_err(|_| sp_runtime::ArithmeticError::Overflow)?;

            Migration::<T>::put(migration);

            Self::deposit_event(Event::MigrationSetted(migration_len));

            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
            let maybe_migration = Migration::<T>::get();

            match maybe_migration {
                Some(mut migration) => {
                    let range_to: usize = T::MigrationsPerBlock::get().into();
                    let to_migrate = range_to.min(migration.len());
                    let migration_items = migration.drain(..to_migrate);

                    for (storage_key, storage_value) in migration_items {
                        frame_support::storage::unhashed::put_raw(&storage_key, &storage_value);
                    }

                    // we checked u16 overflow on migration set
                    let to_migrate_typed: u16 = to_migrate.try_into().unwrap_or_default();

                    Self::deposit_event(Event::MigrationProcessed(to_migrate_typed));

                    if migration.is_empty() {
                        Migration::<T>::kill();
                        Self::deposit_event(Event::Migrated());
                    } else {
                        Migration::<T>::put(migration);
                    }

                    <T as frame_system::Config>::SystemWeightInfo::set_storage(
                        to_migrate_typed.into(),
                    )
                }
                None => T::DbWeight::get().reads(1),
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New migration with N length added
        MigrationSetted(u16),
        /// Migration processed N items
        MigrationProcessed(u16),
        /// Migration completed
        Migrated(),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// We cannot start new migration before current ended
        MigrationIsInProgress,
    }

    /// Current migration
    #[pallet::storage]
    #[pallet::getter(fn migration)]
    pub type Migration<T: Config> = StorageValue<_, Vec<KeyValue>>;
}
