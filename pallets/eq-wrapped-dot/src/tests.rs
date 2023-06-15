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
use eq_primitives::balance::{BalanceGetter, EqCurrency};
use eq_primitives::mocks::XcmRouterCachedMessagesMock;
use eq_primitives::wrapped_dot::EqDotPrice;
use eq_primitives::SignedBalance;
use eq_utils::ONE_TOKEN;
use eq_xcm::relay_interface::call::{RelayChainCall, StakingCall};
use frame_support::{assert_err, assert_ok};

pub const ONE_DOT: Balance = 10_000_000_000;

pub type ModuleWrappedDot = Pallet<Test>;
pub type ModuleBalances = eq_balances::Pallet<Test>;

fn init_wrapped_dot_supply() {
    let account_id = 777u64;
    let balance = 1500 * ONE_TOKEN;

    ModuleBalances::make_free_balance_be(
        &account_id,
        asset::EQDOT,
        SignedBalance::Positive(balance),
    );

    CurrentBalance::<Test>::put(StakingBalance {
        transferable: 200 * ONE_TOKEN,
        staked: 1000 * ONE_TOKEN,
    });

    let current_balance = CurrentBalance::<Test>::get();
    assert_eq!(current_balance.transferable, 200 * ONE_TOKEN);
    assert_eq!(current_balance.staked, 1000 * ONE_TOKEN);
}

#[test]
fn get_price_should_work() {
    new_test_ext().execute_with(|| {
        let price_coeff: Option<FixedI64> = ModuleWrappedDot::get_price_coeff();
        assert!(price_coeff.is_none());

        let account_id = 1u64;
        let balance = 1000u128;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQDOT,
            SignedBalance::Positive(balance),
        );
        let price_coeff: Option<FixedI64> = ModuleWrappedDot::get_price_coeff();
        assert_eq!(price_coeff, Some(FixedI64::zero()));

        let staking_state = StakingBalance {
            transferable: 1000u128,
            staked: 100u128,
        };

        CurrentBalance::<Test>::put(staking_state);
        let price_coeff: Option<FixedI64> = ModuleWrappedDot::get_price_coeff();
        assert_eq!(
            price_coeff.map(|c| c * OracleMock::get_price(&asset::DOT).expect("Has price")),
            Some(FixedI64::from_rational(4400, balance) * EqDotWithdrawFee::get().into())
        );
    });
}

#[test]
fn deposit_should_work() {
    new_test_ext().execute_with(|| {
        init_wrapped_dot_supply();
        let account_id = 1u64;
        let deposit_amount = 100u128 * ONE_TOKEN;

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::DOT,
            SignedBalance::Positive(deposit_amount),
        );

        let balance_before = CurrentBalance::<Test>::get();
        let mint_amount = Pallet::<Test>::calc_mint_wrapped_amount(deposit_amount).unwrap();

        assert_ok!(ModuleWrappedDot::deposit(
            RuntimeOrigin::signed(account_id),
            deposit_amount
        ));

        let balance_after = CurrentBalance::<Test>::get();
        assert_eq!(
            balance_after.transferable,
            balance_before.transferable + deposit_amount
        );
        assert_eq!(balance_after.staked, balance_before.staked);
        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::EQDOT),
            SignedBalance::Positive(mint_amount)
        );
    });
}

#[test]
fn deposit_should_fail_when_insufficient_deposit() {
    new_test_ext().execute_with(|| {
        init_wrapped_dot_supply();
        let account_id = 1u64;
        let deposit_amount = 2u128 * ONE_TOKEN;

        assert_err!(
            ModuleWrappedDot::deposit(RuntimeOrigin::signed(account_id), deposit_amount),
            Error::<Test>::InsufficientDeposit
        );
    });
}

#[test]
fn calc_mint_amount_should_work() {
    new_test_ext().execute_with(|| {
        init_wrapped_dot_supply();
        let deposit_amount = 10u128 * ONE_TOKEN;

        /*
        eqdot amount = (total_supply / total + staked) * deposit_amount = 1500 / 1200 * 10 = 1.25 DOT
        */
        assert_eq!(
            Pallet::<Test>::calc_mint_wrapped_amount(deposit_amount),
            Ok(12500000000)
        );
    });
}

#[test]
fn calc_mint_amount_should_fail_when_no_supply() {
    new_test_ext().execute_with(|| {
        let deposit_amount = 2u128 * ONE_TOKEN;

        assert_err!(
            Pallet::<Test>::calc_mint_wrapped_amount(deposit_amount),
            Error::<Test>::MathError
        );
    });
}

#[test]
fn withdraw_should_fail_when_insufficient_amount() {
    new_test_ext().execute_with(|| {
        init_wrapped_dot_supply();

        let account_id = 1u64;
        let withdraw_dot = 4u128 * ONE_TOKEN;
        assert_err!(
            ModuleWrappedDot::withdraw(
                RuntimeOrigin::signed(account_id),
                WithdrawAmount::Dot(withdraw_dot)
            ),
            Error::<Test>::InsufficientWithdraw
        );

        let withdraw_wrapped_dot = 1u128 * ONE_TOKEN;
        assert_err!(
            ModuleWrappedDot::withdraw(
                RuntimeOrigin::signed(account_id),
                WithdrawAmount::EqDot(withdraw_wrapped_dot)
            ),
            Error::<Test>::InsufficientWithdraw
        );
    });
}

#[test]
fn withdraw_dot_amount_when_transferable_enough() {
    new_test_ext().execute_with(|| {
        init_wrapped_dot_supply();
        let account_id = 1u64;
        let withdraw_amount = 50 * ONE_TOKEN;

        let initial_wrapped = 100 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQDOT,
            SignedBalance::Positive(initial_wrapped),
        );

        let amount_to_burn = Pallet::<Test>::calc_burn_wrapped_amount(withdraw_amount).unwrap();

        assert_ok!(ModuleWrappedDot::withdraw(
            RuntimeOrigin::signed(account_id),
            WithdrawAmount::Dot(withdraw_amount)
        ));

        assert_eq!(CurrentBalance::<Test>::get().transferable, 150 * ONE_TOKEN);
        assert_eq!(CurrentBalance::<Test>::get().staked, 1000 * ONE_TOKEN);

        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::DOT),
            SignedBalance::Positive(withdraw_amount)
        );

        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::EQDOT),
            SignedBalance::Positive(initial_wrapped - amount_to_burn)
        );
    });
}

#[test]
fn withdraw_eqdot_amount_when_transferable_enough() {
    new_test_ext().execute_with(|| {
        init_wrapped_dot_supply();
        let account_id = 1u64;

        let withdraw_eqdot_amount = 50 * ONE_TOKEN;

        let initial_wrapped = 100 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQDOT,
            SignedBalance::Positive(initial_wrapped),
        );

        let amount_to_deposit = Pallet::<Test>::calc_deposit_amount(withdraw_eqdot_amount).unwrap();
        let transferable_before = CurrentBalance::<Test>::get().transferable;
        assert_ok!(ModuleWrappedDot::withdraw(
            RuntimeOrigin::signed(account_id),
            WithdrawAmount::EqDot(withdraw_eqdot_amount)
        ));

        assert_eq!(
            CurrentBalance::<Test>::get().transferable,
            transferable_before - amount_to_deposit
        );
        assert_eq!(CurrentBalance::<Test>::get().staked, 1000 * ONE_TOKEN);

        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::DOT),
            SignedBalance::Positive(amount_to_deposit)
        );

        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::EQDOT),
            SignedBalance::Positive(initial_wrapped - withdraw_eqdot_amount)
        );
    });
}

#[test]
fn withdraw_dot_send_xcm_when_transferable_not_enough() {
    new_test_ext().execute_with(|| {
        init_wrapped_dot_supply();
        let account_id = 1u64;
        let pallet_account_id = <Test as Config>::PalletId::get().into_account_truncating();
        let withdraw_dot_amount = 210 * ONE_TOKEN;

        let initial_wrapped = 400 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQDOT,
            SignedBalance::Positive(initial_wrapped),
        );

        let amount_to_burn = Pallet::<Test>::calc_burn_wrapped_amount(withdraw_dot_amount).unwrap();
        let amount_to_burn_without_fee = <Test as Config>::WithdrawFee::get() * amount_to_burn;

        assert_ok!(ModuleWrappedDot::withdraw(
            RuntimeOrigin::signed(account_id),
            WithdrawAmount::Dot(withdraw_dot_amount)
        ));

        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::EQDOT),
            SignedBalance::Positive(initial_wrapped - amount_to_burn_without_fee)
        );

        assert_eq!(
            ModuleBalances::get_balance(&pallet_account_id, &asset::EQDOT),
            SignedBalance::Positive(amount_to_burn_without_fee)
        );

        let withdraw_queue = WithdrawQueue::<Test>::get();
        assert_eq!(withdraw_queue.len(), 1);
        assert_eq!(
            withdraw_queue[withdraw_queue.len() - 1],
            (account_id, withdraw_dot_amount, amount_to_burn_without_fee)
        );

        let unbond_amount = balance_into_xcm(withdraw_dot_amount, DOT_DECIMALS).unwrap();
        let call = RelayChainCall::Staking(StakingCall::Unbond(unbond_amount));
        assert_extrinsic_sent(call);
    });
}

#[test]
fn withdraw_eqdot_send_xcm_when_transferable_not_enough() {
    new_test_ext().execute_with(|| {
        init_wrapped_dot_supply();
        let account_id = 1u64;
        let pallet_account_id = <Test as Config>::PalletId::get().into_account_truncating();
        let withdraw_eqdot_amount = 350 * ONE_TOKEN;

        let initial_wrapped = 400 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQDOT,
            SignedBalance::Positive(initial_wrapped),
        );

        let deposit_amount = Pallet::<Test>::calc_deposit_amount(withdraw_eqdot_amount).unwrap();
        let deposit_amount_without_fee =
            <Test as Config>::WithdrawFee::get().saturating_reciprocal_mul(deposit_amount);

        assert_ok!(ModuleWrappedDot::withdraw(
            RuntimeOrigin::signed(account_id),
            WithdrawAmount::EqDot(withdraw_eqdot_amount)
        ));

        assert_eq!(
            ModuleBalances::get_balance(&account_id, &asset::EQDOT),
            SignedBalance::Positive(initial_wrapped - withdraw_eqdot_amount)
        );

        assert_eq!(
            ModuleBalances::get_balance(&pallet_account_id, &asset::EQDOT),
            SignedBalance::Positive(withdraw_eqdot_amount)
        );

        let withdraw_queue = WithdrawQueue::<Test>::get();
        assert_eq!(withdraw_queue.len(), 1);
        assert_eq!(
            withdraw_queue[withdraw_queue.len() - 1],
            (
                account_id,
                deposit_amount_without_fee,
                withdraw_eqdot_amount
            )
        );

        let unbond_amount = balance_into_xcm(deposit_amount_without_fee, DOT_DECIMALS).unwrap();
        let call = RelayChainCall::Staking(StakingCall::Unbond(unbond_amount));
        assert_extrinsic_sent(call);
    });
}

#[test]
fn withdraw_clear_queue() {
    new_test_ext().execute_with(|| {
        init_wrapped_dot_supply();
        let account_id = 1u64;
        // let pallet_account_id = WrappedDotPalletId::get().into_account_truncating();
        let withdraw_eqdot_amount = 350 * ONE_TOKEN;

        let initial_wrapped = 350 * ONE_TOKEN;
        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQDOT,
            SignedBalance::Positive(initial_wrapped),
        );

        assert_ok!(ModuleWrappedDot::withdraw(
            RuntimeOrigin::signed(account_id),
            WithdrawAmount::EqDot(withdraw_eqdot_amount)
        ));

        let deposit_amount = Pallet::<Test>::calc_deposit_amount(withdraw_eqdot_amount).unwrap();
        let deposit_amount_without_fee =
            <Test as Config>::WithdrawFee::get().saturating_reciprocal_mul(deposit_amount);

        let withdraw_queue = WithdrawQueue::<Test>::get();
        assert_eq!(withdraw_queue.len(), 1);
        assert_eq!(
            withdraw_queue[withdraw_queue.len() - 1],
            (
                account_id,
                deposit_amount_without_fee,
                withdraw_eqdot_amount
            )
        );

        let new_account_id = 2u64;
        assert_ok!(ModuleWrappedDot::deposit(
            RuntimeOrigin::signed(new_account_id),
            100 * ONE_TOKEN
        ));

        let mut current_balance = CurrentBalance::<Test>::get();
        let transferable_before_queue_processing = current_balance.transferable;
        let _ = ModuleWrappedDot::clear_withdraw_queue(&mut current_balance);

        let withdraw_queue = WithdrawQueue::<Test>::get();
        assert_eq!(withdraw_queue.len(), 0);

        assert_eq!(
            current_balance,
            StakingBalance {
                transferable: transferable_before_queue_processing - deposit_amount_without_fee,
                staked: 1000 * ONE_TOKEN,
            }
        );
    });
}

#[test]
fn calc_burn_wrapped_amount_should_work() {
    new_test_ext().execute_with(|| {
        let account_id = 777u64;

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQDOT,
            SignedBalance::Positive(100 * ONE_TOKEN),
        );

        CurrentBalance::<Test>::put(StakingBalance {
            transferable: 20 * ONE_TOKEN,
            staked: 100 * ONE_TOKEN,
        });

        let withdraw_amount = 10 * ONE_TOKEN;
        assert_eq!(
            Pallet::<Test>::calc_burn_wrapped_amount(withdraw_amount),
            Ok(8422536410)
        );
    });
}

#[test]
fn calc_deposit_amount_should_work() {
    new_test_ext().execute_with(|| {
        let account_id = 777u64;

        ModuleBalances::make_free_balance_be(
            &account_id,
            asset::EQDOT,
            SignedBalance::Positive(100 * ONE_TOKEN),
        );

        CurrentBalance::<Test>::put(StakingBalance {
            transferable: 20 * ONE_TOKEN,
            staked: 100 * ONE_TOKEN,
        });

        let burn_wrapped_dot_amount = 10 * ONE_TOKEN;
        assert_eq!(
            Pallet::<Test>::calc_deposit_amount(burn_wrapped_dot_amount),
            Ok(11872908000)
        );
    });
}

#[test]
fn rebalance_staking() {
    new_test_ext().execute_with(|| {
        let mut staking_balance = StakingBalance {
            transferable: 250 * ONE_TOKEN,
            staked: 750 * ONE_TOKEN,
        }; // RC = 25%, needs to reduce

        assert_ok!(ModuleWrappedDot::rebalance_staking(&mut staking_balance));
        assert_eq!(
            staking_balance,
            StakingBalance {
                transferable: 150 * ONE_TOKEN,
                staked: 850 * ONE_TOKEN,
            }
        );
        assert_extrinsic_sent(RelayChainCall::Staking(StakingCall::BondExtra(
            100 * ONE_DOT,
        )));
        XcmRouterCachedMessagesMock::clear();

        let mut staking_balance = StakingBalance {
            transferable: 50 * ONE_TOKEN,
            staked: 950 * ONE_TOKEN,
        }; // RC = 5%, needs to increase

        assert_eq!(TotalUnlocking::<Test>::get(), 0);

        assert_ok!(ModuleWrappedDot::rebalance_staking(&mut staking_balance));
        // unbond call should increase TotalUnlocking, but shouldn't change staked/transferable

        assert_eq!(
            staking_balance,
            StakingBalance {
                transferable: 50 * ONE_TOKEN,
                staked: 950 * ONE_TOKEN,
            }
        );
        assert_eq!(TotalUnlocking::<Test>::get(), 100 * ONE_TOKEN);

        assert_extrinsic_sent(RelayChainCall::Staking(StakingCall::Unbond(100 * ONE_DOT)));
        XcmRouterCachedMessagesMock::clear();

        let mut staking_balance = StakingBalance {
            transferable: 200 * ONE_TOKEN,
            staked: 800 * ONE_TOKEN,
        }; // RC = 20%, no need to change

        assert_ok!(ModuleWrappedDot::rebalance_staking(&mut staking_balance));
        assert_eq!(
            staking_balance,
            StakingBalance {
                transferable: 200 * ONE_TOKEN,
                staked: 800 * ONE_TOKEN,
            }
        );
        assert_eq!(XcmRouterCachedMessagesMock::get(), vec![]);
    });
}
