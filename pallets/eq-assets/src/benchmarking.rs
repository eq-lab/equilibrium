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

//! # Equilibrium Assets Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use eq_primitives::asset::Asset;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use sp_std::vec;

const NEW_ASSET: Asset = Asset(7954895577252324724); //::from_bytes(b"newasset"); 0x6E65776173736574

benchmarks! {
    add_asset {
        let stored_asset = Pallet::<T>::get_asset_data(&NEW_ASSET);
        assert!(stored_asset.is_err());

        let name = "NewAsset";
    }: _(
        RawOrigin::Root,
        Vec::<u8>::from(name),
        0_u128,
        0_i64,
        Permill::zero(),
        Permill::zero(),
        AssetXcmData::None,
        Permill::zero(),
        0_u64,
        AssetType::Physical,
        false,
        Percent::zero(),
        Permill::one(),
        vec![]
    )
    verify{
        let stored_asset = Pallet::<T>::get_asset_data(&NEW_ASSET);
        assert!(stored_asset.is_ok());
        assert_eq!(stored_asset.unwrap().id, NEW_ASSET);
    }

    remove_asset {
        let name = "NewAsset";
        let new_asset: AssetData<Asset> = AssetData {
            id: NEW_ASSET,
            lot: EqFixedU128::from_inner(10),
            price_step: FixedI64::from_inner(10),
            maker_fee: Permill::from_parts(10),
            taker_fee: Permill::from_parts(10),
            asset_xcm_data: AssetXcmData::SelfReserved,
            debt_weight: Permill::from_parts(1),
            lending_debt_weight: Permill::one(),
            buyout_priority: 200_u64,
            asset_type: AssetType::Physical,
            is_dex_enabled: false,
            collateral_discount: Percent::one()
        };
        let _ = Assets::<T>::mutate(|value| *value = Some(vec![new_asset]));
        let stored_asset = Pallet::<T>::get_asset_data(&NEW_ASSET);
        assert!(stored_asset.is_ok());
    }: _(RawOrigin::Root, NEW_ASSET)
    verify {
        let assets_to_remove = Pallet::<T>::assets_to_remove().unwrap();
        assert!(assets_to_remove.contains(&NEW_ASSET));
    }

    update_asset {
        let name = "NewAsset";
        let new_asset = AssetData {
            id: NEW_ASSET,
            lot: EqFixedU128::from_inner(0),
            price_step: FixedI64::from_inner(0),
            maker_fee: Permill::zero(),
            taker_fee: Permill::zero(),
            asset_xcm_data: AssetXcmData::None,
            debt_weight: Permill::zero(),
            lending_debt_weight: Permill::from_percent(1),
            buyout_priority: 100_u64,
            asset_type: AssetType::Physical,
            is_dex_enabled: false,
            collateral_discount: Percent::one()
        };
        let _ = Assets::<T>::mutate(|value| *value = Some(vec![new_asset]));
        let stored_asset = Pallet::<T>::get_asset_data(&NEW_ASSET);
        assert!(stored_asset.is_ok());
    }: _(RawOrigin::Root,
        NEW_ASSET,
        Some(10),
        Some(10),
        Some(Permill::from_parts(10)),
        Some(Permill::from_parts(10)),
        Some(AssetXcmData::SelfReserved),
        Some(Permill::from_parts(10)),
        Some(200),
        Some(AssetType::Physical),
        Some(false),
        Some(Percent::one()),
        Some(Permill::one())
    )
    verify {
        let stored_asset = Pallet::<T>::get_asset_data(&NEW_ASSET);
        let updated_asset = AssetData {
            id: NEW_ASSET,
            lot: EqFixedU128::from_inner(10),
            price_step: FixedI64::from_inner(10),
            maker_fee: Permill::from_parts(10),
            taker_fee: Permill::from_parts(10),
            asset_xcm_data: AssetXcmData::SelfReserved,
            debt_weight: Permill::from_parts(10),
            lending_debt_weight: Permill::one(),
            buyout_priority: 200_u64,
            asset_type: AssetType::Physical,
            is_dex_enabled: false,
            collateral_discount: Percent::one()
        };
        assert!(stored_asset.is_ok());
        assert_eq!(stored_asset.unwrap(), updated_asset);
    }
}
