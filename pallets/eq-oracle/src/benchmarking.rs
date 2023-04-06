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

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Call;
use eq_primitives::asset;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::FixedI64;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config: eq_assets::Config + eq_whitelists::Config + crate::Config {}

benchmarks! {
    set_price {
        let b in 1 .. 20;

        for i in 0..b {
            let price_setter: T::AccountId = account("price_setter", i, 0);
            eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
                .unwrap();
            crate::Pallet::<T>::set_price(RawOrigin::Signed(price_setter).into(), asset::BTC, FixedI64::one())
                .unwrap();
        }

        let caller: T::AccountId = whitelisted_caller();
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), caller.clone())
            .unwrap();

    }: _ (RawOrigin::Signed(caller), asset::BTC, FixedI64::one())
    verify {

    }
}
