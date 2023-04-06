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

use crate::mock::*;
use crate::{Error, WhiteList};
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::{asset, OrderSide, OrderType};
use frame_support::{assert_err, assert_ok};
use sp_runtime::FixedI64;

type ModuleMarketMaker = crate::Pallet<Test>;

#[test]
fn add_to_whitelist_works() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        assert_eq!(WhiteList::<Test>::get(account_id), None);
        assert_ok!(ModuleMarketMaker::add_to_whitelist(
            Origin::root(),
            account_id
        ));
        assert_eq!(WhiteList::<Test>::get(account_id), Some(()));
    })
}

#[test]
fn remove_from_whitelist() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        WhiteList::<Test>::insert(account_id, ());

        assert_ok!(ModuleMarketMaker::remove_from_whitelist(
            Origin::root(),
            account_id
        ));
        assert_eq!(WhiteList::<Test>::get(account_id), None);
    })
}

#[test]
fn create_order_works() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        assert_ok!(ModuleMarketMaker::add_to_whitelist(
            Origin::root(),
            account_id
        ));

        assert_ok!(ModuleMarketMaker::create_order(
            Origin::signed(account_id),
            asset::DOT,
            OrderType::Market,
            OrderSide::Buy,
            EqFixedU128::from(1)
        ));
    })
}

#[test]
fn create_order_should_fail_when_not_whitelisted_account() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        assert_err!(
            ModuleMarketMaker::create_order(
                Origin::signed(account_id),
                asset::DOT,
                OrderType::Market,
                OrderSide::Buy,
                EqFixedU128::from(1)
            ),
            Error::<Test>::NotWhitelistedAccount
        );
    })
}

#[test]
fn delete_order_works() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        assert_ok!(ModuleMarketMaker::add_to_whitelist(
            Origin::root(),
            account_id
        ));

        assert_ok!(ModuleMarketMaker::delete_order(
            Origin::signed(account_id),
            asset::DOT,
            1,
            FixedI64::from(1)
        ));
    })
}

#[test]
fn delete_order_should_fail_when_not_whitelisted_account() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        assert_err!(
            ModuleMarketMaker::delete_order(
                Origin::signed(account_id),
                asset::DOT,
                1,
                FixedI64::from(1)
            ),
            Error::<Test>::NotWhitelistedAccount
        );
    })
}
