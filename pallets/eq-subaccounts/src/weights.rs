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
    fn transfer_to_bailsman_register() -> Weight;
    fn transfer_to_borrower_register() -> Weight;
    fn transfer_to_bailsman_and_redistribute(r: u32) -> Weight;
    fn transfer_to_subaccount() -> Weight;
    fn transfer_from_subaccount() -> Weight;
    fn transfer_from_subaccount_redistribute(r: u32) -> Weight;
}

// for tests
impl crate::WeightInfo for () {
    fn transfer_to_bailsman_register() -> Weight {
        Weight::zero()
    }

    fn transfer_to_borrower_register() -> Weight {
        Weight::zero()
    }

    fn transfer_to_bailsman_and_redistribute(_r: u32) -> Weight {
        Weight::zero()
    }

    fn transfer_to_subaccount() -> Weight {
        Weight::zero()
    }

    fn transfer_from_subaccount() -> Weight {
        Weight::zero()
    }

    fn transfer_from_subaccount_redistribute(_r: u32) -> Weight {
        Weight::zero()
    }
}
