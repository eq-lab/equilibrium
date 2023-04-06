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

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

// Deposit and burn removed due to it is not production feature
pub trait WeightInfo {
    fn enable_transfers() -> Weight;
    fn disable_transfers() -> Weight;
    fn transfer() -> Weight;
    fn allow_xcm_transfers_native_for(z: u32) -> Weight;
    fn forbid_xcm_transfers_native_for(z: u32) -> Weight;
    fn update_xcm_transfer_native_limit() -> Weight;
    fn xcm_transfer_native() -> Weight;
    fn xcm_transfer() -> Weight;
    fn on_initialize(a: u32) -> Weight;
}

// for tests
impl crate::WeightInfo for () {
    fn enable_transfers() -> Weight {
        Weight::zero()
    }
    fn disable_transfers() -> Weight {
        Weight::zero()
    }
    fn transfer() -> Weight {
        Weight::zero()
    }
    fn allow_xcm_transfers_native_for(_z: u32) -> Weight {
        Weight::zero()
    }
    fn forbid_xcm_transfers_native_for(_z: u32) -> Weight {
        Weight::zero()
    }
    fn update_xcm_transfer_native_limit() -> Weight {
        Weight::zero()
    }
    fn xcm_transfer_native() -> Weight {
        Weight::zero()
    }
    fn xcm_transfer() -> Weight {
        Weight::zero()
    }

    fn on_initialize(_a: u32) -> Weight {
        Weight::zero()
    }
}
