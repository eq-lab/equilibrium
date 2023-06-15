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
#![allow(unused_imports)]

use super::*;
use crate::mock::*;

use eq_primitives::balance::{DebtCollateralDiscounted, EqCurrency};
use eq_primitives::balance_number::{abs_checked_sub, EqFixedU128};
use eq_primitives::{
    asset, BalanceChange, OrderAggregateBySide, PriceGetter, PriceSetter, SignedBalance,
};
use eq_utils::{
    fixed::{eq_fixedu128_from_balance, fixedi128_from_balance, fixedi128_from_fixedi64},
    ONE_TOKEN,
};
use frame_support::dispatch::DispatchError::BadOrigin;
use frame_support::dispatch::{DispatchError, DispatchErrorWithPostInfo, Pays, PostDispatchInfo};
use frame_support::traits::OffchainWorker;
use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_arithmetic::{FixedI128, FixedPointNumber, FixedU128};
use sp_core::offchain::{
    testing::{TestOffchainExt, TestTransactionPoolExt},
    TransactionPoolExt,
};

pub const USER: u64 = 0x1;

#[test]
fn margincall_external_origin_none() {
    let origin = frame_system::RawOrigin::None;
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let r = ModuleMarginCall::try_margincall_external(origin.into(), USER);
        assert_eq!(
            r,
            Err(DispatchErrorWithPostInfo {
                post_info: PostDispatchInfo {
                    actual_weight: None,
                    pays_fee: Pays::Yes,
                },
                error: DispatchError::BadOrigin,
            })
        );
    });
}

#[test]
fn margincall_external() {
    let mut ext = new_test_ext();

    let origin = frame_system::RawOrigin::Signed(USER);
    ext.execute_with(|| {
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::BTC,
            SignedBalance::<Balance>::Positive(10 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(962380 * ONE_TOKEN),
        );
        let state = ModuleMarginCall::check_margin(&USER);
        assert_ok!(state);
        assert_eq!(state.unwrap(), MarginState::SubCritical);
        assert_ok!(ModuleMarginCall::try_margincall_external(
            origin.into(),
            USER
        ));
        let DebtCollateralDiscounted {
            debt: d,
            collateral: c,
            discounted_collateral: _,
        } = ModuleBalances::get_debt_and_collateral(&USER).unwrap();
        assert_eq!(d, Balance::zero());
        assert_eq!(c, Balance::zero());
    });
}

use eq_primitives::OrderChange;
#[allow(unused_imports)]
use eq_primitives::{MarginCallManager, MarginState};
use sp_arithmetic::traits::Bounded;
use sp_runtime::traits::One;
use sp_runtime::{FixedI64, Permill};

#[test]
fn margincall_good() {
    let mut ext = new_test_ext();

    ext.execute_with(|| {
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::BTC,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(9623 * ONE_TOKEN),
        );
        let r = ModuleMarginCall::check_margin(&USER).unwrap();
        assert_eq!(r, MarginState::Good);
    });
}

#[test]
fn margincall_subgood() {
    new_test_ext().execute_with(|| {
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::BTC,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(962380 * ONE_TOKEN),
        );
        let r = ModuleMarginCall::check_margin(&USER).unwrap();
        assert_eq!(r, MarginState::SubGood);
    });
}

#[test]
fn margincall_maintenance_start() {
    new_test_ext().execute_with(|| {
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::BTC,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(975001 * ONE_TOKEN),
        );
        let r = ModuleMarginCall::check_margin(&USER).unwrap();
        assert!(matches!(r, MarginState::MaintenanceStart));
        let r = ModuleMarginCall::check_margin(&USER).unwrap();
        assert!(matches!(r, MarginState::MaintenanceStart));
        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert!(matches!(r, MarginState::MaintenanceIsGoing));
        assert!(<MaintenanceTimers<Test>>::contains_key(&USER));
    });
}

#[test]
fn margincall_maintenance_margin_supplied_to_good() {
    new_test_ext().execute_with(|| {
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::BTC,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(975001 * ONE_TOKEN),
        );
        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert!(matches!(r, MarginState::MaintenanceIsGoing));
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(900000 * ONE_TOKEN),
        );
        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert_eq!(r, MarginState::MaintenanceEnd);
        assert_eq!(<MaintenanceTimers<Test>>::contains_key(&USER), false);
    });
}

#[test]
fn margincall_maintenance_timer_is_over() {
    new_test_ext().execute_with(|| {
        let collateral: Balance = 100 * ONE_TOKEN; //100.5 BTC
        let debt: Balance = 975001 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::BTC,
            SignedBalance::<Balance>::Positive(collateral),
        );
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(debt),
        );

        // collateral * price - debt * (1 + penalty)
        let btc_price = <mock::Test as Config>::PriceGetter::get_price(&asset::BTC).unwrap();
        let expected_rest_collateral = eq_fixedu128_from_balance(collateral) * btc_price
            - eq_fixedu128_from_balance(debt) * (EqFixedU128::one() + CriticalMargin::get());

        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert!(matches!(r, MarginState::MaintenanceIsGoing));

        ModuleTimestamp::set_timestamp(ModuleTimestamp::get() + 86_401_000);

        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert!(matches!(r, MarginState::MaintenanceTimeOver));
        assert_eq!(<MaintenanceTimers<Test>>::contains_key(&USER), false);
        let DebtCollateralDiscounted {
            debt: d,
            collateral: c,
            discounted_collateral: _,
        } = ModuleBalances::get_debt_and_collateral(&USER).unwrap();
        assert_eq!(d, Balance::zero());
        assert_eq!(
            c,
            balance_from_eq_fixedu128::<Balance>(expected_rest_collateral).unwrap()
        );
    });
}

#[test]
fn margincall_subcritical() {
    new_test_ext().execute_with(|| {
        let collateral: Balance = 100_500_000_000; //100.5 BTC
        let debt: Balance = 999999 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::BTC,
            SignedBalance::<Balance>::Positive(collateral),
        );
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(debt),
        );

        // collateral * price - debt * (1 + penalty)
        let btc_price = <mock::Test as Config>::PriceGetter::get_price(&asset::BTC).unwrap();
        let expected_rest_collateral = eq_fixedu128_from_balance(collateral) * btc_price
            - eq_fixedu128_from_balance(debt) * (EqFixedU128::one() + CriticalMargin::get());

        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert_eq!(r, MarginState::SubCritical);
        assert_eq!(<MaintenanceTimers<Test>>::contains_key(&USER), false);
        let DebtCollateralDiscounted {
            debt: d,
            collateral: c,
            discounted_collateral: _,
        } = ModuleBalances::get_debt_and_collateral(&USER).unwrap();
        assert_eq!(d, Balance::zero());
        assert_eq!(
            c,
            balance_from_eq_fixedu128::<Balance>(expected_rest_collateral).unwrap()
        );
    });
}

#[test]
fn margincall_with_orders() {
    new_test_ext().execute_with(|| {
        let collateral: Balance = 100 * ONE_TOKEN; //100.5 BTC
        let debt: Balance = 975001 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::BTC,
            SignedBalance::<Balance>::Positive(collateral),
        );
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(debt),
        );

        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        println!("{:?}", r);
        assert_eq!(r, MarginState::MaintenanceIsGoing);

        mock::OrderAggregatesMock::set_order_aggregates(vec![(
            asset::BTC,
            OrderAggregateBySide::new(EqFixedU128::one(), EqFixedU128::one(), OrderSide::Sell)
                .unwrap(),
        )]);

        ModuleTimestamp::set_timestamp(ModuleTimestamp::get() + 86_401_000);

        let no_orders = mock::OrderAggregatesMock::get_asset_weights(&USER).is_empty();
        println!("{no_orders:?}");

        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert_eq!(r, MarginState::MaintenanceIsGoing);
        assert_eq!(<MaintenanceTimers<Test>>::contains_key(&USER), true);

        mock::OrderAggregatesMock::set_order_aggregates(vec![]);

        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert_eq!(r, MarginState::MaintenanceTimeOver);
        assert_eq!(<MaintenanceTimers<Test>>::contains_key(&USER), false);

        let collateral: Balance = 100_500_000_000; //100.5 BTC
        let debt: Balance = 999999 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::BTC,
            SignedBalance::<Balance>::Positive(collateral),
        );
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::EQD,
            SignedBalance::<Balance>::Negative(debt),
        );
        mock::OrderAggregatesMock::set_order_aggregates(vec![(
            asset::BTC,
            OrderAggregateBySide::new(EqFixedU128::one(), EqFixedU128::one(), OrderSide::Sell)
                .unwrap(),
        )]);

        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert_eq!(r, MarginState::MaintenanceIsGoing);

        mock::OrderAggregatesMock::set_order_aggregates(vec![]);

        let r = ModuleMarginCall::try_margincall(&USER).unwrap();
        assert_eq!(r, MarginState::SubCritical);
    });
}

#[test]
fn margin_is_increasing() {
    new_test_ext().execute_with(|| {
        let _ = OracleMock::set_price(0, asset::ETH, FixedI64::saturating_from_integer(7));
        let _ = OracleMock::set_price(0, asset::DOT, FixedI64::saturating_from_integer(4));

        // collateral = 50 * price
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::ETH,
            SignedBalance::<Balance>::Positive(50 * ONE_TOKEN),
        );

        // debt = 50 * price
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::DOT,
            SignedBalance::<Balance>::Negative(50 * ONE_TOKEN),
        );

        let balance_changes = vec![BalanceChange {
            asset: asset::DOT,
            change: SignedBalance::Positive(20 * ONE_TOKEN),
        }];

        let current_margin = ModuleMarginCall::calculate_portfolio_margin(&USER, &[], &[]).unwrap();
        let new_margin =
            ModuleMarginCall::calculate_portfolio_margin(&USER, &balance_changes, &[]).unwrap();
        assert!(new_margin > current_margin);
        assert_eq!(true, new_margin.1);
    });
}

#[test]
fn margin_decreasing() {
    new_test_ext().execute_with(|| {
        let _ = OracleMock::set_price(0, asset::ETH, FixedI64::saturating_from_integer(7));
        let _ = OracleMock::set_price(0, asset::DOT, FixedI64::saturating_from_integer(4));

        // collateral = 100 * price
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::ETH,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );

        // debt  = 10 * price
        ModuleBalances::make_free_balance_be(
            &USER,
            asset::DOT,
            SignedBalance::<Balance>::Negative(10 * ONE_TOKEN),
        );

        let balance_changes = vec![BalanceChange {
            asset: asset::ETH,
            change: SignedBalance::Negative(20 * ONE_TOKEN),
        }];

        let current_margin = ModuleMarginCall::calculate_portfolio_margin(&USER, &[], &[]).unwrap();
        let new_margin =
            ModuleMarginCall::calculate_portfolio_margin(&USER, &balance_changes, &[]).unwrap();
        assert!(new_margin < current_margin);
        assert_eq!(false, new_margin.1);
    });
}

#[test]
fn calculate_portfolio_margin_when_debt_is_zero_should_return_max_value() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::BTC,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );

        let (margin, _) =
            ModuleMarginCall::calculate_portfolio_margin(&account_id, &[], &[]).unwrap();
        assert_eq!(margin, EqFixedU128::max_value())
    });
}

#[test]
fn calculate_portfolio_margin_when_collateral_is_zero_should_return_min_value() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQD,
            SignedBalance::<Balance>::Negative(100 * ONE_TOKEN),
        );

        let margin = ModuleMarginCall::calculate_portfolio_margin(&account_id, &[], &[]);
        assert_err!(margin, Error::<Test>::ZeroCollateral);
    });
}

#[test]
fn calculate_portfolio_margin_with_zero_discount_asset() {
    new_test_ext().execute_with(|| {
        let account_id = 100u64;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::ETH,
            SignedBalance::<Balance>::Positive(1 * ONE_TOKEN),
        );

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQD,
            SignedBalance::<Balance>::Negative(100 * ONE_TOKEN),
        );

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::USDC,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );

        let balances = ModuleBalances::iterate_account_balances(&account_id);

        let margin =
            ModuleMarginCall::calculate_portfolio_margin_for_balances(&account_id, &balances, &[])
                .unwrap();
        let margin_without_discount =
            EqFixedU128::saturating_from_rational(250 + 100 - 100, 250 + 100);
        let margin_with_discount = EqFixedU128::saturating_from_rational(250 - 100, 250);
        assert_ne!(margin, margin_without_discount);
        assert_eq!(margin, margin_with_discount);
    });
}

#[test]
fn calculate_portfolio_margin_with_one_half_discount_asset() {
    new_test_ext().execute_with(|| {
        let account_id = 101u64;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::ETH,
            SignedBalance::<Balance>::Positive(1 * ONE_TOKEN),
        );

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQD,
            SignedBalance::<Balance>::Negative(100 * ONE_TOKEN),
        );

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::USDT,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );

        let (margin, _) =
            ModuleMarginCall::calculate_portfolio_margin(&account_id, &[], &[]).unwrap();
        let margin_without_discount =
            EqFixedU128::saturating_from_rational(250 + 100 - 100, 250 + 100);
        let margin_with_discount = EqFixedU128::saturating_from_rational(250 + 50 - 100, 250 + 50);
        assert_ne!(margin, margin_without_discount);
        assert_eq!(margin, margin_with_discount);
    });
}

// #[test]
// fn calculate_margin_coefficients_should_work_with_empty_changes() {
//     assert_eq!(
//         ModuleMarginCall::calculate_margin_coefficients(&[], &[]),
//         Ok((EqFixedU128::zero(), EqFixedU128::zero()))
//     );
// }

// #[test]
// fn calculate_margin_coefficients_without_order_changes() {
//     new_test_ext().execute_with(|| {
//         let asset_weights = vec![
//             (asset::ETH, EqFixedU128::from(12 * 2), EqFixedU128::from(12)),
//             (asset::BTC, EqFixedU128::from(1 * 100), EqFixedU128::from(1)),
//         ];

//         let _ = OracleMock::set_price_inner(0, asset::ETH, FixedI64::from(3));
//         let _ = OracleMock::set_price_inner(0, asset::BTC, FixedI64::from(99));

//         let maybe_coefficients =
//             ModuleMarginCall::calculate_margin_coefficients(&asset_weights, &[]);
//         assert_ok!(maybe_coefficients);
//         let (sum_amount_by_price_diff, sum_amount_by_price) = maybe_coefficients.unwrap();

//         let one_plus_max_fee = {
//             let eth_asset_data = AssetGetterMock::get_asset_data(&ETH).unwrap();
//             let btc_asset_data = AssetGetterMock::get_asset_data(&BTC).unwrap();
//             let max_fee = eth_asset_data.taker_fee.max(
//                 eth_asset_data
//                     .maker_fee
//                     .max(btc_asset_data.maker_fee.max(btc_asset_data.taker_fee)),
//             );

//             EqFixedU128::one() + max_fee
//         };

//         let expected_sum_amount_by_price_diff = EqFixedU128::from(12)
//             * abs_checked_sub(
//                 &(EqFixedU128::from(2) * one_plus_max_fee),
//                 &EqFixedU128::from(3),
//             )
//             .unwrap()
//             + EqFixedU128::from(1)
//                 * abs_checked_sub(
//                     &EqFixedU128::from(99),
//                     &(EqFixedU128::from(100) * one_plus_max_fee),
//                 )
//                 .unwrap();
//         let expected_sum_amount_by_price = 12 * 3 + 1 * 99;

//         assert_eq!(sum_amount_by_price_diff, expected_sum_amount_by_price_diff);
//         assert_eq!(
//             sum_amount_by_price,
//             EqFixedU128::from(expected_sum_amount_by_price)
//         );
//     });
// }

// #[test]
// fn calculate_margin_coefficients_without_weights() {
//     new_test_ext().execute_with(|| {
//         let order_changes = vec![
//             OrderChange {
//                 asset: &asset::ETH,
//                 amount: EqFixedU128::from(20),
//                 price: FixedI64::from(3),
//             },
//             OrderChange {
//                 asset: &asset::DOT,
//                 amount: EqFixedU128::from(1),
//                 price: FixedI64::from(1),
//             },
//         ];

//         let _ = OracleMock::set_price_inner(0, asset::ETH, FixedI64::from(5));
//         let _ = OracleMock::set_price_inner(0, asset::DOT, FixedI64::from(1));

//         let maybe_coefficients =
//             ModuleMarginCall::calculate_margin_coefficients(&[], &order_changes);
//         assert_ok!(maybe_coefficients);
//         let (sum_amount_by_price_diff, sum_amount_by_price) = maybe_coefficients.unwrap();

//         let one_plus_max_fee = {
//             let eth_asset_data = AssetGetterMock::get_asset_data(&ETH).unwrap();
//             let btc_asset_data = AssetGetterMock::get_asset_data(&BTC).unwrap();
//             let max_fee = eth_asset_data.taker_fee.max(
//                 eth_asset_data
//                     .maker_fee
//                     .max(btc_asset_data.maker_fee.max(btc_asset_data.taker_fee)),
//             );

//             EqFixedU128::one() + max_fee
//         };

//         let expected_sum_amount_by_price_diff = EqFixedU128::from(20)
//             * abs_checked_sub(
//                 &(EqFixedU128::from(3) * one_plus_max_fee),
//                 &EqFixedU128::from(5),
//             )
//             .unwrap()
//             + EqFixedU128::from(1)
//                 * abs_checked_sub(
//                     &(EqFixedU128::from(1) * one_plus_max_fee),
//                     &EqFixedU128::from(1),
//                 )
//                 .unwrap();

//         let expected_sum_amount_by_price = 20 * 5 + 1 * 1;

//         assert_eq!(sum_amount_by_price_diff, expected_sum_amount_by_price_diff);
//         assert_eq!(
//             sum_amount_by_price,
//             EqFixedU128::from(expected_sum_amount_by_price)
//         );
//     });
// }

#[test]
fn calculate_portfolio_margin_should_be_worse_after_new_order() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        let side = OrderSide::Sell;
        let order_changes_1 = vec![OrderChange {
            asset: asset::ETH,
            amount: EqFixedU128::from(20),
            price: FixedI64::from(3),
            side,
        }];

        let _ = OracleMock::set_price(account_id, asset::ETH, FixedI64::from(5));
        let _ = OracleMock::set_price(account_id, asset::DOT, FixedI64::from(1));

        //collateral = 100
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQD,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );

        let margin_1 =
            ModuleMarginCall::calculate_portfolio_margin(&account_id, &[], &order_changes_1)
                .unwrap();

        OrderAggregatesMock::set_order_aggregates(vec![(
            asset::ETH,
            OrderAggregateBySide::new(EqFixedU128::from(20), EqFixedU128::from(3), side).unwrap(),
        )]);

        let order_changes_2 = vec![OrderChange {
            asset: asset::ETH,
            amount: EqFixedU128::from(10),
            price: FixedI64::from(3),
            side: OrderSide::Sell,
        }];

        let margin_2 =
            ModuleMarginCall::calculate_portfolio_margin(&account_id, &[], &order_changes_2)
                .unwrap();

        assert!(margin_1 > margin_2);
        assert_eq!(false, margin_2.1);
    });
}

#[test]
fn t() {
    new_test_ext().execute_with(|| {
        let owner = 123;
        let btc_value = 200_000_000;
        ModuleBalances::make_free_balance_be(
            &owner,
            asset::BTC,
            SignedBalance::Positive(btc_value),
        );
        // ModuleBalances::make_free_balance_be(asset::EQ, &owner, SignedBalance::Positive(50_000_000_000));
        let _ = OracleMock::set_price(owner, asset::BTC, FixedI64::saturating_from_integer(50_000));
        // let _ = OracleMock::set_price_inner(owner, asset::EQ, FixedI64::saturating_from_integer(2));

        let asset = asset::BTC;
        let amount = EqFixedU128::saturating_from_integer(3);
        let price = FixedI64::saturating_from_integer(50_000);

        let res = ModuleMarginCall::check_margin_with_change(
            &owner,
            &[],
            &[OrderChange {
                asset,
                amount,
                price,
                side: OrderSide::Sell,
            }],
        );

        println!("result {:?}", res);

        let _ = OracleMock::set_price(owner, asset::BTC, FixedI64::saturating_from_integer(47_100));

        let res = ModuleMarginCall::check_margin_with_change(
            &owner,
            &[],
            &[OrderChange {
                asset,
                amount,
                price,
                side: OrderSide::Sell,
            }],
        );

        println!("result {:?}", res);
    });
}

#[test]
fn calculate_portfolio_margin_should_use_collateral_discount() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        assert_ok!(EqAssets::update_asset(
            RawOrigin::Root.into(),
            asset::BTC,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(Percent::from_rational(8u32, 10u32)),
            None
        ));

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::BTC,
            SignedBalance::<u128>::Positive(10 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQD,
            SignedBalance::<u128>::Positive(1000 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::ETH,
            SignedBalance::<u128>::Negative(200 * ONE_TOKEN),
        );

        let (margin, _) =
            ModuleMarginCall::calculate_portfolio_margin(&account_id, &[], &[]).unwrap();

        //Expected margin = 1 - debt/coll = 1 - 200(ETH) * 250 / 10(BTC) * 10_000 * 0.8 + 1_000(EQD) = 0.382716049
        assert_eq!(margin, EqFixedU128::from_float(0.382716049));
    });
}

#[test]
fn calc_margin_with_sell_orders() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        assert_ok!(EqAssets::update_asset(
            RawOrigin::Root.into(),
            asset::BTC,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(Percent::from_rational(8u32, 10u32)),
            None
        ));

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::BTC,
            SignedBalance::<u128>::Positive(20 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQD,
            SignedBalance::<u128>::Positive(1000 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::ETH,
            SignedBalance::<u128>::Negative(200 * ONE_TOKEN),
        );

        OrderAggregatesMock::set_order_aggregates(vec![
            (
                asset::ETH,
                OrderAggregateBySide::new(
                    EqFixedU128::from(20),
                    EqFixedU128::from(251),
                    OrderSide::Sell,
                )
                .unwrap(),
            ),
            (
                asset::DOT,
                OrderAggregateBySide::new(
                    EqFixedU128::from(500),
                    EqFixedU128::from(4),
                    OrderSide::Buy,
                )
                .unwrap(),
            ),
        ]);

        let order_change = OrderChange {
            asset: asset::ETH,
            amount: EqFixedU128::from(20),
            price: FixedI64::from(251),
            side: OrderSide::Sell,
        };

        let (margin, _) =
            ModuleMarginCall::calculate_portfolio_margin(&account_id, &[], &[order_change])
                .unwrap();

        /* Expected values:
        1) sell_margin
            collateral = 20(BTC) * 10_000 * 0.8 + max(0, -200(ETH debt) - 20(ETH sell order) - 20(ETH sell aggr)) * 250 * 0.8 + max(0, 1000 + 40*251) =
            = 160000 + 11040 = 171040
            debt = 0 * 250 + min(0, -200(ETH debt) - 20(ETH order) - 20(ETH aggr)) * 250 + min(0, 1000 + 40*251) =
            = -60000
            sell_margin = 1 - 60000/171040 = 0,649204864

        2) buy_margin
           collateral = 20(BTC) * 10_000 * 0.8 + max(0, -200(ETH debt)) * 250 * 0.8 + max(0, 1000) = 161000
           debt = 0 * 250 + min(0, -200(ETH debt)) * 250 + min(0, 1000 + 40*251) = -50000
           buy_margin = 1 - 50000/161000 = 0,689440994

        3) margin = min (sell_margin, buy_margin) = 0,649204864
         */

        assert_eq!(margin, EqFixedU128::from_float(0.649204864));
    });
}

#[test]
fn calc_margin_with_buy_sell_orders() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        assert_ok!(EqAssets::update_asset(
            RawOrigin::Root.into(),
            asset::BTC,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(Percent::from_rational(8u32, 10u32)),
            None
        ));

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::BTC,
            SignedBalance::<u128>::Positive(20 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQD,
            SignedBalance::<u128>::Positive(1000 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::ETH,
            SignedBalance::<u128>::Negative(200 * ONE_TOKEN),
        );

        let mut eth_order_aggregate = OrderAggregateBySide::new(
            EqFixedU128::from(20),
            EqFixedU128::from(251),
            OrderSide::Sell,
        )
        .unwrap();

        eth_order_aggregate
            .add(
                EqFixedU128::from(10),
                EqFixedU128::from(245),
                OrderSide::Buy,
            )
            .unwrap();

        let dot_order_aggregate = OrderAggregateBySide::new(
            EqFixedU128::from(550),
            EqFixedU128::from(10),
            OrderSide::Buy,
        )
        .unwrap();

        OrderAggregatesMock::set_order_aggregates(vec![
            (asset::ETH, eth_order_aggregate),
            (asset::DOT, dot_order_aggregate),
        ]);

        let order_change = OrderChange {
            asset: asset::ETH,
            amount: EqFixedU128::from(20),
            price: FixedI64::from(251),
            side: OrderSide::Sell,
        };

        let (margin, _) =
            ModuleMarginCall::calculate_portfolio_margin(&account_id, &[], &[order_change])
                .unwrap();

        /* Expected values:
        1) sell_margin
            collateral = 20(BTC) * 10_000 * 0.8 + max(0, -200(ETH debt) - 20(ETH sell aggr) - 20(ETH sell ord)) * 250 * 0.8 + max(0, 1000 + 40*251) =
            = 160000 + 11040 = 171040
            debt = 0 * 250 + min(0, -200(ETH debt) - 20(ETH aggr)-20(ETH sell ord)) * 250 + min(0, 1000 + 20*251) =
            = -60000
            sell_margin = 1 - 60000/171040 = 0,649204864

        2) buy_margin
           collateral = 20(BTC) * 10_000 * 0.8
                + max(0, -200(ETH debt) + 10(ETH buy aggr)) * 250 * 0.8
                + max(0, 0 + 550(DOT buy aggr)) * 4 * 0.8
                + max(0, 1000 - 10*245-550*10) = 161760
           debt = 0 * 250
                + min(0, -200(ETH debt) +10(ETH buy aggr)) * 250
                + min(0, 0 + 550(DOT buy aggr)) * 4
                + min(0, 1000 - 10*245 - 550*10) = -54450
           buy_margin = 1 - 54450/161760 = 0,663390208

        3) margin = min (sell_margin, buy_margin) = 0,663390208
         */

        assert_eq!(margin, EqFixedU128::from_float(0.649204864));
    });
}
