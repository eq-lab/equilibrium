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
use crate::mock::{
    new_test_ext, BalancesModuleId, ModuleBalances, OracleMock, RuntimeOrigin, SlashMock,
    SubaccountsManagerMock, Test,
};
use crate::mock::{Balance, TimeMock, FAIL_ACC};
use eq_primitives::asset::*;
use eq_primitives::{asset, PriceSetter};
use eq_utils::ONE_TOKEN;
use frame_support::pallet_prelude::Hooks;
use frame_support::traits::OnUnbalanced;
use frame_support::{assert_err, assert_noop, assert_ok, dispatch::DispatchError::BadOrigin};
use frame_system::RawOrigin;
use mock::{clear_eq_buyout_args, get_eq_buyout_args};
use sp_runtime::FixedI64;

/// who, balance, debt, currency
macro_rules! assert_balance {
    ($who:expr, $balance:expr, $debt:expr, $currency:expr) => {
        assert_eq!(
            ModuleBalances::total_balance(&$who, $currency),
            $balance,
            "assert balance failed"
        );
        assert_eq!(
            ModuleBalances::debt(&$who, $currency),
            $debt,
            "assert debt failed"
        );
    };
}

#[test]
fn no_balances() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;

        assert_balance!(account_id_1, 0, 0, EQD);
        assert_balance!(account_id_2, 0, 0, EQD);

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_1),
            EQD,
            account_id_2,
            10
        ));

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_1),
            EQD,
            account_id_2,
            0
        ));

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_1),
            EQD,
            account_id_1,
            100
        ));

        assert_ok!(ModuleBalances::withdraw(
            &account_id_1,
            EQD,
            0,
            true,
            None,
            WithdrawReasons::empty(),
            ExistenceRequirement::KeepAlive,
        ));

        assert_balance!(account_id_1, 0, 10, EQD);
        assert_balance!(account_id_2, 10, 0, EQD);
    });
}

#[test]
fn unknown_asset() {
    new_test_ext().execute_with(|| {
        let unknown = Asset::from_bytes(b"unknown").unwrap();
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;

        assert_balance!(account_id_1, 0, 0, unknown);
        assert_balance!(account_id_2, 0, 0, unknown);

        assert_err!(
            ModuleBalances::transfer(
                RuntimeOrigin::signed(account_id_1),
                unknown,
                account_id_2,
                10
            ),
            eq_assets::Error::<Test>::AssetNotExists
        );

        assert_balance!(account_id_1, 0, 0, unknown);
        assert_balance!(account_id_2, 0, 0, unknown);
    });
}

#[test]
fn get_balances() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;

        assert_eq!(
            ModuleBalances::get_balance(&account_id_1, &EQD),
            SignedBalance::Positive(0 as u128)
        );
        assert_eq!(
            ModuleBalances::get_balance(&account_id_1, &BTC),
            SignedBalance::Positive(1_000_000_000_000 as u128)
        );

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_1),
            EQD,
            account_id_2,
            10
        ));

        assert_eq!(
            ModuleBalances::get_balance(&account_id_1, &EQD),
            SignedBalance::Negative(10 as u128)
        );
        for (currency, balance) in ModuleBalances::iterate_account_balances(&account_id_1) {
            match currency {
                EQD => assert_eq!(balance, SignedBalance::Negative(10 as u128)),
                BTC => assert_eq!(balance, SignedBalance::Positive(1_000_000_000_000 as u128)),
                _ => assert_eq!(balance, SignedBalance::Positive(0 as u128)),
            }
        }

        for (account, balances) in ModuleBalances::iterate_balances() {
            if account == account_id_1 {
                for (curr, balance) in balances {
                    match curr {
                        EQD => assert_eq!(balance, SignedBalance::Negative(10 as u128)),
                        BTC => {
                            assert_eq!(balance, SignedBalance::Positive(1_000_000_000_000 as u128))
                        }
                        _ => assert_eq!(balance, SignedBalance::Positive(0 as u128)),
                    }
                }
            }
        }
    });
}

#[test]
fn make_free_balance() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;

        ModuleBalances::make_free_balance_be(&account_id_1, EQD, SignedBalance::Positive(10));
        ModuleBalances::make_free_balance_be(&account_id_2, EQD, SignedBalance::Negative(20));

        assert_balance!(&account_id_1, 10, 0, EQD);
        assert_balance!(&account_id_2, 0, 20, EQD);
    });
}

#[test]
fn deposit() {
    new_test_ext().execute_with(|| {
        let unknown_asset = Asset::from_bytes(b"unknown").unwrap();
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;

        assert_ok!(ModuleBalances::deposit(
            RawOrigin::Root.into(),
            EQD,
            account_id_1,
            10
        ));

        assert_balance!(&account_id_1, 10, 0, EQD);

        assert_err!(
            ModuleBalances::deposit(RawOrigin::Root.into(), unknown_asset, account_id_1, 10),
            eq_assets::Error::<Test>::AssetNotExists
        );
        assert_err!(
            ModuleBalances::deposit(Some(account_id_1).into(), EQD, account_id_1, 10),
            BadOrigin
        );

        assert_ok!(ModuleBalances::disable_transfers(RawOrigin::Root.into()));
        assert_err!(
            ModuleBalances::deposit(RawOrigin::Root.into(), EQD, account_id_2, 30),
            Error::<Test>::TransfersAreDisabled
        );
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        assert_ok!(ModuleBalances::deposit(
            RawOrigin::Root.into(),
            EQD,
            account_id_1,
            20
        ));

        assert_balance!(&account_id_1, 30, 0, EQD);
    });
}

#[test]
fn zero_deposit() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;
        let account_not_exist: u64 = 1001;

        assert_ok!(ModuleBalances::deposit(
            RawOrigin::Root.into(),
            EQD,
            account_id_1,
            10
        ));
        assert_balance!(&account_id_1, 10, 0, EQD);

        assert_ok!(ModuleBalances::deposit(
            RawOrigin::Root.into(),
            EQD,
            account_id_1,
            0
        ));
        assert_ok!(ModuleBalances::deposit_into_existing(
            &account_id_1,
            EQD,
            0,
            None
        ));
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_2,
            EQD,
            0,
            true,
            None
        ));

        assert_err!(
            ModuleBalances::deposit_into_existing(&account_not_exist, EQD, 10, None),
            Error::<Test>::DeadAccount
        );
        assert_balance!(&account_id_1, 10, 0, EQD);
        assert_balance!(&account_id_2, 0, 0, EQD);
    });
}

#[test]
fn burn() {
    new_test_ext().execute_with(|| {
        let unknown_asset = Asset::from_bytes(b"unknown").unwrap();
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;

        assert_ok!(ModuleBalances::burn(
            RawOrigin::Root.into(),
            EQD,
            account_id_2,
            10
        ));
        assert_balance!(&account_id_2, 0, 10, EQD);

        assert_err!(
            ModuleBalances::burn(RawOrigin::Root.into(), unknown_asset, account_id_1, 10),
            eq_assets::Error::<Test>::AssetNotExists
        );
        assert_err!(
            ModuleBalances::burn(Some(account_id_1).into(), EQD, account_id_1, 10),
            BadOrigin
        );
        assert_ok!(ModuleBalances::disable_transfers(RawOrigin::Root.into()));
        assert_err!(
            ModuleBalances::burn(RawOrigin::Root.into(), EQD, account_id_2, 30),
            Error::<Test>::TransfersAreDisabled
        );
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        assert_ok!(ModuleBalances::burn(
            RawOrigin::Root.into(),
            EQD,
            account_id_2,
            30
        ));
        assert_balance!(&account_id_2, 0, 40, EQD);
    });
}

#[test]
fn enable_disable_transfers() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;

        assert_err!(
            ModuleBalances::disable_transfers(Some(account_id_1).into()),
            BadOrigin
        );
        assert_ok!(ModuleBalances::disable_transfers(RawOrigin::Root.into()));

        assert_err!(
            ModuleBalances::transfer(RuntimeOrigin::signed(account_id_2), EQD, account_id_1, 10),
            Error::<Test>::TransfersAreDisabled
        );

        assert_err!(
            ModuleBalances::enable_transfers(Some(account_id_1).into()),
            BadOrigin
        );

        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_2),
            EQD,
            account_id_1,
            10
        ));
    });
}

#[test]
fn test_aggregates_balances() {
    new_test_ext().execute_with(|| {
        let account_id_10: u64 = 10;
        let account_id_20: u64 = 20;
        let account_id_30: u64 = 30;

        assert_balance!(&account_id_10, 10_000_000_000, 0, EQD);

        assert_balance!(&account_id_10, 10_000_000_000, 0, EQD);
        assert_balance!(&account_id_20, 20_000_000_000, 0, EQD);
        assert_balance!(&account_id_30, 30_000_000_000, 0, EQD);

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_10),
            EQD,
            account_id_20,
            25_000_000_000
        ));

        assert_balance!(&account_id_10, 0, 15_000_000_000, EQD);
        assert_balance!(&account_id_20, 45_000_000_000, 0, EQD);
        assert_balance!(&account_id_30, 30_000_000_000, 0, EQD);

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_20),
            EQD,
            account_id_30,
            57_000_000_000
        ));

        assert_balance!(&account_id_10, 0, 15_000_000_000, EQD);
        assert_balance!(&account_id_20, 0, 12_000_000_000, EQD);
        assert_balance!(&account_id_30, 87_000_000_000, 0, EQD);

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_30),
            EQD,
            account_id_10,
            90_000_000_000
        ));

        assert_balance!(&account_id_10, 75_000_000_000, 0, EQD);
        assert_balance!(&account_id_20, 0, 12_000_000_000, EQD);
        assert_balance!(&account_id_30, 0, 3_000_000_000, EQD);
    });
}

#[test]
fn test_deposit() {
    new_test_ext().execute_with(|| {
        let account_id_100: u64 = 100;
        let account_id_200: u64 = 200;
        let account_id_300: u64 = 300;
        let account_id_400: u64 = 400;

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_100,
            EQD,
            50,
            true,
            None
        ));
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_200,
            EQD,
            100,
            true,
            None
        ));
        assert_ok!(ModuleBalances::deposit_into_existing(
            &account_id_100,
            EQD,
            50,
            None
        ));
        assert_ok!(ModuleBalances::deposit_into_existing(
            &account_id_200,
            EQD,
            100,
            None
        ));
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_300,
            EQD,
            300,
            true,
            None
        ));
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_400,
            EQD,
            400,
            true,
            None
        ));

        assert_balance!(&account_id_100, 100, 0, EQD);
        assert_balance!(&account_id_200, 200, 0, EQD);
        assert_balance!(&account_id_300, 300, 0, EQD);
        assert_balance!(&account_id_400, 400, 0, EQD);

        assert_ok!(ModuleBalances::deposit_into_existing(
            &account_id_100,
            EOS,
            100,
            None
        ));
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_200,
            EOS,
            200,
            true,
            None
        ));
        assert_ok!(ModuleBalances::deposit_into_existing(
            &account_id_300,
            EOS,
            300,
            None
        ));
        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_400,
            EOS,
            400,
            true,
            None
        ));

        assert_balance!(&account_id_100, 100, 0, EOS);
        assert_balance!(&account_id_200, 200, 0, EOS);
        assert_balance!(&account_id_300, 300, 0, EOS);
        assert_balance!(&account_id_400, 400, 0, EOS);
    });
}

#[test]
fn test_ensure_can_withdraw_and_withdraw() {
    new_test_ext().execute_with(|| {
        let account_id_100: u64 = 100;
        let account_id_200: u64 = 200;
        let account_id_300: u64 = 300;
        let account_id_400: u64 = 400;

        assert_ok!(ModuleBalances::ensure_can_withdraw(
            &account_id_100,
            EQD,
            100,
            WithdrawReasons::empty(),
            0
        ));
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            &account_id_200,
            EQD,
            200,
            WithdrawReasons::empty(),
            0
        ));
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            &account_id_300,
            EQD,
            300,
            WithdrawReasons::empty(),
            0
        ));
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            &account_id_400,
            EQD,
            400,
            WithdrawReasons::empty(),
            0
        ));

        ModuleBalances::make_free_balance_be(&account_id_100, EQD, SignedBalance::Negative(100));
        ModuleBalances::make_free_balance_be(&account_id_200, EQD, SignedBalance::Negative(200));
        ModuleBalances::make_free_balance_be(&account_id_300, EQD, SignedBalance::Negative(300));
        ModuleBalances::make_free_balance_be(&account_id_400, EQD, SignedBalance::Negative(400));

        assert_balance!(&account_id_100, 0, 100, EQD);
        assert_balance!(&account_id_200, 0, 200, EQD);
        assert_balance!(&account_id_300, 0, 300, EQD);
        assert_balance!(&account_id_400, 0, 400, EQD);
    });
}

#[test]
fn balance_checker_not_allow() {
    new_test_ext().execute_with(|| {
        let account_forbidden: u64 = FAIL_ACC;
        let account_id_1: u64 = 2;

        assert_ok!(ModuleBalances::deposit_creating(
            &account_forbidden,
            BTC,
            100,
            false,
            None
        ));

        assert_balance!(account_forbidden, 100, 0, BTC);

        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                &account_forbidden,
                DOT,
                10,
                WithdrawReasons::empty(),
                0
            ),
            DispatchError::Other("Expected error")
        );

        assert_err!(
            ModuleBalances::withdraw(
                &account_forbidden,
                DOT,
                10,
                true,
                None,
                WithdrawReasons::empty(),
                ExistenceRequirement::KeepAlive,
            ),
            DispatchError::Other("Expected error")
        );

        assert_err!(
            ModuleBalances::transfer(
                RuntimeOrigin::signed(account_forbidden),
                DOT,
                account_id_1,
                10
            ),
            DispatchError::Other("Expected error")
        );

        assert_err!(
            ModuleBalances::transfer(
                RuntimeOrigin::signed(account_id_1),
                DOT,
                account_forbidden,
                10
            ),
            DispatchError::Other("Expected error")
        );

        assert_err!(
            ModuleBalances::currency_transfer(
                &account_forbidden,
                &account_id_1,
                DOT,
                10,
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::Common,
                true,
            ),
            DispatchError::Other("Expected error")
        );

        assert_err!(
            ModuleBalances::deposit_into_existing(&account_forbidden, BTC, 50, None),
            DispatchError::Other("Expected error")
        );
        assert_ok!(ModuleBalances::deposit_creating(
            &account_forbidden,
            DOT,
            50,
            true,
            None
        ));

        assert_balance!(account_forbidden, 0, 0, DOT);
    })
}

#[test]
fn test_transfer_enabling_disabling() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;

        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_1),
            EQD,
            account_id_2,
            10
        ));

        assert_ok!(ModuleBalances::disable_transfers(RawOrigin::Root.into()));

        assert_err!(
            ModuleBalances::transfer(RuntimeOrigin::signed(account_id_2), EQD, account_id_1, 10),
            Error::<Test>::TransfersAreDisabled
        );

        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_2),
            EQD,
            account_id_1,
            10
        ));
    });
}

#[test]
fn free_balance() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;
        assert_eq!(ModuleBalances::free_balance(&account_id_1, EQD), 0);
        assert_eq!(
            ModuleBalances::free_balance(&account_id_1, BTC),
            1_000_000_000_000
        );

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id_1),
            EQD,
            account_id_2,
            10
        ));

        assert_eq!(ModuleBalances::free_balance(&account_id_1, EQD), 0);

        assert_eq!(ModuleBalances::minimum_balance_value(), 20);
    });
}

#[test]
fn overflow() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;
        assert_ok!(ModuleBalances::deposit_into_existing(
            &account_id_1,
            EQD,
            u128::MAX,
            None
        ));
        assert_balance!(account_id_1, u128::MAX, 0, EQD);

        assert_err!(
            ModuleBalances::deposit_into_existing(&account_id_1, EQD, 100, None),
            ArithmeticError::Overflow
        );

        assert_err!(
            ModuleBalances::transfer(RuntimeOrigin::signed(account_id_2), EQD, account_id_1, 10),
            ArithmeticError::Overflow
        );

        assert_err!(
            ModuleBalances::currency_transfer(
                &account_id_2,
                &account_id_1,
                EQD,
                10,
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::Common,
                true,
            ),
            ArithmeticError::Overflow
        );

        assert_balance!(account_id_1, u128::MAX, 0, EQD);

        assert_ok!(ModuleBalances::withdraw(
            &account_id_2,
            EQD,
            u128::MAX,
            true,
            None,
            WithdrawReasons::empty(),
            ExistenceRequirement::KeepAlive,
        ));

        assert_err!(
            ModuleBalances::withdraw(
                &account_id_2,
                EQD,
                10,
                true,
                None,
                WithdrawReasons::empty(),
                ExistenceRequirement::KeepAlive,
            ),
            ArithmeticError::Overflow
        );

        assert_balance!(account_id_2, 0, u128::MAX, EQD);
    });
}

#[test]
fn currency_transfer_call_buyout_eq() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 101;
        let account_not_exist: u64 = 101;

        clear_eq_buyout_args();

        assert_ok!(ModuleBalances::currency_transfer(
            &account_id_1,
            &account_not_exist,
            BTC,
            100,
            ExistenceRequirement::AllowDeath,
            eq_primitives::TransferReason::Common,
            true,
        ));

        assert_eq!(get_eq_buyout_args(), None);
    });
}

#[test]
fn currency_total_issuance() {
    new_test_ext().execute_with(|| {
        assert_eq!(ModuleBalances::currency_total_issuance(EQD), 1000);
    });
}

#[test]
fn no_delete_with_non_zero_refcount() {
    new_test_ext().execute_with(|| {
        let with_non_zero_refcount: u64 = 20;
        frame_system::Pallet::<Test>::inc_providers(&with_non_zero_refcount);

        assert_eq!(
            ModuleBalances::can_be_deleted(&with_non_zero_refcount).unwrap(),
            false
        );

        assert_err!(
            ModuleBalances::delete_account(&with_non_zero_refcount),
            Error::<Test>::NotAllowedToDeleteAccount
        );
    });
}

#[test]
fn no_delete_with_balance_more_than_minimal_collateral() {
    new_test_ext().execute_with(|| {
        let with_non_zero_refcount: u64 = 10;
        // has 10_000_000_000 EQD from genesis
        // ExistentialDeposit = 20 EQD

        assert_eq!(
            ModuleBalances::can_be_deleted(&with_non_zero_refcount).unwrap(),
            false
        );

        assert_err!(
            ModuleBalances::delete_account(&with_non_zero_refcount),
            Error::<Test>::NotAllowedToDeleteAccount
        );
    });
}

#[test]
fn minimal_existential_deposit_basic() {
    new_test_ext().execute_with(|| {
        let account_with_minimal_balance: u64 = 15;
        // has 15 EQ from genesis
        // ExistentialDeposit = 20 USD or ExistentialDepositBasic = 15 EQ

        let account_ready_to_delete: u64 = 16;
        // has 13 EQ from genesis

        let _ = OracleMock::set_price(1, asset::EQ, FixedI64::saturating_from_integer(1));

        assert_eq!(
            ModuleBalances::can_be_deleted(&account_with_minimal_balance).unwrap(),
            false
        );

        assert_err!(
            ModuleBalances::delete_account(&account_with_minimal_balance),
            Error::<Test>::NotAllowedToDeleteAccount
        );

        assert_eq!(
            ModuleBalances::can_be_deleted(&account_ready_to_delete).unwrap(),
            true
        );

        assert_ok!(ModuleBalances::delete_account(&account_ready_to_delete));
    });
}

#[test]
fn minimal_existential_deposit_basic_when_transfer() {
    new_test_ext().execute_with(|| {
        let account_with_minimal_balance: u64 = 17;
        let destination_account: u64 = 18;
        let not_enough_transfer_amount = 4 as u128;
        let success_transfer_amount = 15 as u128;

        let _ = OracleMock::set_price(1, asset::EQ, FixedI64::saturating_from_integer(1));

        assert_err!(
            ModuleBalances::transfer(
                RuntimeOrigin::signed(account_with_minimal_balance),
                asset::EQ,
                destination_account,
                not_enough_transfer_amount
            ),
            Error::<Test>::NotEnoughToKeepAlive
        );

        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_with_minimal_balance),
            asset::EQ,
            destination_account,
            success_transfer_amount
        ));
    });
}

#[test]
fn delete_account() {
    new_test_ext().execute_with(|| {
        let account_id = 30; // has 30_000_000_000 EQD
        let account_id_2 = 20;

        // ExistentialDeposit - 20

        // transfer
        assert_ok!(ModuleBalances::transfer(
            RuntimeOrigin::signed(account_id),
            EQD,
            account_id_2,
            29_999_999_999
        ));

        println!(
            "{:?} {:?}",
            ModuleBalances::total_balance(&account_id, EQD),
            ModuleBalances::minimum_balance_value()
        );

        // account_id balance is 1 (EQD)

        println!(
            "pr {}",
            frame_system::Pallet::<Test>::providers(&account_id)
        );
        println!(
            "cn {}",
            frame_system::Pallet::<Test>::consumers(&account_id)
        );
        // delete
        assert_ok!(ModuleBalances::delete_account(&account_id));

        // no account store
        assert!(frame_system::Pallet::<Test>::providers(&account_id) == 0);
    });
}

#[test]
fn exchange() {
    new_test_ext().execute_with(|| {
        let account1 = 1; // 1000_000_000_000 BTC
        let account2 = 2; // 2000_000_000_000 BTC
        let initial_btc_1 = ModuleBalances::get_balance(&account1, &BTC).abs();
        let initial_btc_2 = ModuleBalances::get_balance(&account2, &BTC).abs();
        assert_noop!(
            ModuleBalances::exchange((&account1, &account2), (&BTC, &BTC), (50, 100)),
            (Error::<Test>::ExchangeSameAsset.into(), None)
        );

        assert_ok!(ModuleBalances::deposit_creating(
            &account1,
            EQD,
            30_000_000_000_000,
            true,
            None
        ));
        let exchange_usd = 30_000_000_000_000;
        let exchange_btc = 1_000_000_000;
        assert_ok!(ModuleBalances::exchange(
            (&account1, &account2),
            (&EQD, &BTC),
            (exchange_usd, exchange_btc)
        ),);
        assert_balance!(account1, 0, 0, EQD);
        assert_balance!(account1, initial_btc_1 + exchange_btc, 0, BTC);
        assert_balance!(account2, exchange_usd, 0, EQD);
        assert_balance!(account2, initial_btc_2 - exchange_btc, 0, BTC);
    });
}

#[test]
fn exchange_single_account() {
    new_test_ext().execute_with(|| {
        let account1 = 1; // has 1000_000_000_000 BTC, see mock
        let initial_btc_1 = ModuleBalances::get_balance(&account1, &BTC).abs();
        assert_noop!(
            ModuleBalances::exchange((&account1, &account1), (&BTC, &BTC), (50, 100)),
            (Error::<Test>::ExchangeSameAsset.into(), None)
        );

        assert_ok!(ModuleBalances::deposit_creating(
            &account1,
            EQD,
            30_000_000_000_000,
            true,
            None
        ));
        let exchange_usd = 30_000_000_000_000;
        let exchange_btc = 1_000_000_000;
        assert_ok!(ModuleBalances::exchange(
            (&account1, &account1),
            (&EQD, &BTC),
            (exchange_usd, exchange_btc)
        ),);
        assert_balance!(account1, exchange_usd, 0, EQD);
        assert_balance!(account1, initial_btc_1, 0, BTC);
    });
}

#[test]
fn exchange_fail_returns_account() {
    new_test_ext().execute_with(|| {
        let acc = 1;

        let res = ModuleBalances::exchange((&FAIL_ACC, &acc), (&BTC, &EQD), (50, 100));

        assert!(res.is_err());
        assert_eq!(res.err().unwrap().0, DispatchError::Other("Expected error"));
        assert_eq!(res.err().unwrap().1, Some(FAIL_ACC));

        let res = ModuleBalances::exchange((&acc, &FAIL_ACC), (&BTC, &EQD), (50, 100));

        assert!(res.is_err());
        assert_eq!(res.err().unwrap().0, DispatchError::Other("Expected error"));
        assert_eq!(res.err().unwrap().1, Some(FAIL_ACC));

        let res = ModuleBalances::exchange((&FAIL_ACC, &acc), (&EQD, &BTC), (50, 100));

        assert!(res.is_err());
        assert_eq!(res.err().unwrap().0, DispatchError::Other("Expected error"));
        assert_eq!(res.err().unwrap().1, Some(FAIL_ACC));

        let res = ModuleBalances::exchange((&acc, &FAIL_ACC), (&EQD, &BTC), (50, 100));

        assert!(res.is_err());
        assert_eq!(res.err().unwrap().0, DispatchError::Other("Expected error"));
        assert_eq!(res.err().unwrap().1, Some(FAIL_ACC));
    });
}

#[test]
fn get_total_debt_and_collateral_should_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;
        assert_eq!(
            Ok(DebtCollateralDiscounted {
                debt: Balance::zero(),
                collateral: Balance::zero(),
                discounted_collateral: Balance::zero()
            }),
            ModuleBalances::get_debt_and_collateral(&account_id_1)
        );

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_1,
            EQD,
            30 * ONE_TOKEN,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_1,
            mock::XDOT1,
            40 * ONE_TOKEN,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_1,
            mock::XDOT2,
            50 * ONE_TOKEN,
            true,
            None
        ));

        let DebtCollateralDiscounted {
            debt,
            collateral,
            discounted_collateral,
        } = ModuleBalances::get_debt_and_collateral(&account_id_1).unwrap();

        assert_eq!((Balance::zero(), 390 * ONE_TOKEN), (debt, collateral,),);

        assert_eq!(325 * ONE_TOKEN, discounted_collateral);
    })
}

#[test]
fn get_total_debt_and_collateral_when_price_getter_return_error_should_fail() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 0;

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_1,
            EQD,
            30_000_000_000_000,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &account_id_1,
            BTC,
            1_000_000_000,
            true,
            None
        ));

        OracleMock::remove(&asset::BTC);
        OracleMock::remove(&asset::EQD);

        assert!(ModuleBalances::get_debt_and_collateral(&account_id_1).is_err());
    })
}

#[test]
fn reserve_should_reduce_account_balance() {
    new_test_ext().execute_with(|| {
        let account1 = 0;
        let initial_balance = 100 * ONE_TOKEN;

        assert_ok!(ModuleBalances::deposit_creating(
            &account1,
            EQD,
            initial_balance,
            true,
            None
        ));
        assert_ok!(ModuleBalances::deposit_creating(
            &account1,
            BTC,
            initial_balance,
            true,
            None
        ));

        let to_reserve = 20 * ONE_TOKEN;

        assert_eq!(Reserved::<Test>::get(account1, EQD), 0);
        assert_eq!(Reserved::<Test>::get(account1, BTC), 0);
        assert_ok!(ModuleBalances::reserve(&account1, EQD, to_reserve));
        assert_ok!(ModuleBalances::reserve(&account1, BTC, to_reserve));
        assert_eq!(
            ModuleBalances::get_balance(&BalancesModuleId::get().into_account_truncating(), &EQD)
                .abs(),
            to_reserve
        );

        assert_eq!(
            ModuleBalances::get_balance(&account1, &EQD).abs(),
            initial_balance - to_reserve
        );
        assert_eq!(Reserved::<Test>::get(account1, EQD), to_reserve);

        assert_eq!(
            ModuleBalances::get_balance(&account1, &BTC).abs(),
            initial_balance - to_reserve
        );
        assert_eq!(Reserved::<Test>::get(account1, BTC), to_reserve);
    })
}

#[test]
fn reserve_should_fail_when_account_balance_less_than_amount_to_reserve() {
    new_test_ext().execute_with(|| {
        let account1 = FAIL_ACC;
        let initial_balance = 1 * ONE_TOKEN;

        let to_reserve = 20 * ONE_TOKEN;

        assert_ok!(ModuleBalances::deposit_creating(
            &account1,
            EQD,
            initial_balance,
            true,
            None
        ));

        assert!(ModuleBalances::reserve(&account1, EQD, to_reserve).is_err());
        assert_eq!(Reserved::<Test>::get(account1, EQD), 0);
    });
}

#[test]
fn reserve_and_slash_reserved() {
    new_test_ext().execute_with(|| {
        let acc1 = &0;
        let initial_balance = 100 * ONE_TOKEN;

        assert_ok!(ModuleBalances::deposit_creating(
            acc1,
            EQD,
            initial_balance,
            true,
            None
        ));

        let to_reserve = 20 * ONE_TOKEN;
        let to_slash = 15 * ONE_TOKEN;

        assert_ok!(ModuleBalances::reserve(acc1, EQD, to_reserve));
        assert_eq!(SlashMock::balance(), 0);
        assert_eq!(Reserved::<Test>::get(acc1, EQD), to_reserve);

        SlashMock::on_unbalanced(ModuleBalances::slash_reserved(acc1, EQD, to_slash).0);
        assert_eq!(SlashMock::balance(), to_slash);
        assert_eq!(Reserved::<Test>::get(acc1, EQD), to_reserve - to_slash);

        SlashMock::on_unbalanced(ModuleBalances::slash_reserved(acc1, EQD, to_slash).0);
        assert_eq!(SlashMock::balance(), to_reserve);
        assert_eq!(Reserved::<Test>::get(acc1, EQD), 0);
    });
}

#[test]
fn unreserve_should_increase_account_balance() {
    new_test_ext().execute_with(|| {
        let account1 = 0;
        let initial_balance = 100 * ONE_TOKEN;

        let to_reserve = 20 * ONE_TOKEN;
        assert_ok!(ModuleBalances::deposit_creating(
            &account1,
            EQD,
            initial_balance,
            true,
            None
        ));
        assert_ok!(ModuleBalances::reserve(&account1, EQD, to_reserve));
        assert_eq!(Reserved::<Test>::get(account1, EQD), to_reserve);

        let to_unreserve = 10 * ONE_TOKEN;
        assert_eq!(
            ModuleBalances::unreserve(&account1, EQD, to_unreserve),
            to_unreserve
        );
        assert_eq!(
            Reserved::<Test>::get(account1, EQD),
            to_reserve - to_unreserve
        );
        assert_eq!(
            ModuleBalances::get_balance(&account1, &EQD).abs(),
            initial_balance - to_reserve + to_unreserve
        );
    });
}

#[test]
fn unreserve_should_return_max_amount_to_unreserve_if_not_enough_to_unreserve() {
    new_test_ext().execute_with(|| {
        let account1 = 0;
        let initial_balance = 100 * ONE_TOKEN;

        let to_reserve = 20 * ONE_TOKEN;
        assert_ok!(ModuleBalances::deposit_creating(
            &account1,
            EQD,
            initial_balance,
            true,
            None
        ));
        assert_ok!(ModuleBalances::reserve(&account1, EQD, to_reserve));
        assert_eq!(Reserved::<Test>::get(account1, EQD), to_reserve);
        assert_eq!(
            ModuleBalances::get_balance(&account1, &EQD).abs(),
            initial_balance - to_reserve
        );

        let to_unreserve = 30 * ONE_TOKEN;
        assert_eq!(
            ModuleBalances::unreserve(&account1, EQD, to_unreserve),
            to_reserve
        );
        assert_eq!(
            ModuleBalances::get_balance(&account1, &EQD).abs(),
            initial_balance
        );
        assert_eq!(ModuleBalances::unreserve(&account1, EQD, to_unreserve), 0);
        assert_eq!(Reserved::<Test>::get(account1, EQD), 0);
    });
}

#[test]
fn unreserve_should_delete_item_when_zero() {
    new_test_ext().execute_with(|| {
        let account1 = 0;
        let initial_balance = 100 * ONE_TOKEN;

        let to_reserve = 20 * ONE_TOKEN;
        assert_ok!(ModuleBalances::deposit_creating(
            &account1,
            EQD,
            initial_balance,
            true,
            None
        ));
        assert_ok!(ModuleBalances::deposit_creating(
            &account1,
            BTC,
            initial_balance,
            true,
            None
        ));
        assert_ok!(ModuleBalances::reserve(&account1, EQD, to_reserve));
        assert_ok!(ModuleBalances::reserve(&account1, BTC, to_reserve));
        assert_eq!(Reserved::<Test>::get(account1, EQD), to_reserve);

        let to_unreserve = 10 * ONE_TOKEN;
        assert_eq!(
            ModuleBalances::unreserve(&account1, EQD, to_unreserve),
            to_unreserve
        );
        assert_eq!(
            Reserved::<Test>::get(account1, EQD),
            to_reserve - to_unreserve
        );
        assert_eq!(
            ModuleBalances::get_balance(&account1, &EQD).abs(),
            initial_balance - to_reserve + to_unreserve
        );
        assert_eq!(
            ModuleBalances::unreserve(&account1, EQD, to_reserve),
            to_unreserve
        );

        assert_eq!(
            ModuleBalances::get_balance(&account1, &EQD).abs(),
            initial_balance
        );

        assert!(!Reserved::<Test>::contains_key(&account1, &EQD));
        assert!(Reserved::<Test>::contains_key(&account1, &BTC));
    });
}

#[test]
fn ensure_xcm_transfer_limit_not_exceeded_works() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 0;

        assert_ok!(ModuleBalances::update_xcm_transfer_native_limit(
            RuntimeOrigin::root(),
            Some(10_000 * ONE_TOKEN)
        ));

        XcmNativeTransfers::<Test>::insert(account_id, (0, 0));

        TimeMock::set_secs(1667907093u64);
        Pallet::<Test>::on_initialize(1);

        assert_ok!(Pallet::<Test>::ensure_xcm_transfer_limit_not_exceeded(
            &account_id,
            5_000 * ONE_TOKEN
        ));
        assert_eq!(
            XcmNativeTransfers::<Test>::get(&account_id),
            Some((0, 1667907093u64))
        );

        Pallet::<Test>::update_xcm_native_transfers(&account_id, 5_000 * ONE_TOKEN);
        assert_eq!(
            XcmNativeTransfers::<Test>::get(&account_id),
            Some((5_000 * ONE_TOKEN, 1667907093u64))
        );

        TimeMock::set_secs(1667907094u64);

        assert_ok!(Pallet::<Test>::ensure_xcm_transfer_limit_not_exceeded(
            &account_id,
            5_000 * ONE_TOKEN
        ));
        Pallet::<Test>::update_xcm_native_transfers(&account_id, 5_000 * ONE_TOKEN);
        assert_eq!(
            XcmNativeTransfers::<Test>::get(&account_id),
            Some((10_000 * ONE_TOKEN, 1667907094u64))
        );

        assert_err!(
            Pallet::<Test>::ensure_xcm_transfer_limit_not_exceeded(&account_id, 1),
            Error::<Test>::XcmTransfersLimitExceeded
        );
    });
}

#[test]
fn ensure_xcm_transfer_limit_not_exceeded_should_reset_old_transfers() {
    new_test_ext().execute_with(|| {
        let account_id = 1;

        assert_ok!(ModuleBalances::update_xcm_transfer_native_limit(
            RuntimeOrigin::root(),
            Some(10_000 * ONE_TOKEN)
        ));

        XcmNativeTransfers::<Test>::insert(account_id, (0, 0));

        TimeMock::set_secs(1667907093u64);
        Pallet::<Test>::on_initialize(1);

        assert_ok!(Pallet::<Test>::ensure_xcm_transfer_limit_not_exceeded(
            &account_id,
            10_000 * ONE_TOKEN
        ));
        Pallet::<Test>::update_xcm_native_transfers(&account_id, 10_000 * ONE_TOKEN);

        TimeMock::set_secs(1667907093u64 + XCM_LIMIT_PERIOD_IN_SEC);
        Pallet::<Test>::on_initialize(1);

        assert_ok!(Pallet::<Test>::ensure_xcm_transfer_limit_not_exceeded(
            &account_id,
            10_000 * ONE_TOKEN
        ));
        assert_eq!(
            XcmNativeTransfers::<Test>::get(&account_id),
            Some((0, 1667907093u64 + XCM_LIMIT_PERIOD_IN_SEC))
        );

        Pallet::<Test>::update_xcm_native_transfers(&account_id, 10_000 * ONE_TOKEN);
    });
}

#[test]
fn ensure_xcm_transfer_limit_not_exceeded_should_forbid() {
    new_test_ext().execute_with(|| {
        let account_id = 1;

        assert_ok!(ModuleBalances::update_xcm_transfer_native_limit(
            RuntimeOrigin::root(),
            Some(10_000 * ONE_TOKEN)
        ));

        assert_err!(
            Pallet::<Test>::ensure_xcm_transfer_limit_not_exceeded(&account_id, 1),
            Error::<Test>::XcmTransfersNotAllowedForAccount
        );
    });
}

#[test]
fn allow_xcm_transfers_native_for_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::update_xcm_transfer_native_limit(
            RuntimeOrigin::root(),
            Some(10_000 * ONE_TOKEN)
        ));

        assert_ok!(ModuleBalances::allow_xcm_transfers_native_for(
            RawOrigin::Root.into(),
            vec![1, 2, 3, 5, 5]
        ));

        assert!(XcmNativeTransfers::<Test>::contains_key(1));
        assert!(XcmNativeTransfers::<Test>::contains_key(2));
        assert!(XcmNativeTransfers::<Test>::contains_key(3));
        assert!(!XcmNativeTransfers::<Test>::contains_key(4));
        assert!(XcmNativeTransfers::<Test>::contains_key(5));

        TimeMock::set_secs(1667907093u64);

        assert_ok!(ModuleBalances::allow_xcm_transfers_native_for(
            RawOrigin::Root.into(),
            vec![5, 6]
        ));

        assert_eq!(XcmNativeTransfers::<Test>::get(5), Some((0, 0)));
        assert_eq!(XcmNativeTransfers::<Test>::get(6), Some((0, 1667907093u64)));
    });
}

#[test]
fn forbid_xcm_transfers_native_for_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::update_xcm_transfer_native_limit(
            RuntimeOrigin::root(),
            Some(10_000 * ONE_TOKEN)
        ));

        assert_ok!(ModuleBalances::allow_xcm_transfers_native_for(
            RawOrigin::Root.into(),
            vec![1, 2, 3, 5, 5]
        ));

        assert_ok!(ModuleBalances::forbid_xcm_transfers_native_for(
            RawOrigin::Root.into(),
            vec![5, 5, 1]
        ));

        assert!(!XcmNativeTransfers::<Test>::contains_key(5));
        assert!(!XcmNativeTransfers::<Test>::contains_key(1));
    });
}

#[test]
fn update_xcm_transfer_native_limit_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(DailyXcmLimit::<Test>::get(), None);

        assert_ok!(ModuleBalances::update_xcm_transfer_native_limit(
            RawOrigin::Root.into(),
            Some(20_000 * ONE_TOKEN)
        ));

        assert_eq!(DailyXcmLimit::<Test>::get(), Some(20_000 * ONE_TOKEN));

        assert_ok!(ModuleBalances::update_xcm_transfer_native_limit(
            RawOrigin::Root.into(),
            None
        ));

        assert_eq!(DailyXcmLimit::<Test>::get(), None);
    });
}

#[test]
fn update_xcm_native_transfers_storage() {
    new_test_ext().execute_with(|| {
        let account_id = 1;
        assert_eq!(XcmNativeTransfers::<Test>::get(account_id), None);

        Pallet::<Test>::update_xcm_native_transfers(&account_id, 10_000 * ONE_TOKEN);
        assert_eq!(XcmNativeTransfers::<Test>::get(account_id), None);
        assert_eq!(DailyXcmLimit::<Test>::get(), None);

        assert_ok!(ModuleBalances::allow_xcm_transfers_native_for(
            RawOrigin::Root.into(),
            vec![1]
        ));
        assert_ok!(ModuleBalances::update_xcm_transfer_native_limit(
            RuntimeOrigin::root(),
            Some(10_000 * ONE_TOKEN)
        ));

        assert_eq!(XcmNativeTransfers::<Test>::get(account_id), Some((0, 0u64)));

        Pallet::<Test>::update_xcm_native_transfers(&account_id, 1 * ONE_TOKEN);
        assert_eq!(
            XcmNativeTransfers::<Test>::get(account_id),
            Some((1 * ONE_TOKEN, 0u64))
        );

        assert_ok!(ModuleBalances::update_xcm_transfer_native_limit(
            RuntimeOrigin::root(),
            None
        ));
        Pallet::<Test>::update_xcm_native_transfers(&account_id, 1 * ONE_TOKEN);
        assert_eq!(
            XcmNativeTransfers::<Test>::get(account_id),
            Some((1 * ONE_TOKEN, 0u64))
        );
    });
}

#[test]
fn vec_map_no_decode_to_enum() {
    use codec::{Decode, Encode};
    use eq_primitives::map;
    #[derive(
        Debug,
        Clone,
        Eq,
        PartialEq,
        codec::Decode,
        codec::Encode,
        scale_info::TypeInfo,
        codec::MaxEncodedLen,
    )]
    enum EnumAccountData<Balance>
    where
        Balance: Default
            + Debug
            + frame_support::pallet_prelude::Member
            + Into<Balance>
            + AtLeast32BitUnsigned,
    {
        V0 {
            lock: Balance,
            balance: VecMap<Asset, SignedBalance<Balance>>,
        },
    }

    let old_acc_data = map![
        EQ => SignedBalance::Positive(ONE_TOKEN)
    ];

    old_acc_data.using_encoded(|slice| {
        let maybe_enum: Option<EnumAccountData<u128>> = Decode::decode(&mut &slice[..])
            .map(Some)
            .unwrap_or_else(|_| None);
        assert!(maybe_enum.is_none());
    });

    let empty_old_acc_data: VecMap<Asset, u128> = VecMap::new();

    empty_old_acc_data.using_encoded(|slice| {
        let maybe_enum: Option<EnumAccountData<u128>> = Decode::decode(&mut &slice[..])
            .map(Some)
            .unwrap_or_else(|_| None);
        assert!(maybe_enum.is_none());
    });
}

#[test]
fn locked_balance_ensure_can_transfer() {
    new_test_ext().execute_with(|| {
        let acc1 = &0;
        assert_ok!(ModuleBalances::deposit_creating(
            acc1,
            EQ,
            11 * ONE_TOKEN,
            true,
            None
        ));

        ModuleBalances::set_lock([0; 8], acc1, 5 * ONE_TOKEN);
        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                acc1,
                EQ,
                6 * ONE_TOKEN + 1,
                WithdrawReasons::TRANSFER,
                0
            ),
            Error::<Test>::Locked,
        );
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            6 * ONE_TOKEN,
            WithdrawReasons::TRANSFER,
            0
        ));
    });
}

#[test]
fn locked_balance_transaction_payment() {
    new_test_ext().execute_with(|| {
        let acc1 = &0;
        assert_ok!(ModuleBalances::deposit_creating(
            acc1,
            EQ,
            11 * ONE_TOKEN,
            true,
            None
        ));

        ModuleBalances::set_lock([0; 8], acc1, 11 * ONE_TOKEN);
        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                acc1,
                EQ,
                6 * ONE_TOKEN,
                WithdrawReasons::TRANSFER,
                0
            ),
            Error::<Test>::Locked,
        );
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            6 * ONE_TOKEN,
            WithdrawReasons::TRANSACTION_PAYMENT,
            0
        ));
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            6 * ONE_TOKEN,
            WithdrawReasons::FEE,
            0
        ));
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            6 * ONE_TOKEN,
            WithdrawReasons::TIP,
            0
        ));
    });
}

#[test]
fn locked_balance_extend_lock() {
    new_test_ext().execute_with(|| {
        let acc1 = &0;
        assert_ok!(ModuleBalances::deposit_creating(
            acc1,
            EQ,
            11 * ONE_TOKEN,
            true,
            None
        ));

        ModuleBalances::set_lock([0; 8], acc1, 5 * ONE_TOKEN);
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            6 * ONE_TOKEN,
            WithdrawReasons::TRANSFER,
            0
        ),);

        ModuleBalances::extend_lock([0; 8], acc1, 10 * ONE_TOKEN);
        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                acc1,
                EQ,
                6 * ONE_TOKEN,
                WithdrawReasons::TRANSFER,
                0
            ),
            Error::<Test>::Locked,
        );

        ModuleBalances::extend_lock([0; 8], acc1, 5 * ONE_TOKEN);
        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                acc1,
                EQ,
                6 * ONE_TOKEN,
                WithdrawReasons::TRANSFER,
                0
            ),
            Error::<Test>::Locked,
        );

        ModuleBalances::set_lock([0; 8], acc1, 5 * ONE_TOKEN);
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            6 * ONE_TOKEN,
            WithdrawReasons::TRANSFER,
            0
        ),);
    });
}

#[test]
fn locked_balance_less_than_lock() {
    new_test_ext().execute_with(|| {
        let acc1 = &0;
        let acc2 = &1;
        assert_ok!(ModuleBalances::deposit_creating(
            acc1,
            EQ,
            11 * ONE_TOKEN,
            true,
            None
        ));

        ModuleBalances::set_lock([0; 8], acc1, 15 * ONE_TOKEN);
        assert_err!(
            ModuleBalances::currency_transfer(
                acc1,
                acc2,
                EQ,
                1,
                ExistenceRequirement::AllowDeath,
                TransferReason::Common,
                true,
            ),
            Error::<Test>::Locked,
        );
        assert_ok!(ModuleBalances::currency_transfer(
            acc2,
            acc1,
            EQ,
            ONE_TOKEN,
            ExistenceRequirement::AllowDeath,
            TransferReason::Common,
            true,
        ));
    });
}

#[test]
fn locked_balance_multiple_locks() {
    new_test_ext().execute_with(|| {
        let acc1 = &0;
        assert_ok!(ModuleBalances::deposit_creating(
            acc1,
            EQ,
            11 * ONE_TOKEN,
            true,
            None
        ));

        // available = balance(11) - locked(5)
        ModuleBalances::set_lock([0; 8], acc1, 5 * ONE_TOKEN);
        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                acc1,
                EQ,
                6 * ONE_TOKEN + 1,
                WithdrawReasons::TRANSFER,
                0
            ),
            Error::<Test>::Locked,
        );
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            6 * ONE_TOKEN,
            WithdrawReasons::TRANSFER,
            0
        ));

        // available = balance(11) - locked(max(10, 5))
        ModuleBalances::set_lock([1; 8], acc1, 10 * ONE_TOKEN);
        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                acc1,
                EQ,
                6 * ONE_TOKEN + 1,
                WithdrawReasons::TRANSFER,
                0
            ),
            Error::<Test>::Locked,
        );
        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                acc1,
                EQ,
                1 * ONE_TOKEN + 1,
                WithdrawReasons::TRANSFER,
                0
            ),
            Error::<Test>::Locked,
        );
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            1 * ONE_TOKEN,
            WithdrawReasons::TRANSFER,
            0
        ));
    });
}

#[test]
fn locked_balance_remove_lock() {
    new_test_ext().execute_with(|| {
        let acc1 = &0;
        assert_ok!(ModuleBalances::deposit_creating(
            acc1,
            EQ,
            11 * ONE_TOKEN,
            true,
            None
        ));

        // available = balance(11) - locked(max(10, 5))
        ModuleBalances::set_lock([0; 8], acc1, 5 * ONE_TOKEN);
        ModuleBalances::set_lock([1; 8], acc1, 10 * ONE_TOKEN);
        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                acc1,
                EQ,
                1 * ONE_TOKEN + 1,
                WithdrawReasons::TRANSFER,
                0
            ),
            Error::<Test>::Locked,
        );
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            1 * ONE_TOKEN,
            WithdrawReasons::TRANSFER,
            0
        ));

        // available = balance(11) - locked(5)
        ModuleBalances::remove_lock([1; 8], acc1);
        assert_err!(
            ModuleBalances::ensure_can_withdraw(
                acc1,
                EQ,
                6 * ONE_TOKEN + 1,
                WithdrawReasons::TRANSFER,
                0
            ),
            Error::<Test>::Locked,
        );
        assert_ok!(ModuleBalances::ensure_can_withdraw(
            acc1,
            EQ,
            6 * ONE_TOKEN,
            WithdrawReasons::TRANSFER,
            0
        ));
    });
}

#[test]
fn allow_crowdloan_dots_swap() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::allow_crowdloan_swap(
            RawOrigin::Root.into(),
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
                CrowdloanDotAsset::CDOT714
            ]
        ));

        assert_eq!(
            AllowedCrowdloanDotsSwap::<Test>::get(),
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
                CrowdloanDotAsset::CDOT714
            ]
        );

        assert_ok!(ModuleBalances::allow_crowdloan_swap(
            RawOrigin::Root.into(),
            vec![CrowdloanDotAsset::XDOT2]
        ));

        assert_eq!(
            AllowedCrowdloanDotsSwap::<Test>::get(),
            vec![CrowdloanDotAsset::XDOT2]
        );
    });
}

#[test]
fn swap_crowdloan_dots() {
    new_test_ext().execute_with(|| {
        let account_1: u64 = 1;
        let account_2: u64 = 2;
        let account_1_sub_account =
            SubaccountsManagerMock::create_subaccount_inner(&account_1, &SubAccType::Borrower)
                .unwrap();

        // deposit main account_1
        ModuleBalances::make_free_balance_be(
            &account_1,
            XDOT,
            SignedBalance::Positive(40 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1,
            XDOT2,
            SignedBalance::Positive(100 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1,
            XDOT3,
            SignedBalance::Positive(215 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1,
            CDOT714,
            SignedBalance::Positive(327 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1,
            CDOT815,
            SignedBalance::Positive(427 * ONE_TOKEN),
        );

        // deposit main account_2
        ModuleBalances::make_free_balance_be(
            &account_2,
            XDOT,
            SignedBalance::Positive(40 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_2,
            CDOT714,
            SignedBalance::Positive(50 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_2,
            CDOT815,
            SignedBalance::Positive(60 * ONE_TOKEN),
        );

        // create debt for account_1_subaccount
        ModuleBalances::make_free_balance_be(
            &account_1_sub_account,
            DOT,
            SignedBalance::Positive((781 + 782 + 783 + 784 + 785) * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1_sub_account,
            XDOT,
            SignedBalance::Negative(781 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1_sub_account,
            XDOT2,
            SignedBalance::Negative(782 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1_sub_account,
            XDOT3,
            SignedBalance::Negative(783 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1_sub_account,
            CDOT714,
            SignedBalance::Negative(784 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1_sub_account,
            CDOT815,
            SignedBalance::Negative(785 * ONE_TOKEN),
        );

        assert_ok!(ModuleBalances::allow_crowdloan_swap(
            RawOrigin::Root.into(),
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
                CrowdloanDotAsset::CDOT714,
                CrowdloanDotAsset::CDOT815
            ],
        ));

        assert_err!(
            ModuleBalances::swap_crowdloan_dots(
                Some(account_1).into(),
                None,
                vec![CrowdloanDotAsset::XDOT2]
            ),
            Error::<Test>::CrowdloanDotSwapNotAllowed
        );

        // main account_1 Crowdloan DOT balances before swap
        assert_balance!(&account_1, 40 * ONE_TOKEN, 0, XDOT);
        assert_balance!(&account_1, 100 * ONE_TOKEN, 0, XDOT2);
        assert_balance!(&account_1, 215 * ONE_TOKEN, 0, XDOT3);
        assert_balance!(&account_1, 327 * ONE_TOKEN, 0, CDOT714);
        assert_balance!(&account_1, 427 * ONE_TOKEN, 0, CDOT815);

        // main account_2 Crowdloan DOT balances before swap
        assert_balance!(&account_2, 40 * ONE_TOKEN, 0, XDOT);
        assert_balance!(&account_2, 50 * ONE_TOKEN, 0, CDOT714);
        assert_balance!(&account_2, 60 * ONE_TOKEN, 0, CDOT815);

        // account_1_subaccount Crowdloan DOT balances before swap
        assert_balance!(&account_1_sub_account, 0, 781 * ONE_TOKEN, XDOT);
        assert_balance!(&account_1_sub_account, 0, 782 * ONE_TOKEN, XDOT2);
        assert_balance!(&account_1_sub_account, 0, 783 * ONE_TOKEN, XDOT3);
        assert_balance!(&account_1_sub_account, 0, 784 * ONE_TOKEN, CDOT714);
        assert_balance!(&account_1_sub_account, 0, 785 * ONE_TOKEN, CDOT815);

        assert_ok!(ModuleBalances::swap_crowdloan_dots(
            Some(account_1).into(),
            None,
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
                CrowdloanDotAsset::CDOT714,
                CrowdloanDotAsset::CDOT815
            ]
        ));

        assert_ok!(ModuleBalances::swap_crowdloan_dots(
            Some(account_1).into(),
            Some(account_2),
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
                CrowdloanDotAsset::CDOT714,
                CrowdloanDotAsset::CDOT815
            ]
        ));

        // main account_1 Crowdloan DOT balances after swap
        assert_balance!(&account_1, 0, 0, XDOT);
        assert_balance!(&account_1, 100 * ONE_TOKEN, 0, XDOT2);
        assert_balance!(&account_1, 0, 0, XDOT3);
        assert_balance!(&account_1, 0, 0, CDOT714);
        assert_balance!(&account_1, 0, 0, CDOT815);
        assert_balance!(&account_1, (40 + 215 + 327 + 427) * ONE_TOKEN, 0, DOT);

        // main account_2 Crowdloan DOT balances after swap
        assert_balance!(&account_2, 0, 0, XDOT);
        assert_balance!(&account_2, 0, 0, CDOT714);
        assert_balance!(&account_2, 0, 0, CDOT815);
        assert_balance!(&account_2, (40 + 50 + 60) * ONE_TOKEN, 0, DOT);

        // account_1_subaccount Crowdloan DOT balances after swap
        assert_balance!(&account_1_sub_account, 0, 0, XDOT);
        assert_balance!(&account_1_sub_account, 0, 782 * ONE_TOKEN, XDOT2);
        assert_balance!(&account_1_sub_account, 0, 0, XDOT3);
        assert_balance!(&account_1_sub_account, 0, 0, CDOT714);
        assert_balance!(&account_1_sub_account, 0, 0, CDOT815);
        assert_balance!(&account_1_sub_account, (782) * ONE_TOKEN, 0, DOT);
    });
}
