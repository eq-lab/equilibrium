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

use super::{Config, Error, ValidityError};
use crate::mock::{
    new_test_ext, AccountId, Balance, DummyValidatorId, ModuleAggregates, ModuleBalances,
    ModuleTreasury, OracleMock, RuntimeCall, RuntimeOrigin, Test, TimeMock,
};
use crate::{Amount, BuyoutLimit, Buyouts, CheckBuyout};
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::{
    asset,
    asset::Asset,
    balance::{BalanceGetter, EqCurrency},
    eqfxu128,
    price::PriceSetter,
    Aggregates, SignedBalance, UserGroup,
};
use eq_utils::{fixed::fixedi64_from_eq_fixedu128, ONE_TOKEN};
use frame_support::dispatch::DispatchInfo;
use frame_support::traits::UnixTime;
use frame_support::weights::Weight;
use frame_support::{assert_err, assert_noop, assert_ok, assert_storage_noop};
use sp_arithmetic::FixedPointNumber;
use sp_runtime::traits::SignedExtension;
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidityError};

fn set_price(asset: &Asset, price: &EqFixedU128) {
    assert_ok!(OracleMock::set_price(
        0,
        *asset,
        fixedi64_from_eq_fixedu128(*price).expect("Positive price")
    ));
}

fn set_pos_balance_with_agg_unsafe(who: &DummyValidatorId, asset: &Asset, amount: EqFixedU128) {
    let balance = SignedBalance::Positive(amount.into_inner());
    ModuleBalances::make_free_balance_be(who, *asset, balance);
}

#[test]
fn eq_not_enough_colat() {
    new_test_ext().execute_with(|| {
        let module_account_id = ModuleTreasury::account_id();
        let account_id = 1;

        set_price(&asset::EQ, &eqfxu128!(25, 0));
        set_price(&asset::BTC, &eqfxu128!(10_000, 0));
        set_price(&asset::ETH, &eqfxu128!(250, 0));
        set_price(&asset::EOS, &eqfxu128!(3, 0));
        set_price(&asset::DOT, &eqfxu128!(17, 0));

        set_pos_balance_with_agg_unsafe(&module_account_id, &asset::EQ, eqfxu128!(1000, 0));

        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQ, eqfxu128!(20, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::BTC, eqfxu128!(0, 5));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::ETH, eqfxu128!(10, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EOS, eqfxu128!(1000, 0));
        assert_ok!(
            <ModuleTreasury as super::EqBuyout<AccountId, Balance>>::eq_buyout(
                &account_id,
                eqfxu128!(420, 0).into_inner(),
            )
        );
        // account balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(440, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );

        // treasury balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(580, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 5).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(10, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(1000, 0).into_inner())
        );
    });
}

#[test]
fn eq_balances_btc_eth_eos() {
    new_test_ext().execute_with(|| {
        let module_account_id = ModuleTreasury::account_id();
        let account_id = 1;

        set_price(&asset::EQ, &eqfxu128!(25, 0));
        set_price(&asset::BTC, &eqfxu128!(10_000, 0));
        set_price(&asset::ETH, &eqfxu128!(250, 0));
        set_price(&asset::EOS, &eqfxu128!(3, 0));
        set_price(&asset::DOT, &eqfxu128!(17, 0));

        set_pos_balance_with_agg_unsafe(&module_account_id, &asset::EQ, eqfxu128!(1000, 0));

        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQ, eqfxu128!(20, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::BTC, eqfxu128!(0, 5));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::ETH, eqfxu128!(10, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EOS, eqfxu128!(1000, 0));
        assert_ok!(
            <ModuleTreasury as super::EqBuyout<AccountId, Balance>>::eq_buyout(
                &account_id,
                eqfxu128!(380, 0).into_inner(),
            )
        );
        // account balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(400, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(16, 666_666_665).into_inner())
        );

        // treasury balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(620, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 5).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(10, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(983, 333_333_335).into_inner())
        );
    });
}

#[test]
fn eq_balances_btc_eth() {
    new_test_ext().execute_with(|| {
        let module_account_id = ModuleTreasury::account_id();
        let account_id = 1;

        set_price(&asset::EQ, &eqfxu128!(25, 0));
        set_price(&asset::BTC, &eqfxu128!(10_000, 0));
        set_price(&asset::ETH, &eqfxu128!(250, 0));
        set_price(&asset::EOS, &eqfxu128!(3, 0));
        set_price(&asset::DOT, &eqfxu128!(17, 0));

        set_pos_balance_with_agg_unsafe(&module_account_id, &asset::EQ, eqfxu128!(1000, 0));

        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQ, eqfxu128!(20, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::BTC, eqfxu128!(0, 5));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::ETH, eqfxu128!(10, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EOS, eqfxu128!(1000, 0));
        assert_ok!(
            <ModuleTreasury as super::EqBuyout<AccountId, Balance>>::eq_buyout(
                &account_id,
                eqfxu128!(260, 0).into_inner(),
            )
        );
        // account balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(280, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(1, 4).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(1000, 0).into_inner())
        );

        // treasury balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(740, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 5).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(8, 6).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
    });
}

#[test]
fn eq_balances_btc() {
    new_test_ext().execute_with(|| {
        let module_account_id = ModuleTreasury::account_id();
        let account_id = 1;

        set_price(&asset::EQ, &eqfxu128!(25, 0));
        set_price(&asset::BTC, &eqfxu128!(10_000, 0));
        set_price(&asset::ETH, &eqfxu128!(250, 0));
        set_price(&asset::EOS, &eqfxu128!(3, 0));
        set_price(&asset::DOT, &eqfxu128!(17, 0));

        set_pos_balance_with_agg_unsafe(&module_account_id, &asset::EQ, eqfxu128!(1000, 0));

        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQ, eqfxu128!(20, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::BTC, eqfxu128!(0, 5));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::ETH, eqfxu128!(10, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EOS, eqfxu128!(1000, 0));
        assert_ok!(
            <ModuleTreasury as super::EqBuyout<AccountId, Balance>>::eq_buyout(
                &account_id,
                eqfxu128!(140, 0).into_inner(),
            )
        );
        // account balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(160, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 115).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(10, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(1000, 0).into_inner())
        );

        // treasury balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(860, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 385).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
    });
}

#[test]
fn eq_no_eq() {
    new_test_ext().execute_with(|| {
        let module_account_id = ModuleTreasury::account_id();
        let account_id = 1;

        set_price(&asset::EQ, &eqfxu128!(25, 0));
        set_price(&asset::BTC, &eqfxu128!(10_000, 0));
        set_price(&asset::ETH, &eqfxu128!(250, 0));
        set_price(&asset::EOS, &eqfxu128!(3, 0));
        set_price(&asset::DOT, &eqfxu128!(17, 0));

        set_pos_balance_with_agg_unsafe(&module_account_id, &asset::EQ, eqfxu128!(100, 0));

        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQ, eqfxu128!(20, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::BTC, eqfxu128!(0, 5));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::ETH, eqfxu128!(10, 0));
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EOS, eqfxu128!(1000, 0));

        // eq total supply is acc + module balances
        assert_eq!(
            ModuleAggregates::get_total(UserGroup::Balances, asset::EQ).collateral,
            eqfxu128!(120, 0).into_inner()
        );

        assert_noop!(
            <ModuleTreasury as super::EqBuyout<AccountId, Balance>>::eq_buyout(
                &account_id,
                eqfxu128!(140, 0).into_inner()
            ),
            Error::<Test>::InsufficientTreasuryBalance
        );
        // account balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(20, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 5).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(10, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(1000, 0).into_inner())
        );

        // treasury balances

        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EQ),
            SignedBalance::Positive(eqfxu128!(100, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::BTC),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::ETH),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
        assert_eq!(
            <Test as Config>::BalanceGetter::get_balance(&module_account_id, &asset::EOS),
            SignedBalance::Positive(eqfxu128!(0, 0).into_inner())
        );
    });
}

#[test]
fn calc_amount_to_exchange_should_fail_when_native_tokens() {
    assert_err!(
        ModuleTreasury::calc_amount_to_exchange(asset::EQ, 1),
        Error::<Test>::WrongAssetToBuyout
    );
}

#[test]
fn calc_amount_to_exchange_works() {
    new_test_ext().execute_with(|| {
        // dot_amount = buyout_amount * (eq_price + fee) / dot_price
        // = 100 * (1 + 1*0.1) / 17 = 6.470588235

        assert_ok!(
            ModuleTreasury::calc_amount_to_exchange(asset::DOT, 100_000_000_000),
            6_470_588_235
        );
    });
}

#[test]
fn calc_buouyt_amount_works() {
    new_test_ext().execute_with(|| {
        // buyout_amount = dot_amount * dot_price / (eq_price + fee)
        // = 100 * 17 / (1 + 1*0.1) = 1545.45455

        assert_ok!(
            ModuleTreasury::calc_buyout_amount(asset::DOT, 100_000_000_000),
            1545_454_545_454
        );
    });
}

#[test]
fn ensure_buyout_not_exceeded_works() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        //without limit
        assert_ok!(ModuleTreasury::ensure_buyout_limit_not_exceeded(
            &account_id,
            u64::MAX.into()
        ));

        //with limit
        BuyoutLimit::<Test>::put(100);
        assert_ok!(ModuleTreasury::ensure_buyout_limit_not_exceeded(
            &account_id,
            100
        ));
        assert_err!(
            ModuleTreasury::ensure_buyout_limit_not_exceeded(&account_id, 101),
            Error::<Test>::BuyoutLimitExceeded
        );
        let now = TimeMock::now().as_secs();

        //with limit and buyouts of prev periods
        BuyoutLimit::<Test>::put(100);
        Buyouts::<Test>::insert(account_id, (100, 0));
        assert_ok!(ModuleTreasury::ensure_buyout_limit_not_exceeded(
            &account_id,
            100
        ));
        assert_eq!(Buyouts::<Test>::get(account_id), (0, now));

        //with limit and buyouts of current period
        BuyoutLimit::<Test>::put(100);
        Buyouts::<Test>::insert(account_id, (80, now));
        assert_ok!(ModuleTreasury::ensure_buyout_limit_not_exceeded(
            &account_id,
            20
        ));
        assert_err!(
            ModuleTreasury::ensure_buyout_limit_not_exceeded(&account_id, 21),
            Error::<Test>::BuyoutLimitExceeded
        );
    });
}

#[test]
fn update_buyouts_works() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        let buyout_amount = 100u128;
        let now = TimeMock::now().as_secs();

        //without limit
        assert_storage_noop!(ModuleTreasury::update_buyouts(&account_id, buyout_amount));

        //with limit
        BuyoutLimit::<Test>::put(100);
        ModuleTreasury::update_buyouts(&account_id, buyout_amount);
        assert_eq!(Buyouts::<Test>::get(account_id), (buyout_amount, now));

        //with limit and with existed buyout
        BuyoutLimit::<Test>::put(100);
        Buyouts::<Test>::insert(&account_id, (50, now));
        ModuleTreasury::update_buyouts(&account_id, buyout_amount);
        assert_eq!(Buyouts::<Test>::get(account_id), (150, now));
    });
}

#[test]
fn buyout_works() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        let initial_eth_balance = 5 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::ETH,
            SignedBalance::Positive(initial_eth_balance),
        );
        ModuleBalances::make_free_balance_be(
            &ModuleTreasury::account_id(),
            asset::EQ,
            SignedBalance::Positive(10_000 * ONE_TOKEN),
        );

        let buyout_amount = 1000 * ONE_TOKEN;
        let exchange_amount =
            ModuleTreasury::calc_amount_to_exchange(asset::ETH, buyout_amount).unwrap();

        assert_ok!(ModuleTreasury::buyout(
            RuntimeOrigin::signed(account_id),
            asset::ETH,
            Amount::Buyout(buyout_amount)
        ));

        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::ETH),
            SignedBalance::Positive(initial_eth_balance - exchange_amount)
        );
        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::EQ),
            SignedBalance::Positive(buyout_amount)
        );
    });
}

#[test]
fn buyout_with_limit() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        let now = TimeMock::now().as_secs();

        let initial_eth_balance = 5 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::ETH,
            SignedBalance::Positive(initial_eth_balance),
        );
        ModuleBalances::make_free_balance_be(
            &ModuleTreasury::account_id(),
            asset::EQ,
            SignedBalance::Positive(10_000 * ONE_TOKEN),
        );

        // prev buyout was 1000 EQ
        BuyoutLimit::<Test>::put(1000 * ONE_TOKEN);
        Buyouts::<Test>::insert(&account_id, (1000 * ONE_TOKEN, 0));

        let buyout_amount = 1000 * ONE_TOKEN;
        let exchange_amount =
            ModuleTreasury::calc_amount_to_exchange(asset::ETH, buyout_amount).unwrap();

        assert_ok!(ModuleTreasury::buyout(
            RuntimeOrigin::signed(account_id),
            asset::ETH,
            Amount::Buyout(buyout_amount)
        ));

        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::ETH),
            SignedBalance::Positive(initial_eth_balance - exchange_amount)
        );
        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::EQ),
            SignedBalance::Positive(buyout_amount)
        );
        assert_eq!(Buyouts::<Test>::get(&account_id), (1000 * ONE_TOKEN, now));
    });
}

#[test]
fn buyout_with_exchange_amount_works() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        let initial_eth_balance = 5 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::ETH,
            SignedBalance::Positive(initial_eth_balance),
        );
        ModuleBalances::make_free_balance_be(
            &ModuleTreasury::account_id(),
            asset::EQ,
            SignedBalance::Positive(10_000 * ONE_TOKEN),
        );

        let exchange_amount = 4 * ONE_TOKEN;
        let buyout_amount =
            ModuleTreasury::calc_buyout_amount(asset::ETH, exchange_amount).unwrap();

        assert_ok!(ModuleTreasury::buyout(
            RuntimeOrigin::signed(account_id),
            asset::ETH,
            Amount::Exchange(exchange_amount)
        ));

        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::ETH),
            SignedBalance::Positive(initial_eth_balance - exchange_amount)
        );
        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::EQ),
            SignedBalance::Positive(buyout_amount)
        );
    });
}

mod signed_extension {
    use super::*;

    pub fn info_from_weight(w: Weight) -> DispatchInfo {
        DispatchInfo {
            weight: w,
            ..Default::default()
        }
    }

    #[test]
    fn validate_should_skip_other_calls() {
        new_test_ext().execute_with(|| {
            let buyout_call =
                RuntimeCall::EqTreasury(crate::Call::update_buyout_limit { limit: None });

            let check = CheckBuyout::<Test>::new();
            let info = info_from_weight(Weight::zero());
            assert_ok!(check.validate(&1, &buyout_call, &info, 0));
        });
    }

    #[test]
    fn validate_should_fail_when_wrong_asset() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;

            // call with wrong Asset
            for asset in [asset::EQ, asset::GENS] {
                let buyout_call = RuntimeCall::EqTreasury(crate::Call::buyout {
                    asset: asset,
                    amount: Amount::Buyout(100 * ONE_TOKEN),
                });
    
                let check = CheckBuyout::<Test>::new();
                let info = info_from_weight(Weight::zero());
    
                assert_err!(
                    check.validate(&account_id, &buyout_call, &info, 1),
                    TransactionValidityError::Invalid(InvalidTransaction::Custom(
                        ValidityError::WrongAssetToBuyout.into()
                    ))
                );
            }
        });
    }

    #[test]
    fn validate_should_fail_when_no_price_found() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;
            let buyout_call = RuntimeCall::EqTreasury(crate::Call::buyout {
                asset: asset::DAI,
                amount: Amount::Buyout(100 * ONE_TOKEN),
            });

            let check = CheckBuyout::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &buyout_call, &info, 1),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    ValidityError::Math.into()
                ))
            );
        });
    }

    #[test]
    fn validate_should_fail_when_not_enough_to_buyout() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;
            let buyout_call = RuntimeCall::EqTreasury(crate::Call::buyout {
                asset: asset::DOT,
                amount: Amount::Buyout(100 * ONE_TOKEN),
            });

            let check = CheckBuyout::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &buyout_call, &info, 1),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    ValidityError::NotEnoughToBuyout.into()
                ))
            );
        });
    }

    #[test]
    fn validate_should_fail_when_limit_exceeded() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;
            let buyout_call = RuntimeCall::EqTreasury(crate::Call::buyout {
                asset: asset::DOT,
                amount: Amount::Buyout(100 * ONE_TOKEN),
            });

            let now = TimeMock::now().as_secs();
            BuyoutLimit::<Test>::put(100 * ONE_TOKEN);
            Buyouts::<Test>::insert(&account_id, (80 * ONE_TOKEN, now));

            ModuleBalances::make_free_balance_be(
                &account_id,
                asset::DOT,
                SignedBalance::Positive(10 * ONE_TOKEN),
            );

            let check = CheckBuyout::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &buyout_call, &info, 1),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    ValidityError::BuyoutLimitExceeded.into()
                ))
            );
        });
    }

    #[test]
    fn validate_should_fail_when_less_than_min_amount_to_buyout() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;
            let buyout_call = RuntimeCall::EqTreasury(crate::Call::buyout {
                asset: asset::DOT,
                amount: Amount::Buyout(100 * ONE_TOKEN - 1),
            });

            ModuleBalances::make_free_balance_be(
                &account_id,
                asset::DOT,
                SignedBalance::Positive(10 * ONE_TOKEN),
            );

            let check = CheckBuyout::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &buyout_call, &info, 1),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    ValidityError::LessThanMinBuyoutAmount.into()
                ))
            );
        });
    }

    #[test]
    fn validate_works() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;
            let buyout_call = RuntimeCall::EqTreasury(crate::Call::buyout {
                asset: asset::DOT,
                amount: Amount::Buyout(110 * ONE_TOKEN),
            });

            ModuleBalances::make_free_balance_be(
                &account_id,
                asset::DOT,
                SignedBalance::Positive(10 * ONE_TOKEN),
            );

            let check = CheckBuyout::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_ok!(check.validate(&account_id, &buyout_call, &info, 1));
        });
    }
}
