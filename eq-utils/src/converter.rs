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

use equilibrium_curve_amm::traits::CheckedConvert;
use sp_arithmetic::{FixedI64, FixedPointNumber, FixedU128, Permill};
use sp_runtime::sp_std::convert::{TryFrom, TryInto};
use sp_runtime::traits::Convert;

pub struct FixedU128Convert;

impl Convert<u64, FixedU128> for FixedU128Convert {
    fn convert(a: u64) -> FixedU128 {
        let accuracy = FixedU128::accuracy() / FixedI64::accuracy() as u128;
        FixedU128::from_inner(a as u128 * accuracy)
    }
}

impl Convert<FixedU128, u64> for FixedU128Convert {
    fn convert(a: FixedU128) -> u64 {
        let accuracy = FixedU128::accuracy() / FixedI64::accuracy() as u128;
        (a.into_inner() / accuracy)
            .try_into()
            .expect("Wrong conversion from FixedU128 to Balance of u64.")
    }
}

impl Convert<Permill, FixedU128> for FixedU128Convert {
    fn convert(a: Permill) -> FixedU128 {
        a.into()
    }
}

impl Convert<u8, FixedU128> for FixedU128Convert {
    fn convert(a: u8) -> FixedU128 {
        FixedU128::saturating_from_integer(a)
    }
}

impl CheckedConvert<usize, FixedU128> for FixedU128Convert {
    fn convert(a: usize) -> Option<FixedU128> {
        Some(FixedU128::saturating_from_integer(u128::try_from(a).ok()?))
    }
}
