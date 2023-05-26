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

use eq_utils::XcmBalance;
use frame_support::{
    parameter_types,
    traits::Get,
    weights::{
        constants::ExtrinsicBaseWeight, Weight, WeightToFeeCoefficient, WeightToFeeCoefficients,
        WeightToFeePolynomial,
    },
};
use smallvec::smallvec;
use sp_runtime::{traits::Convert, Perbill};
use sp_std::marker::PhantomData;
use xcm::v3::{Weight as XcmWeight, Xcm};

pub const WEIGHT_PER_SECOND: u128 =
    frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND as u128;
pub const WEIGHT_PER_MILLIS: u128 = WEIGHT_PER_SECOND / 1_000;
pub const WEIGHT_PER_MICROS: u128 = WEIGHT_PER_MILLIS / 1_000;
pub const WEIGHT_PER_NANOS: u128 = WEIGHT_PER_MICROS / 1_000;
pub const EXTRINSICS_PER_SECOND: u128 =
    WEIGHT_PER_SECOND / ExtrinsicBaseWeight::get().ref_time() as u128;
pub const POLKADOT_EXTRINSIC_BASE_WEIGHT: u128 = 85_212 * WEIGHT_PER_NANOS;
pub const KUSAMA_EXTRINSIC_BASE_WEIGHT: u128 = 86_309 * WEIGHT_PER_NANOS;

pub mod acala;
pub mod astar;
pub mod bifrost;
pub mod crust;
pub mod interlay;
pub mod kusama;
pub mod moonbeam;
pub mod parallel;
pub mod phala;
pub mod polkadot;
pub mod statemint;

pub struct XcmToFee<BaseXcmWeight, WeightToFee>(PhantomData<(BaseXcmWeight, WeightToFee)>);
impl<'xcm, Call, BaseXcmWeight, WeightToFee> Convert<&'xcm Xcm<Call>, XcmBalance>
    for XcmToFee<BaseXcmWeight, WeightToFee>
where
    BaseXcmWeight: Get<XcmWeight>,
    WeightToFee: frame_support::weights::WeightToFee,
    WeightToFee::Balance: Into<XcmBalance>,
{
    fn convert(xcm: &'xcm Xcm<Call>) -> XcmBalance {
        let base_xcm_weight = BaseXcmWeight::get();
        let weight = Weight::from_parts(
            xcm.len() as u64 * base_xcm_weight.ref_time(),
            base_xcm_weight.proof_size(),
        );
        2 * WeightToFee::weight_to_fee(&weight).into()
    }
}

#[test]
fn expected_fees() {
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

    fn print_fee<'xcm, T: Convert<&'xcm Xcm<()>, XcmBalance>>(
        xcm: &'xcm Xcm<()>,
        asset: &str,
        decimals: u8,
    ) {
        let fee = T::convert(xcm);
        let fee_local = eq_utils::balance_from_xcm::<u128>(fee, decimals).unwrap();
        println!("{:?} {:?} {:?}", asset, fee_local, fee);
    }

    println!("Acala:");
    print_fee::<acala::aca::XcmToFee>(&xcm, "aca", 12);
    print_fee::<acala::ausd::XcmToFee>(&xcm, "ausd", 12);
    print_fee::<acala::eq::XcmToFee>(&xcm, "eq", 9);
    print_fee::<acala::eqd::XcmToFee>(&xcm, "eqd", 9);
    println!("");

    println!("Astar:");
    print_fee::<astar::astr::XcmToFee>(&xcm, "astr", 18);
    print_fee::<astar::eq::XcmToFee>(&xcm, "eq", 9);
    print_fee::<astar::eqd::XcmToFee>(&xcm, "eqd", 9);
    println!("");

    println!("Bifrost:");
    print_fee::<bifrost::bnc::XcmToFee>(&xcm, "bnc", 12);
    print_fee::<bifrost::eq::XcmToFee>(&xcm, "eq", 9);
    print_fee::<bifrost::eqd::XcmToFee>(&xcm, "eqd", 9);
    println!("");

    println!("Crust:");
    print_fee::<crust::cru::XcmToFee>(&xcm, "cru", 12);
    print_fee::<crust::eqd::XcmToFee>(&xcm, "eqd", 9);
    println!("");

    println!("Interlay:");
    print_fee::<interlay::intr::XcmToFee>(&xcm, "intr", 10);
    print_fee::<interlay::ibtc::XcmToFee>(&xcm, "ibtc", 8);
    println!("");

    println!("Moonbeam:");
    print_fee::<moonbeam::glmr::XcmToFee>(&xcm, "glmr", 18);
    print_fee::<moonbeam::eq::XcmToFee>(&xcm, "eq", 9);
    println!("");

    println!("Parallel:");
    print_fee::<parallel::para::XcmToFee>(&xcm, "para", 12);
    print_fee::<parallel::eq::XcmToFee>(&xcm, "eq", 9);
    print_fee::<parallel::eqd::XcmToFee>(&xcm, "eqd", 9);
    print_fee::<parallel::cdot613::XcmToFee>(&xcm, "cdot613", 10);
    print_fee::<parallel::cdot714::XcmToFee>(&xcm, "cdot714", 10);
    print_fee::<parallel::cdot815::XcmToFee>(&xcm, "cdot815", 10);
    println!("");

    println!("Phala:");
    print_fee::<phala::pha::XcmToFee>(&xcm, "pha", 12);
    print_fee::<phala::eq::XcmToFee>(&xcm, "eq", 9);
    print_fee::<phala::eqd::XcmToFee>(&xcm, "eqd", 9);
    println!("");
}
