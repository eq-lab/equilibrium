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

//! # Equilibrium Sudo multisignature pallet.
//!
//! Equilibrium Sudo multisignature pallet is a Substrate module that provides
//! root access via multisignature.
//!
//! It works as follows: the pallet is initialized by a set of accounts
//! that are able to vote on multisignature calls (signatory accounts). A minimal number of votes (threshold) is set that
//! is allowed for a multisignature call to proceed. The proposer of the call, that can be any account of the network, makes the first signature.
//! Signatory accounts can vote as either approve or cancel said call and if a number of approvals or cancellations exceeds
//! the set threshold then the call is either sudo-ed or removed respectively.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod benchmarking;
pub mod weights;
pub use weights::WeightInfo;

use sp_std::cmp::min;
use sp_std::prelude::*;
use sp_std::vec;

use frame_support::Parameter;

use frame_support::{
    dispatch::{DispatchResultWithPostInfo, GetDispatchInfo},
    traits::{Get, UnfilteredDispatchable},
};

#[allow(unused_imports)]
use frame_support::debug;
use frame_system::ensure_root;
use frame_system::ensure_signed;

use eq_utils::eq_ensure;

use codec::{Decode, Encode};
use core::convert::TryInto;
pub use pallet::*;
use sp_io::hashing::blake2_256;

//32 bytes as a standard
pub type CallHash = [u8; 32];

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::DispatchResult;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// A sudo-able call.
        type Call: Parameter
            + UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>
            + GetDispatchInfo
            + From<pallet::Call<Self>>;
        /// Maximal number of signatories
        #[pallet::constant]
        type MaxSignatories: Get<u32>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// Just a bunch of bytes, but they should decode to a valid `Call`.
    pub type OpaqueCall = Vec<u8>;

    /// An open multisig operation (proposal).
    #[derive(Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug, scale_info::TypeInfo)]
    pub struct Multisig<AccountId> {
        /// The call itself in a binary
        pub call: OpaqueCall,
        /// The account who proposed it
        pub proposer: AccountId,
        /// Approvals achieved so far
        pub approvals: Vec<AccountId>,
        /// Votes to cancel the proposal
        pub cancels: Vec<AccountId>,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The storage has been initialized
        Initialized,
        /// A key has been added to the multisig signatory list
        KeyAdded(T::AccountId),
        /// A key has been removed to the multisig signatory list
        KeyRemoved(T::AccountId),
        /// The signatory threshold was modified; a new value is supplied.
        ThresholdModified(u32),
        /// A new multisig proposal
        NewProposal(T::AccountId, CallHash),
        /// The proposal was cancelled
        ProposalCancelled(CallHash),
        /// The proposal was approved
        ProposalApproved(T::AccountId, CallHash),
        /// Sudo was executed on the proposal after enough signatures
        MultisigSudid(CallHash, DispatchResult),
        /// Sudo critical failure
        SudoFailed(CallHash),
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::error]
    pub enum Error<T> {
        /// The key is already in the multisig signatory list
        AlreadyInKeyList,
        /// The key is not in the multisig signatory list
        NotInKeyList,
        /// The threshold is invalid
        InvalidThresholdValue,
        /// Too few signatories for the set threshold
        FewSignatories,
        /// The proposal not found in the map
        ProposalNotFound,
        /// Trying to delete a proposal that is not ours
        NotProposalOwner,
        /// The account already approved a proposal
        AlreadyApproved,
        /// The account already voted to cancel a proposal
        AlreadyCancelled,
    }

    /// The multisig signatory key list.
    #[pallet::storage]
    #[pallet::getter(fn keys)]
    pub type Keys<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, bool, ValueQuery>;

    /// The threshold required to proceed a call.
    #[pallet::storage]
    #[pallet::getter(fn threshold)]
    pub type Threshold<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The map storing proposals by a call hash key (CallHash)
    #[pallet::storage]
    #[pallet::getter(fn multisigs)]
    pub type MultisigProposals<T: Config> =
        StorageMap<_, Identity, [u8; 32], Multisig<T::AccountId>, OptionQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub keys: Vec<T::AccountId>,
        pub threshold: u32,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                keys: Default::default(),
                threshold: 3,
            }
        }
    }

    #[cfg(feature = "std")]
    impl<T: Config> GenesisConfig<T> {
        /// Direct implementation of `GenesisBuild::build_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn build_storage(&self) -> Result<sp_runtime::Storage, String> {
            <Self as GenesisBuild<T>>::build_storage(self)
        }

        /// Direct implementation of `GenesisBuild::assimilate_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn assimilate_storage(&self, storage: &mut sp_runtime::Storage) -> Result<(), String> {
            <Self as GenesisBuild<T>>::assimilate_storage(self, storage)
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |config: &GenesisConfig<T>| {
                for &ref who in config.keys.iter() {
                    <Keys<T>>::insert(who, true);
                }
                <Threshold<T>>::put(config.threshold);
            };
            extra_genesis_builder(self);
        }
    }

    impl<T: Config> Pallet<T> {
        /// Attempt to decode and return a call, provided by the user or from the storage.
        fn decode_proposal(call_hash: &[u8; 32]) -> Option<(<T as Config>::Call, usize)> {
            let mb_data = <MultisigProposals<T>>::get(&call_hash).map(|p| p.call);
            if mb_data.is_none() {
                return None;
            } else {
                let data = mb_data.unwrap();
                Decode::decode(&mut &data[..]).ok().map(|d| (d, data.len()))
            }
        }

        /// Sudo which is called after enough signatories have approved a call
        fn sudo(call_hash: &CallHash) {
            //decode the call
            let maybe_call = Self::decode_proposal(call_hash);

            //Clean the storage anyway
            <MultisigProposals<T>>::remove(&call_hash);

            match maybe_call {
                Some((call, _)) => {
                    //sudo the call
                    let res = call.dispatch_bypass_filter(frame_system::RawOrigin::Root.into());
                    let dispatch_result = res.map(|_| ()).map_err(|e| e.error);
                    Self::deposit_event(Event::<T>::MultisigSudid(*call_hash, dispatch_result));
                }
                None => {
                    Self::deposit_event(Event::<T>::SudoFailed(*call_hash));
                }
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Adds a key to the multisig signatory list. Requires root.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::add_key())]
        pub fn add_key(origin: OriginFor<T>, key: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            eq_ensure!(
                !<Keys<T>>::contains_key(&key),
                Error::<T>::AlreadyInKeyList,
                target: "eq_multisig_sudo",
                "{}:{}. Account is already in the multisig signatory list. Who: {:?}.",
                file!(),
                line!(),
                key
            );

            <Keys<T>>::insert(&key, true);

            Self::deposit_event(Event::KeyAdded(key));

            Ok(().into())
        }

        /// Removes a key from the multisig signatory list. Requires root.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::remove_key())]
        pub fn remove_key(origin: OriginFor<T>, key: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            eq_ensure!(
                <Keys<T>>::contains_key(&key),
                Error::<T>::NotInKeyList,
                target: "eq_multisig_sudo",
                "{}:{}. Account was not found in the multisig signatory list. Who: {:?}.",
                file!(),
                line!(),
                key
            );

            let threshold = Self::threshold();
            eq_ensure!(
                (<Keys<T>>::iter().count() - 1) as u32 >= threshold,
                Error::<T>::FewSignatories,
                target: "eq_multisig_sudo",
                "{}:{}. Removing this account would render proposals undoable as there would be less multisig signatories than the threshold {:?}. Who: {:?}.",
                file!(),
                line!(),
                threshold,
                key
            );

            <Keys<T>>::remove(&key);

            Self::deposit_event(Event::KeyRemoved(key));

            Ok(().into())
        }

        /// Modifies the multisig threshold value i.e. the required number of signatories for a call to proceed. Requires root.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::modify_threshold())]
        pub fn modify_threshold(
            origin: OriginFor<T>,
            new_value: u32,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let max_value = min(T::MaxSignatories::get() as usize, <Keys<T>>::iter().count());

            eq_ensure!(
                new_value > 0 && new_value as usize <= max_value,
                Error::<T>::InvalidThresholdValue,
                target: "eq_multisig_sudo",
                "{}:{}. Invalid threshold value {:?}. The value must be between 1 and {:?}.",
                file!(),
                line!(),
                new_value,
                max_value
            );

            <Threshold<T>>::put(new_value);

            Self::deposit_event(Event::ThresholdModified(Self::threshold()));
            Ok(Pays::No.into())
        }

        /// Proposes a call to be signed. Requires account to be in multisig signatory list.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::propose())]
        pub fn propose(
            origin: OriginFor<T>,
            call: Box<<T as Config>::Call>,
        ) -> DispatchResultWithPostInfo {
            // This is a public call, so we ensure that an origin is a signed account.
            let who = ensure_signed(origin)?;
            // checking that key is in list
            eq_ensure!(
                <Keys<T>>::contains_key(&who),
                Error::<T>::NotInKeyList,
                target: "eq_multisig_sudo",
                "{}:{}. Account must be in multisig signatory list. Who: {:?}",
                file!(),
                line!(),
                who
            );

            let call_data: OpaqueCall = Encode::encode(&call);

            let call_hash = (
                b"CALLHASH",
                who.clone(),
                &call_data[..],
                <frame_system::Pallet<T>>::block_number(),
            )
                .using_encoded(blake2_256);

            let new_proposal = Multisig {
                proposer: who.clone(),
                call: call_data,
                approvals: vec![who.clone()],
                cancels: vec![],
            };

            <MultisigProposals<T>>::insert(call_hash, new_proposal);

            Self::deposit_event(Event::<T>::NewProposal(who, call_hash));

            //if threshold is unity, immediately dispatch the call
            if Self::threshold() == 1 {
                Self::sudo(&call_hash);
            }

            // Sudo user does not pay a fee.
            Ok(Pays::No.into())
        }

        /// Approves a proposal. Requires an account be in the multisig signatory list.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::approve())]
        pub fn approve(origin: OriginFor<T>, call_hash: [u8; 32]) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            // are we in list?
            eq_ensure!(
                <Keys<T>>::contains_key(&who),
                Error::<T>::NotInKeyList,
                target: "eq_multisig_sudo",
                "{}:{}. Account must be in multisig signatory list. Who: {:?}, proposal hash: {:?}",
                file!(),
                line!(),
                who,
                call_hash
            );
            // is there a proposal?
            eq_ensure!(
                <MultisigProposals<T>>::contains_key(&call_hash),
                Error::<T>::ProposalNotFound,
                target: "eq_multisig_sudo",
                "{}:{}. Proposed call was not found. Who: {:?}, proposal hash: {:?}",
                file!(),
                line!(),
                who,
                call_hash
            );

            let approvals = <MultisigProposals<T>>::get(&call_hash)
                .map(|p| p.approvals)
                .unwrap_or(vec![]);
            // maybe already signed?
            eq_ensure!(
                !approvals.contains(&who),
                Error::<T>::AlreadyApproved,
                target: "eq_multisig_sudo",
                "{}:{}. Account already approved this proposal. Who: {:?}, proposal hash: {:?}",
                file!(),
                line!(),
                who,
                call_hash
            );
            //mutate the approvals of the multisig
            <MultisigProposals<T>>::mutate(&call_hash, |mb_ms| {
                if let Some(ref mut ms) = mb_ms {
                    ms.approvals.push(who.clone());
                }
                // existence checked above with contains_key
            });

            Self::deposit_event(Event::<T>::ProposalApproved(who, call_hash));

            // Check if enough signatories, then call sudo
            let approvals = <MultisigProposals<T>>::get(&call_hash)
                .map(|p| p.approvals)
                .unwrap_or(vec![]);
            let threshold = Self::threshold();
            if approvals.len() as u32 >= threshold {
                Self::sudo(&call_hash);
            }

            // Sudo user does not pay a fee.
            Ok(Pays::No.into())
        }

        /// Cancels an earlier submitted proposal.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::cancel_proposal())]
        pub fn cancel_proposal(
            origin: OriginFor<T>,
            call_hash: [u8; 32],
        ) -> DispatchResultWithPostInfo {
            // This is a public call, so we ensure that the origin is some signed account.
            let who = ensure_signed(origin)?;
            // is there a proposal?
            eq_ensure!(
                <MultisigProposals<T>>::contains_key(&call_hash),
                Error::<T>::ProposalNotFound,
                target: "eq_multisig_sudo",
                "{}:{}. Proposed call was not found. Who: {:?}, proposal hash: {:?}",
                file!(),
                line!(),
                who,
                call_hash
            );
            // are we in list?
            eq_ensure!(
                <Keys<T>>::contains_key(&who),
                Error::<T>::NotInKeyList,
                target: "eq_multisig_sudo",
                "{}:{}. Account must be in multisig signatory list. Who: {:?}, proposal hash: {:?}",
                file!(),
                line!(),
                who,
                call_hash
            );
            // maybe already cancelled?
            let cancels = <MultisigProposals<T>>::get(&call_hash)
                .map(|p| p.cancels)
                .unwrap_or(vec![]);
            eq_ensure!(
                !cancels.contains(&who),
                Error::<T>::AlreadyCancelled,
                target: "eq_multisig_sudo",
                "{}:{}. Account already cancelled this proposal. Who: {:?}, proposal hash: {:?}",
                file!(),
                line!(),
                who,
                call_hash
            );

            //mutate the cancels of the multisig
            <MultisigProposals<T>>::mutate(&call_hash, |mb_ms| {
                if let Some(ref mut ms) = mb_ms {
                    ms.cancels.push(who.clone());
                }
            });

            //check if enough cancels and then cancel the proposal
            let cancels = <MultisigProposals<T>>::get(&call_hash)
                .map(|p| p.cancels)
                .unwrap_or(vec![]);
            if cancels.len() as u32 >= Self::threshold() {
                <MultisigProposals<T>>::remove(&call_hash);
                Self::deposit_event(Event::<T>::ProposalCancelled(call_hash));
            }
            // Sudo user does not pay a fee.
            Ok(Pays::No.into())
        }
    }
}
