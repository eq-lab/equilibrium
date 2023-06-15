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

#![allow(dead_code)]

use core::convert::TryInto;

use eq_primitives::asset::{Asset, AssetData};
use sp_std::vec::Vec;

mod commit_85c486cb55336769e0543a66be0b2bafec90f62b {
    use codec::{Decode, Encode};
    use eq_primitives::asset::AssetType;
    use sp_runtime::FixedU128;

    pub type DebtWeightType = sp_runtime::FixedI128;

    #[derive(Decode, Encode, Eq, PartialEq, Clone, Copy, Debug, scale_info::TypeInfo)]
    pub struct MultiAsset;
    #[derive(Decode, Encode, Eq, PartialEq, Clone, Copy, Debug, scale_info::TypeInfo)]
    pub struct MultiLocation;

    #[derive(Decode, Encode, Clone, Debug, Eq, PartialEq)]
    /// Struct with asset params
    pub struct AssetData<Asset, F> {
        pub id: Asset, //str -> [u8; 8] -> u64
        pub lot: FixedU128,
        pub price_step: FixedU128,
        pub maker_fee: FixedU128,
        pub taker_fee: FixedU128,
        pub multi_asset: Option<MultiAsset>, // for using in future
        pub multi_location: Option<MultiLocation>, // for using in future
        pub debt_weight: F,                  // change to FixedU128 after completing SMAR-528,
        pub buyout_priority: u64,
        pub asset_type: AssetType,
        pub is_dex_enabled: bool, // can be used in dex
    }
}

mod commit_f5bcc13e2e3a1e42751c27f1f8854c1abff323a6 {
    use codec::{Decode, Encode};
    use eq_primitives::asset::AssetType;
    use eq_primitives::balance_number::EqFixedU128;
    use sp_runtime::FixedI64;

    pub type DebtWeightType = sp_runtime::FixedI128;

    #[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
    /// Struct with asset params
    pub struct AssetData<Asset> {
        pub id: Asset,
        pub lot: EqFixedU128,
        pub price_step: FixedI64,
        pub maker_fee: EqFixedU128,
        pub taker_fee: EqFixedU128,
        pub asset_xcm_data: eq_primitives::asset::AssetXcmData,
        pub debt_weight: EqFixedU128,
        pub buyout_priority: u64,
        pub asset_type: AssetType,
        pub is_dex_enabled: bool,
        pub collateral_enabled: bool,
    }
}

mod commit_9422e055156686a7cd322556537babf47dbd2ccb {
    use codec::{Decode, Encode};
    use eq_primitives::asset::{AssetType, AssetXcmData};
    use eq_primitives::balance_number::EqFixedU128;
    use sp_runtime::{FixedI64, FixedPointNumber, Permill};
    #[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
    /// Struct with asset params
    pub struct AssetData<Asset> {
        pub id: Asset, //str -> [u8; 8] -> u64
        pub lot: EqFixedU128,
        pub price_step: FixedI64,
        pub maker_fee: EqFixedU128,
        pub taker_fee: EqFixedU128,
        pub asset_xcm_data: AssetXcmData,
        pub debt_weight: EqFixedU128,
        pub lending_debt_weight: Permill,
        pub buyout_priority: u64,
        pub asset_type: AssetType,
        pub is_dex_enabled: bool, // can be used in dex
        pub collateral_discount: EqFixedU128,
    }

    pub fn permill_from_eq_fixed_u128(a: EqFixedU128) -> Permill {
        sp_runtime::helpers_128bit::multiply_by_rational_with_rounding(
            a.into_inner() as u128,
            1_000_000,
            EqFixedU128::DIV as u128,
            sp_runtime::Rounding::NearestPrefDown,
        )
        .map(|v| Permill::from_parts(v as u32))
        .expect("All values in [0, 1]")
    }
}

mod commit_6e40ae2bc5651a80c7a3abb76c9d89089e01823d {
    use core::convert::TryInto;

    use codec::{Decode, Encode};
    use eq_primitives::{asset::AssetType, balance_number::EqFixedU128};
    use sp_runtime::{FixedI64, Percent, Permill};

    #[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
    /// Struct with asset params
    pub struct AssetData<Asset> {
        pub id: Asset, //str -> [u8; 8] -> u64
        pub lot: EqFixedU128,
        pub price_step: FixedI64,
        pub maker_fee: Permill,
        pub taker_fee: Permill,
        pub asset_xcm_data: AssetXcmData,
        pub debt_weight: Permill,
        pub lending_debt_weight: Permill,
        pub buyout_priority: u64,
        pub asset_type: AssetType,
        pub is_dex_enabled: bool, // can be used in dex
        pub collateral_discount: Percent,
    }

    #[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
    pub enum AssetXcmData {
        /// Token cannot be transfered via XCM
        None,
        /// Token that belong to this parachain
        SelfReserved,
        /// Token from another parachain
        OtherReserved(OtherReservedData),
    }

    #[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
    pub struct OtherReservedData {
        pub multi_location: xcm::v2::MultiLocation,
        pub decimals: u8,
    }

    type NewAssetXcmData = eq_primitives::asset::AssetXcmData;

    impl TryInto<NewAssetXcmData> for AssetXcmData {
        type Error = ();
        fn try_into(self) -> Result<NewAssetXcmData, ()> {
            Ok(match self {
                Self::None => NewAssetXcmData::None,
                Self::SelfReserved => NewAssetXcmData::SelfReserved,
                Self::OtherReserved(OtherReservedData {
                    decimals,
                    multi_location,
                }) => NewAssetXcmData::OtherReserved(eq_primitives::asset::OtherReservedData {
                    decimals,
                    multi_location: multi_location.try_into().expect("ok"),
                }),
            })
        }
    }
}

use commit_6e40ae2bc5651a80c7a3abb76c9d89089e01823d as previous;

pub fn migrate_assets_data(
    old_assets_data: Option<Vec<previous::AssetData<Asset>>>,
) -> Option<Vec<AssetData<Asset>>> {
    old_assets_data
        .unwrap_or_default()
        .into_iter()
        .map(
            |previous::AssetData {
                 id,
                 lot,
                 price_step,
                 maker_fee,
                 taker_fee,
                 asset_xcm_data,
                 debt_weight,
                 lending_debt_weight,
                 buyout_priority,
                 asset_type,
                 is_dex_enabled,
                 collateral_discount,
             }| {
                AssetData {
                    id,
                    lot,
                    price_step,
                    maker_fee,
                    taker_fee,
                    asset_xcm_data: asset_xcm_data.try_into().expect("ok"),
                    debt_weight,
                    buyout_priority,
                    asset_type,
                    is_dex_enabled,
                    collateral_discount,
                    lending_debt_weight,
                }
            },
        )
        .collect::<Vec<_>>()
        .into()
}
