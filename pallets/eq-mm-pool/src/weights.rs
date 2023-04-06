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
    fn create_pool() -> Weight;
    fn change_min_amount() -> Weight;
    fn set_epoch_duration() -> Weight;
    fn add_manager() -> Weight;
    // fn remove_manager() -> Weight;
    fn set_allocations(b: u32) -> Weight;

    fn borrow() -> Weight;
    fn repay() -> Weight;

    fn deposit() -> Weight;
    fn request_withdrawal() -> Weight;
    fn withdraw() -> Weight;

    fn global_advance_epoch(b: u32) -> Weight;
}

// for tests
impl crate::WeightInfo for () {
    fn create_pool() -> Weight {
        Weight::zero()
    }

    fn change_min_amount() -> Weight {
        Weight::zero()
    }

    fn set_epoch_duration() -> Weight {
        Weight::zero()
    }

    fn add_manager() -> Weight {
        Weight::zero()
    }

    // fn remove_manager() -> Weight {
    //     Weight::zero()
    // }

    fn set_allocations(_: u32) -> Weight {
        Weight::zero()
    }

    fn borrow() -> Weight {
        Weight::zero()
    }

    fn repay() -> Weight {
        Weight::zero()
    }

    fn deposit() -> Weight {
        Weight::zero()
    }

    fn request_withdrawal() -> Weight {
        Weight::zero()
    }

    fn withdraw() -> Weight {
        Weight::zero()
    }

    fn global_advance_epoch(_: u32) -> Weight {
        Weight::zero()
    }
}
