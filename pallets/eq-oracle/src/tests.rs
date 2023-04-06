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

use frame_support::{assert_err, assert_ok};
use sp_arithmetic::FixedI64;

use eq_primitives::asset;

use crate::{
    mock::{
        new_test_ext, EqAssets, ModuleOracle, ModuleSystem, ModuleTimestamp, ModuleWhitelist, Test,
    },
    price_source::WithUrl,
};

use super::*;

pub type Sign = sp_core::sr25519::Public;

fn set_price(
    account: Sign,
    asset: Asset,
    price: f64,
    block_number: u64,
) -> DispatchResultWithPostInfo {
    let dummy_signature = sp_core::sr25519::Signature([0u8; 64]);
    let payload = PricePayload {
        public: account,
        asset,
        price: FixedI64::from_inner((price * (FixedI64::accuracy() as f64)) as i64),
        block_number,
    };
    ModuleOracle::set_price_unsigned(
        frame_system::RawOrigin::None.into(),
        payload,
        dummy_signature,
    )
}

fn set_price_ok(account: Sign, asset: Asset, price: f64, block_number: u64) {
    assert_ok!(set_price(account, asset, price, block_number));
}

fn check_price(asset: Asset, price: f64) {
    assert_eq!(
        ModuleOracle::get_price::<FixedI64>(&asset).unwrap(),
        FixedI64::from_inner((price * (FixedI64::accuracy() as f64)) as i64)
    );
}

fn check_error(dr: DispatchResultWithPostInfo, msg: &str) {
    let a: &str = From::<
        sp_runtime::DispatchErrorWithPostInfo<frame_support::weights::PostDispatchInfo>,
    >::from(dr.expect_err(""));
    assert_eq!(a, msg);
}

fn time_move(time: &mut u64, step: u64) {
    println!(
        "timemove: time: {} sec, step: {} sec, current: {} sec.",
        *time,
        step,
        *time + step
    );

    *time = *time + step;
    ModuleTimestamp::set_timestamp(*time * 1000);
    ModuleSystem::set_block_number(*time / 6);
}

#[test]
///Test
fn main_test() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [0; 32] };
        let account_id_2 = Sign { 0: [1; 32] };

        check_error(
            set_price(account_id_1, asset::EQ, 1., 0),
            "NotAllowedToSubmitPrice",
        );
        assert_err!(
            set_price(account_id_1, asset::EQ, 2., 0),
            Error::<Test>::NotAllowedToSubmitPrice
        );

        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_2
        ));

        assert_err!(
            set_price(account_id_1, asset::EQ, 0., 0),
            Error::<Test>::PriceIsZero
        );
        assert_err!(
            set_price(account_id_1, asset::EQ, -1., 0),
            Error::<Test>::PriceIsNegative
        );
        assert_err!(
            set_price(account_id_1, asset::EQD, 1., 0),
            Error::<Test>::WrongCurrency
        );
        /*assert_err!(
            set_price(account_id_1, Currency::Unknown, 1.),
            Error::<Test>::WrongCurrency
        );*/

        set_price_ok(account_id_1, asset::EQ, 100_000.17, 1);
        set_price_ok(account_id_2, asset::EQ, 200_000.13, 1);

        set_price_ok(account_id_1, asset::BTC, 10.19, 1);
        set_price_ok(account_id_2, asset::BTC, 20.23, 1);

        check_price(asset::EQ, 150_000.15);
        check_price(asset::BTC, 15.21);

        // ModuleTimestamp::set_timestamp(2000); todo check
        ModuleSystem::set_block_number(2);

        set_price_ok(account_id_1, asset::EQ, 10_000., 2);
        set_price_ok(account_id_2, asset::EQ, 20_000., 2);

        check_price(asset::EQ, 15_000.);
    });
}

#[test]
fn set_price_not_from_whitelist() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [0; 32] };

        check_error(
            set_price(account_id_1, asset::EQ, 1., 0),
            "NotAllowedToSubmitPrice",
        );
        assert_err!(
            set_price(account_id_1, asset::EQ, 2., 0),
            Error::<Test>::NotAllowedToSubmitPrice
        );
    });
}

#[test]
fn set_price_from_whitelist() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [0; 32] };

        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));
        set_price_ok(account_id_1, asset::EQ, 100_000., 0);
    });
}

#[test]
fn set_median_price() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [1; 32] };
        let account_id_2 = Sign { 0: [2; 32] };
        let account_id_3 = Sign { 0: [3; 32] };
        let account_id_4 = Sign { 0: [4; 32] };
        let account_id_5 = Sign { 0: [5; 32] };
        let account_id_6 = Sign { 0: [6; 32] };
        let account_id_7 = Sign { 0: [7; 32] };

        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_2
        ));
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_3
        ));
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_4
        ));
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_5
        ));
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_6
        ));
        set_price_ok(account_id_1, asset::EQ, 35_000., 0);
        check_price(asset::EQ, 35_000.);
        ModuleSystem::set_block_number(2);
        set_price_ok(account_id_1, asset::EQ, 40_000., 2);
        check_price(asset::EQ, 40_000.);
        set_price_ok(account_id_2, asset::EQ, 50_000., 2);
        check_price(asset::EQ, 45_000.);
        set_price_ok(account_id_3, asset::EQ, 130_000., 2);
        check_price(asset::EQ, 50_000.);
        set_price_ok(account_id_4, asset::EQ, 1_000., 2);
        check_price(asset::EQ, 45_000.);
        set_price_ok(account_id_5, asset::EQ, 120_000., 2);
        check_price(asset::EQ, 50_000.);
        set_price_ok(account_id_6, asset::EQ, 2_000., 2);
        check_price(asset::EQ, 45_000.);
        ModuleSystem::set_block_number(3);
        set_price_ok(account_id_1, asset::EQ, 60_000., 3);
        check_price(asset::EQ, 55_000.);

        assert_ok!(ModuleWhitelist::remove_from_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));
        assert_ok!(ModuleWhitelist::remove_from_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_2
        ));
        ModuleSystem::set_block_number(4);
        set_price_ok(account_id_3, asset::EQ, 5_000., 4);
        check_price(asset::EQ, 27_500.);

        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_7
        ));
        ModuleSystem::set_block_number(5);
        set_price_ok(account_id_3, asset::EQ, 70_000., 5);
        check_price(asset::EQ, 55_000.);

        // data_point price timeout
        ModuleSystem::set_block_number(6);
        set_price_ok(account_id_3, asset::EQ, 30_000., 6);
        ModuleTimestamp::set_timestamp(2000);
        set_price_ok(account_id_4, asset::EQ, 40_000., 6);
        set_price_ok(account_id_5, asset::EQ, 50_000., 6);
        set_price_ok(account_id_6, asset::EQ, 60_000., 6);
        set_price_ok(account_id_7, asset::EQ, 70_000., 6);
        check_price(asset::EQ, 55_000.);
    });
}

#[test]
fn set_price_twice_block_moved() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [0; 32] };

        ModuleTimestamp::set_timestamp(2000);
        ModuleSystem::set_block_number(1);

        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));

        set_price_ok(account_id_1, asset::EQ, 10_000., 1);

        ModuleSystem::set_block_number(2);

        set_price_ok(account_id_1, asset::EQ, 20_000., 2);
    });
}

#[test]
fn set_price_twice_time_moved() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [0; 32] };

        ModuleTimestamp::set_timestamp(2000);
        ModuleSystem::set_block_number(1);

        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));

        set_price_ok(account_id_1, asset::EQ, 10_000., 1);

        ModuleTimestamp::set_timestamp(3000);

        check_error(
            set_price(account_id_1, asset::EQ, 20_000., 1),
            "PriceAlreadyAdded",
        );
    });
}

#[test]
fn not_set_price_twice() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [0; 32] };

        ModuleTimestamp::set_timestamp(2000);
        ModuleSystem::set_block_number(1);

        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));
        set_price_ok(account_id_1, asset::EQ, 10_000., 1);

        assert_err!(
            set_price(account_id_1, asset::EQ, 20_000., 1),
            Error::<Test>::PriceAlreadyAdded
        );
    });
}

#[test]
fn check_json_reader() {
    new_test_ext().execute_with(|| {
        assert_err!(
            JsonPriceSource::fetch_price_from_json("".to_string(), "USD"),
            PriceSourceError::DeserializationError
        );
        assert_err!(
            JsonPriceSource::fetch_price_from_json("rtdfgfdgfdgf".to_string(), "USD"),
            PriceSourceError::DeserializationError
        );
        assert_err!(
            JsonPriceSource::fetch_price_from_json("{USD:2.98}".to_string(), "USD"),
            PriceSourceError::DeserializationError
        );
        assert_err!(
            JsonPriceSource::fetch_price_from_json("{\"USD\":'2.98'}".to_string(), "USD"),
            PriceSourceError::DeserializationError
        );

        let val = FixedI64::from_inner((2.98 * (FixedI64::accuracy() as f64)) as i64);
        assert_eq!(
            JsonPriceSource::fetch_price_from_json("{\"USD\":2.98}".to_string(), "USD"),
            Ok(val)
        );
        assert_eq!(
            JsonPriceSource::fetch_price_from_json("{\"USD\":\"2.98\"}".to_string(), "USD"),
            Ok(val)
        );

        assert_err!(
            JsonPriceSource::fetch_price_from_json("{\"price\":\"2.98\"}".to_string(), "USD"),
            PriceSourceError::JsonParseError
        );

        assert_err!(
            JsonPriceSource::fetch_price_from_json("{\"price\":\"2.98\"}".to_string(), "USD"),
            PriceSourceError::JsonParseError
        );

        assert_eq!(
            JsonPriceSource::fetch_price_from_json(
                "{\"price\": {\"last\": \"2.98\"}}".to_string(),
                "price.last"
            ),
            Ok(val)
        );

        assert_eq!(
            JsonPriceSource::fetch_price_from_json(
                "{\"price\": [\"3.46\", \"2.98\"]}".to_string(),
                "price[1]"
            ),
            Ok(val)
        );

        assert_eq!(
            JsonPriceSource::fetch_price_from_json(
                "{\"price\": {\"last\": [\"2.98\"]}}".to_string(),
                "price.last[0]"
            ),
            Ok(val)
        );

        assert_eq!(
            JsonPriceSource::fetch_price_from_json("[\"2.98\"]".to_string(), "[0]"),
            Ok(val)
        );

        assert_eq!(
            JsonPriceSource::fetch_price_from_json(
                "{\"data\": [ {\"data\": [ { \"price\": \"2.98\" } ] } ] }".to_string(),
                "data[0].data[0].price"
            ),
            Ok(val)
        );
    });
}

#[test]
fn invalid_prices() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [0; 32] };
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));
        assert_eq!(ModuleWhitelist::in_whitelist(&account_id_1), true);
        assert_err!(
            set_price(account_id_1, asset::EQ, 0., 0),
            Error::<Test>::PriceIsZero
        );
        assert_err!(
            set_price(account_id_1, asset::EQ, -1., 0),
            Error::<Test>::PriceIsNegative
        );
    });
}

#[test]
fn test_timeout() {
    new_test_ext().execute_with(|| {
        let mut time: u64 = 0;
        time_move(&mut time, 10000);
        time_move(&mut time, 10000);
        time_move(&mut time, 10000);

        let account_id = Sign { 0: [0; 32] };

        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id
        ));

        set_price_ok(account_id, asset::EQ, 0.000_000_001, 0);
        check_price(asset::EQ, 0.000_000_001);

        time_move(&mut time, 7199);

        check_price(asset::EQ, 0.000_000_001);

        time_move(&mut time, 1);

        assert_err!(
            ModuleOracle::get_price::<FixedI64>(&asset::EQ),
            Error::<Test>::PriceTimeout
        );
    });
}

#[test]
fn invalid_currencies() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [0; 32] };
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));

        assert_err!(
            set_price(account_id_1, asset::EQD, 1., 0),
            Error::<Test>::WrongCurrency
        );
        /*assert_err!(
            set_price(account_id_1, Currency::Unknown, 1.),
            Error::<Test>::WrongCurrency
        );*/
    });
}

#[test]
fn should_build_on_genesis_price_points() {
    new_test_ext().execute_with(|| {
        let default_price_point = PricePoint {
            block_number: frame_system::Pallet::<Test>::block_number(),
            timestamp: 0,
            last_fin_recalc_timestamp: 0,
            price: FixedI64::saturating_from_integer(-1i32),
            data_points: Vec::<
                DataPoint<
                    <mock::Test as frame_system::Config>::AccountId,
                    <mock::Test as frame_system::Config>::BlockNumber,
                >,
            >::new(),
        };

        assert_eq!(<PricePoints<Test>>::contains_key(asset::EQ), true);
        assert_eq!(<PricePoints<Test>>::contains_key(asset::BTC), true);
        assert_eq!(<PricePoints<Test>>::contains_key(asset::ETH), true);
        assert_eq!(<PricePoints<Test>>::contains_key(asset::EQD), false);

        assert_eq!(
            ModuleOracle::price_points(asset::EQ).unwrap(),
            default_price_point
        );
        assert_eq!(
            ModuleOracle::price_points(asset::ETH).unwrap(),
            default_price_point
        );
        assert_eq!(
            ModuleOracle::price_points(asset::BTC).unwrap(),
            default_price_point
        );
    });
}

#[test]
fn set_price_when_stored_price_newer_should_fail() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [0; 32] };
        let account_id_2 = Sign { 0: [2; 32] };
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_2
        ));

        ModuleSystem::set_block_number(2);

        set_price_ok(account_id_1, asset::EQ, 100_000.17, 2);

        set_price_ok(account_id_2, asset::EQ, 100_000., 1);

        assert_err!(
            set_price(account_id_1, asset::EQ, 100_100., 1),
            Error::<Test>::PriceAlreadyAdded
        );

        assert_err!(
            set_price(account_id_1, asset::EQ, 100_100., 2),
            Error::<Test>::PriceAlreadyAdded
        );
    });
}

#[test]
fn filter_prices_from_test() {
    new_test_ext().execute_with(|| {
        let account_id_1 = Sign { 0: [1; 32] };
        let account_id_2 = Sign { 0: [2; 32] };
        let account_id_3 = Sign { 0: [3; 32] };
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_1
        ));
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_2
        ));
        assert_ok!(ModuleWhitelist::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id_3
        ));

        set_price_ok(account_id_1, asset::EQ, 80_000., 1);
        set_price_ok(account_id_2, asset::EQ, 90_000., 1);
        set_price_ok(account_id_3, asset::EQ, 100_000., 1);

        let price_point = <PricePoints<Test>>::get(asset::EQ).unwrap();

        assert_eq!(price_point.price, FixedI64::saturating_from_integer(90_000));
        ModuleOracle::filter_prices_from(&account_id_1);

        let price_point = <PricePoints<Test>>::get(asset::EQ).unwrap();

        assert_eq!(price_point.price, FixedI64::saturating_from_integer(95_000));

        for data_point in price_point.data_points {
            assert!(data_point.account_id != account_id_1);
        }

        set_price_ok(account_id_1, asset::EQ, 110_000., 1);

        let price_point = <PricePoints<Test>>::get(asset::EQ).unwrap();

        assert_eq!(
            price_point.price,
            FixedI64::saturating_from_integer(100_000)
        );
    });
}

#[test]
fn on_initialize_asset_removal_with_non_zero_collat() {
    new_test_ext().execute_with(|| {
        use frame_support::{traits::OnInitialize, StorageMap};
        eq_assets::AssetsToRemove::<Test>::put(vec![asset::BTC]);
        financial_pallet::PriceLogs::<Test>::insert::<Asset, financial_pallet::PriceLog<I64F64>>(
            asset::BTC,
            Default::default(),
        );
        assert_eq!(
            financial_pallet::Pallet::<Test>::price_logs(asset::BTC).unwrap(),
            Default::default()
        );
        assert_eq!(EqAssets::assets_to_remove(), Some(vec![asset::BTC]));

        ModuleOracle::on_initialize(1);
        assert_eq!(EqAssets::assets_to_remove(), Some(vec![asset::BTC]));
        assert_eq!(
            financial_pallet::Pallet::<Test>::price_logs(asset::BTC).unwrap(),
            Default::default()
        );
    });
}

#[test]
fn on_initialize_asset_removal_with_zero_collat() {
    new_test_ext().execute_with(|| {
        use frame_support::{traits::OnInitialize, StorageMap};
        eq_assets::AssetsToRemove::<Test>::put(vec![asset::ETH]);
        financial_pallet::PriceLogs::<Test>::insert::<Asset, financial_pallet::PriceLog<I64F64>>(
            asset::ETH,
            Default::default(),
        );
        assert_eq!(
            financial_pallet::Pallet::<Test>::price_logs(asset::ETH).unwrap(),
            Default::default()
        );
        assert_eq!(EqAssets::assets_to_remove(), Some(vec![asset::ETH]));

        ModuleOracle::on_initialize(1);
        assert_eq!(EqAssets::assets_to_remove(), Some(vec![]));
        assert!(financial_pallet::Pallet::<Test>::price_logs(asset::ETH).is_none());
    });
}

#[test]
fn url_symbol_case() {
    let huobi_url_template = "https://api.huobi.pro/market/history/trade?symbol={$}usdt&size=1";
    let huobi_url = asset::BTC.get_url(huobi_url_template, "");

    assert!(huobi_url.is_ok());

    assert_eq!(
        huobi_url.unwrap().0,
        "https://api.huobi.pro/market/history/trade?symbol=btcusdt&size=1"
    );

    let kraken_url_template = "https://api.kraken.com/0/public/Ticker?pair={$}USD";
    let kraken_url = asset::BTC.get_url(kraken_url_template, "");

    assert!(kraken_url.is_ok());

    assert_eq!(
        kraken_url.unwrap().0,
        "https://api.kraken.com/0/public/Ticker?pair=XXBTZUSD"
    );
}
