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

use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn set_config() -> Weight;
    fn swap_eq_to_q() -> Weight;
}

// for tests
impl crate::WeightInfo for () {
    fn set_config() -> Weight {
        Weight::zero()
    }
    fn swap_eq_to_q() -> Weight {
        Weight::zero()
    }
}
