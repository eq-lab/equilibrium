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

/// Copy-paste from Acala's runtime constants.
/// WeightToFee is the most accurate way to predict fee for XCM execution on Karura.
/// May be it will be separate crate once and we can avoid this.
/// Any way we can specify fee amount by ourselves.
pub const ACALA_UNIT: XcmBalance = 1_000_000_000_000;
pub const ACALA_CENT: XcmBalance = ACALA_UNIT / 100;
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
    type Balance = XcmBalance;
    fn polynomial() -> WeightToFeeCoefficients<XcmBalance> {
        let p = (ACALA_CENT * 8 / 1_000) * EXTRINSICS_PER_SECOND;
        let q = 10 * WEIGHT_PER_SECOND;
        smallvec![WeightToFeeCoefficient {
            coeff_integer: p / q,
            coeff_frac: Perbill::from_rational(p % q, q),
            negative: false,
            degree: 1,
        }]
    }
}
