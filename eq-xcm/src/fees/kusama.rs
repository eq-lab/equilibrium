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

parameter_types! {
    pub const BaseXcmWeight: XcmWeight = 1_000_000_000;
}

/// Copy-paste from Kusama's runtime constants.
/// WeightToFee is the most accurate way to predict fee for XCM execution on Kusama.
/// May be it will be separate crate once and we can avoid this.
/// Any way we can specify fee amount by ourselves.
pub const KUSAMA_UNITS: XcmBalance = 1_000_000_000_000;
pub const KUSAMA_CENTS: XcmBalance = KUSAMA_UNITS / 30_000;

pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
    type Balance = XcmBalance;
    fn polynomial() -> WeightToFeeCoefficients<XcmBalance> {
        // in Kusama, extrinsic base weight (smallest non-zero weight) is mapped to 1/10 CENT:
        let p = KUSAMA_CENTS;
        let q = 10 * XcmBalance::from(ExtrinsicBaseWeight::get().ref_time());
        smallvec![WeightToFeeCoefficient {
            coeff_integer: p / q,
            coeff_frac: Perbill::from_rational(p % q, q),
            negative: false,
            degree: 1,
        }]
    }
}
