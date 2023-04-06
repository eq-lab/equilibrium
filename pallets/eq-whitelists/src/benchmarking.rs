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

//! # Equilibrium Whitelists Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_std::prelude::*;

const SEED: u32 = 0;

pub trait Config: crate::Config {}
pub struct Pallet<T: Config>(crate::Pallet<T>);

benchmarks! {
    add_to_whitelist {
        let user = account("user", 0, SEED);
    }: _(RawOrigin::Root, user)
    verify {
        let user = account("user", 0, SEED);
        assert!(crate::Pallet::<T>::in_whitelist(&user));
    }

    remove_from_whitelist {
        let user: T::AccountId = account("user", 0, SEED);
        let _ = crate::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), user.clone());
        assert!(crate::Pallet::<T>::in_whitelist(&user));
    }: _(RawOrigin::Root, user)
    verify {
        let user = account("user", 0, SEED);
        assert!(!crate::Pallet::<T>::in_whitelist(&user));
    }
}
