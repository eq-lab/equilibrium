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

//! # Equilibrium MultisigSudo Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

use crate::pallet as EqMultisigSudo;

const SEED: u32 = 0;

benchmarks! {
    add_key {
        let user: T::AccountId = account("user", 0, SEED);
        assert_eq!(Keys::<T>::get(&user), false);
    }: _(RawOrigin::Root, user.clone())
    verify {
        assert_eq!(Keys::<T>::get(&user), true);
    }

    remove_key {
        let user1: T::AccountId = account("user", 0, SEED);
        let user2: T::AccountId = account("user", 1, SEED);
        let user3: T::AccountId = account("user", 2, SEED);

        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user1.clone()).unwrap();
        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user2).unwrap();
        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user3).unwrap();

        crate::pallet::Pallet::<T>::modify_threshold(RawOrigin::Root.into(), 2).unwrap();

    }: _(RawOrigin::Root, user1.clone())
    verify {
        assert_eq!(Keys::<T>::get(&user1), false);
    }

    modify_threshold {
        let threshold_value = 2u32;
        let user1: T::AccountId = account("user", 0, SEED);
        let user2: T::AccountId = account("user", 1, SEED);

        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user1.clone()).unwrap();
        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user2).unwrap();

        assert_eq!(Threshold::<T>::get(), 1u32);
        assert_eq!(Keys::<T>::iter().count(), 3);
    }: _(RawOrigin::Root, threshold_value)
    verify {
        let threshold_value = 2u32;
        assert_eq!(Threshold::<T>::get(), threshold_value);
    }

    propose {
        let user: T::AccountId = account("user", 0, SEED);

        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user.clone()).unwrap();
        crate::pallet::Pallet::<T>::modify_threshold(RawOrigin::Root.into(), 2).unwrap();

        let call: <T as Config>::Call = EqMultisigSudo::Call::<T>::modify_threshold{ new_value: 1 }.into();
        let call_data: OpaqueCall = Encode::encode(&call);
        let call_hash = (
            b"CALLHASH",
            user.clone(),
            &call_data[..],
            <frame_system::Pallet<T>>::block_number(),
        ).using_encoded(blake2_256);

    }: _(RawOrigin::Signed(user.clone()), Box::new(call.clone()))

    approve {
        let user1: T::AccountId = account("user", 0, SEED);
        let user2: T::AccountId = account("user", 1, SEED);

        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user1.clone()).unwrap();
        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user2.clone()).unwrap();
        crate::pallet::Pallet::<T>::modify_threshold(RawOrigin::Root.into(), 2).unwrap();

        let call: <T as Config>::Call = EqMultisigSudo::Call::<T>::modify_threshold{ new_value: 1 }.into();
        let call_data: OpaqueCall = Encode::encode(&call);
        let call_hash = (
            b"CALLHASH",
            user1.clone(),
            &call_data[..],
            <frame_system::Pallet<T>>::block_number(),
        ).using_encoded(blake2_256);

        crate::pallet::Pallet::<T>::propose(RawOrigin::Signed(user1.clone()).into(), Box::new(call)).unwrap();
    }: _(RawOrigin::Signed(user2.clone()), call_hash.clone())
    verify {
        // proposal executed and removed
        assert_eq!(MultisigProposals::<T>::get(&call_hash), None);
        assert_eq!(Threshold::<T>::get(), 1u32);
    }

    cancel_proposal {
        let user1: T::AccountId = account("user0", 0, SEED);
        let user2: T::AccountId = account("user1", 1, SEED);
        let user3: T::AccountId = account("user2", 2, SEED);

        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user1.clone()).unwrap();
        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user2.clone()).unwrap();
        crate::pallet::Pallet::<T>::add_key(RawOrigin::Root.into(), user3.clone()).unwrap();
        crate::pallet::Pallet::<T>::modify_threshold(RawOrigin::Root.into(), 2).unwrap();

        let call: <T as Config>::Call = EqMultisigSudo::Call::<T>::modify_threshold{ new_value: 1 }.into();
        let call_data: OpaqueCall = Encode::encode(&call);
        let call_hash = (
            b"CALLHASH",
            user1.clone(),
            &call_data[..],
            <frame_system::Pallet<T>>::block_number(),
        ).using_encoded(blake2_256);

        crate::pallet::Pallet::<T>::propose(RawOrigin::Signed(user1.clone()).into(), Box::new(call)).unwrap();
        crate::pallet::Pallet::<T>::cancel_proposal(RawOrigin::Signed(user2.clone()).into(), call_hash.clone()).unwrap();
    }: _(RawOrigin::Signed(user3.clone()), call_hash.clone())
    verify {
        // proposal canceled and removed
        assert_eq!(MultisigProposals::<T>::get(&call_hash), None);
        assert_eq!(Threshold::<T>::get(), 2u32);
    }
}
