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

//! # Equilibrium SessionManager Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::assert_ok;
use frame_support::codec::Decode;
use frame_system::RawOrigin;
use pallet_session;
use sp_runtime::traits::TrailingZeroInput;

const SEED: u32 = 0;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config: pallet_session::Config + crate::Config {}

benchmarks! {
    add_validator{
        let acc: T::AccountId = account("user", 0, SEED);
        let validator = <T as pallet::Config>::ValidatorIdOf::convert(acc.clone()).unwrap();
        assert_eq!(Validators::<T>::get(validator.clone()), false);
        frame_system::Pallet::<T>::inc_providers(&acc.clone());
        assert_eq!(frame_system::Pallet::<T>::account_exists(&acc.clone()), true);

        let key = <T as pallet_session::Config>::Keys::decode(&mut TrailingZeroInput::zeroes()).unwrap();
        let proof: Vec<u8> = vec![0,1,2,3];
        let origin = RawOrigin::Signed(acc.clone()).into();
        assert_ok!(pallet_session::Pallet::<T>::set_keys(origin, key, proof));
    }: _(RawOrigin::Root, validator)
    verify{
        let acc: T::AccountId = account("user", 0, SEED);
        let validator = <T as pallet::Config>::ValidatorIdOf::convert(acc.clone()).unwrap();
        assert_eq!(Validators::<T>::get(validator), true);
    }

    remove_validator{
        let acc: T::AccountId = account("user", 0, SEED);
        let validator = <T as pallet::Config>::ValidatorIdOf::convert(acc.clone()).unwrap();
        let _ = Validators::<T>::mutate(validator.clone(), |value| *value = true);
        assert_eq!(Validators::<T>::get(validator.clone()), true);
    }: _(RawOrigin::Root, validator)
    verify{
        let acc: T::AccountId = account("user", 0, SEED);
        let validator = <T as pallet::Config>::ValidatorIdOf::convert(acc.clone()).unwrap();
        assert_eq!(Validators::<T>::get(validator), false);
    }
}
