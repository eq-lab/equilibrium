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

/// Constant values used within the runtime.
pub const MILLIASTR: XcmBalance = 1_000_000_000_000_000;

pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
    type Balance = XcmBalance;
    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        // in Astar, extrinsic base weight (smallest non-zero weight) is mapped to 1/10 mASTR:
        let p = MILLIASTR / 10;
        let q = 85_795 * WEIGHT_PER_NANOS;
        smallvec::smallvec![WeightToFeeCoefficient {
            degree: 1,
            negative: false,
            coeff_frac: Perbill::from_rational(p % q, q),
            coeff_integer: p / q,
        }]
    }
}

#[test]
fn t() {
    use xcm::v3::{
        AssetId::Concrete, Fungibility::Fungible, Instruction::*, MultiAsset, MultiLocation,
        WeightLimit, WildMultiAsset::All,
    };

    let asset_multilocation = MultiLocation::parent();

    let multi_asset = MultiAsset {
        id: Concrete(asset_multilocation),
        fun: Fungible(1),
    };
    let multi_assets = vec![multi_asset.clone()].into();
    let beneficiary = MultiLocation::parent();
    let xcm: Xcm<()> = Xcm(vec![
        ReserveAssetDeposited(multi_assets),
        ClearOrigin,
        BuyExecution {
            fees: multi_asset,
            weight_limit: WeightLimit::Unlimited,
        },
        DepositAsset {
            assets: All.into(),
            beneficiary,
        },
    ]);
    let fee = XcmToFee::convert(&xcm);
    let fee_local: u128 = eq_utils::balance_from_xcm(fee, 9).unwrap();
    println!("{:?} {:?}", fee, fee_local);
}
