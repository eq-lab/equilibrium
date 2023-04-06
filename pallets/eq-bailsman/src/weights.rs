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

pub trait WeightInfo {
    fn toggle_auto_redistribution() -> Weight;
    fn redistribute(z: u32) -> Weight;
    fn redistribute_unsigned(z: u32) -> Weight;
    fn on_initialize() -> Weight;
    fn on_finalize(z: u32) -> Weight;
}

// for tests
impl crate::WeightInfo for () {
    fn toggle_auto_redistribution() -> Weight {
        Weight::zero()
    }

    fn redistribute(_z: u32) -> Weight {
        Weight::zero()
    }

    fn redistribute_unsigned(_z: u32) -> Weight {
        Weight::zero()
    }

    fn on_initialize() -> Weight {
        Weight::zero()
    }

    fn on_finalize(_z: u32) -> Weight {
        Weight::zero()
    }
}
