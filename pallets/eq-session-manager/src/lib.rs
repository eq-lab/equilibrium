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

//! # Equilibrium Session Manager Pallet
//!
//! Equilibrium's Session Manager Pallet is a Substrate module that manages
//! validation of Equilibrium POA substrate

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

use core::convert::TryInto;
use eq_primitives::{AccountRefCounter, AccountRefCounts};
use eq_utils::eq_ensure;
use frame_support::{traits::ValidatorRegistration, Parameter};
use pallet_session::SessionManager;
use sp_runtime::traits::{Convert, MaybeSerializeDeserialize, Member};
use sp_staking::SessionIndex;
use sp_std::prelude::*;

mod mock;
mod tests;

pub mod benchmarking;
pub mod weights;
pub use weights::WeightInfo;

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
        type ValidatorsManagementOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Representation of validator id
        type ValidatorId: Member
            + Parameter
            + MaybeSerializeDeserialize
            + Into<<Self as frame_system::Config>::AccountId>;
        /// Manages validator registration and unregistration
        type RegistrationChecker: ValidatorRegistration<<Self as pallet::Config>::ValidatorId>;
        /// A conversion from account ID to validator ID.
        type ValidatorIdOf: Convert<
            <Self as frame_system::Config>::AccountId,
            Option<<Self as pallet::Config>::ValidatorId>,
        >;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::add_validator())]
        pub fn add_validator(
            origin: OriginFor<T>,
            validator_id: <T as pallet::Config>::ValidatorId,
        ) -> DispatchResultWithPostInfo {
            T::ValidatorsManagementOrigin::ensure_origin(origin)?;

            let is_registered = T::RegistrationChecker::is_registered(&validator_id);
            eq_ensure!(
                is_registered,
                Error::<T>::NotRegistered,
                target: "eq_session_manager",
                "{}:{}. Validator is not registered. Validator id: {:?}.",
                file!(),
                line!(),
                validator_id
            );

            let validator = <Validators<T>>::get(&validator_id);
            eq_ensure!(
                !validator,
                Error::<T>::AlreadyAdded,
                target: "eq_session_manager",
                "{}:{}. Validator is already added. Validator id: {:?}.",
                file!(),
                line!(),
                validator_id
            );

            <Validators<T>>::insert(&validator_id, true);

            <IsChanged<T>>::put(true);

            log::warn!("Validator {:?} added", validator_id);

            // Substrate's Session Module increment consumers, so wee don't need to do that
            AccountRefCounter::<T>::inc_ref(&validator_id.clone().into());

            Self::deposit_event(Event::ValidatorAdded(validator_id));

            Ok(().into())
        }

        /// Removes validator. Root authorization required to remove validator.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_validator())]
        pub fn remove_validator(
            origin: OriginFor<T>,
            validator_id: <T as pallet::Config>::ValidatorId,
        ) -> DispatchResultWithPostInfo {
            T::ValidatorsManagementOrigin::ensure_origin(origin)?;

            let validator = <Validators<T>>::get(&validator_id);
            eq_ensure!(
                validator,
                Error::<T>::AlreadyRemoved,
                target: "eq_session_manager",
                "{}:{}. Validator is already removed. Validator id: {:?}.",
                file!(),
                line!(),
                validator_id
            );

            <Validators<T>>::remove(&validator_id);

            <IsChanged<T>>::put(true);

            log::warn!("Validator {:?} removed", validator_id);

            AccountRefCounter::<T>::dec_ref(&validator_id.clone().into());

            Self::deposit_event(Event::ValidatorRemoved(validator_id));

            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Validator successfully added
        /// \[who\]
        ValidatorAdded(<T as pallet::Config>::ValidatorId),
        /// Validator successfully removed
        /// \[who\]
        ValidatorRemoved(<T as pallet::Config>::ValidatorId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Validator was not added because he is already active
        AlreadyAdded,
        /// Validator was not removed: there is no active validator with this id
        AlreadyRemoved,
        /// Validator was not added because validator is not registered
        NotRegistered,
    }

    /// Pallet storage - list of all active validators
    #[pallet::storage]
    #[pallet::getter(fn validators)]
    pub type Validators<T: Config> =
        StorageMap<_, Blake2_128Concat, <T as pallet::Config>::ValidatorId, bool, ValueQuery>;

    /// Pallet storage - flag showing that active validators list changed
    /// during a session
    #[pallet::storage]
    #[pallet::getter(fn is_changed)]
    pub type IsChanged<T: Config> = StorageValue<_, bool, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub validators: Vec<<T as pallet::Config>::ValidatorId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                validators: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |config: &GenesisConfig<T>| {
                for &ref validator in config.validators.iter() {
                    <Validators<T>>::insert(validator, true);
                    AccountRefCounter::<T>::inc_ref(&validator.clone().into());
                }
                <IsChanged<T>>::put(true);
            };
            extra_genesis_builder(self);
        }
    }
}

impl<T: Config> Pallet<T> {
    fn commit() {
        <IsChanged<T>>::put(false);
    }
}

/// Substrate session manager trait
impl<T: Config> SessionManager<<T as pallet::Config>::ValidatorId> for Pallet<T> {
    fn new_session(_: SessionIndex) -> Option<Vec<<T as pallet::Config>::ValidatorId>> {
        let result = if <IsChanged<T>>::get() {
            Some(<Validators<T>>::iter().map(|(k, _v)| k).collect())
        } else {
            None
        };

        Self::commit();

        result
    }
    fn start_session(_: SessionIndex) {}
    fn end_session(_: SessionIndex) {}
}
