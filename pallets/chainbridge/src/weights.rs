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
    fn set_threshold() -> Weight;
    fn set_resource() -> Weight;
    fn remove_resource() -> Weight;
    fn whitelist_chain() -> Weight;
    fn add_relayer() -> Weight;
    fn remove_relayer() -> Weight;
    fn acknowledge_proposal() -> Weight;
    fn reject_proposal() -> Weight;
    fn eval_vote_state() -> Weight;
    fn redistribute_fees(z: u32) -> Weight;
}

// for tests
impl crate::WeightInfo for () {
    fn set_threshold() -> Weight {
        Weight::zero()
    }
    fn set_resource() -> Weight {
        Weight::zero()
    }
    fn remove_resource() -> Weight {
        Weight::zero()
    }
    fn whitelist_chain() -> Weight {
        Weight::zero()
    }
    fn add_relayer() -> Weight {
        Weight::zero()
    }
    fn remove_relayer() -> Weight {
        Weight::zero()
    }
    fn acknowledge_proposal() -> Weight {
        Weight::zero()
    }
    fn reject_proposal() -> Weight {
        Weight::zero()
    }
    fn eval_vote_state() -> Weight {
        Weight::zero()
    }
    fn redistribute_fees(_z: u32) -> Weight {
        Weight::zero()
    }
}
