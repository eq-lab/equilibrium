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

use eq_primitives::balance::EqCurrency;
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::{asset, PriceGetter, TotalAggregates};
use eq_utils::{
    fixed::{balance_from_eq_fixedu128, eq_fixedu128_from_balance},
    ONE_TOKEN,
};
use frame_support::{assert_err, assert_noop, assert_ok};
use sp_arithmetic::Permill;
use sp_runtime::FixedPointNumber;

use super::*;
use crate::mock::*;

fn get_debt_weight(asset: Asset) -> EqFixedU128 {
    EqAssets::get_asset_data(&asset)
        .expect("Asset exist")
        .debt_weight
        .into()
}

fn get_lenging_debt_weight(asset: Asset) -> Permill {
    EqAssets::get_asset_data(&asset)
        .expect("Asset exist")
        .lending_debt_weight
}

fn free_to_borrow(asset: Asset) -> Balance {
    let (lenders_lendable, bails_lendable): (Balance, Balance) =
        EqLending::get_lendable_parts(asset);

    let debt: Balance = EqLending::get_total_debt(asset);
    let free_to_borrow = get_lenging_debt_weight(asset) * lenders_lendable
        + get_debt_weight(asset).saturating_mul_int(bails_lendable)
        - debt;
    free_to_borrow
}

#[test]
fn transfer_usd_ok() {
    new_test_ext().execute_with(|| {
        let account_id_from = 21;
        let account_id_to = 22;

        assert_ok!(EqBalances::deposit_creating(
            &account_id_from,
            asset::EQD,
            50,
            true,
            None
        ));

        assert_ok!(EqBalances::deposit_creating(
            &account_id_to,
            asset::EQD,
            50,
            true,
            None
        ));

        // no debt
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::EQD,
            account_id_to,
            10
        ));
        // with debt
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::EQD,
            account_id_to,
            50
        ));
    });
}

#[test]
fn transfer_pos_prev_more_or_eq_than_change_ok() {
    new_test_ext().execute_with(|| {
        let account_id_from = 21;
        let account_id_to = 22;

        let prev = 2;
        let change = prev / 2;

        assert!(prev > change);

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_from,
            asset::BTC,
            prev,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_to,
            asset::EQD,
            50,
            true,
            None
        ));

        // prev > change
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_to,
            change
        ));

        let prev = change;
        assert_eq!(prev, change);
        // prev == change
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_to,
            change
        ));
    });
}

#[test]
fn transfer_with_debt_ok() {
    new_test_ext().execute_with(|| {
        let account_id_from = 21;
        let account_id_to = 22;
        let account_id_bails = 23;

        let bails_collateral_btc = 50_000_000_000;

        // Step 0: Preparations

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_to,
            asset::BTC,
            1,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_bails,
            asset::BTC,
            bails_collateral_btc,
            true,
            None
        ));

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_bails,
            UserGroup::Bailsmen,
            true
        ));

        println!(
            "ModuleAggregates total {:?}",
            ModuleBalances::get_balance(&account_id_bails, &asset::BTC)
        );

        assert_eq!(
            ModuleAggregates::get_total(UserGroup::Bailsmen, asset::BTC).collateral,
            bails_collateral_btc
        );

        assert_ne!(
            get_debt_weight(asset::BTC), // 0.2 in mock
            EqFixedU128::zero(),
            "asset::BTC debt weight must not be zero for test"
        );

        // max_btc_debt is 10_000_000_000 for weight=0.2 and bails_collateral_btc=50_000_000_000
        let max_btc_debt = EqFixedU128::saturating_from_integer(bails_collateral_btc)
            * get_debt_weight(asset::BTC);

        let max_btc_debt = max_btc_debt.into_inner() / EqFixedU128::accuracy();

        let prev = 2;
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_from,
            asset::BTC,
            prev,
            true,
            None
        ));

        // Step 1: debt < max_btc_debt

        let change = prev * 2;
        let debt = change - prev; // 2
        assert!(change > prev && debt < max_btc_debt);
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_to,
            change
        ));

        // Step 2: new_debt < max_btc_debt

        let new_debt = max_btc_debt - debt - 1; // 7
        let change = new_debt - debt;
        assert!(debt + change < max_btc_debt);

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_to,
            change
        ));

        // Step 3: transfer from bailsman - decrease bails collateral

        let new_max_btc_debt = max_btc_debt - 1;

        let transfer_from_bails = eq_fixedu128_from_balance(bails_collateral_btc)
            - eq_fixedu128_from_balance(new_max_btc_debt) / get_debt_weight(asset::BTC);

        let transfer_from_bails: Balance = balance_from_eq_fixedu128(transfer_from_bails).unwrap();

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_bails),
            asset::BTC,
            account_id_to,
            transfer_from_bails
        ));

        let new_bails_collat_btc = bails_collateral_btc - transfer_from_bails;

        assert_eq!(
            ModuleAggregates::get_total(UserGroup::Bailsmen, asset::BTC).collateral,
            new_bails_collat_btc
        );

        // recalc max_btc_debt
        let calc_max_btc_debt = new_max_btc_debt;

        let new_max_btc_debt = EqFixedU128::saturating_from_integer(new_bails_collat_btc)
            * get_debt_weight(asset::BTC);

        let new_max_btc_debt = new_max_btc_debt.into_inner() / EqFixedU128::accuracy();

        assert_eq!(calc_max_btc_debt, new_max_btc_debt);

        // Step 4: new_debt == max_btc_debt

        let max_correct_debt = new_max_btc_debt;
        let change = max_correct_debt - new_debt;
        assert!(new_debt + change == new_max_btc_debt);
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_to,
            change
        ));
    });
}

#[test]
fn transfer_pos_prev_err() {
    new_test_ext().execute_with(|| {
        let account_id_from = 21;
        let account_id_to = 22;
        let account_id_bails = 23;

        let bails_collateral_btc = 50;

        // Step 0: Preparations

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_to,
            asset::BTC,
            1,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_bails,
            asset::BTC,
            bails_collateral_btc,
            true,
            None
        ));

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_bails,
            UserGroup::Bailsmen,
            true
        ));

        assert_ne!(
            get_debt_weight(asset::BTC), // 0.2 in mock
            EqFixedU128::zero(),
            "asset::BTC debt weight must not be zero for test"
        );

        // max_btc_debt is 10 for weight=0.2 and bails_collateral_btc=50
        let max_btc_debt = EqFixedU128::saturating_from_integer(bails_collateral_btc)
            * get_debt_weight(asset::BTC);

        let max_btc_debt: Balance = max_btc_debt.into_inner() / EqFixedU128::accuracy();

        let prev = 2;
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_from,
            asset::BTC,
            prev,
            true,
            None
        ));

        // Step 1: total_debt + debt_change > max_asset_debt
        let change = prev + max_btc_debt + 1;
        assert_err!(
            ModuleBalances::transfer(
                RuntimeOrigin::signed(account_id_from),
                asset::BTC,
                account_id_to,
                change
            ),
            Error::<Test>::DebtExceedLiquidity
        );
    });
}

#[test]
fn transfer_negative_prev_err() {
    new_test_ext().execute_with(|| {
        let account_id_from = 21;
        let account_id_to = 22;
        let account_id_bails = 23;

        let bails_collateral_btc = 50;

        // Step 0: Preparations

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_to,
            asset::BTC,
            1,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_bails,
            asset::BTC,
            bails_collateral_btc,
            true,
            None
        ));

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_bails,
            UserGroup::Bailsmen,
            true
        ));

        assert_ne!(
            get_debt_weight(asset::BTC), // 0.2 in mock
            EqFixedU128::zero(),
            "asset::BTC debt weight must not be zero for test"
        );

        // max_btc_debt is 10 for weight=0.2 and bails_collateral_btc=50
        let max_btc_debt = EqFixedU128::saturating_from_integer(bails_collateral_btc)
            * get_debt_weight(asset::BTC);

        let max_btc_debt = max_btc_debt.into_inner() / EqFixedU128::accuracy();

        let prev = 2;
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_from,
            asset::BTC,
            prev,
            true,
            None
        ));

        let change = prev * 2;
        let debt = change - prev;
        assert!(change > prev && debt < max_btc_debt);
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_to,
            change
        ));

        // Step 1: total_debt + debt_change > max_asset_debt
        let change = max_btc_debt - debt + 1;
        assert_err!(
            ModuleBalances::transfer(
                RuntimeOrigin::signed(account_id_from),
                asset::BTC,
                account_id_to,
                change
            ),
            Error::<Test>::DebtExceedLiquidity
        );
    });
}

#[test]
fn transfer_from_bails_err() {
    new_test_ext().execute_with(|| {
        let account_id_from = 21;
        let account_id_to = 22;
        let account_id_bails = ACCOUNT_BAILSMAN_1;

        let bails_collateral_btc = 50_000_000_000;

        // Step 0: Preparations

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_to,
            asset::BTC,
            1,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_bails,
            asset::BTC,
            bails_collateral_btc,
            true,
            None
        ));

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_bails,
            UserGroup::Bailsmen,
            true
        ));

        assert_ne!(
            get_debt_weight(asset::BTC), // 0.2 in mock
            EqFixedU128::zero(),
            "asset::BTC debt weight must not be zero for test"
        );

        // max_btc_debt is 10 for weight=0.2 and bails_collateral_btc=50
        let max_btc_debt =
            eq_fixedu128_from_balance(bails_collateral_btc) * get_debt_weight(asset::BTC);

        let max_btc_debt: Balance = balance_from_eq_fixedu128(max_btc_debt).unwrap();

        // Step 1: bails_collateral_btc < total_debt
        let prev = 2_000_000_000;
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_from,
            asset::BTC,
            prev,
            true,
            None
        ));

        let change = prev * 2;
        let debt = change - prev;
        assert!(change > prev && debt < max_btc_debt);
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_to,
            change
        ));

        let change = bails_collateral_btc - debt + 1;

        assert_err!(
            ModuleBalances::transfer(
                RuntimeOrigin::signed(account_id_bails),
                asset::BTC,
                account_id_to,
                change
            ),
            Error::<Test>::DebtExceedLiquidity
        );
    });
}

#[test]
fn lender_pool_deposit() {
    new_test_ext().execute_with(|| {
        TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000);

        assert_eq!(
            EqBalances::get_balance(&1, &asset::ETH),
            SignedBalance::Positive(1000)
        );
        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(1),
            asset::ETH,
            400
        ));
        assert_eq!(EqLending::lender(&1, &asset::ETH).unwrap().value, 400);
        assert_eq!(
            EqBalances::get_balance(&1, &asset::ETH),
            SignedBalance::Positive(600)
        );

        assert_eq!(
            EqBalances::get_balance(&2, &asset::ETH),
            SignedBalance::Positive(1000)
        );
        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(2),
            asset::ETH,
            600
        ));
        assert_eq!(EqLending::lender(&2, &asset::ETH).unwrap().value, 600);
        assert_eq!(
            EqBalances::get_balance(&2, &asset::ETH),
            SignedBalance::Positive(400)
        );

        assert_eq!(
            EqBalances::get_balance(&3, &asset::EQD),
            SignedBalance::Positive(1000)
        );

        assert_err!(
            EqLending::deposit(RuntimeOrigin::signed(3), asset::EQD, 100),
            Error::<Test>::WrongAssetType
        );
    });
}

#[test]
fn transfer_with_unreg_bails_ok() {
    new_test_ext().execute_with(|| {
        let account_id_from = 21;
        let account_id_to = 22;
        let account_id_bails_1 = 23;
        let account_id_bails_2 = 24;

        let bails_collateral_btc_1: Balance = 25_000_000_000;
        let bails_collateral_btc_2: Balance = 25_000_000_000;
        let bails_collateral_btc = bails_collateral_btc_1 + bails_collateral_btc_2;

        // Step 0: Preparations

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_to,
            asset::BTC,
            1,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_bails_1,
            asset::BTC,
            bails_collateral_btc_1,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_bails_2,
            asset::BTC,
            bails_collateral_btc_2,
            true,
            None
        ));

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_bails_1,
            UserGroup::Bailsmen,
            true
        ));
        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_bails_2,
            UserGroup::Bailsmen,
            true
        ));

        assert_ne!(
            get_debt_weight(asset::BTC), // 0.2 in mock
            EqFixedU128::zero(),
            "asset::BTC debt weight must not be zero for test"
        );

        // max_btc_debt is 10_000_000_000 for weight=0.2 and bails_collateral_btc=50_000_000_000
        let max_btc_debt = EqFixedU128::saturating_from_integer(bails_collateral_btc)
            * get_debt_weight(asset::BTC);

        let max_btc_debt = max_btc_debt.into_inner() / EqFixedU128::accuracy();

        // generate total_debt, where max_btc_debt < total_debt

        let prev = 2_000_000_000;
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_from,
            asset::BTC,
            prev,
            true,
            None
        ));

        let change = prev * 2;
        let debt = change - prev; // 2_000_000_000
        assert!(change > prev && debt < max_btc_debt);
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_to,
            change
        ));

        // Step 1: transfer from bails to unreg

        let btc_price = OracleMock::get_price(&asset::BTC).unwrap();

        let bails_collateral_in_usd = eq_fixedu128_from_balance(bails_collateral_btc_1) * btc_price;

        let transfer_to_unreg_in_usd =
            bails_collateral_in_usd - eq_fixedu128_from_balance(MinimalCollateral::get());

        let transfer_to_unreg =
            balance_from_eq_fixedu128::<Balance>(transfer_to_unreg_in_usd / btc_price).unwrap() + 1;

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_bails_1),
            asset::BTC,
            account_id_to,
            transfer_to_unreg
        ));

        // in runtime subaccs pallet make unreg_bailman
        // but we do it directly in test

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_bails_1,
            UserGroup::Bailsmen,
            false
        ));

        let new_bails_collateral_btc = bails_collateral_btc - bails_collateral_btc_1;
        let new_max_btc_debt = EqFixedU128::saturating_from_integer(new_bails_collateral_btc)
            * get_debt_weight(asset::BTC);

        let new_max_btc_debt = new_max_btc_debt.into_inner() / EqFixedU128::accuracy();

        let TotalAggregates {
            collateral: current_bails_collateral,
            debt: _,
        } = ModuleAggregates::get_total(UserGroup::Bailsmen, asset::BTC);

        assert_eq!(new_bails_collateral_btc, current_bails_collateral);

        assert!(new_max_btc_debt < max_btc_debt);

        assert!(debt < new_max_btc_debt);
    });
}

#[test]
fn transfer_with_unreg_bails_err() {
    new_test_ext().execute_with(|| {
        let account_id_from = 21;
        let account_id_to = 22;
        let account_id_bails_1 = ACCOUNT_BAILSMAN_1;
        let account_id_bails_2 = ACCOUNT_BAILSMAN_2;

        let bails_collateral_btc_1: Balance = 40_000_000_001;
        let bails_collateral_btc_2: Balance = 9_999_999_999;
        let bails_collateral_btc = bails_collateral_btc_1 + bails_collateral_btc_2;

        // Step 0: Preparations

        assert_ok!(EqBalances::deposit_creating(
            &account_id_to,
            asset::BTC,
            1,
            true,
            None
        ));

        assert_ok!(EqBalances::deposit_creating(
            &account_id_bails_1,
            asset::BTC,
            bails_collateral_btc_1,
            true,
            None
        ));

        assert_ok!(EqBalances::deposit_creating(
            &account_id_bails_2,
            asset::BTC,
            bails_collateral_btc_2,
            true,
            None
        ));

        assert_ok!(EqAggregates::set_usergroup(
            &account_id_bails_1,
            UserGroup::Bailsmen,
            true
        ));
        assert_ok!(EqAggregates::set_usergroup(
            &account_id_bails_2,
            UserGroup::Bailsmen,
            true
        ));

        assert_ne!(
            get_debt_weight(asset::BTC), // 0.2 in mock
            EqFixedU128::zero(),
            "asset::BTC debt weight must not be zero for test"
        );

        // max_btc_debt is 10_000_000_000 for weight=0.2 and bails_collateral_btc=50_000_000_000
        let max_btc_debt = EqFixedU128::saturating_from_integer(bails_collateral_btc)
            * get_debt_weight(asset::BTC);

        let max_btc_debt = max_btc_debt.into_inner() / EqFixedU128::accuracy();

        // generate total_debt, where max_btc_debt < total_debt

        let prev = 2_000_000_000;
        assert_ok!(EqBalances::deposit_creating(
            &account_id_from,
            asset::BTC,
            prev,
            true,
            None
        ));

        let change = prev * 2;
        let debt = change - prev; // 2_000_000_000
        assert!(change > prev && debt < max_btc_debt);
        assert_ok!(EqBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_to,
            change
        ));

        // Step 1: transfer from bails to unreg

        let btc_price = OracleMock::get_price(&asset::BTC).unwrap();

        let bails_collateral_in_usd = eq_fixedu128_from_balance(bails_collateral_btc_1) * btc_price;

        let transfer_to_unreg_in_usd =
            bails_collateral_in_usd - eq_fixedu128_from_balance(MinimalCollateral::get());

        let transfer_to_unreg =
            balance_from_eq_fixedu128::<Balance>(transfer_to_unreg_in_usd / btc_price).unwrap() + 1;

        assert_noop!(
            EqBalances::transfer(
                RuntimeOrigin::signed(account_id_bails_1),
                asset::BTC,
                account_id_to,
                transfer_to_unreg
            ),
            Error::<Test>::BailsmanCantBeUnregistered
        );

        let TotalAggregates {
            collateral: current_bails_collateral,
            debt: _,
        } = EqAggregates::get_total(UserGroup::Bailsmen, asset::BTC);

        assert_eq!(current_bails_collateral, bails_collateral_btc);

        let TotalAggregates {
            collateral: _,
            debt: total_btc_debt,
        } = EqAggregates::get_total(UserGroup::Balances, asset::BTC);

        assert_eq!(total_btc_debt, debt);
    });
}

#[test]
fn lender_pool_withdraw_ok() {
    new_test_ext().execute_with(|| {
        TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000);

        assert_eq!(
            EqBalances::get_balance(&1, &asset::ETH),
            SignedBalance::Positive(1000)
        );

        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(1),
            asset::ETH,
            400
        ));
        assert_eq!(EqLending::lender(&1, &asset::ETH).unwrap().value, 400);
        assert_eq!(
            EqBalances::get_balance(&1, &asset::ETH),
            SignedBalance::Positive(600)
        );

        assert_noop!(
            EqLending::withdraw(RuntimeOrigin::signed(1), asset::ETH, 500),
            Error::<Test>::NotEnoughToWithdraw
        );
        assert_ok!(EqLending::withdraw(
            RuntimeOrigin::signed(1),
            asset::ETH,
            200
        ));

        assert_eq!(EqLending::lender(&1, &asset::ETH).unwrap().value, 200);
        assert_eq!(
            EqBalances::get_balance(&1, &asset::ETH),
            SignedBalance::Positive(800)
        );
    });
}

#[test]
fn lender_pool_withdraw_not_enough() {
    new_test_ext().execute_with(|| {
        // TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000);

        assert_eq!(
            EqBalances::get_balance(&1, &asset::ETH),
            SignedBalance::Positive(1000)
        );

        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(1),
            asset::ETH,
            400
        ));
        assert_eq!(EqLending::lender(&1, &asset::ETH).unwrap().value, 400);
        assert_eq!(
            EqBalances::get_balance(&1, &asset::ETH),
            SignedBalance::Positive(600)
        );

        assert_noop!(
            EqLending::withdraw(RuntimeOrigin::signed(1), asset::ETH, 500),
            Error::<Test>::NotEnoughToWithdraw
        );
    });
}

#[test]
fn lender_pool_borrow_ok() {
    new_test_ext().execute_with(|| {
        TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000);

        let main = 0;
        let borr = 666;
        let lender = 1;

        assert_ok!(EqAggregates::set_usergroup(
            &borr,
            UserGroup::Balances,
            true
        ));

        assert_ok!(EqLending::do_deposit(&lender, asset::BTC, 50));

        // Alright, get your BTC
        assert_ok!(EqBalances::currency_transfer(
            &borr,
            &main,
            asset::BTC,
            45,
            frame_support::traits::ExistenceRequirement::KeepAlive,
            eq_primitives::TransferReason::Common,
            true
        ));
        assert_eq!(
            EqBalances::get_balance(&borr, &asset::BTC),
            SignedBalance::Negative(45),
        );

        assert_eq!(
            EqAssets::get_asset_data(&asset::BTC)
                .unwrap()
                .lending_debt_weight,
            Permill::from_percent(90)
        );
        assert_eq!(
            EqLending::get_total_debt(asset::BTC),
            45, // == lending_debt_weight 0.9 * total_lendable (50), you cannot borrow more
        );
        let (lenders_lendable, bails_lendable) = EqLending::get_lendable_parts(asset::BTC);
        assert_eq!(lenders_lendable + bails_lendable, 50);

        // Cannot decrease lender collateral
        assert_err!(
            EqLending::do_withdraw(&lender, asset::BTC, 1),
            Error::<Test>::DebtExceedLiquidity,
        );
    });
}

#[test]
fn lender_pool_borrow_in_only_bailsmen_period() {
    new_test_ext().execute_with(|| {
        TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000 - 1);

        let main = 0;
        let borr = 666;
        let lender = 1;

        assert_ok!(EqAggregates::set_usergroup(
            &borr,
            UserGroup::Balances,
            true
        ));

        assert_ok!(EqLending::do_deposit(&lender, asset::BTC, 50));
        // Liquidity is enough for borrowing 10 BTC
        assert_eq!(EqLending::aggregates(asset::BTC), 50);
        // But actual lendable amount is 0
        let (lenders_lendable, bails_lendable) = EqLending::get_lendable_parts(asset::BTC);
        let total_lendable = lenders_lendable + bails_lendable;
        assert_eq!(total_lendable, 0);

        assert_err!(
            EqBalances::currency_transfer(
                &borr,
                &main,
                asset::BTC,
                10,
                frame_support::traits::ExistenceRequirement::KeepAlive,
                eq_primitives::TransferReason::Common,
                true
            ),
            Error::<Test>::DebtExceedLiquidity,
        );

        TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000);
        // Now it is equal
        assert_eq!(EqLending::aggregates(asset::BTC), 50);

        let (lenders_lendable, _bails_lendable) = EqLending::get_lendable_parts(asset::BTC);
        assert_eq!(lenders_lendable, 50);
    });
}

#[test]
fn lender_pool_borrow_low_liquidity() {
    new_test_ext().execute_with(|| {
        TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000);

        let main = 0;
        let borr = 666;
        let lender = 1;

        assert_ok!(EqAggregates::set_usergroup(
            &borr,
            UserGroup::Balances,
            true
        ));

        assert_ok!(EqLending::do_deposit(&lender, asset::BTC, 10));

        // Try to borrow with low liquidity
        assert_err!(
            EqBalances::currency_transfer(
                &borr,
                &main,
                asset::BTC,
                10,
                frame_support::traits::ExistenceRequirement::KeepAlive,
                eq_primitives::TransferReason::Common,
                true
            ),
            Error::<Test>::DebtExceedLiquidity,
        );
        assert_eq!(
            EqBalances::get_balance(&borr, &asset::BTC),
            SignedBalance::Positive(0),
        );
        assert_eq!(
            EqAggregates::get_total(UserGroup::Balances, asset::BTC).debt,
            0,
        );
    });
}

#[test]
fn add_reward_without_lenders() {
    new_test_ext().execute_with(|| {
        TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000);

        use eq_primitives::LendingPoolManager as _;

        assert_err!(
            EqLending::add_reward(asset::ETH, 10),
            Error::<Test>::NoLendersToClaim,
        );
    });
}

#[test]
fn reward_flow() {
    new_test_ext().execute_with(|| {
        OnlyBailsmanTill::<Test>::put(0);

        fn last_reward_step() -> EqFixedU128 {
            EqLending::rewards(asset::ETH)
        }

        use eq_primitives::LendingPoolManager as _;

        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(1),
            asset::ETH,
            400
        ));
        assert_eq!(EqBalances::total_balance(&1, asset::ETH), 600);

        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(2),
            asset::ETH,
            600
        ));
        assert_eq!(EqBalances::total_balance(&2, asset::ETH), 400);

        assert_ok!(EqLending::add_reward(asset::ETH, 10));
        assert_eq!(
            last_reward_step(),
            EqFixedU128::saturating_from_rational(10, 1000)
        ); // 0 + 10/1000

        assert_ok!(EqLending::add_reward(asset::ETH, 10));
        assert_eq!(
            last_reward_step(),
            EqFixedU128::saturating_from_rational(20, 1000)
        ); // 10/1000 + 10/1000

        assert_ok!(EqLending::add_reward(asset::ETH, 10));
        assert_eq!(
            last_reward_step(),
            EqFixedU128::saturating_from_rational(30, 1000)
        ); // 20/1000 + 10/1000

        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(1),
            asset::ETH,
            100
        ));
        assert_eq!(EqBalances::total_balance(&1, asset::ETH), 600 - 100);
        assert_eq!(EqBalances::total_balance(&1, asset::EQ), 12);

        assert_ok!(EqLending::withdraw(
            RuntimeOrigin::signed(1),
            asset::ETH,
            300
        ));
        assert_eq!(EqBalances::total_balance(&1, asset::EQ), 12);
        assert_eq!(EqBalances::total_balance(&1, asset::ETH), 600 - 100 + 300);

        assert_ok!(EqLending::withdraw(
            RuntimeOrigin::signed(2),
            asset::ETH,
            400
        ));
        assert_eq!(EqBalances::total_balance(&2, asset::ETH), 400 + 400);
        assert_eq!(EqBalances::total_balance(&2, asset::EQ), 18);

        assert_ok!(EqLending::add_reward(asset::ETH, 20));
        assert_eq!(
            last_reward_step(),
            EqFixedU128::saturating_from_rational(80, 1000)
        ); // 30/1000 + 20/400

        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(2),
            asset::ETH,
            100
        ));
        assert_eq!(EqBalances::total_balance(&2, asset::ETH), 400 + 400 - 100);
        assert_eq!(EqBalances::total_balance(&2, asset::EQ), 18 + 10);

        assert_ok!(EqLending::add_reward(asset::ETH, 10));
        assert_eq!(
            last_reward_step(),
            EqFixedU128::saturating_from_rational(100, 1000)
        ); // 80/1000 + 10/500

        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(1),
            asset::ETH,
            100
        ));
        assert_eq!(
            EqBalances::total_balance(&1, asset::ETH),
            600 - 100 + 300 - 100
        );
        assert_eq!(EqBalances::total_balance(&1, asset::EQ), 12 + 14);

        assert_ok!(EqLending::deposit(
            RuntimeOrigin::signed(2),
            asset::ETH,
            100
        ));
        assert_eq!(
            EqBalances::total_balance(&2, asset::ETH),
            400 + 400 - 100 - 100
        );
        assert_eq!(EqBalances::total_balance(&2, asset::EQ), 18 + 10 + 6);

        assert_ok!(EqLending::withdraw(
            RuntimeOrigin::signed(2),
            asset::ETH,
            400
        ));
        assert_eq!(
            EqBalances::total_balance(&2, asset::ETH),
            400 + 400 - 100 - 100 + 400
        );
        assert_eq!(EqBalances::total_balance(&2, asset::EQ), 18 + 10 + 6);
        assert_eq!(EqLending::lender(2, asset::ETH), None);
    });
}

#[test]
fn make_debt_and_transfer_to_bailsman() {
    new_test_ext().execute_with(|| {
        let account_id_from = 21;
        let account_id_to = 22;
        let account_id_bails = ACCOUNT_BAILSMAN_1;

        let bails_collateral_btc = 50_000_000_000;

        // Step 0: Preparations

        assert_ok!(EqBalances::deposit_creating(
            &account_id_to,
            asset::BTC,
            1,
            true,
            None
        ));
        // to create account
        assert_ok!(EqBalances::deposit_creating(
            &account_id_from,
            asset::BTC,
            1,
            true,
            None
        ));

        assert_ok!(EqBalances::deposit_creating(
            &account_id_bails,
            asset::BTC,
            bails_collateral_btc,
            true,
            None
        ));

        assert_ok!(EqAggregates::set_usergroup(
            &account_id_bails,
            UserGroup::Bailsmen,
            true
        ));

        assert_ne!(
            get_debt_weight(asset::BTC), // 0.2 in mock
            EqFixedU128::zero(),
            "asset::BTC debt weight must not be zero for test"
        );

        // for debt creating
        EqBalances::make_free_balance_be(&account_id_from, asset::BTC, SignedBalance::zero());
        let free_to_borrow_before = free_to_borrow(asset::BTC);
        println!("free_to_borrow_before = {:?}", free_to_borrow_before);
        assert_ok!(EqBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::BTC,
            account_id_bails,
            free_to_borrow_before
        ));

        let free_to_borrow_after = free_to_borrow(asset::BTC);
        println!("free_to_borrow_after = {:?}", free_to_borrow_after);
    });
}

#[test]
fn check_bails_pool_after_unreg_when_enough_liquidity() {
    new_test_ext().execute_with(|| {
        TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000);

        let main = 0;
        let lender = 1;
        let borr = 666;
        let bailsman_1 = 11;
        let bailsman_2 = 12;

        assert_ok!(EqAggregates::set_usergroup(
            &borr,
            UserGroup::Balances,
            true
        ));
        assert_ok!(EqAggregates::set_usergroup(
            &bailsman_1,
            UserGroup::Bailsmen,
            true
        ));
        assert_ok!(EqAggregates::set_usergroup(
            &bailsman_2,
            UserGroup::Bailsmen,
            true
        ));

        EqBalances::make_free_balance_be(
            &bailsman_1,
            asset::BTC,
            SignedBalance::<u128>::Positive(30),
        );
        EqBalances::make_free_balance_be(
            &bailsman_2,
            asset::BTC,
            SignedBalance::<u128>::Positive(90),
        );

        assert_ok!(EqLending::do_deposit(&lender, asset::BTC, 10));
        assert_ok!(EqBalances::currency_transfer(
            &borr,
            &main,
            asset::BTC,
            9,
            frame_support::traits::ExistenceRequirement::KeepAlive,
            eq_primitives::TransferReason::Common,
            true
        ));

        assert_eq!(
            EqBalances::get_balance(&borr, &asset::BTC),
            SignedBalance::Negative(9),
        );

        assert_ok!(EqLending::check_bails_pool_after_unreg(&bailsman_1));
    });
}

#[test]
fn check_bails_pool_after_unreg_when_not_enough_liquidity() {
    new_test_ext().execute_with(|| {
        // Only bailsmen period
        TimeMock::set(OnlyBailsmanTill::<Test>::get() * 1_000 - 1);

        let main = 0;
        let lender = 1;
        let borr = 666;
        let bailsman_1 = 11;

        assert_ok!(EqAggregates::set_usergroup(
            &borr,
            UserGroup::Balances,
            true
        ));
        assert_ok!(EqAggregates::set_usergroup(
            &bailsman_1,
            UserGroup::Bailsmen,
            true
        ));

        EqBalances::make_free_balance_be(
            &bailsman_1,
            asset::BTC,
            SignedBalance::<u128>::Positive(45),
        );

        assert_ok!(EqLending::do_deposit(&lender, asset::BTC, 10));
        // liquidity = .2 * 45 = 9
        assert_ok!(EqBalances::currency_transfer(
            &borr,
            &main,
            asset::BTC,
            9,
            frame_support::traits::ExistenceRequirement::KeepAlive,
            eq_primitives::TransferReason::Common,
            true
        ));

        assert_eq!(
            EqBalances::get_balance(&borr, &asset::BTC),
            SignedBalance::Negative(9),
        );

        assert_err!(
            EqLending::check_bails_pool_after_unreg(&bailsman_1),
            Error::<Test>::BailsmanCantBeUnregistered
        );

        TimeMock::set(OnlyBailsmanTill::<Test>::get());
    });
}

#[test]
fn baisman_withdraw_usd_ok() {
    new_test_ext().execute_with(|| {
        let account_id_bails = 21;
        let account_id_from = 22;
        let account_id_to = 23;

        let bails_collateral_btc = 50 * ONE_TOKEN;
        let bails_collateral_eqd = 1000 * ONE_TOKEN;

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_bails,
            asset::BTC,
            bails_collateral_btc,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_bails,
            asset::EQD,
            bails_collateral_eqd,
            true,
            None
        ));

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_bails,
            UserGroup::Bailsmen,
            true
        ));

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_from),
            asset::EQD,
            account_id_to,
            50
        ));

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_bails),
            asset::EQD,
            account_id_to,
            bails_collateral_eqd
        ));
    });
}
