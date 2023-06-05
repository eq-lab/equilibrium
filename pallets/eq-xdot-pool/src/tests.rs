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

use sp_std::ops::{Add, Sub};
use substrate_fixed::types::I64F64;

use crate::mock::{new_test_ext, Balance, Balances, RuntimeOrigin, Test, Timestamp, Xdot, XDOT};
use crate::yield_math::YieldMath;

use super::*;
use eq_primitives::asset::{Asset, DOT};
use eq_primitives::balance::BalanceGetter;
use eq_primitives::balance::EqCurrency;
use eq_primitives::xdot_pool::XdotBalanceConvert as BalanceConvert;
use eq_primitives::SignedBalance;
use eq_utils::ONE_TOKEN;
use frame_support::{assert_err, assert_noop, assert_ok};

const BASE_TOKEN: Asset = DOT;
const X_BASE_TOKEN: Asset = XDOT;
const ONE_MILLION: Balance = 1_000_000 * ONE_TOKEN;

fn setup_pool() -> PoolId {
    let g1 = I64F64::from_num(0.95);
    let g2 = I64F64::from_num(1000) / I64F64::from_num(950);
    let now = eq_rate::Pallet::<Test>::now().as_secs();
    let maturity = now + 3 * 30 * 24 * 60 * 60; // three months
    let ts = I64F64::from_num(1) / I64F64::from_num(315576000); // 1 / Seconds in 10 years
    assert_ok!(Xdot::do_create_pool(
        BASE_TOKEN,
        X_BASE_TOKEN,
        g1,
        g2,
        maturity,
        ts,
        TEST_ACC
    ));
    let pool_id = Xdot::pool_count() - 1;
    Initializer::<Test>::remove(pool_id); // to pass init check
    pool_id
}

fn update_pool_balances(
    pool_id: PoolId,
    base_balance: Option<Balance>,
    xbase_balance: Option<Balance>,
) {
    let pool = pool(pool_id);
    if let Some(new_base_balance) = base_balance {
        Balances::make_free_balance_be(
            &pool.account,
            BASE_TOKEN,
            SignedBalance::Positive(new_base_balance),
        );
    }

    if let Some(new_xbase_balance) = xbase_balance {
        Balances::make_free_balance_be(
            &pool.account,
            X_BASE_TOKEN,
            SignedBalance::Positive(new_xbase_balance),
        );
    }
}

fn pool(pool_id: PoolId) -> XdotPoolInfo<Test> {
    Xdot::pools(pool_id).unwrap()
}

#[test]
fn mint_adds_initial_liquidity() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let account_id: u64 = 2;
        assert_ok!(Xdot::mint(
            RuntimeOrigin::signed(account_id),
            pool_id,
            (0, 1),
            (1, 1),
            ONE_MILLION,
            0,
            0,
        ));
        let pool = pool(pool_id);
        assert_eq!(
            Balances::get_balance(&account_id, &pool.pool_asset),
            SignedBalance::Positive(ONE_MILLION)
        );

        assert_eq!(
            SignedBalance::Positive(ONE_MILLION),
            Balances::get_balance(&pool.account, &BASE_TOKEN)
        );

        let xbase_pool_balance = Balances::get_balance(&pool.account, &X_BASE_TOKEN);

        let expected_lp_issuance = ONE_MILLION;

        assert_eq!(
            SignedBalance::Positive(pool.virtual_xbase_balance().unwrap()),
            xbase_pool_balance
                .add_balance(&expected_lp_issuance)
                .expect("No overflow")
        );
    });
}

#[test]
fn mint_adds_liquidity_with_zero_fy_token() {
    new_test_ext().execute_with(|| {
        let temp_acc = 1;
        let account_id = 2;
        let pool_id = setup_pool();
        let first_mint = ONE_MILLION;
        assert_ok!(Xdot::mint(
            RuntimeOrigin::signed(temp_acc),
            pool_id,
            (0, 1),
            (1, 1),
            first_mint,
            0,
            0,
        ));
        // // After initializing, donate base to simulate having reached zero fyToken through trading

        let actual_pool = pool(pool_id);

        update_pool_balances(
            pool_id,
            Some(actual_pool.base_balance() + ONE_MILLION),
            None,
        );

        // We don't allow to mint with zero fy_in
        // because we expect that pool has fy tokens after initialization
        assert_err!(
            Xdot::mint(
                RuntimeOrigin::signed(account_id),
                pool_id,
                (0, 1),
                (1, 1),
                ONE_MILLION,
                0,
                0,
            ),
            Error::<Test>::TooLowXbaseIn
        );

        // // The user got as minted tokens half of the amount he supplied as base, because supply doesn't equal base in the pool anymore
        // let expected_mint = ONE_MILLION / 2;
        // assert_eq!(
        //     Balances::get_balance(&account_id, &actual_pool.pool_asset),
        //     SignedBalance::Positive(expected_mint)
        // );

        // let pool = pool(pool_id);

        // assert_eq!(
        //     SignedBalance::Positive(pool.virtual_xbase_balance().unwrap()),
        //     Balances::get_balance(&pool.account, &X_BASE_TOKEN)
        //         .add_balance(first_mint)
        //         .expect("No overflow")
        //         .add_balance(expected_mint)
        //         .expect("No overflow")
        // );
    });
}

const TEST_ACC: u64 = 12345;

fn initial_liquidity(pool_id: PoolId, base: Balance, xbase: Balance) -> XdotPoolInfo<Test> {
    assert_ok!(Xdot::mint(
        RuntimeOrigin::signed(TEST_ACC),
        pool_id,
        (0, 1),
        (1, 1),
        base,
        0,
        0,
    ));

    let pool = pool(pool_id);

    update_pool_balances(pool_id, None, Some(pool.xbase_balance() + xbase));

    Xdot::pools(pool_id).unwrap()
}

fn init_liquidity_for_mint(pool_id: PoolId) -> XdotPoolInfo<Test> {
    initial_liquidity(pool_id, ONE_MILLION, ONE_MILLION / 9)
}

fn init_liquidity_for_trade(pool_id: PoolId) -> XdotPoolInfo<Test> {
    initial_liquidity(pool_id, ONE_MILLION, ONE_MILLION)
}

fn init_liquidity_for_trade_with_extra_fy_token(pool_id: PoolId) -> XdotPoolInfo<Test> {
    initial_liquidity(pool_id, ONE_MILLION, ONE_MILLION);
    let additional_fy_token = ONE_TOKEN * 30;

    assert_ok!(Xdot::sell_xbase(
        RuntimeOrigin::signed(TEST_ACC),
        pool_id,
        additional_fy_token,
        0
    ));

    Xdot::pools(pool_id).unwrap()
}

#[test]
fn mint_with_initial_liquidity_mints_liquidity_tokens_returning_base_surplus() {
    new_test_ext().execute_with(|| {
        let fy_token_in = ONE_TOKEN;
        let pool_id = setup_pool();
        init_liquidity_for_mint(pool_id);

        let expected_minted = 9 * ONE_TOKEN;
        let expected_base_in = 9 * ONE_TOKEN;

        let pool_tokens_before = Balances::get_balance(&TEST_ACC, &pool(pool_id).pool_asset);
        let base_tokens_before = expected_base_in + ONE_TOKEN;
        Balances::make_free_balance_be(
            &TEST_ACC,
            BASE_TOKEN,
            SignedBalance::Positive(base_tokens_before),
        );
        // imagine that another account initiate liquidyty

        let pool_base_balance_before = Balances::get_balance(&pool(pool_id).account, &BASE_TOKEN);

        assert_ok!(Xdot::mint(
            RuntimeOrigin::signed(TEST_ACC),
            pool_id,
            (0, 1),
            (u32::MAX, 1),
            base_tokens_before,
            fy_token_in,
            0
        ));

        let minted = Balances::get_balance(&TEST_ACC, &pool(pool_id).pool_asset)
            .sub(pool_tokens_before.clone());

        assert_eq!(minted, SignedBalance::Positive(expected_minted));
        assert_eq!(
            Balances::get_balance(&TEST_ACC, &BASE_TOKEN),
            SignedBalance::Positive(ONE_TOKEN)
        );

        let pool = pool(pool_id);
        assert_eq!(
            pool_base_balance_before
                .add_balance(&expected_base_in)
                .unwrap(),
            Balances::get_balance(&pool.account, &BASE_TOKEN)
        );

        assert_eq!(
            SignedBalance::Positive(pool.virtual_xbase_balance().unwrap()),
            Balances::get_balance(&pool.account, &X_BASE_TOKEN)
                .add(minted)
                .add(pool_tokens_before)
        );
    });
}
#[test]
fn mint_with_initial_liquidity_mints_liquidity_tokens_with_base_only() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let pool_before = init_liquidity_for_mint(pool_id);
        let fy_token_to_buy = ONE_TOKEN / 1000;

        let expected_minted = 9000000; // 0.009000000  (0.009000000081000000 in yield contract)
        let expected_base_in = 9997536; // 0.009997536 (0.009998536777719936 in yield contract)

        let account_id = 1;
        Balances::make_free_balance_be(
            &account_id,
            BASE_TOKEN,
            SignedBalance::Positive(expected_base_in),
        );

        let pool_tokens_before = Balances::get_balance(&TEST_ACC, &pool(pool_id).pool_asset);
        let pool_base_before = pool_before.base_balance();
        let virtual_xbase_before = pool_before.virtual_xbase_balance().unwrap();

        assert_ok!(Xdot::mint(
            RuntimeOrigin::signed(account_id),
            pool_id,
            (0, 1),
            (u32::MAX, 1),
            expected_base_in,
            0,
            fy_token_to_buy,
        ));

        let pool_after = pool(pool_id);
        let base_in = pool_after.base_balance() - pool_base_before;
        let minted = Balances::get_balance(&account_id, &pool(pool_id).pool_asset);

        assert_eq!(minted, SignedBalance::Positive(expected_minted));

        assert_eq!(base_in, expected_base_in);

        assert_eq!(
            SignedBalance::Positive(pool_base_before + base_in),
            Balances::get_balance(&pool_after.account, &BASE_TOKEN)
        );
        assert_eq!(
            minted.add_balance(&virtual_xbase_before).unwrap(),
            Balances::get_balance(&pool_after.account, &X_BASE_TOKEN)
                .add(minted)
                .add(pool_tokens_before)
        );
    });
}

#[test]
fn mint_with_initial_liquidity_doesnt_mint_if_ratio_drops() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let pool_before = init_liquidity_for_mint(pool_id);
        let fy_token_to_buy = ONE_TOKEN / 1000;
        let account_id = 1;

        update_pool_balances(pool_id, Some(pool_before.base_balance() + ONE_TOKEN), None);

        let actual_pool = pool(pool_id);
        let base_balance_num = BalanceConvert::convert(actual_pool.base_balance());
        let virtual_xbase_balance_num =
            BalanceConvert::convert(actual_pool.virtual_xbase_balance().unwrap());
        let lp_total_supply_num = BalanceConvert::convert(actual_pool.lp_total_supply);
        let min_ratio_num = base_balance_num / (virtual_xbase_balance_num - lp_total_supply_num);

        update_pool_balances(pool_id, None, Some(pool_before.xbase_balance() + ONE_TOKEN));

        let min_ratio = (min_ratio_num.int().to_num(), 1); // some dirty hack

        assert_noop!(
            Xdot::mint(
                RuntimeOrigin::signed(account_id),
                pool_id,
                min_ratio,
                (u32::MAX, 1),
                0,
                0,
                fy_token_to_buy
            ),
            Error::<Test>::WrongMinRatio
        );
    });
}

#[test]
fn mint_with_initial_liquidity_doesnt_mint_if_ratio_rises() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let before_pool = init_liquidity_for_mint(pool_id);
        let fy_token_to_buy = ONE_TOKEN / 1000;
        let account_id = 1;

        update_pool_balances(pool_id, Some(before_pool.base_balance() + ONE_TOKEN), None);
        let actual_pool = pool(pool_id);
        let base_balance_num = BalanceConvert::convert(actual_pool.base_balance());
        let virtual_xbase_balance_num =
            BalanceConvert::convert(actual_pool.virtual_xbase_balance().unwrap());
        let lp_total_supply_num = BalanceConvert::convert(actual_pool.lp_total_supply);
        let max_ratio = base_balance_num / (virtual_xbase_balance_num - lp_total_supply_num);
        // println!("base_balance_num {:?}\nvirtual_xbase_balance_num {:?}\nlp_total_supply_num {:?}\nmaxRatio {:?}\n", base_balance_num, virtual_xbase_balance_num, lp_total_supply_num, max_ratio);

        update_pool_balances(pool_id, Some(actual_pool.base_balance() + ONE_TOKEN), None);

        let max_ratio = (
            BalanceConvert::convert(max_ratio).unwrap() as u32,
            ONE_TOKEN as u32,
        );
        assert_noop!(
            Xdot::mint(
                RuntimeOrigin::signed(account_id),
                pool_id,
                (0, 1),
                max_ratio,
                0,
                0,
                fy_token_to_buy
            ),
            Error::<Test>::WrongMaxRatio
        );
    });
}

#[test]
fn mint_with_initial_liquidity_burns_liquidity_tokens() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let before_pool = init_liquidity_for_mint(pool_id);
        let account_id = 1;
        let base_balance = before_pool.base_balance();
        let fy_token_balance =
            before_pool.virtual_xbase_balance().unwrap() - before_pool.lp_total_supply;

        let lp_tokens_in = ONE_TOKEN;

        let expected_base_out = 1000000000; // 1.0
        let expected_fy_token_out = 111111111; // 0.111111111

        assert_ok!(Xdot::burn(
            RuntimeOrigin::signed(account_id),
            pool_id,
            (0, 1),
            (u32::MAX, 1),
            lp_tokens_in,
            false
        ));

        let actual_pool = pool(pool_id);
        let base_out = base_balance - actual_pool.base_balance();
        let fy_token_balance_after = actual_pool.xbase_balance();
        let xbase_out = fy_token_balance - fy_token_balance_after;

        assert_eq!(base_out, expected_base_out);
        assert_eq!(xbase_out, expected_fy_token_out);

        assert_eq!(base_balance - base_out, actual_pool.base_balance());

        assert_eq!(fy_token_balance - xbase_out, actual_pool.xbase_balance());

        assert_eq!(
            before_pool.lp_total_supply,
            actual_pool.lp_total_supply + lp_tokens_in
        );

        assert_eq!(
            Balances::get_balance(&account_id, &actual_pool.base_asset),
            SignedBalance::Positive(base_out)
        );
        assert_eq!(
            Balances::get_balance(&account_id, &actual_pool.xbase_asset),
            SignedBalance::Positive(xbase_out)
        );
    });
}

#[test]
fn mint_with_initial_liquidity_burns_liquidity_tokens_to_base() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let before_pool = init_liquidity_for_mint(pool_id);
        let account_id = 1;
        let base_balance = before_pool.base_balance();
        let lp_tokens_in = ONE_TOKEN * 2;

        let expected_base_out = 2221615752; // 2.221615752 (original yield is 2.221614753)

        assert_ok!(Xdot::burn(
            RuntimeOrigin::signed(account_id),
            pool_id,
            (0, 1),
            (u32::MAX, 1),
            lp_tokens_in,
            true
        ));

        let actual_pool = pool(pool_id);
        let base_out = base_balance - actual_pool.base_balance();

        assert_eq!(base_out, expected_base_out);

        assert_eq!(base_balance - base_out, actual_pool.base_balance());
        let fy_token_balance_after =
            actual_pool.virtual_xbase_balance().unwrap() - actual_pool.lp_total_supply;

        assert_eq!(
            Balances::get_balance(&actual_pool.account, &actual_pool.xbase_asset),
            SignedBalance::Positive(fy_token_balance_after)
        );
    });
}

#[test]
fn mint_with_initial_liquidity_doesnt_burn_if_ratio_drops() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let before_pool = init_liquidity_for_mint(pool_id);
        let account_id = 1;
        let lp_tokens_in = ONE_TOKEN * 2;
        let base_balance_num = BalanceConvert::convert(before_pool.base_balance());
        let virtual_xbase_balance_num =
            BalanceConvert::convert(before_pool.virtual_xbase_balance().unwrap());
        let lp_total_supply_num = BalanceConvert::convert(before_pool.lp_total_supply);
        let min_ratio_num = base_balance_num / (virtual_xbase_balance_num - lp_total_supply_num);

        update_pool_balances(pool_id, None, Some(before_pool.xbase_balance() + ONE_TOKEN));
        let min_ratio = (min_ratio_num.int().to_num(), 1); // some dirty hack

        assert_noop!(
            Xdot::burn(
                RuntimeOrigin::signed(account_id),
                pool_id,
                min_ratio,
                (u32::MAX, 1),
                lp_tokens_in,
                true,
            ),
            Error::<Test>::WrongMinRatio
        );
    });
}

#[test]
fn mint_with_initial_liquidity_doesnt_burn_if_ratio_rises() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let before_pool = init_liquidity_for_mint(pool_id);
        let account_id = 1;
        let lp_tokens_in = ONE_TOKEN * 2;

        let base_balance_num = BalanceConvert::convert(before_pool.base_balance());
        let virtual_xbase_balance_num =
            BalanceConvert::convert(before_pool.virtual_xbase_balance().unwrap());
        let lp_total_supply_num = BalanceConvert::convert(before_pool.lp_total_supply);
        let max_ratio = base_balance_num / (virtual_xbase_balance_num - lp_total_supply_num);

        update_pool_balances(pool_id, Some(before_pool.base_balance() + ONE_TOKEN), None);
        let max_ratio = (
            BalanceConvert::convert(max_ratio).unwrap() as u32,
            ONE_TOKEN as u32,
        );
        assert_noop!(
            Xdot::burn(
                RuntimeOrigin::signed(account_id),
                pool_id,
                (0, 1),
                max_ratio,
                lp_tokens_in,
                true,
            ),
            Error::<Test>::WrongMaxRatio
        );
    });
}

#[test]
fn sells_fy_token() {
    new_test_ext().execute_with(|| {
        let fy_token_in = ONE_TOKEN;
        let acc = 1;
        let pool_id = setup_pool();
        let pool = init_liquidity_for_trade(pool_id);
        let pool_base_balance_before = pool.base_balance();
        let pool_xbase_balance_before = pool.xbase_balance();

        let base_out_preview =
            YieldMath::<I64F64, yield_math::YieldConvert>::base_out_for_fy_token_in(
                BalanceConvert::convert(pool.base_balance()),
                BalanceConvert::convert(pool.virtual_xbase_balance().unwrap()),
                BalanceConvert::convert(fy_token_in),
                yield_math::YieldConvert::convert(Xdot::time_till_maturity(pool.maturity).unwrap()),
                pool.ts,
                pool.g2,
                false,
            )
            .map(BalanceConvert::convert)
            .unwrap()
            .unwrap();

        let expected_base_out = 982182102; // 0.982182102; in original yield tests 0.982181107697934918

        assert_ok!(Xdot::sell_xbase(
            RuntimeOrigin::signed(acc),
            pool_id,
            fy_token_in,
            0
        ));

        let base_out = Balances::get_balance(&acc, &BASE_TOKEN);

        assert_eq!(base_out, SignedBalance::Positive(expected_base_out));

        assert_eq!(base_out_preview, expected_base_out);

        assert_eq!(
            pool.base_balance(),
            pool_base_balance_before - base_out.abs()
        );

        assert_eq!(
            pool.xbase_balance(),
            pool_xbase_balance_before + fy_token_in
        );
    });
}

#[test]
fn does_not_sell_fy_token_beyond_slippage() {
    new_test_ext().execute_with(|| {
        let fy_token_in = ONE_TOKEN;
        let acc = 1;
        let pool_id = setup_pool();
        init_liquidity_for_trade(pool_id);

        assert_noop!(
            Xdot::sell_xbase(
                RuntimeOrigin::signed(acc),
                pool_id,
                fy_token_in,
                Balance::MAX
            ),
            Error::<Test>::SellXBaseTooLowForMin
        );
    });
}

#[test]
fn donates_base_and_sells_fy_token() {
    new_test_ext().execute_with(|| {
        let fy_token_in = ONE_TOKEN;
        let base_donation = ONE_TOKEN;
        let acc = 1;
        let pool_id = setup_pool();
        let pool = init_liquidity_for_trade(pool_id);
        let pool_base_balance_before = pool.base_balance();
        let pool_xbase_balance_before = pool.xbase_balance();

        update_pool_balances(
            pool_id,
            Some(pool_base_balance_before + base_donation),
            None,
        );

        assert_ok!(Xdot::sell_xbase(
            RuntimeOrigin::signed(acc),
            pool_id,
            fy_token_in,
            0
        ));

        assert_eq!(
            pool.xbase_balance(),
            pool_xbase_balance_before + fy_token_in
        );
    });
}

#[test]
fn buys_base() {
    new_test_ext().execute_with(|| {
        let base_out = ONE_TOKEN;
        let acc = 1;
        let pool_id = setup_pool();
        let pool = init_liquidity_for_trade(pool_id);
        let pool_base_balance_before = pool.base_balance();
        let pool_xbase_balance_before = pool.xbase_balance();

        let fy_token_in_preview =
            YieldMath::<I64F64, yield_math::YieldConvert>::fy_token_in_for_base_out(
                BalanceConvert::convert(pool.base_balance()),
                BalanceConvert::convert(pool.virtual_xbase_balance().unwrap()),
                BalanceConvert::convert(base_out),
                yield_math::YieldConvert::convert(Xdot::time_till_maturity(pool.maturity).unwrap()),
                pool.ts,
                pool.g2,
            )
            .map(BalanceConvert::convert)
            .unwrap()
            .unwrap();

        let expected_fy_token_in = 1018141134; // 1.018141134;  in yield test 1.018142118489570119

        assert_ok!(Xdot::buy_base(
            RuntimeOrigin::signed(acc),
            pool_id,
            base_out,
            ONE_MILLION,
            Balance::MAX
        ));

        let pool_xbase_balance_current = pool.xbase_balance();
        let fy_token_in = pool_xbase_balance_current - pool_xbase_balance_before;

        assert_eq!(
            Balances::get_balance(&acc, &BASE_TOKEN),
            SignedBalance::Positive(base_out)
        );

        assert_eq!(
            Balances::get_balance(&acc, &X_BASE_TOKEN),
            SignedBalance::Negative(fy_token_in)
        );

        assert_eq!(fy_token_in, expected_fy_token_in);
        assert_eq!(fy_token_in_preview, expected_fy_token_in);
        assert_eq!(pool.base_balance(), pool_base_balance_before - base_out);

        assert_eq!(
            pool.xbase_balance(),
            pool_xbase_balance_before + fy_token_in
        );
    });
}

#[test]
fn does_not_buy_base_beyond_slippage() {
    new_test_ext().execute_with(|| {
        let base_out = ONE_TOKEN;
        let acc = 1;
        let pool_id = setup_pool();
        init_liquidity_for_trade(pool_id);

        assert_noop!(
            Xdot::buy_base(
                RuntimeOrigin::signed(acc),
                pool_id,
                base_out,
                ONE_MILLION,
                0
            ),
            Error::<Test>::BuyBaseTooMuchForMax
        );
    });
}

#[test]
fn donates_fy_token_and_buys_base() {
    new_test_ext().execute_with(|| {
        let base_out = ONE_TOKEN;
        let fy_token_donation = ONE_TOKEN;
        let acc = 1;
        let pool_id = setup_pool();
        let pool = init_liquidity_for_trade(pool_id);
        let pool_base_balance_before = pool.base_balance();
        let pool_xbase_balance_before = pool.xbase_balance();

        update_pool_balances(
            pool_id,
            None,
            Some(pool_xbase_balance_before + fy_token_donation),
        );

        assert_ok!(Xdot::buy_base(
            RuntimeOrigin::signed(acc),
            pool_id,
            base_out,
            ONE_MILLION,
            Balance::MAX
        ));

        assert_eq!(pool.base_balance(), pool_base_balance_before - base_out);
    });
}

#[test]
fn sells_base() {
    new_test_ext().execute_with(|| {
        let acc = 1;
        let pool_id = setup_pool();
        let pool = init_liquidity_for_trade_with_extra_fy_token(pool_id);
        let pool_base_balance_before = pool.base_balance();
        let pool_xbase_balance_before = pool.xbase_balance();
        let base_in = ONE_TOKEN;

        let virtual_xbase_balance = pool.virtual_xbase_balance().unwrap();
        let fy_token_out_preview = Xdot::sell_base_preview(
            pool.maturity,
            pool.base_balance(),
            virtual_xbase_balance,
            pool.ts,
            pool.g1,
            base_in,
        )
        .unwrap();
        let expected_fy_token_out = 1016359012; // 1.016359012; yield: 1.016357964987807283

        assert_ok!(Xdot::sell_base(
            RuntimeOrigin::signed(acc),
            pool_id,
            base_in,
            0
        ));

        let fy_token_out = Balances::get_balance(&acc, &X_BASE_TOKEN);

        assert_eq!(
            Balances::get_balance(&acc, &BASE_TOKEN),
            SignedBalance::Negative(base_in)
        );

        assert_eq!(fy_token_out, SignedBalance::Positive(expected_fy_token_out));

        assert_eq!(fy_token_out, SignedBalance::Positive(fy_token_out_preview));
        assert_eq!(pool.base_balance(), pool_base_balance_before + base_in);
        assert_eq!(
            pool.xbase_balance(),
            pool_xbase_balance_before - fy_token_out.abs()
        );
    });
}

#[test]
fn does_not_sell_base_beyond_slippage() {
    new_test_ext().execute_with(|| {
        let base_in = ONE_TOKEN;
        let acc = 1;
        let pool_id = setup_pool();
        init_liquidity_for_trade_with_extra_fy_token(pool_id);
        assert_noop!(
            Xdot::sell_base(RuntimeOrigin::signed(acc), pool_id, base_in, Balance::MAX),
            Error::<Test>::SellBaseTooLowForMin
        );
    });
}

#[test]
fn donates_fy_token_and_sells_base() {
    new_test_ext().execute_with(|| {
        let base_in = ONE_TOKEN;
        let fy_token_donation = ONE_TOKEN;
        let pool_id = setup_pool();
        let pool = init_liquidity_for_trade_with_extra_fy_token(pool_id);
        let acc = 1;
        let pool_xbase_balance_before = pool.xbase_balance();
        let pool_base_balance_before = pool.base_balance();

        update_pool_balances(
            pool_id,
            None,
            Some(pool_xbase_balance_before + fy_token_donation),
        );

        assert_ok!(Xdot::sell_base(
            RuntimeOrigin::signed(acc),
            pool_id,
            base_in,
            0
        ),);

        assert_eq!(pool.base_balance(), pool_base_balance_before + base_in);
    })
}

#[test]
fn buys_fy_token() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let pool = init_liquidity_for_trade_with_extra_fy_token(pool_id);
        let acc = 1;
        let pool_xbase_balance_before = pool.xbase_balance();
        let pool_base_balance_before = pool.base_balance();
        let fy_token_out = ONE_TOKEN;
        let virtual_xbase_balance = pool.virtual_xbase_balance().unwrap();
        let base_balance = pool.base_balance();

        let base_in_preview = Xdot::buy_xbase_preview(
            pool.maturity,
            base_balance,
            virtual_xbase_balance,
            pool.ts,
            pool.g1,
            fy_token_out,
        )
        .unwrap();
        let expected_base_in = 983904297; // 0.983904297; yield 0.983905317931461992

        assert_ok!(Xdot::buy_xbase(
            RuntimeOrigin::signed(acc),
            pool_id,
            ONE_MILLION,
            fy_token_out,
            Balance::MAX,
        ));

        let pool_base_balance_after = pool.base_balance();
        let base_in = pool_base_balance_after - pool_base_balance_before;

        assert_eq!(
            Balances::get_balance(&acc, &X_BASE_TOKEN),
            SignedBalance::Positive(fy_token_out)
        );

        assert_eq!(
            Balances::get_balance(&acc, &BASE_TOKEN),
            SignedBalance::Negative(base_in)
        );

        assert_eq!(base_in, expected_base_in);
        assert_eq!(base_in_preview, expected_base_in);
        assert_eq!(pool.base_balance(), pool_base_balance_before + base_in);
        assert_eq!(
            pool.xbase_balance(),
            pool_xbase_balance_before - fy_token_out
        );
    });
}

#[test]
fn does_not_buy_fy_token_beyond_slippage() {
    new_test_ext().execute_with(|| {
        let fy_token_out = ONE_TOKEN;
        let acc = 1;
        let pool_id = setup_pool();
        init_liquidity_for_trade_with_extra_fy_token(pool_id);
        assert_noop!(
            Xdot::buy_xbase(
                RuntimeOrigin::signed(acc),
                pool_id,
                ONE_MILLION,
                fy_token_out,
                0
            ),
            Error::<Test>::BuyXbaseTooMuchForMax
        );
    });
}

#[test]
fn donates_base_and_buys_fy_token() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let pool = init_liquidity_for_trade_with_extra_fy_token(pool_id);
        let acc = 1;
        let pool_xbase_balance_before = pool.xbase_balance();

        let fy_token_out = ONE_TOKEN;
        let base_donation = ONE_TOKEN;

        // await base.mint(pool.address, bases.add(base_donation))

        // await pool.buyFYToken(user2, fy_token_out, MAX, OVERRIDES)
        assert_ok!(Xdot::buy_xbase(
            RuntimeOrigin::signed(acc),
            pool_id,
            base_donation,
            fy_token_out,
            Balance::MAX,
        ));

        assert_eq!(
            pool.xbase_balance(),
            pool_xbase_balance_before - fy_token_out
        );
    });
}

#[test]
fn once_mature() {
    new_test_ext().execute_with(|| {
        let pool_id = setup_pool();
        let pool = init_liquidity_for_trade_with_extra_fy_token(pool_id);
        let acc = 1;
        Timestamp::set_timestamp((pool.maturity + 1) * 1000);
        let mut base_balance = Balances::get_balance(&acc, &pool.base_asset);
        let mut xbase_balance = Balances::get_balance(&acc, &pool.xbase_asset);

        assert_ok!(Xdot::sell_base(
            RuntimeOrigin::signed(acc),
            pool_id,
            ONE_TOKEN,
            0
        ));
        let base_balance_1 = Balances::get_balance(&acc, &pool.base_asset);
        let xbase_balance_1 = Balances::get_balance(&acc, &pool.xbase_asset);
        assert_eq!(
            base_balance,
            base_balance_1.clone().add_balance(&ONE_TOKEN).unwrap()
        );
        assert_eq!(
            xbase_balance,
            xbase_balance_1.clone().sub_balance(&ONE_TOKEN).unwrap()
        );
        base_balance = base_balance_1;
        xbase_balance = xbase_balance_1;

        assert_ok!(Xdot::buy_base(
            RuntimeOrigin::signed(acc),
            pool_id,
            ONE_TOKEN,
            ONE_MILLION,
            Balance::MAX
        ));
        let base_balance_2 = Balances::get_balance(&acc, &pool.base_asset);
        let xbase_balance_2 = Balances::get_balance(&acc, &pool.xbase_asset);
        assert_eq!(
            base_balance,
            base_balance_2.clone().sub_balance(&ONE_TOKEN).unwrap()
        );
        assert_eq!(
            xbase_balance,
            xbase_balance_2.clone().add_balance(&ONE_TOKEN).unwrap()
        );
        base_balance = base_balance_2;
        xbase_balance = xbase_balance_2;

        assert_ok!(Xdot::sell_xbase(
            RuntimeOrigin::signed(acc),
            pool_id,
            ONE_TOKEN,
            0
        ));
        let base_balance_3 = Balances::get_balance(&acc, &pool.base_asset);
        let xbase_balance_3 = Balances::get_balance(&acc, &pool.xbase_asset);
        assert_eq!(
            base_balance,
            base_balance_3.clone().sub_balance(&ONE_TOKEN).unwrap()
        );
        assert_eq!(
            xbase_balance,
            xbase_balance_3.clone().add_balance(&ONE_TOKEN).unwrap()
        );
        base_balance = base_balance_3;
        xbase_balance = xbase_balance_3;

        assert_ok!(Xdot::buy_xbase(
            RuntimeOrigin::signed(acc),
            pool_id,
            ONE_MILLION,
            ONE_TOKEN,
            Balance::MAX
        ));
        let base_balance_4 = Balances::get_balance(&acc, &pool.base_asset);
        let xbase_balance_4 = Balances::get_balance(&acc, &pool.xbase_asset);
        assert_eq!(
            base_balance,
            base_balance_4.add_balance(&ONE_TOKEN).unwrap()
        );
        assert_eq!(
            xbase_balance,
            xbase_balance_4.sub_balance(&ONE_TOKEN).unwrap()
        );
    });
}

fn create_and_init_pool_for_max_values_check(
    creator: u64,
    pool_base_init: Balance,
    pool_xbase_init: Balance,
    maturity: u64,
    ts_period: I64F64,
    g1: I64F64,
    g2: I64F64,
) -> PoolId {
    assert_ok!(Xdot::create_pool(
        RuntimeOrigin::root(),
        creator,
        BASE_TOKEN,
        X_BASE_TOKEN,
        g1,
        g2,
        maturity,
        ts_period,
    ));
    let pool_id = Xdot::pool_count() - 1;

    assert_ok!(Xdot::initialize(
        RuntimeOrigin::signed(creator),
        pool_id,
        pool_base_init,
        pool_xbase_init,
    ));

    pool_id
}

// just to see what's happening
#[test]
fn max_base_out_calculation() {
    new_test_ext().execute_with(|| {
        let creator = 1;
        let pool_base_init = 10_000 * ONE_TOKEN;
        let pool_xbase_init = pool_base_init / 3;
        let g1 = I64F64::from_num(0.95);
        let g2 = I64F64::from_num(1) / g1;
        let ts_period = I64F64::from_num(2 * 365 * 24 * 60 * 60);
        let ts = I64F64::from_num(1) / ts_period; // 1 / Seconds in 2 years
        let now = eq_rate::Pallet::<Test>::now().as_secs();
        let maturity = now + 672 * 24 * 60 * 60; // 672 days

        let pool_id = create_and_init_pool_for_max_values_check(
            creator,
            pool_base_init,
            pool_xbase_init,
            maturity,
            ts_period,
            g1,
            g2,
        );
        let pool_on_init = pool(pool_id);

        let mut base_to_buy = pool_base_init;
        let xbase_to_sell = 999_999_999 * ONE_TOKEN;
        let trader = 2;

        let ttm_num = I64F64::from_num(672 * 24 * 60 * 60);
        let pool_base_init_num = BalanceConvert::convert(pool_base_init);
        let pool_virt_xbase_init_num =
            BalanceConvert::convert(pool_on_init.virtual_xbase_balance().unwrap());
        let mut base_to_buy_num = BalanceConvert::convert(base_to_buy);

        println!("test ttm {:?}", ttm_num);
        println!("test pool_base_init {:?}", pool_base_init_num);
        println!("test pool_virt_xbase_init {:?}", pool_virt_xbase_init_num);
        println!("test base_to_buy {:?}", base_to_buy_num);
        println!("test ts {:?}", ts);
        println!("test g2 {:?}", g2);

        let mut result_math =
            YieldMath::<I64F64, yield_math::YieldConvert>::fy_token_in_for_base_out(
                pool_base_init_num,
                pool_virt_xbase_init_num,
                base_to_buy_num,
                ttm_num,
                ts,
                g2,
            );

        println!(
            "base_to_buy_num {:?} fy_token_in_for_base_out {:?}",
            base_to_buy_num, result_math
        );

        let mut result_ex = Xdot::buy_base(
            RuntimeOrigin::signed(trader),
            pool_id,
            base_to_buy,
            xbase_to_sell,
            xbase_to_sell,
        );

        while result_ex.is_err() {
            result_math = YieldMath::<I64F64, yield_math::YieldConvert>::fy_token_in_for_base_out(
                pool_base_init_num,
                pool_virt_xbase_init_num,
                base_to_buy_num,
                ttm_num,
                ts,
                g2,
            );
            result_ex = Xdot::buy_base(
                RuntimeOrigin::signed(trader),
                pool_id,
                base_to_buy,
                xbase_to_sell,
                xbase_to_sell,
            );

            base_to_buy = base_to_buy - ONE_TOKEN;
            base_to_buy_num = BalanceConvert::convert(base_to_buy);
        }

        println!("test ttm {:?}", ttm_num);
        println!("test pool_base_init {:?}", pool_base_init_num);
        println!("test pool_virt_xbase_init {:?}", pool_virt_xbase_init_num);
        println!("test base_to_buy {:?}", base_to_buy_num);
        println!("test ts {:?}", ts);
        println!("test g2 {:?}", g2);

        // 59234681223530.34273551300033975567

        println!("MAX base_to_buy {:?}", base_to_buy);
        println!("xbase in        {:?}", result_math);

        // panic!();
    });
}

#[test]
fn max_xbase_out_test() {
    new_test_ext().execute_with(|| {
        let creator = 1;
        let pool_base_init = 10_000 * ONE_TOKEN;
        let pool_xbase_init = pool_base_init / 3;
        let g1 = I64F64::from_num(95) / I64F64::from_num(100);
        let g2 = I64F64::from_num(1) / g1;
        let ts_period = I64F64::from_num(2 * 365 * 24 * 60 * 60);
        let ts = I64F64::from_num(1) / ts_period; // 1 / Seconds in 2 years
        let now = eq_rate::Pallet::<Test>::now().as_secs();
        let maturity = now + 672 * 24 * 60 * 60; // 672 days

        let pool_id = create_and_init_pool_for_max_values_check(
            creator,
            pool_base_init,
            pool_xbase_init,
            maturity,
            ts_period,
            g1,
            g2,
        );
        let pool_on_init = pool(pool_id);

        let base_to_sell = 999_999_999 * ONE_TOKEN;
        let trader = 2;

        let ttm_num = I64F64::from_num(672 * 24 * 60 * 60);
        let pool_base_init_num = BalanceConvert::convert(pool_base_init);
        let pool_virt_xbase_init_num =
            BalanceConvert::convert(pool_on_init.virtual_xbase_balance().unwrap());

        println!("ttm {:?}", ttm_num);
        println!("pool_base {:?}", pool_base_init);
        println!(
            "pool_virt_xbase {:?}",
            pool_on_init.virtual_xbase_balance().unwrap()
        );
        println!("pool_xbase {:?}", pool_on_init.xbase_balance());
        println!("ts {:?}", ts);
        println!("g1 {:?}", g1);
        println!(
            "alpha {:?}\n",
            YieldMath::<I64F64, yield_math::YieldConvert>::compute_a(ttm_num, ts, g1, false)
        );

        // On init
        println!("On init");

        let max_fy_token_out_num = YieldMath::<I64F64, yield_math::YieldConvert>::max_fy_token_out(
            pool_base_init_num,
            pool_virt_xbase_init_num,
            ttm_num,
            ts,
            g1,
        )
        .unwrap();

        let max_fy_token_out = BalanceConvert::convert(max_fy_token_out_num).unwrap();
        println!(
            "max_fy_token_out {:?} ({:?})\n",
            max_fy_token_out_num, max_fy_token_out
        );

        assert_err!(
            Xdot::buy_xbase(
                RuntimeOrigin::signed(trader),
                pool_id,
                base_to_sell,
                max_fy_token_out + 1,
                base_to_sell,
            ),
            Error::<Test>::XbaseBalanceTooLow
        );

        assert_ok!(Xdot::buy_xbase(
            RuntimeOrigin::signed(trader),
            pool_id,
            base_to_sell,
            max_fy_token_out,
            base_to_sell,
        ));

        // "Clear" state
        Balances::make_free_balance_be(
            &pool_on_init.account,
            BASE_TOKEN,
            SignedBalance::Positive(pool_base_init),
        );
        Balances::make_free_balance_be(
            &pool_on_init.account,
            X_BASE_TOKEN,
            SignedBalance::Positive(pool_xbase_init),
        );

        // 1000 trades
        let trade_amount = 100 * ONE_TOKEN;

        println!("Buy and sell 100 xDot, 1000 times");

        for _ in 0..1000 {
            assert_ok!(Xdot::buy_xbase(
                RuntimeOrigin::signed(trader),
                pool_id,
                base_to_sell,
                trade_amount,
                base_to_sell,
            ));

            assert_ok!(Xdot::sell_xbase(
                RuntimeOrigin::signed(trader),
                pool_id,
                trade_amount,
                0
            ));
        }

        let current_pool = pool(pool_id);

        let base_before = current_pool.base_balance();
        let xbase_before = current_pool.xbase_balance();

        println!("pool_base {:?}", base_before);
        println!(
            "pool_virt_xbase {:?}",
            current_pool.virtual_xbase_balance().unwrap()
        );
        println!("pool_xbase {:?}", current_pool.xbase_balance());

        let max_fy_token_out_num = YieldMath::<I64F64, yield_math::YieldConvert>::max_fy_token_out(
            BalanceConvert::convert(current_pool.base_balance()),
            BalanceConvert::convert(current_pool.virtual_xbase_balance().unwrap()),
            ttm_num,
            ts,
            g1,
        )
        .unwrap();

        let max_fy_token_out = BalanceConvert::convert(max_fy_token_out_num).unwrap();
        println!(
            "max_fy_token_out {:?} ({:?})\n",
            max_fy_token_out_num, max_fy_token_out
        );

        assert_err!(
            Xdot::buy_xbase(
                RuntimeOrigin::signed(trader),
                pool_id,
                base_to_sell,
                max_fy_token_out + 2,
                base_to_sell,
            ),
            Error::<Test>::XbaseBalanceTooLow
        );

        assert_ok!(Xdot::buy_xbase(
            RuntimeOrigin::signed(trader),
            pool_id,
            base_to_sell,
            max_fy_token_out,
            base_to_sell,
        ));

        // "Clear" state
        Balances::make_free_balance_be(
            &pool_on_init.account,
            BASE_TOKEN,
            SignedBalance::Positive(base_before),
        );
        Balances::make_free_balance_be(
            &pool_on_init.account,
            X_BASE_TOKEN,
            SignedBalance::Positive(xbase_before),
        );

        // wait half of maturity
        println!("Wait half of maturity");
        let now = eq_rate::Pallet::<Test>::now().as_secs();
        let half_of_maturity = now + maturity / 2;
        Timestamp::set_timestamp(half_of_maturity * 1000);
        let ttm_num = ttm_num / yield_math::YieldConvert::convert(2);

        println!("ttm {:?}", ttm_num);

        let max_fy_token_out_num = YieldMath::<I64F64, yield_math::YieldConvert>::max_fy_token_out(
            BalanceConvert::convert(current_pool.base_balance()),
            BalanceConvert::convert(current_pool.virtual_xbase_balance().unwrap()),
            ttm_num,
            ts,
            g1,
        )
        .unwrap();

        let max_fy_token_out = BalanceConvert::convert(max_fy_token_out_num).unwrap();
        println!(
            "max_fy_token_out {:?} ({:?})\n",
            max_fy_token_out_num, max_fy_token_out
        );

        assert_err!(
            Xdot::buy_xbase(
                RuntimeOrigin::signed(trader),
                pool_id,
                base_to_sell,
                max_fy_token_out + 1,
                base_to_sell,
            ),
            Error::<Test>::XbaseBalanceTooLow
        );

        assert_ok!(Xdot::buy_xbase(
            RuntimeOrigin::signed(trader),
            pool_id,
            base_to_sell,
            max_fy_token_out,
            base_to_sell,
        ));

        // "Clear" state
        Balances::make_free_balance_be(
            &pool_on_init.account,
            BASE_TOKEN,
            SignedBalance::Positive(base_before),
        );
        Balances::make_free_balance_be(
            &pool_on_init.account,
            X_BASE_TOKEN,
            SignedBalance::Positive(xbase_before),
        );

        // 1000 trades
        println!("Buy and sell 100 xDot, 1000 times");

        for _ in 0..1000 {
            assert_ok!(Xdot::buy_xbase(
                RuntimeOrigin::signed(trader),
                pool_id,
                base_to_sell,
                trade_amount,
                base_to_sell,
            ));

            assert_ok!(Xdot::sell_xbase(
                RuntimeOrigin::signed(trader),
                pool_id,
                trade_amount,
                0
            ));
        }

        let current_pool = pool(pool_id);

        let base_before = current_pool.base_balance();

        println!("pool_base {:?}", base_before);
        println!(
            "pool_virt_xbase {:?}",
            current_pool.virtual_xbase_balance().unwrap()
        );
        println!("pool_xbase {:?}", current_pool.xbase_balance());

        let max_fy_token_out_num = YieldMath::<I64F64, yield_math::YieldConvert>::max_fy_token_out(
            BalanceConvert::convert(current_pool.base_balance()),
            BalanceConvert::convert(current_pool.virtual_xbase_balance().unwrap()),
            ttm_num,
            ts,
            g1,
        )
        .unwrap();

        let max_fy_token_out = BalanceConvert::convert(max_fy_token_out_num).unwrap();
        println!(
            "max_fy_token_out {:?} ({:?})",
            max_fy_token_out_num, max_fy_token_out
        );

        assert_err!(
            Xdot::buy_xbase(
                RuntimeOrigin::signed(trader),
                pool_id,
                base_to_sell,
                max_fy_token_out + 1,
                base_to_sell,
            ),
            Error::<Test>::XbaseBalanceTooLow
        );

        assert_ok!(Xdot::buy_xbase(
            RuntimeOrigin::signed(trader),
            pool_id,
            base_to_sell,
            max_fy_token_out,
            base_to_sell,
        ));

        // panic!("SUCCESS");
    });
}

#[test]
fn optimal_mint_calc() {
    new_test_ext().execute_with(|| {
        let base_balances = [
            ONE_TOKEN * 10_000,
            ONE_TOKEN * 100_000,
            ONE_TOKEN * 1_000_000,
        ];
        let g1 = I64F64::from_num(95) / I64F64::from_num(100);
        let ts_period = I64F64::from_num(2 * 365 * 24 * 60 * 60);
        let ts = I64F64::from_num(1) / ts_period; // 1 / Seconds in 2 years
        let now = eq_rate::Pallet::<Test>::now().as_secs();
        let maturity = now + 672 * 24 * 60 * 60; // 672 days

        for base_balance in base_balances {
            for xbase_fraction in [10, 100, 1_000] {
                let xbase_balance = base_balance / xbase_fraction;
                let lp_supply = base_balance;
                let virtual_xbase_balance = xbase_balance + lp_supply;
                let lp_supply_num = BalanceConvert::convert(lp_supply);

                let base_balance_num = BalanceConvert::convert(base_balance);
                let xbase_balance_num = BalanceConvert::convert(xbase_balance);

                println!(
                    "-------------\nbase / xbase: {:?}/{:?}  {:?}\n-------------\n",
                    base_balance_num, xbase_balance_num, xbase_fraction
                );
                'outer: for base_in in [
                    1,
                    ONE_TOKEN / 100_000_000,
                    ONE_TOKEN / 10_000_000,
                    ONE_TOKEN / 100_000_000,
                    ONE_TOKEN / 10_000_000,
                    ONE_TOKEN / 1_000_000,
                    ONE_TOKEN / 100_000,
                    ONE_TOKEN / 10_000,
                    ONE_TOKEN / 1000,
                    ONE_TOKEN / 100,
                    ONE_TOKEN / 10,
                    ONE_TOKEN * 10,
                    ONE_TOKEN * 100,
                    base_balance / 100,
                    base_balance / 10,
                    base_balance / 2,
                ] {
                    let base_in_num = BalanceConvert::convert(base_in);
                    println!("base_in {:?} ({:?})", base_in_num, base_in);

                    let mint = |base_to_sell: Balance| -> Result<(I64F64, I64F64), ()> {
                        let base_to_sell_num = BalanceConvert::convert(base_to_sell);
                        let xbase_to_buy_num = xbase_balance_num * (base_in_num - base_to_sell_num)
                            / (base_balance_num + base_in_num);
                        let xbase_to_buy = BalanceConvert::convert(xbase_to_buy_num).unwrap();
                        println!("xbase_to_buy {:?}", xbase_to_buy);
                        let actual_base_to_sell = Xdot::buy_xbase_preview(
                            maturity,
                            base_balance,
                            virtual_xbase_balance,
                            ts,
                            g1,
                            xbase_to_buy,
                        )
                        .map_err(|_| ())?;

                        let actual_base_to_sell_num = BalanceConvert::convert(actual_base_to_sell);

                        let tokens_minted_num = lp_supply_num
                            .checked_mul(xbase_to_buy_num)
                            .unwrap()
                            .checked_div(xbase_balance_num.checked_sub(xbase_to_buy_num).unwrap())
                            .unwrap();

                        let actual_base_in = actual_base_to_sell_num
                            .checked_add(
                                base_balance_num
                                    .checked_add(actual_base_to_sell_num)
                                    .unwrap()
                                    .checked_mul(tokens_minted_num)
                                    .unwrap()
                                    .checked_div(lp_supply_num)
                                    .unwrap(),
                            )
                            .unwrap();

                        Ok((tokens_minted_num, actual_base_in))
                    };
                    let mut left = 0;
                    let mut right = base_in;
                    let mut _max = (I64F64::from_num(0i32), I64F64::from_num(0i32));
                    let mut i = 0;
                    loop {
                        let m1 = left + (right - left) / 5;
                        let m2 = right - (right - left) / 5;
                        let mint_m1 = mint(m1);
                        let mint_m2 = mint(m2);

                        if mint_m1.is_err() || mint_m2.is_err() {
                            println!("convert res from base_in_for_fy_token_out error");
                            continue 'outer;
                        }
                        let mint_m1 = mint_m1.unwrap();
                        let mint_m2 = mint_m2.unwrap();

                        _max = if mint_m1.1 > mint_m2.1 {
                            mint_m1
                        } else {
                            mint_m2
                        };
                        println!("m1 {:?} {:?}", BalanceConvert::convert(m1), mint_m1);
                        println!("m2 {:?} {:?}", BalanceConvert::convert(m2), mint_m2);
                        i += 1;
                        if base_in_num - _max.1
                            < BalanceConvert::convert(base_in) / I64F64::from_num(5)
                        {
                            break;
                        }
                        if mint_m1.0 < mint_m2.0 {
                            left = m1;
                        } else {
                            right = m2;
                        }
                    }
                    // println!("left {:?}", mint(left));
                    // println!("right {:?}", mint(right));
                    println!("{:?}: _max {:?}\n", i, _max);
                }
            }
        }
    });
}

#[test]
fn init_for_prod() {
    new_test_ext().execute_with(|| {
        let ts_period = 43804800;
        let ts_period = I64F64::from_num(ts_period) / I64F64::from_num(0.95) + I64F64::from_num(1);
        let ts = I64F64::from_num(1) / ts_period;

        let creator = 1;

        let g1 = I64F64::from_num(95) / I64F64::from_num(100);
        let g2 = I64F64::from_num(100) / I64F64::from_num(95);
        let maturity: u64 = 1698036889;

        println!("g1              {:?} | {:?}", g1, g1.to_bits());
        println!("g2              {:?} | {:?}", g2, g2.to_bits());
        println!("maturity        {:?}", maturity);
        println!("ts              {:?} | {:?}", ts, ts.to_bits());
        println!(
            "ts_period       {:?} | {:?}",
            ts_period,
            ts_period.to_bits()
        );

        assert_ok!(Xdot::create_pool(
            RuntimeOrigin::root(),
            creator,
            BASE_TOKEN,
            X_BASE_TOKEN,
            g1,
            g2,
            maturity,
            ts_period,
        ));

        assert_ok!(Xdot::initialize(
            RuntimeOrigin::signed(creator),
            0,
            50000_000000000,
            9853_000000000,
        ));
    });
}
