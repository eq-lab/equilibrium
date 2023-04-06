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

#![cfg(test)]

use super::*;
use crate::mock::*;
use eq_primitives::asset::{self, Asset, AssetData, AssetType};
use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::{traits::One, FixedPointNumber};

fn create_empty_asset(id: Asset) -> AssetData<Asset> {
    AssetData {
        id,
        lot: Default::default(),
        price_step: Default::default(),
        maker_fee: Default::default(),
        taker_fee: Default::default(),
        asset_xcm_data: AssetXcmData::None,
        debt_weight: Default::default(),
        buyout_priority: u64::MAX,
        asset_type: AssetType::Physical,
        is_dex_enabled: true,
        collateral_discount: Percent::one(),
        lending_debt_weight: Permill::one(),
    }
}

#[test]
fn new_assets_event_called() {
    new_test_ext().execute_with(|| {
        assert_eq!(new_assets_called(), 0);

        let btc: AssetData<Asset> = create_empty_asset(asset::BTC); // 6452323
        assert_ok!(ModuleAssets::do_add_asset(
            btc.id,
            btc.lot,
            btc.price_step,
            btc.maker_fee,
            btc.taker_fee,
            AssetXcmData::None,
            btc.debt_weight,
            btc.buyout_priority,
            AssetType::Physical,
            true,
            btc.collateral_discount,
            Permill::one(),
            vec![FixedI64::one()]
        ));

        assert_eq!(new_assets_called(), 1);

        let dot: AssetData<Asset> = create_empty_asset(asset::DOT); // 6582132
        assert_ok!(ModuleAssets::do_add_asset(
            dot.id,
            dot.lot,
            dot.price_step,
            dot.maker_fee,
            dot.taker_fee,
            AssetXcmData::None,
            dot.debt_weight,
            dot.buyout_priority,
            AssetType::Physical,
            true,
            dot.collateral_discount,
            Permill::one(),
            vec![FixedI64::one()]
        ));

        assert_eq!(new_assets_called(), 2);
    });
}

#[test]
fn adding_asset_without_prices_as_collat_fails() {
    new_test_ext().execute_with(|| {
        let dot: AssetData<Asset> = create_empty_asset(asset::DOT); // 6582132
        assert_err!(
            ModuleAssets::add_asset(
                RawOrigin::Root.into(),
                dot.id.to_str_bytes(),
                dot.lot.into_inner(),
                dot.price_step.into_inner(),
                dot.maker_fee,
                dot.taker_fee,
                AssetXcmData::None,
                dot.debt_weight,
                dot.buyout_priority,
                AssetType::Physical,
                true,
                dot.collateral_discount,
                dot.lending_debt_weight,
                vec![]
            ),
            Error::<Test>::CollateralMustBeDisabledWithoutPrices
        );
    });
}

#[test]
fn assets_sorted() {
    new_test_ext().execute_with(|| {
        // prepare assets
        let eq: AssetData<Asset> = create_empty_asset(asset::EQ); // 25969
        let btc: AssetData<Asset> = create_empty_asset(asset::BTC); // 6452323
        let dot: AssetData<Asset> = create_empty_asset(asset::DOT); // 6582132
        let eth: AssetData<Asset> = create_empty_asset(asset::ETH); // 6648936

        // check that assets is empty
        assert_eq!(ModuleAssets::assets(), None);

        // add assets in not sorted order
        assert_ok!(ModuleAssets::add_asset(
            RawOrigin::Root.into(),
            btc.id.to_str_bytes(),
            btc.lot.into_inner(),
            btc.price_step.into_inner(),
            btc.maker_fee,
            btc.taker_fee,
            AssetXcmData::None,
            btc.debt_weight,
            btc.buyout_priority,
            AssetType::Physical,
            true,
            btc.collateral_discount,
            btc.lending_debt_weight,
            vec![FixedI64::one()]
        ));
        assert_ok!(ModuleAssets::add_asset(
            RawOrigin::Root.into(),
            dot.id.to_str_bytes(),
            dot.lot.into_inner(),
            dot.price_step.into_inner(),
            dot.maker_fee,
            dot.taker_fee,
            AssetXcmData::None,
            dot.debt_weight,
            dot.buyout_priority,
            AssetType::Physical,
            true,
            dot.collateral_discount,
            dot.lending_debt_weight,
            vec![FixedI64::one()]
        ));
        assert_ok!(ModuleAssets::add_asset(
            RawOrigin::Root.into(),
            eq.id.to_str_bytes(),
            eq.lot.into_inner(),
            eq.price_step.into_inner(),
            eq.maker_fee,
            eq.taker_fee,
            AssetXcmData::None,
            eq.debt_weight,
            eq.buyout_priority,
            AssetType::Physical,
            true,
            eq.collateral_discount,
            eq.lending_debt_weight,
            vec![FixedI64::one()]
        ));

        // read storage and check that vec of assets is sorted as we expect (by asset id)
        assert_eq!(
            ModuleAssets::assets(),
            Some(vec![eq.clone(), btc.clone(), dot.clone()])
        );

        // add assets in sorted order
        assert_ok!(ModuleAssets::add_asset(
            RawOrigin::Root.into(),
            eth.id.to_str_bytes(),
            eth.lot.into_inner(),
            eth.price_step.into_inner(),
            eth.maker_fee,
            eth.taker_fee,
            AssetXcmData::None,
            eth.debt_weight,
            eth.buyout_priority,
            AssetType::Physical,
            true,
            eth.collateral_discount,
            eth.lending_debt_weight,
            vec![FixedI64::one()]
        ));

        // read storage and check that vec of assets is sorted as we expect (by asset id)
        assert_eq!(ModuleAssets::assets(), Some(vec![eq, btc, dot, eth]));

        // add new asset, get it with AssetGetter and check it's str representation
        let new_asset: AssetData<Asset> =
            create_empty_asset(Asset::from_bytes(b"newasset").expect("Unexpected"));

        assert_ok!(ModuleAssets::add_asset(
            RawOrigin::Root.into(),
            new_asset.id.to_str_bytes(),
            new_asset.lot.into_inner(),
            new_asset.price_step.into_inner(),
            new_asset.maker_fee,
            new_asset.taker_fee,
            AssetXcmData::None,
            new_asset.debt_weight,
            new_asset.buyout_priority,
            AssetType::Physical,
            true,
            new_asset.collateral_discount,
            new_asset.lending_debt_weight,
            vec![FixedI64::one()]
        ));

        assert_eq!(
            ModuleAssets::get_asset_data(&new_asset.id).unwrap(),
            new_asset
        );
    });
}

#[test]
fn assets_errors() {
    new_test_ext().execute_with(|| {
        // prepare asset
        let btc: AssetData<Asset> = create_empty_asset(asset::BTC); // 6452323

        // check that assets is empty
        assert_eq!(ModuleAssets::assets(), None);

        // add BTC asset
        assert_ok!(ModuleAssets::add_asset(
            RawOrigin::Root.into(),
            btc.id.to_str_bytes(),
            btc.lot.into_inner(),
            btc.price_step.into_inner(),
            btc.maker_fee,
            btc.taker_fee,
            AssetXcmData::None,
            btc.debt_weight,
            btc.buyout_priority,
            AssetType::Physical,
            true,
            btc.collateral_discount,
            btc.lending_debt_weight,
            vec![FixedI64::one()]
        ));

        // Ensure the expected error is thrown when add asset what already exist.
        assert_noop!(
            ModuleAssets::add_asset(
                RawOrigin::Root.into(),
                btc.id.to_str_bytes(),
                btc.lot.into_inner(),
                btc.price_step.into_inner(),
                btc.maker_fee,
                btc.taker_fee,
                AssetXcmData::None,
                btc.debt_weight,
                btc.buyout_priority,
                AssetType::Physical,
                true,
                btc.collateral_discount,
                btc.lending_debt_weight,
                vec![FixedI64::one()]
            ),
            Error::<Test>::AssetAlreadyExists
        );

        // Remove asset
        assert_ok!(ModuleAssets::remove_asset(RawOrigin::Root.into(), btc.id));

        // Ensure the expected error is thrown when remove asset what not exist.
        assert_noop!(
            ModuleAssets::remove_asset(RawOrigin::Root.into(), btc.id),
            Error::<Test>::AssetAlreadyToBeRemoved
        );

        // Ensure the expected error is thrown when asset name is too long.
        let long_names = vec!["xyzxyzxyz", "stabledoge"];
        for long_name in long_names {
            assert_noop!(
                ModuleAssets::add_asset(
                    RawOrigin::Root.into(),
                    Vec::from(long_name),
                    btc.lot.into_inner(),
                    btc.price_step.into_inner(),
                    btc.maker_fee,
                    btc.taker_fee,
                    AssetXcmData::None,
                    btc.debt_weight,
                    btc.buyout_priority,
                    AssetType::Physical,
                    true,
                    btc.collateral_discount,
                    btc.lending_debt_weight,
                    vec![FixedI64::one()]
                ),
                Error::<Test>::AssetNameWrongLength
            );
        }

        // Ensure the expected error is thrown when create asset with UPPERCASE latin symbols in name.
        let uppercase_names = vec!["usdt!", "#eth", "%eq", "*btc", "xyzxyzx:"];
        for name in uppercase_names {
            assert_noop!(
                ModuleAssets::add_asset(
                    RawOrigin::Root.into(),
                    Vec::<u8>::from(name),
                    btc.lot.into_inner(),
                    btc.price_step.into_inner(),
                    btc.maker_fee,
                    btc.taker_fee,
                    AssetXcmData::None,
                    btc.debt_weight,
                    btc.buyout_priority,
                    AssetType::Physical,
                    true,
                    btc.collateral_discount,
                    btc.lending_debt_weight,
                    vec![FixedI64::one()]
                ),
                Error::<Test>::AssetNameWrongSymbols
            );
        }
    });
}

#[test]
fn assets_add_remove() {
    new_test_ext().execute_with(|| {
        // prepare assets
        // let eq: AssetData<Asset> = create_empty_asset(asset::EQ); // 25969
        let btc: AssetData<Asset> = create_empty_asset(asset::BTC); // 6452323
                                                                    // let dot: AssetData<Asset> = create_empty_asset(asset::DOT); // 6582132
                                                                    // let eth: AssetData<Asset> = create_empty_asset(asset::ETH); // 6648936

        // check that assets is empty
        assert_eq!(ModuleAssets::assets(), None);

        // add assets in not sorted order
        assert_ok!(ModuleAssets::add_asset(
            RawOrigin::Root.into(),
            btc.id.to_str_bytes(),
            btc.lot.into_inner(),
            btc.price_step.into_inner(),
            btc.maker_fee,
            btc.taker_fee,
            AssetXcmData::None,
            btc.debt_weight,
            btc.buyout_priority,
            AssetType::Physical,
            true,
            btc.collateral_discount,
            btc.lending_debt_weight,
            vec![FixedI64::one()]
        ));

        // check that assets storage is not empty
        assert_ne!(ModuleAssets::assets(), None);
        // check that assets are equal
        assert_eq!(ModuleAssets::get_asset_data(&btc.id).unwrap(), btc);

        // remove asset
        assert_ok!(ModuleAssets::remove_asset(
            RawOrigin::Root.into(),
            asset::BTC
        ));

        // check that assets removal queue isn't empty
        assert_eq!(ModuleAssets::assets_to_remove(), Some(vec![btc.id]));
    });
}

#[test]
fn assets_with_digits() {
    new_test_ext().execute_with(|| {
        // check that assets is empty
        assert_eq!(ModuleAssets::assets(), None);

        // create asset with valid names
        let uppercase_names = vec!["eq1", "ETH", "btc9", "try0", "X5", "404"];
        let assets = uppercase_names
            .into_iter()
            .map(|name| {
                assert_ok!(ModuleAssets::add_asset(
                    RawOrigin::Root.into(),
                    Vec::<u8>::from(name),
                    0_u128,
                    0_i64,
                    Permill::zero(),
                    Permill::zero(),
                    AssetXcmData::None,
                    Permill::zero(),
                    100_u64,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                    vec![FixedI64::one()]
                ));

                // remove asset
                let asset = asset::Asset::from_bytes(&Vec::<u8>::from(name)).unwrap();
                assert_ok!(ModuleAssets::remove_asset(RawOrigin::Root.into(), asset,));
                asset
            })
            .collect();

        // check that assets storage is not empty
        assert_eq!(ModuleAssets::assets_to_remove(), Some(assets));
    });
}

#[test]
fn add_asset_and_update() {
    new_test_ext().execute_with(|| {
        // check that assets are empty
        assert_eq!(ModuleAssets::assets(), None);

        // create asset with valid name
        let btc: AssetData<Asset> = create_empty_asset(asset::BTC); // 6452323

        // add assets in not sorted order
        assert_ok!(ModuleAssets::add_asset(
            RawOrigin::Root.into(),
            btc.id.to_str_bytes(),
            btc.lot.into_inner(),
            btc.price_step.into_inner(),
            btc.maker_fee,
            btc.taker_fee,
            AssetXcmData::None,
            btc.debt_weight,
            btc.buyout_priority,
            AssetType::Physical,
            true,
            btc.collateral_discount,
            btc.lending_debt_weight,
            vec![FixedI64::one()]
        ));

        // check that assets storage is not empty
        assert_ne!(ModuleAssets::assets(), None);
        // check that assets are equal
        assert_eq!(ModuleAssets::get_asset_data(&btc.id), Ok(btc.clone()));

        let new_name = "newToken";
        assert_noop!(
            ModuleAssets::update_asset(
                RawOrigin::Root.into(),
                asset::Asset::from_bytes(&Vec::<u8>::from(new_name)).unwrap(),
                Some(btc.lot.into_inner()),
                Some(btc.price_step.into_inner()),
                Some(btc.maker_fee),
                Some(btc.taker_fee),
                None,
                Some(btc.debt_weight),
                Some(btc.buyout_priority),
                Some(AssetType::Native),
                Some(true),
                Some(btc.collateral_discount),
                Some(Permill::from_parts(900_000))
            ),
            Error::<Test>::AssetNotExists
        );

        assert_ok!(ModuleAssets::update_asset(
            RawOrigin::Root.into(),
            btc.id,
            Some(10_u128),
            Some(10_i64),
            Some(Permill::from_parts(10_u32)),
            Some(Permill::from_parts(10_u32)),
            None,
            Some(Permill::from_parts(10_u32)),
            Some(200_u64),
            None,
            Some(false),
            Some(Percent::from_parts(10_u8)),
            Some(Permill::from_parts(900_000))
        ));

        let result: AssetData<Asset> = AssetData {
            id: btc.id,
            lot: EqFixedU128::from_inner(10_u128),
            price_step: FixedI64::from_inner(10_i64),
            maker_fee: Permill::from_parts(10_u32),
            taker_fee: Permill::from_parts(10_u32),
            asset_xcm_data: AssetXcmData::None,
            debt_weight: Permill::from_parts(10_u32),
            buyout_priority: 200_u64,
            asset_type: AssetType::Physical,
            is_dex_enabled: false,
            collateral_discount: Percent::from_parts(10_u8),
            lending_debt_weight: Permill::from_parts(900_000),
        };
        // check that assets are equal
        assert_eq!(ModuleAssets::get_asset_data(&btc.id).unwrap(), result);
    });
}

#[test]
fn update_only_one_field_in_asset() {
    new_test_ext().execute_with(|| {
        // check that assets are empty
        assert_eq!(ModuleAssets::assets(), None);

        // create asset with valid name
        let mut btc: AssetData<Asset> = create_empty_asset(asset::BTC); // 6452323

        // add assets in not sorted order
        assert_ok!(ModuleAssets::add_asset(
            RawOrigin::Root.into(),
            btc.id.to_str_bytes(),
            btc.lot.into_inner(),
            btc.price_step.into_inner(),
            btc.maker_fee,
            btc.taker_fee,
            AssetXcmData::None,
            btc.debt_weight,
            btc.buyout_priority,
            btc.asset_type.clone(),
            btc.is_dex_enabled,
            btc.collateral_discount,
            btc.lending_debt_weight,
            vec![FixedI64::one()],
        ));

        // check that assets storage is not empty
        assert_ne!(ModuleAssets::assets(), None);
        // check that assets are equal
        assert_eq!(ModuleAssets::get_asset_data(&btc.id).unwrap(), btc.clone());

        let new_buyout_priority = 7;
        btc.buyout_priority = new_buyout_priority;

        assert_ok!(ModuleAssets::update_asset(
            RawOrigin::Root.into(),
            btc.id,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(new_buyout_priority),
            None,
            None,
            None,
            None,
        ));

        // check that assets are equal
        assert_eq!(ModuleAssets::get_asset_data(&btc.id).unwrap(), btc.clone());
    });
}
