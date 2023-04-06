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

use super::*;

pub type XcmToFee = crate::fees::XcmToFee<BaseXcmWeight, WeightToFee>;

/// Copy-paste from Moonriver's runtime constants.
pub const SUPPLY_FACTOR: XcmBalance = 100;
pub const KILOWEI: XcmBalance = 1_000;
pub const WEIGHT_FEE: XcmBalance = 100 * KILOWEI * SUPPLY_FACTOR;
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
    type Balance = XcmBalance;
    fn polynomial() -> WeightToFeeCoefficients<XcmBalance> {
        smallvec![WeightToFeeCoefficient {
            coeff_integer: WEIGHT_FEE,
            coeff_frac: Perbill::zero(),
            negative: false,
            degree: 1,
        }]
    }
}
