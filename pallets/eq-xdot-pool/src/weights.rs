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
    fn transfer_native() -> Weight;
    fn transfer_basic() -> Weight;
    fn transfer() -> Weight;
    fn remark() -> Weight;
}

// for tests/temp
impl crate::WeightInfo for () {
    fn transfer_basic() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn transfer_native() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn transfer() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn remark() -> Weight {
        Weight::from_parts(10_000, 0)
    }
}
