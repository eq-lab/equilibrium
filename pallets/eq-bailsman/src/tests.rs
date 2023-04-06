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
use core::slice::Iter;
use eq_primitives::{
    asset, balance::BalanceGetter, map, subaccount::SubAccType, Aggregates, SignedBalance,
};
use eq_utils::ONE_TOKEN;
use frame_support::{assert_err, assert_noop, assert_ok, traits::Hooks};
use sp_arithmetic::FixedI64;
use sp_runtime::FixedPointNumber;
use std::iter::FromIterator;
type AccountId = u64;
type TestPrice = <mock::Test as Config>::PriceGetter;

#[macro_export]
macro_rules! positive_balance_ok {
    ($who:expr, $currency:expr, $amount:expr) => {
        let value = $amount * BALANCE_ACCURACY;
        assert_eq!(
            ModuleBalances::get_balance($who, $currency),
            SignedBalance::Positive(value as u128),
            "{:?}",
            $currency
        );
    };
}

#[macro_export]
macro_rules! negative_balance_ok {
    ($who:expr, $currency:expr, $amount:expr) => {
        let value = $amount * BALANCE_ACCURACY;
        assert_eq!(
            ModuleBalances::get_balance($who, $currency),
            SignedBalance::Negative(value as u128),
            "{:?}",
            $currency
        );
    };
}

fn positive_currencies_iterator() -> Iter<'static, Asset> {
    static CURRENCIES: [Asset; 3] = [asset::BTC, asset::EOS, asset::DOT];
    CURRENCIES.iter()
}

fn negative_currencies_iterator() -> Iter<'static, Asset> {
    static CURRENCIES: [Asset; 3] = [asset::EQ, asset::ETH, asset::CRV];
    CURRENCIES.iter()
}

fn set_pos_balance_with_agg_unsafe(who: &AccountId, currency: &asset::Asset, amount: f64) {
    let value = amount * BALANCE_ACCURACY;
    let balance = SignedBalance::Positive(value as u128);
    ModuleBalances::make_free_balance_be(who, *currency, balance);
}

fn set_neg_balance_with_agg_unsafe(who: &AccountId, currency: &asset::Asset, amount: f64) {
    let value = amount * BALANCE_ACCURACY;
    let balance = SignedBalance::Negative(value as u128);
    ModuleBalances::make_free_balance_be(who, *currency, balance);
}

#[macro_export]
macro_rules! check_total_bailsman_issuance {
    ($currency:expr, $value:expr) => {
        let amount = $value * BALANCE_ACCURACY;
        let fixed64 = FixedI64::from_inner(amount as i64);
        let balance: <mock::Test as Config>::Balance =
            From::<u128>::from(fixed64.into_inner() as u128);

        assert_eq!(
            ModuleAggregates::get_total(UserGroup::Bailsmen, *$currency).collateral,
            balance
        );
    };
}

fn check_total_bailsman_debt(currency: asset::Asset, value: f64) {
    let amount = value * BALANCE_ACCURACY;
    let fixed64 = FixedI64::from_inner(amount as i64);
    let balance: <mock::Test as Config>::Balance = From::<u128>::from(fixed64.into_inner() as u128);

    assert_eq!(
        ModuleAggregates::get_total(UserGroup::Bailsmen, currency).debt,
        balance,
    );
}

fn check_total_borrower_debt(currency: asset::Asset, value: f64) {
    let amount = value * BALANCE_ACCURACY;
    let fixed64 = FixedI64::from_inner(amount as i64);
    let balance: <mock::Test as Config>::Balance = From::<u128>::from(fixed64.into_inner() as u128);

    assert_eq!(
        ModuleAggregates::get_total(UserGroup::Borrowers, currency).debt,
        balance
    );
}

fn check_total_borrower_collateral(currency: asset::Asset, value: f64) {
    let amount = value * BALANCE_ACCURACY;
    let fixed64 = FixedI64::from_inner(amount as i64);
    let balance: <mock::Test as Config>::Balance = From::<u128>::from(fixed64.into_inner() as u128);

    assert_eq!(
        ModuleAggregates::get_total(UserGroup::Borrowers, currency).collateral,
        balance
    );
}
fn update_collat_param(acc: &AccountId, currency: asset::Asset, value: f64) {
    let cur_balance = ModuleBalances::get_balance(&acc, &currency);
    let amount = cur_balance
        .add_balance(&((value * BALANCE_ACCURACY) as u128))
        .unwrap();
    ModuleBalances::make_free_balance_be(&acc, currency, amount);
}

fn update_debt_param(acc: &AccountId, currency: asset::Asset, value: f64) {
    let cur_balance = ModuleBalances::get_balance(&acc, &currency);
    let amount = cur_balance
        .sub_balance(&((value * BALANCE_ACCURACY) as u128))
        .unwrap();
    ModuleBalances::make_free_balance_be(&acc, currency, amount);
}

#[test]
fn is_enough_to_become_bailsman_true() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0)
        }

        let is_enough = ModuleBailsman::is_enough_to_become_bailsman(&account_id_1);

        assert_ok!(is_enough);

        let (is_enough, debt, collateral, min_collateral_value) = is_enough.unwrap();

        assert_eq!(is_enough, true);

        assert_eq!(debt, Balance::zero());

        assert_eq!((collateral > min_collateral_value), true);
    });
}

#[test]
fn is_enough_to_become_bailsman_not_enough_collat() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::EOS, 0.000001);

        let is_enough = ModuleBailsman::is_enough_to_become_bailsman(&account_id_1);

        assert_ok!(is_enough);

        let (is_enough, debt, collateral, min_collateral_value) = is_enough.unwrap();

        assert_eq!(is_enough, false);

        assert_eq!(debt, Balance::zero());

        assert_eq!((collateral <= min_collateral_value), true);
    });
}

#[test]
fn is_enough_to_become_bailsman_has_debt() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0)
        }

        set_neg_balance_with_agg_unsafe(&account_id_1, &asset::DOT, 100.0);

        let is_enough = ModuleBailsman::is_enough_to_become_bailsman(&account_id_1);

        assert_ok!(is_enough);

        let (is_enough, debt, collateral, min_collateral_value) = is_enough.unwrap();

        assert_eq!(is_enough, false);

        assert!(debt > Balance::zero());

        assert_eq!((collateral > min_collateral_value), true);
    });
}

#[test]
fn is_enough_to_become_bailsman_not_enough_and_has_debt() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::EOS, 0.000001);
        set_neg_balance_with_agg_unsafe(&account_id_1, &asset::DOT, 100.0);

        let is_enough = ModuleBailsman::is_enough_to_become_bailsman(&account_id_1);

        assert_ok!(is_enough);

        let (is_enough, debt, collateral, min_collateral_value) = is_enough.unwrap();

        assert_eq!(is_enough, false);

        assert!(debt > Balance::zero());

        assert_eq!((collateral <= min_collateral_value), true);
    });
}

#[test]
fn register_bailsman_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        let account_id_2 = 1;
        let bails_acc = ModuleBailsman::get_account_id();
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0);
            set_pos_balance_with_agg_unsafe(&account_id_2, &currency, 10.0);
        }

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );
        assert_eq!(LastDistribution::<Test>::get(account_id_1), Some(0));

        // generate distribution and register another bailsmen
        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(10_000));

        update_collat_param(&bails_acc, asset::BTC, 1.0);
        update_debt_param(&bails_acc, asset::EQD, 52500.0);

        ModuleBailsman::on_initialize(1);
        let (distr_id, queue) = DistributionQueue::<Test>::get();
        assert_eq!(distr_id, 1);
        assert_eq!(queue.len(), 1);

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_2));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Bailsmen),
            true
        );
        assert_eq!(LastDistribution::<Test>::get(account_id_2), Some(1));
    });
}

#[test]
fn register_bailsman_with_collat_less_than_min_error() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 0.0);
        }

        type TestPrice = <mock::Test as Config>::PriceGetter;
        for currency in iterator_with_usd() {
            TestPrice::set_price_mock(currency, &FixedI64::saturating_from_integer(1));
        }

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::EOS, 1.0);
        assert_err!(
            ModuleBailsman::register_bailsman(&account_id_1),
            Error::<Test>::CollateralMustBeMoreThanMin
        );

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::EOS, 0.0);
        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::BTC, 1.0);
        assert_err!(
            ModuleBailsman::register_bailsman(&account_id_1),
            Error::<Test>::CollateralMustBeMoreThanMin
        );

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::BTC, 0.0);
        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::ETH, 1.0);
        assert_err!(
            ModuleBailsman::register_bailsman(&account_id_1),
            Error::<Test>::CollateralMustBeMoreThanMin
        );

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::ETH, 0.0);
        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::EQ, 1.0);
        assert_err!(
            ModuleBailsman::register_bailsman(&account_id_1),
            Error::<Test>::CollateralMustBeMoreThanMin
        );

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::EQ, 0.0);
        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::EQD, 1.0);
        assert_err!(
            ModuleBailsman::register_bailsman(&account_id_1),
            Error::<Test>::CollateralMustBeMoreThanMin
        );
    });
}

#[test]
fn register_bailsman_twice_error() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0);
        }

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );

        assert_err!(
            ModuleBailsman::register_bailsman(&account_id_1),
            Error::<Test>::AlreadyBailsman
        );
    });
}

#[test]
fn unregister_bailsman_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0);
        }

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );

        assert_ok!(ModuleBailsman::unregister_bailsman(&account_id_1));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            false
        );

        assert!(!LastDistribution::<Test>::contains_key(&account_id_1));
    });
}

#[test]
fn unregister_not_an_bailsman_error() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;

        assert_err!(
            ModuleBailsman::unregister_bailsman(&account_id_1),
            Error::<Test>::NotBailsman
        );
    });
}

#[test]
fn register_bailsman_aggregates_total_collateral_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        let account_id_2 = 1;

        let mut balance_value_1 = 10.0;
        let mut balance_value_2 = 15.0;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, balance_value_1);
            balance_value_1 = balance_value_1 + 5.0;
            set_pos_balance_with_agg_unsafe(&account_id_2, &currency, balance_value_2);
            balance_value_2 = balance_value_2 + 5.0;
        }

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        assert_ok!(ModuleBailsman::register_bailsman(&account_id_2));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Bailsmen),
            true
        );

        let mut result_value = 25;
        for currency in iterator_with_usd() {
            check_total_bailsman_issuance!(currency, result_value as f64);
            result_value = result_value + 10;
        }
    });
}

#[test]
fn register_bailsman_aggregates_total_collateral_for_2_accs_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        let account_id_2 = 3;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0);
            set_pos_balance_with_agg_unsafe(&account_id_2, &currency, 10.0);
        }

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        assert_ok!(ModuleBailsman::register_bailsman(&account_id_2));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Bailsmen),
            true
        );

        for currency in iterator_with_usd() {
            check_total_bailsman_issuance!(currency, 20.0);
        }
    });
}

#[test]
fn unregister_bailsman_aggregates_total_collateral_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0);
        }

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );

        for currency in iterator_with_usd() {
            check_total_bailsman_issuance!(currency, 10.0);
        }

        assert_ok!(ModuleBailsman::unregister_bailsman(&account_id_1));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            false
        );

        for currency in iterator_with_usd() {
            check_total_bailsman_issuance!(currency, 0.0);
        }
    });
}

#[test]
fn unregister_bailsman_aggregates_total_collateral_for_2_accs_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        let account_id_2 = 3;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0);
            set_pos_balance_with_agg_unsafe(&account_id_2, &currency, 10.0);
        }

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        assert_ok!(ModuleBailsman::register_bailsman(&account_id_2));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Bailsmen),
            true
        );

        assert_ok!(ModuleBailsman::unregister_bailsman(&account_id_1));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            false
        );

        for currency in iterator_with_usd() {
            check_total_bailsman_issuance!(currency, 10.0);
        }
    });
}

#[test]
fn cannot_unreg_bailsman_with_debt() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0);
        }

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );

        positive_balance_ok!(&account_id_1, &asset::EOS, 10.0);

        for currency in iterator_with_usd() {
            if *currency == asset::EQ || *currency == asset::EQD {
                set_neg_balance_with_agg_unsafe(&account_id_1, currency, 10.0);
                negative_balance_ok!(&account_id_1, currency, 10.0);
            }
        }

        assert_noop!(
            ModuleBailsman::unregister_bailsman(&account_id_1),
            Error::<Test>::BailsmanHasDebt
        );

        assert!(ModuleAggregates::in_usergroup(
            &account_id_1,
            UserGroup::Bailsmen
        ));
    });
}

#[test]
fn reinit_negative_surplus() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        let bails_acc = ModuleBailsman::get_account_id();
        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 10.0);
        }

        check_total_bailsman_issuance!(&asset::EQD, 0.0);
        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        SubaccountsManagerMock::set_account_owner(100, SubAccType::Bailsman);

        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );
        check_total_bailsman_issuance!(&asset::EQD, 10.0);

        update_debt_param(&bails_acc, asset::EQD, 500.0);
        update_collat_param(&bails_acc, asset::EQ, 510.0);

        ModuleBailsman::on_initialize(1);

        assert_ok!(ModuleBailsman::redistribute(
            Origin::signed(account_id_1),
            account_id_1
        ));

        check_total_bailsman_issuance!(&asset::EQD, 0.0);

        check_total_bailsman_issuance!(&asset::EQ, 520.0);
        check_total_bailsman_debt(asset::EQD, 490.0);
        check_total_bailsman_debt(asset::EQ, 0.0);
        negative_balance_ok!(&account_id_1, &asset::EQD, 490.0);
        positive_balance_ok!(&account_id_1, &asset::EQ, 520.0);

        ModuleBailsman::on_finalize(1);
    });
}

#[test]
fn receive_position_borrower_pays_all_debts() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        let bails_acc = ModuleBailsman::get_account_id();

        TestPrice::set_price_mock(&asset::EOS, &FixedI64::saturating_from_integer(2));
        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(10));
        TestPrice::set_price_mock(&asset::ETH, &FixedI64::saturating_from_integer(7));
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::EQ, &FixedI64::saturating_from_integer(5));
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(4));
        TestPrice::set_price_mock(&asset::CRV, &FixedI64::saturating_from_integer(4));

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_1,
            UserGroup::Borrowers,
            true
        ));

        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Borrowers),
            true
        );

        let zero_balance = 0_f64;
        let value_balance = 20_f64;

        println!("Check balances before test start.");
        for currency in iterator_with_usd() {
            positive_balance_ok!(&account_id_1, currency, zero_balance);
            positive_balance_ok!(&bails_acc, currency, zero_balance);
            check_total_bailsman_issuance!(currency, zero_balance);
            check_total_bailsman_debt(*currency, zero_balance);
            check_total_borrower_debt(*currency, zero_balance);
            check_total_borrower_collateral(*currency, zero_balance);
        }

        for currency in positive_currencies_iterator() {
            ModuleBalances::make_free_balance_be(
                &account_id_1,
                *currency,
                SignedBalance::<Balance>::Positive(value_balance as u128 * ONE_TOKEN),
            );
        }
        for currency in negative_currencies_iterator() {
            ModuleBalances::make_free_balance_be(
                &account_id_1,
                *currency,
                SignedBalance::<Balance>::Negative(value_balance as u128 * ONE_TOKEN),
            );
        }

        println!("Check balances after balances set.");
        for currency in positive_currencies_iterator() {
            positive_balance_ok!(&bails_acc, currency, zero_balance);
            check_total_bailsman_issuance!(currency, zero_balance);
            check_total_bailsman_debt(*currency, zero_balance);
            check_total_borrower_debt(*currency, zero_balance);

            positive_balance_ok!(&account_id_1, currency, value_balance);
            check_total_borrower_collateral(*currency, value_balance);
        }
        for currency in negative_currencies_iterator() {
            positive_balance_ok!(&bails_acc, currency, zero_balance);
            check_total_bailsman_issuance!(currency, zero_balance);
            check_total_bailsman_debt(*currency, zero_balance);
            check_total_borrower_collateral(*currency, zero_balance);

            negative_balance_ok!(&account_id_1, currency, value_balance);
            check_total_borrower_debt(*currency, value_balance);
        }

        assert_ok!(ModuleBailsman::receive_position(
            &(account_id_1 as u64),
            false
        ));

        println!("Check balances after received position.");
        for currency in positive_currencies_iterator() {
            positive_balance_ok!(&account_id_1, currency, zero_balance);
            check_total_borrower_collateral(*currency, zero_balance);
            check_total_borrower_debt(*currency, zero_balance);
            check_total_bailsman_debt(*currency, zero_balance);

            positive_balance_ok!(&bails_acc, currency, value_balance);
            check_total_bailsman_issuance!(currency, value_balance);
        }

        for currency in negative_currencies_iterator() {
            positive_balance_ok!(&account_id_1, currency, zero_balance);
            check_total_borrower_collateral(*currency, zero_balance);
            check_total_borrower_debt(*currency, zero_balance);
            check_total_bailsman_issuance!(currency, zero_balance);

            negative_balance_ok!(&bails_acc, currency, value_balance);
            check_total_bailsman_debt(*currency, value_balance);
        }
    })
}

#[test]
fn receive_position_borrower_pays_all_debts_remains_collateral() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        let bails_acc = ModuleBailsman::get_account_id();
        type TestPrice = <mock::Test as Config>::PriceGetter;

        TestPrice::set_price_mock(&asset::EOS, &FixedI64::saturating_from_integer(3));
        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(10));
        TestPrice::set_price_mock(&asset::ETH, &FixedI64::saturating_from_integer(25));
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::EQ, &FixedI64::saturating_from_integer(2));
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(4));
        TestPrice::set_price_mock(&asset::CRV, &FixedI64::saturating_from_integer(5));

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_1,
            UserGroup::Borrowers,
            true
        ));

        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Borrowers),
            true
        );

        let zero_balance = 0_f64;
        let collat_balance = 20_f64;
        let debt_balance = 10_f64;

        println!("Check balances before test start.");
        for currency in iterator_with_usd() {
            positive_balance_ok!(&account_id_1, currency, zero_balance);
            positive_balance_ok!(&bails_acc, currency, zero_balance);
            check_total_bailsman_issuance!(currency, zero_balance);
            check_total_bailsman_debt(*currency, zero_balance);
            check_total_borrower_debt(*currency, zero_balance);
            check_total_borrower_collateral(*currency, zero_balance);
        }

        for currency in positive_currencies_iterator() {
            ModuleBalances::make_free_balance_be(
                &account_id_1,
                *currency,
                SignedBalance::<Balance>::Positive(collat_balance as u128 * ONE_TOKEN),
            );
        }
        for currency in negative_currencies_iterator() {
            ModuleBalances::make_free_balance_be(
                &account_id_1,
                *currency,
                SignedBalance::<Balance>::Negative(debt_balance as u128 * ONE_TOKEN),
            );
        }

        println!("Check balances after balances set.");
        for currency in positive_currencies_iterator() {
            positive_balance_ok!(&bails_acc, currency, zero_balance);
            check_total_bailsman_issuance!(currency, zero_balance);
            check_total_bailsman_debt(*currency, zero_balance);
            check_total_borrower_debt(*currency, zero_balance);

            positive_balance_ok!(&account_id_1, currency, collat_balance);
            check_total_borrower_collateral(*currency, collat_balance);
        }
        for currency in negative_currencies_iterator() {
            positive_balance_ok!(&bails_acc, currency, zero_balance);
            check_total_bailsman_issuance!(currency, zero_balance);
            check_total_bailsman_debt(*currency, zero_balance);
            check_total_borrower_collateral(*currency, zero_balance);

            negative_balance_ok!(&account_id_1, currency, debt_balance);
            check_total_borrower_debt(*currency, debt_balance);
        }

        positive_balance_ok!(&bails_acc, &asset::EQD, zero_balance);
        check_total_bailsman_issuance!(&asset::EQD, zero_balance);
        check_total_bailsman_debt(asset::EQD, zero_balance);

        positive_balance_ok!(&account_id_1, &asset::EQD, zero_balance);
        check_total_borrower_collateral(asset::EQD, zero_balance);
        check_total_borrower_debt(asset::EQD, zero_balance);

        // Transfers order: Btc, Crv, Eth, Eos, Dot, Eq
        assert_ok!(ModuleBailsman::receive_position(
            &(account_id_1 as u64),
            false
        ));

        println!("Check balances after received position.");
        for currency in positive_currencies_iterator() {
            check_total_borrower_debt(*currency, zero_balance);
            check_total_bailsman_debt(*currency, zero_balance);
            if currency.get_id() == asset::DOT.get_id() {
                positive_balance_ok!(&account_id_1, currency, 4.6);
                check_total_borrower_collateral(*currency, 4.6);

                positive_balance_ok!(&bails_acc, currency, 15.4);
                check_total_bailsman_issuance!(currency, 15.4);
                continue;
            }
            positive_balance_ok!(&account_id_1, currency, zero_balance);
            check_total_borrower_collateral(*currency, zero_balance);

            positive_balance_ok!(&bails_acc, currency, collat_balance);
            check_total_bailsman_issuance!(currency, collat_balance);
        }

        for currency in negative_currencies_iterator() {
            positive_balance_ok!(&account_id_1, currency, zero_balance);
            check_total_borrower_collateral(*currency, zero_balance);
            check_total_borrower_debt(*currency, zero_balance);
            check_total_bailsman_issuance!(currency, zero_balance);

            negative_balance_ok!(&bails_acc, currency, debt_balance);
            check_total_bailsman_debt(*currency, debt_balance);
        }

        positive_balance_ok!(&bails_acc, &asset::EQD, zero_balance);
        check_total_bailsman_issuance!(&asset::EQD, zero_balance);
        check_total_bailsman_debt(asset::EQD, zero_balance);

        positive_balance_ok!(&account_id_1, &asset::EQD, zero_balance);
        check_total_borrower_collateral(asset::EQD, zero_balance);
        check_total_borrower_debt(asset::EQD, zero_balance);
    })
}

#[test]
fn reinit_checks() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        let account_id_2 = 1;
        let distribution_acc = DISTRIBUTION_ACC.into_account_truncating();
        let bails_acc = ModuleBailsman::get_account_id();
        type TestPrice = <mock::Test as Config>::PriceGetter;

        // ---------------------------------------- Step 1 ----------------------------------------

        for currency in iterator_with_usd() {
            set_pos_balance_with_agg_unsafe(&account_id_1, &currency, 0.0);
            set_pos_balance_with_agg_unsafe(&account_id_2, &currency, 0.0);
            set_pos_balance_with_agg_unsafe(&bails_acc, &currency, 0.0);
        }

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::BTC, 10.0);
        positive_balance_ok!(&account_id_1, &asset::BTC, 10.0);
        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::EQD, 5000.0);
        positive_balance_ok!(&account_id_1, &asset::EQD, 5000.0);
        set_pos_balance_with_agg_unsafe(&account_id_2, &asset::BTC, 20.0);
        positive_balance_ok!(&account_id_2, &asset::BTC, 20.0);
        set_pos_balance_with_agg_unsafe(&account_id_2, &asset::ETH, 100.0);
        positive_balance_ok!(&account_id_2, &asset::ETH, 100.0);

        TestPrice::set_price_mock(&asset::EOS, &FixedI64::saturating_from_integer(3));
        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(10000));
        TestPrice::set_price_mock(&asset::ETH, &FixedI64::saturating_from_integer(250));
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::EQ, &FixedI64::saturating_from_integer(2));
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(4));

        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        assert_ok!(ModuleBailsman::register_bailsman(&account_id_2));

        SubaccountsManagerMock::set_account_owner(100, SubAccType::Bailsman);

        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Bailsmen),
            true
        );

        check_total_bailsman_issuance!(&asset::EOS, 0.0);
        check_total_bailsman_issuance!(&asset::BTC, 30.0);
        check_total_bailsman_issuance!(&asset::ETH, 100.0);
        check_total_bailsman_issuance!(&asset::EQD, 5000.0);
        check_total_bailsman_issuance!(&asset::EQ, 0.0);

        // ---------------------------------------- Step 2 ----------------------------------------
        update_collat_param(&bails_acc, asset::BTC, 1.0);
        update_collat_param(&bails_acc, asset::ETH, 50.0);
        update_collat_param(&bails_acc, asset::EOS, 10_000.0);
        update_collat_param(&bails_acc, asset::EQ, 1000.0);
        update_debt_param(&bails_acc, asset::EQD, 52500.0);

        ModuleBailsman::on_initialize(1);

        assert_ok!(ModuleBailsman::redistribute(
            Origin::signed(account_id_1),
            account_id_2
        ));
        assert_ok!(ModuleBailsman::redistribute(
            Origin::signed(account_id_1),
            account_id_1
        ));

        ModuleBailsman::on_finalize(1);

        println!(
            "{:?}",
            ModuleBalances::get_balance(&account_id_1, &asset::EQD)
        );

        positive_balance_ok!(&account_id_1, &asset::BTC, 10.318181819);
        negative_balance_ok!(&account_id_1, &asset::EQD, 11704.545454546);
        positive_balance_ok!(&account_id_1, &asset::EQ, 318.181818182);
        positive_balance_ok!(&account_id_1, &asset::EOS, 3181.818181819);
        positive_balance_ok!(&account_id_1, &asset::ETH, 15.909090910);

        positive_balance_ok!(&account_id_2, &asset::BTC, 20.681818181);
        positive_balance_ok!(&account_id_2, &asset::ETH, 134.090909090);
        positive_balance_ok!(&account_id_2, &asset::EOS, 6818.181818181);
        positive_balance_ok!(&account_id_2, &asset::EQ, 681.818181818);
        negative_balance_ok!(&account_id_2, &asset::EQD, 35795.454545454);

        positive_balance_ok!(&distribution_acc, &asset::BTC, 0.0);
        positive_balance_ok!(&distribution_acc, &asset::ETH, 0.0);
        positive_balance_ok!(&distribution_acc, &asset::EOS, 0.0);
        positive_balance_ok!(&distribution_acc, &asset::EQ, 0.0);
        positive_balance_ok!(&distribution_acc, &asset::EQD, 0.0);

        check_total_bailsman_issuance!(&asset::BTC, 31.0);
        check_total_bailsman_issuance!(&asset::ETH, 150.0);
        check_total_bailsman_issuance!(&asset::EOS, 10_000.0);
        check_total_bailsman_issuance!(&asset::EQ, 1000.0);
        check_total_bailsman_issuance!(&asset::EQD, 0.0);
        check_total_bailsman_debt(asset::EQD, 47500.0);

        // ---------------------------------------- Step 3 ----------------------------------------

        update_collat_param(&bails_acc, asset::BTC, 0.5);
        update_collat_param(&bails_acc, asset::ETH, 33.0);
        update_collat_param(&bails_acc, asset::EOS, 0.0);
        update_collat_param(&bails_acc, asset::EQ, 333.0);
        update_debt_param(&bails_acc, asset::EQD, 13250.0);

        check_total_bailsman_issuance!(&asset::BTC, 31.5);
        check_total_bailsman_issuance!(&asset::ETH, 183.0);
        check_total_bailsman_issuance!(&asset::EOS, 10_000.0);
        check_total_bailsman_issuance!(&asset::EQ, 1333.0);
        check_total_bailsman_issuance!(&asset::EQD, 0.0);
        check_total_bailsman_debt(asset::EQD, 13250.0 + 47500.0);

        ModuleBailsman::on_initialize(1);

        assert_ok!(ModuleBailsman::redistribute(
            Origin::signed(account_id_1),
            account_id_2
        ));
        assert_ok!(ModuleBailsman::redistribute(
            Origin::signed(account_id_1),
            account_id_1
        ));

        ModuleBailsman::on_finalize(1);

        positive_balance_ok!(&account_id_1, &asset::BTC, 10.477272729);
        positive_balance_ok!(&account_id_1, &asset::ETH, 26.409090911);
        positive_balance_ok!(&account_id_1, &asset::EOS, 3181.818181819);
        positive_balance_ok!(&account_id_1, &asset::EQ, 424.136363645);
        negative_balance_ok!(&account_id_1, &asset::EQD, 15920.454545791);

        positive_balance_ok!(&account_id_2, &asset::BTC, 21.022727271);
        positive_balance_ok!(&account_id_2, &asset::ETH, 156.590909089);
        positive_balance_ok!(&account_id_2, &asset::EOS, 6818.181818181);
        positive_balance_ok!(&account_id_2, &asset::EQ, 908.863636355);
        negative_balance_ok!(&account_id_2, &asset::EQD, 44829.545454209);

        check_total_bailsman_issuance!(&asset::BTC, 31.5);
        check_total_bailsman_issuance!(&asset::ETH, 183.0);
        check_total_bailsman_issuance!(&asset::EOS, 10000.0);
        check_total_bailsman_issuance!(&asset::EQD, 0.0);
        check_total_bailsman_issuance!(&asset::EQ, 1333.0);
        check_total_bailsman_debt(asset::EQD, 60750.0);

        positive_balance_ok!(&distribution_acc, &asset::BTC, 0.0);
        positive_balance_ok!(&distribution_acc, &asset::ETH, 0.0);
        positive_balance_ok!(&distribution_acc, &asset::EOS, 0.0);
        positive_balance_ok!(&distribution_acc, &asset::EQ, 0.0);
        positive_balance_ok!(&distribution_acc, &asset::EQD, 0.0);
    });
}

#[test]
fn should_unreg_bailsman() {
    new_test_ext().execute_with(|| {
        let account_id = 0;
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQD, 11.0); // MinimalCollateral is 5$

        assert_ok!(ModuleBailsman::register_bailsman(&account_id));
        set_neg_balance_with_agg_unsafe(&account_id, &asset::CRV, 1.0); // CRV price 5$
        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![(asset::EQD, SignedBalance::Negative(ONE_TOKEN))],
            None
        ));

        assert!(
            !ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![(asset::EQD, SignedBalance::Negative(ONE_TOKEN))],
                None
            )
            .unwrap(),
            "Should not be unregged"
        );

        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![(asset::EQD, SignedBalance::Negative(ONE_TOKEN * 6))],
            None
        ));

        assert!(
            ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![(asset::EQD, SignedBalance::Negative(ONE_TOKEN * 6))],
                None
            )
            .unwrap(),
            "Should be unregged"
        );

        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![(asset::EQD, SignedBalance::Negative(ONE_TOKEN * 7))],
            None
        ));

        assert!(
            ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![(asset::EQD, SignedBalance::Negative(ONE_TOKEN * 7))],
                None
            )
            .unwrap(),
            "Should be unregged"
        );
    });
}
#[test]
fn can_change_true() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        let eq_price = FixedI64::saturating_from_integer(5);

        type TestPrice = <mock::Test as Config>::PriceGetter;
        TestPrice::set_price_mock(&asset::EOS, &FixedI64::saturating_from_integer(2));
        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(10000));
        TestPrice::set_price_mock(&asset::ETH, &FixedI64::saturating_from_integer(7));
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::EQ, &eq_price);
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(4));
        TestPrice::set_price_mock(&asset::CRV, &FixedI64::saturating_from_integer(4));

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::BTC, 1.0);
        set_neg_balance_with_agg_unsafe(&account_id_1, &asset::EQD, 9522.0);

        assert_ok!(ModuleBailsman::can_change_balance(
            &account_id_1,
            &vec![(asset::EQD, SignedBalance::Negative(1 * ONE_TOKEN))],
            None,
        ),);
    });
}

#[test]
fn should_unreg_bailsman_multiple_assets() {
    new_test_ext().execute_with(|| {
        let account_id = 0;
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQD, 11.0); // MinimalCollateral is 5$

        assert_ok!(ModuleBailsman::register_bailsman(&account_id));
        set_neg_balance_with_agg_unsafe(&account_id, &asset::CRV, 1.0); // CRV price 5$

        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![
                (asset::EQD, SignedBalance::Negative(ONE_TOKEN)),
                (asset::EQ, SignedBalance::Negative(ONE_TOKEN))
            ],
            None
        ));

        assert!(
            ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![
                    (asset::EQD, SignedBalance::Negative(ONE_TOKEN)),
                    (asset::EQ, SignedBalance::Negative(ONE_TOKEN))
                ],
                None
            )
            .unwrap(),
            "Should be unregged"
        );

        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![
                (asset::EQD, SignedBalance::Negative(ONE_TOKEN * 3)),
                (asset::EQ, SignedBalance::Negative(ONE_TOKEN * 3))
            ],
            None
        ));

        assert!(
            ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![
                    (asset::EQD, SignedBalance::Negative(ONE_TOKEN * 3)),
                    (asset::EQ, SignedBalance::Negative(ONE_TOKEN * 3))
                ],
                None
            )
            .unwrap(),
            "Should be unregged"
        );

        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![
                (asset::EQD, SignedBalance::Negative(ONE_TOKEN * 3)),
                (asset::EQ, SignedBalance::Negative(ONE_TOKEN * 4))
            ],
            None
        ));

        assert!(
            ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![
                    (asset::EQD, SignedBalance::Negative(ONE_TOKEN * 3)),
                    (asset::EQ, SignedBalance::Negative(ONE_TOKEN * 4))
                ],
                None
            )
            .unwrap(),
            "Should be unregged"
        );
    });
}

#[test]
fn should_unreg_bailsman_non_one_discount_assets() {
    new_test_ext().execute_with(|| {
        let account_id = 100;
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQD, 9.0); // MinimalCollateral is 5$

        assert_ok!(ModuleBailsman::register_bailsman(&account_id));
        set_neg_balance_with_agg_unsafe(&account_id, &asset::CRV, 1.0); // CRV price 5$

        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![(asset::ETH, SignedBalance::Negative(0))],
            None
        ));

        assert!(
            ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![(asset::ETH, SignedBalance::Negative(0))],
                None
            )
            .unwrap(),
            "Should be unregged"
        );

        // zero discount, should have no impact
        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![(
                asset::USDC,
                SignedBalance::Positive(1_000_000_000 * ONE_TOKEN)
            )],
            None
        ));

        assert!(
            ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![
                    (
                        asset::USDC,
                        SignedBalance::Positive(1_000_000_000 * ONE_TOKEN)
                    ),
                    (asset::ETH, SignedBalance::Negative(0))
                ],
                None
            )
            .unwrap(),
            "Should be unregged"
        );

        // 0.5 discount, now we should have slightly less than minimal collatetal
        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![
                (asset::USDT, SignedBalance::Positive(2 * ONE_TOKEN - 1)),
                (asset::ETH, SignedBalance::Negative(0))
            ],
            None
        ));

        assert!(
            ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![
                    (asset::USDT, SignedBalance::Positive(2 * ONE_TOKEN - 1)),
                    (asset::ETH, SignedBalance::Negative(0))
                ],
                None
            )
            .unwrap(),
            "Should be unregged"
        );

        // Now reaching mininal collateral
        assert_ok!(ModuleBailsman::should_unreg_bailsman(
            &account_id,
            &vec![
                (asset::USDT, SignedBalance::Positive(2 * ONE_TOKEN)),
                (asset::ETH, SignedBalance::Negative(0))
            ],
            None
        ));

        assert!(
            !ModuleBailsman::should_unreg_bailsman(
                &account_id,
                &vec![
                    (asset::USDT, SignedBalance::Positive(2 * ONE_TOKEN)),
                    (asset::ETH, SignedBalance::Negative(0))
                ],
                None
            )
            .unwrap(),
            "Shouldn't be unregged"
        );
    });
}

#[test]
fn can_change_balance_when_margin_state_is_not_good() {
    new_test_ext().execute_with(|| {
        let eq_price = FixedI64::saturating_from_integer(5);

        type TestPrice = <mock::Test as Config>::PriceGetter;
        TestPrice::set_price_mock(&asset::EOS, &FixedI64::saturating_from_integer(2));
        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(10000));
        TestPrice::set_price_mock(&asset::ETH, &FixedI64::saturating_from_integer(7));
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::EQ, &eq_price);
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(4));
        TestPrice::set_price_mock(&asset::CRV, &FixedI64::saturating_from_integer(4));

        set_pos_balance_with_agg_unsafe(&ACCOUNT_ID_BAD_SUB_GOOD, &asset::BTC, 1.0);
        set_neg_balance_with_agg_unsafe(&ACCOUNT_ID_BAD_SUB_GOOD, &asset::EQD, 9522.0);

        assert_err!(
            ModuleBailsman::can_change_balance(
                &ACCOUNT_ID_BAD_SUB_GOOD,
                &vec![(asset::EQD, SignedBalance::Negative(2 * ONE_TOKEN))],
                None,
            ),
            Error::<Test>::WrongMargin
        );
    });
}

#[test]
fn can_change_on_positive_change() {
    new_test_ext().execute_with(|| {
        let eq_price = FixedI64::saturating_from_integer(5);

        type TestPrice = <mock::Test as Config>::PriceGetter;
        TestPrice::set_price_mock(&asset::EOS, &FixedI64::saturating_from_integer(2));
        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(10000));
        TestPrice::set_price_mock(&asset::ETH, &FixedI64::saturating_from_integer(7));
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::EQ, &eq_price);
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(4));
        TestPrice::set_price_mock(&asset::CRV, &FixedI64::saturating_from_integer(4));

        set_pos_balance_with_agg_unsafe(&ACCOUNT_ID_BAD_SUB_GOOD, &asset::BTC, 1.0);
        set_neg_balance_with_agg_unsafe(&ACCOUNT_ID_BAD_SUB_GOOD, &asset::EQD, 9522.0);

        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(9000));

        assert_ok!(ModuleBailsman::can_change_balance(
            &ACCOUNT_ID_BAD_SUB_GOOD,
            &vec![(asset::EQD, SignedBalance::Positive(ONE_TOKEN))],
            None,
        ),);
    });
}

#[test]
fn cant_change_if_margin_decreased() {
    new_test_ext().execute_with(|| {
        let eq_price = FixedI64::saturating_from_integer(5);

        type TestPrice = <mock::Test as Config>::PriceGetter;
        TestPrice::set_price_mock(&asset::EOS, &FixedI64::saturating_from_integer(2));
        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(10000));
        TestPrice::set_price_mock(&asset::ETH, &FixedI64::saturating_from_integer(7));
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::EQ, &eq_price);
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(4));
        TestPrice::set_price_mock(&asset::CRV, &FixedI64::saturating_from_integer(4));

        set_pos_balance_with_agg_unsafe(&ACCOUNT_ID_BAD_SUB_GOOD, &asset::BTC, 1.0);
        set_neg_balance_with_agg_unsafe(&ACCOUNT_ID_BAD_SUB_GOOD, &asset::EQD, 9522.0);

        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(9000));

        assert_err!(
            ModuleBailsman::can_change_balance(
                &ACCOUNT_ID_BAD_SUB_GOOD,
                &vec![
                    (asset::EQD, SignedBalance::Positive(ONE_TOKEN * 10)),
                    (asset::DOT, SignedBalance::Negative(ONE_TOKEN * 2))
                ],
                None,
            ),
            crate::Error::<Test>::WrongMargin
        );
    });
}

#[test]
fn can_change_if_margin_increased() {
    new_test_ext().execute_with(|| {
        let eq_price = FixedI64::saturating_from_integer(5);

        type TestPrice = <mock::Test as Config>::PriceGetter;
        TestPrice::set_price_mock(&asset::EOS, &FixedI64::saturating_from_integer(2));
        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(10000));
        TestPrice::set_price_mock(&asset::ETH, &FixedI64::saturating_from_integer(7));
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::EQ, &eq_price);
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(4));
        TestPrice::set_price_mock(&asset::CRV, &FixedI64::saturating_from_integer(4));

        set_pos_balance_with_agg_unsafe(&ACCOUNT_ID_SUB_GOOD, &asset::BTC, 1.0);
        set_neg_balance_with_agg_unsafe(&ACCOUNT_ID_SUB_GOOD, &asset::EQD, 9522.0);

        TestPrice::set_price_mock(&asset::BTC, &FixedI64::saturating_from_integer(9000));

        assert_ok!(ModuleBailsman::can_change_balance(
            &ACCOUNT_ID_SUB_GOOD,
            &vec![
                (asset::EQD, SignedBalance::Positive(ONE_TOKEN * 10)),
                (asset::DOT, SignedBalance::Negative(ONE_TOKEN * 1)),
            ],
            None,
        ));
    });
}

#[test]
fn can_change_bailsman_has_debt() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;

        type TestPrice = <mock::Test as Config>::PriceGetter;
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        println!("1");
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(6));
        println!("1");

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::DOT, 1.0);
        println!("1");
        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        println!("1");
        set_neg_balance_with_agg_unsafe(&account_id_1, &asset::EQD, 1.0);
        println!("1");
        assert_err!(
            ModuleBailsman::can_change_balance(
                &account_id_1,
                &vec![(asset::DOT, SignedBalance::Negative(500_000_000))],
                None,
            ),
            Error::<Test>::BailsmanHasDebt
        );
    });
}

#[test]
fn can_change_bailsman_has_debt_increasing_collat_when_shoud_unreg() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;

        type TestPrice = <mock::Test as Config>::PriceGetter;
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(6));

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::DOT, 1.0);
        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        set_neg_balance_with_agg_unsafe(&account_id_1, &asset::EQD, 1.0);
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(3));
        assert_ok!(ModuleBailsman::can_change_balance(
            &account_id_1,
            &vec![(asset::DOT, SignedBalance::Positive(500_000_000))],
            None
        ));
    });
}

#[test]
fn can_change_bailsman_has_debt_decreasing_collat_when_shoud_unreg() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;

        type TestPrice = <mock::Test as Config>::PriceGetter;
        TestPrice::set_price_mock(&asset::EQD, &FixedI64::saturating_from_integer(1));
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(6));

        set_pos_balance_with_agg_unsafe(&account_id_1, &asset::DOT, 1.0);
        assert_ok!(ModuleBailsman::register_bailsman(&account_id_1));
        set_neg_balance_with_agg_unsafe(&account_id_1, &asset::EQD, 1.0);
        TestPrice::set_price_mock(&asset::DOT, &FixedI64::saturating_from_integer(3));
        assert_err!(
            ModuleBailsman::can_change_balance(
                &account_id_1,
                &vec![(asset::DOT, SignedBalance::Negative(100_000_000))],
                None,
            ),
            Error::<Test>::BailsmanHasDebt
        );
    });
}

pub fn iterator_with_usd() -> Iter<'static, Asset> {
    static CURRENCIES: [Asset; 7] = [
        asset::EQ,
        asset::ETH,
        asset::BTC,
        asset::EQD,
        asset::EOS,
        asset::DOT,
        asset::CRV,
    ];
    CURRENCIES.iter()
}

#[test]
fn can_change_balance_to_module_account_should_work() {
    new_test_ext().execute_with(|| {
        let account_id = ModuleBailsman::get_account_id();

        assert_ok!(ModuleBailsman::can_change_balance(
            &account_id,
            &Vec::new(),
            None
        ));
    });
}

#[test]
fn can_change_balance_to_distribution_account_should_fail() {
    new_test_ext().execute_with(|| {
        let distr_account_id = DISTRIBUTION_ACC.into_account_truncating();

        assert_err!(
            ModuleBailsman::can_change_balance(&distr_account_id, &Vec::new(), None),
            Error::<Test>::TempBalancesTransfer
        );
    });
}

#[test]
fn can_change_balance_when_temp_balance_more_than_min_temp_baance_should_fail() {
    new_test_ext().execute_with(|| {
        let account_id = 1;
        let module_account_id = ModuleBailsman::get_account_id();

        ModuleBalances::make_free_balance_be(
            &module_account_id,
            asset::EQD,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id,
            UserGroup::Bailsmen,
            true
        ));

        assert_err!(
            ModuleBailsman::can_change_balance(&account_id, &Vec::new(), None),
            Error::<Test>::TempBalancesNotDistributed
        );
    });
}

#[test]
fn can_change_balance_when_bailsman_has_debt_should_fail() {
    new_test_ext().execute_with(|| {
        let account_id = 100;

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQ,
            SignedBalance::<Balance>::Negative(100 * ONE_TOKEN),
        );

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id,
            UserGroup::Bailsmen,
            true
        ));

        assert_err!(
            ModuleBailsman::can_change_balance(
                &account_id,
                &vec![(asset::EQD, SignedBalance::Negative(200))],
                None,
            ),
            Error::<Test>::BailsmanHasDebt
        );
    });
}

#[test]
fn add_queue_on_initialize() {
    new_test_ext().execute_with(|| {
        let temp_balances = BailsmanModuleId::get().into_account_truncating();
        let distribution_balances = DISTRIBUTION_ACC.into_account_truncating();
        let positive_balance = 2 * ONE_TOKEN;
        let negative_balance = ONE_TOKEN;
        let bailsman_acc = 333;
        let bailsman_collat = 10000 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &temp_balances,
            asset::BTC,
            SignedBalance::Positive(positive_balance),
        );
        ModuleBalances::make_free_balance_be(
            &temp_balances,
            asset::ETH,
            SignedBalance::Negative(negative_balance),
        );

        ModuleBailsman::on_initialize(0);

        assert_eq!((0, map![]), ModuleBailsman::distribution_queue());

        ModuleBalances::make_free_balance_be(
            &bailsman_acc,
            asset::EQD,
            SignedBalance::Positive(bailsman_collat),
        );

        assert_ok!(ModuleBailsman::register_bailsman(&bailsman_acc));
        assert_eq!((0, map![]), ModuleBailsman::distribution_queue());

        assert_eq!((0, VecMap::new()), ModuleBailsman::distribution_queue());
        assert_eq!(
            ModuleBalances::get_balance(&temp_balances, &asset::BTC),
            SignedBalance::Positive(positive_balance)
        );
        assert_eq!(
            ModuleBalances::get_balance(&temp_balances, &asset::ETH),
            SignedBalance::Negative(negative_balance)
        );
        assert_eq!(
            ModuleBalances::get_balance(&distribution_balances, &asset::BTC),
            SignedBalance::Positive(Balance::zero())
        );
        assert_eq!(
            ModuleBalances::get_balance(&distribution_balances, &asset::ETH),
            SignedBalance::Positive(Balance::zero())
        );

        ModuleBailsman::on_initialize(0);

        assert_eq!(
            (
                1,
                map![
                    1 => Distribution {
                        total_usd: bailsman_collat,
                        remaining_bailsmen: 1,
                        distribution_balances: map![
                            asset::BTC => SignedBalance::Positive(positive_balance),
                            asset::ETH => SignedBalance::Negative(negative_balance)
                        ].into(),
                        prices: map![
                            asset::BTC => OracleMock::get_price(&asset::BTC).unwrap(),
                            asset::ETH => OracleMock::get_price(&asset::ETH).unwrap()
                        ].into()
                    }
                ]
            ),
            ModuleBailsman::distribution_queue()
        );
        assert_eq!(ModuleBailsman::get_current_distribution_id(), 1,);

        assert_eq!(
            ModuleBalances::get_balance(&temp_balances, &asset::BTC),
            SignedBalance::Positive(Balance::zero())
        );
        assert_eq!(
            ModuleBalances::get_balance(&temp_balances, &asset::ETH),
            SignedBalance::Positive(Balance::zero())
        );
        assert_eq!(
            ModuleBalances::get_balance(&distribution_balances, &asset::BTC),
            SignedBalance::Positive(positive_balance)
        );
        assert_eq!(
            ModuleBalances::get_balance(&distribution_balances, &asset::ETH),
            SignedBalance::Negative(negative_balance)
        );
    });
}

#[test]
fn clear_queue_on_finalize() {
    new_test_ext().execute_with(|| {
        let positive_balance = 2.0;
        let negative_balance = 1.0;

        DistributionQueue::<Test>::put(
            (2, map![
                1 => Distribution {
                    total_usd: 0,
                    remaining_bailsmen: 0,
                    distribution_balances: map![
                        asset::BTC => SignedBalance::Positive(positive_balance as u128 * ONE_TOKEN),
                        asset::ETH => SignedBalance::Negative(negative_balance as u128 * ONE_TOKEN),
                    ].into(),
                    prices: map![
                        asset::BTC => OracleMock::get_price(&asset::BTC).unwrap(),
                        asset::ETH => OracleMock::get_price(&asset::ETH).unwrap()
                    ].into()
                },
                2 => Distribution {
                    total_usd: 0,
                    remaining_bailsmen: 1,
                    distribution_balances: map![
                        asset::BTC => SignedBalance::Negative(negative_balance as u128 * ONE_TOKEN),
                        asset::ETH => SignedBalance::Positive(positive_balance as u128 * ONE_TOKEN),
                    ].into(),
                    prices: map![
                        asset::BTC => OracleMock::get_price(&asset::BTC).unwrap(),
                        asset::ETH => OracleMock::get_price(&asset::ETH).unwrap()
                    ].into()
                }
            ])
        );

        ModuleBailsman::on_finalize(1);

        assert_eq!(
            ModuleBailsman::distribution_queue(),
            (2, map![
                2 => Distribution {
                    total_usd: 0,
                    remaining_bailsmen: 1,
                    distribution_balances: map![
                        asset::BTC => SignedBalance::Negative(negative_balance as u128 * ONE_TOKEN),
                        asset::ETH => SignedBalance::Positive(positive_balance as u128 * ONE_TOKEN),
                    ].into(),
                    prices: map![
                        asset::BTC => OracleMock::get_price(&asset::BTC).unwrap(),
                        asset::ETH => OracleMock::get_price(&asset::ETH).unwrap()
                    ].into()
                }
            ])
        );

        DistributionQueue::<Test>::mutate(|(_, queue)| {
            queue.iter_mut().for_each(|(_, distr)| {
                distr.remaining_bailsmen = 0;
            })
        });

        ModuleBailsman::on_finalize(2);

        assert_eq!(
            ModuleBailsman::distribution_queue(),
            (2, map![])
        );
    });
}

#[test]
fn apply_distribution_works() {
    let prices = map![
        asset::BTC => EqFixedU128::from(1),
        asset::ETH => EqFixedU128::from(1),
    ];
    let distr = Distribution {
        prices: prices.into(),
        distribution_balances: VecMap::from_iter(&vec![
            (asset::BTC, SignedBalance::Positive(200)),
            (asset::ETH, SignedBalance::Positive(300)),
            (asset::EQD, SignedBalance::Negative(100)),
        ])
        .into(),
        total_usd: 1000,
        remaining_bailsmen: 1,
    };

    let mut before_distr_balances = map![asset::EQD => SignedBalance::Positive(50)];
    let mut accumulator = VecMap::new();

    ModuleBailsman::apply_distribution(&mut before_distr_balances, &distr, &mut accumulator)
        .unwrap();

    assert_eq!(accumulator[&asset::BTC], SignedBalance::Positive(10));
    assert_eq!(accumulator[&asset::ETH], SignedBalance::Positive(15));
    assert_eq!(accumulator[&asset::EQD], SignedBalance::Negative(5));

    assert_eq!(
        before_distr_balances[&asset::BTC],
        SignedBalance::Positive(10)
    );
    assert_eq!(
        before_distr_balances[&asset::ETH],
        SignedBalance::Positive(15)
    );
    assert_eq!(
        before_distr_balances[&asset::EQD],
        SignedBalance::Positive(45)
    );
}

#[test]
fn get_rest_from_distribution_account() {
    new_test_ext().execute_with(|| {
        let distr_acc = &DISTRIBUTION_ACC.into_account_truncating();

        ModuleBalances::make_free_balance_be(distr_acc, asset::BTC, SignedBalance::Negative(101));
        ModuleBalances::make_free_balance_be(distr_acc, asset::ETH, SignedBalance::Positive(102));
        ModuleBalances::make_free_balance_be(distr_acc, asset::EQD, SignedBalance::Negative(103));
        ModuleBalances::make_free_balance_be(distr_acc, asset::EQ, SignedBalance::Positive(103));

        let mut transfers = VecMap::new();
        transfers.insert(asset::BTC, SignedBalance::Negative(100));
        transfers.insert(asset::ETH, SignedBalance::Positive(100));
        transfers.insert(asset::EQD, SignedBalance::Negative(100));

        ModuleBailsman::get_rest_from_distribution_account(&mut transfers);

        assert_eq!(transfers[&asset::BTC], SignedBalance::Negative(101));
        assert_eq!(transfers[&asset::ETH], SignedBalance::Positive(102));
        assert_eq!(transfers[&asset::EQD], SignedBalance::Negative(103));
    });
}
