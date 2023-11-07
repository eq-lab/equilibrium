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

use super::Error;
use crate::mock::{
    new_test_ext, transfers_disabled_test_ext, ModuleBalances, ModuleVesting, RuntimeOrigin,
    System, Test,
};
use eq_primitives::balance::EqCurrency;
use eq_primitives::vestings::EqVestingSchedule;
use eq_primitives::{asset, balance::BalanceGetter, SignedBalance};
use eq_utils::fx128;
use frame_support::pallet_prelude::Hooks;
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use sp_arithmetic::FixedI128;
use sp_runtime::traits::BadOrigin;

fn set_pos_balance_with_agg_unsafe(who: &u64, currency: &asset::Asset, amount: FixedI128) {
    let balance = SignedBalance::Positive(amount.into_inner() as u128);
    ModuleBalances::make_free_balance_be(who, *currency, balance);
}

#[test]
fn vest_no_vesting() {
    new_test_ext().execute_with(|| {
        let module_account_id = ModuleVesting::account_id();
        let account_id = 1;

        assert_err!(
            ModuleVesting::vest(RuntimeOrigin::signed(account_id),),
            Error::<Test>::NotVesting
        );

        assert_eq!(ModuleVesting::vesting(1), Option::None);

        assert_eq!(
            <ModuleBalances as BalanceGetter<u64, u128>>::get_balance(&1, &asset::EQ),
            eq_primitives::SignedBalance::Positive(fx128!(0, 0).into_inner() as u128)
        );
        assert_eq!(
            <ModuleBalances as BalanceGetter<u64, u128>>::get_balance(
                &module_account_id,
                &asset::EQ
            ),
            eq_primitives::SignedBalance::Positive(fx128!(0, 0).into_inner() as u128)
        );
        assert_eq!(
            <ModuleBalances as BalanceGetter<u64, u128>>::get_balance(&2, &asset::EQ),
            eq_primitives::SignedBalance::Positive(0)
        );
    });
}

#[test]
fn add_vesting_schedule() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let vesting_info = super::VestingInfo {
            locked: fx128!(10, 0).into_inner() as u128,
            per_block: fx128!(1, 0).into_inner() as u128,
            starting_block: 10,
        };

        let who = 1;
        let refs_before = frame_system::Pallet::<Test>::providers(&who);

        assert_ok!(
            <ModuleVesting as EqVestingSchedule<u128, u64>>::add_vesting_schedule(
                &who,
                vesting_info.locked,
                vesting_info.per_block,
                vesting_info.starting_block
            )
        );
        assert_eq!(ModuleVesting::vesting(who), Option::Some(vesting_info));
        assert_eq!(
            frame_system::Pallet::<Test>::providers(&who),
            refs_before + 1
        );

        assert_err!(
            <ModuleVesting as EqVestingSchedule<u128, u64>>::add_vesting_schedule(
                &who,
                vesting_info.locked,
                vesting_info.per_block,
                vesting_info.starting_block
            ),
            Error::<Test>::ExistingVestingSchedule
        );
    });
}

#[test]
fn add_zero_vesting_schedule() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(
            <ModuleVesting as EqVestingSchedule<u128, u64>>::add_vesting_schedule(
                &1,
                0,
                fx128!(10, 0).into_inner() as u128,
                10
            )
        );
        assert_eq!(ModuleVesting::vesting(1), Option::None);
    });
}

#[test]
fn forced_vested_transfer() {
    new_test_ext().execute_with(|| {
        let module_account_id = ModuleVesting::account_id();
        let account_id = 1;
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQ, fx128!(100, 0));

        System::set_block_number(1);

        let vesting_info = super::VestingInfo {
            locked: fx128!(10, 0).into_inner() as u128,
            per_block: fx128!(1, 0).into_inner() as u128,
            starting_block: 10,
        };
        assert_err!(
            ModuleVesting::force_vested_transfer(Some(1).into(), 1, 2, vesting_info),
            BadOrigin
        );

        assert_ok!(ModuleVesting::force_vested_transfer(
            RawOrigin::Root.into(),
            1,
            2,
            vesting_info
        ));

        System::set_block_number(11);

        assert_ok!(ModuleVesting::vest(RuntimeOrigin::signed(2),));

        assert_eq!(
            <ModuleBalances as BalanceGetter<u64, u128>>::get_balance(&1, &asset::EQ),
            eq_primitives::SignedBalance::Positive(fx128!(90, 0).into_inner() as u128)
        );
        assert_eq!(
            <ModuleBalances as BalanceGetter<u64, u128>>::get_balance(
                &module_account_id,
                &asset::EQ
            ),
            eq_primitives::SignedBalance::Positive(fx128!(9, 0).into_inner() as u128)
        );
        assert_eq!(
            <ModuleBalances as BalanceGetter<u64, u128>>::get_balance(&2, &asset::EQ),
            eq_primitives::SignedBalance::Positive(fx128!(1, 0).into_inner() as u128)
        );
    });
}

#[test]
fn forced_transfers_disabled() {
    transfers_disabled_test_ext().execute_with(|| {
        let module_account_id = ModuleVesting::account_id();
        let account_id = 1;
        set_pos_balance_with_agg_unsafe(&account_id, &asset::EQ, fx128!(100, 0));

        let vesting_info = super::VestingInfo {
            locked: fx128!(10, 0).into_inner() as u128,
            per_block: fx128!(1, 0).into_inner() as u128,
            starting_block: 10,
        };

        assert_err!(
            ModuleVesting::force_vested_transfer(RawOrigin::Root.into(), 1, 2, vesting_info),
            Error::<Test>::TransfersAreDisabled
        );

        assert_eq!(ModuleVesting::vesting(2), Option::None);
        assert_eq!(ModuleVesting::vested(2), Option::None);

        assert_eq!(
            <ModuleBalances as BalanceGetter<u64, u128>>::get_balance(&1, &asset::EQ),
            eq_primitives::SignedBalance::Positive(fx128!(100, 0).into_inner() as u128)
        );
        assert_eq!(
            <ModuleBalances as BalanceGetter<u64, u128>>::get_balance(
                &module_account_id,
                &asset::EQ
            ),
            eq_primitives::SignedBalance::Positive(fx128!(0, 0).into_inner() as u128)
        );
        assert_eq!(
            <ModuleBalances as BalanceGetter<u64, u128>>::get_balance(&2, &asset::EQ),
            eq_primitives::SignedBalance::Positive(0)
        );
    });
}

#[test]
fn does_not_remove_vestings_with_zero_value() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_eq!(ModuleVesting::accounts_per_block(), 0);

        let vesting_info = super::VestingInfo {
            locked: fx128!(10, 0).into_inner() as u128,
            per_block: fx128!(1, 0).into_inner() as u128,
            starting_block: 10,
        };

        let who = 1;

        assert_ok!(<ModuleVesting as frame_support::traits::VestingSchedule<
            u64,
        >>::add_vesting_schedule(
            &who,
            vesting_info.locked,
            vesting_info.per_block,
            vesting_info.starting_block
        ));
        assert_eq!(ModuleVesting::vesting(who), Option::Some(vesting_info));

        let refs_before = frame_system::Pallet::<Test>::providers(&who);
        ModuleVesting::on_initialize(2);

        assert_eq!(ModuleVesting::vesting(who), Some(vesting_info));
        assert_eq!(frame_system::Pallet::<Test>::providers(&who), refs_before);
    });
}

#[test]
fn remove_vesting_schedule() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(ModuleVesting::set_accounts_per_block_removed(
            RawOrigin::Root.into(),
            2
        ));

        let vesting_info = super::VestingInfo {
            locked: fx128!(10, 0).into_inner() as u128,
            per_block: fx128!(1, 0).into_inner() as u128,
            starting_block: 10,
        };

        let who = 1;

        assert_ok!(<ModuleVesting as frame_support::traits::VestingSchedule<
            u64,
        >>::add_vesting_schedule(
            &who,
            vesting_info.locked,
            vesting_info.per_block,
            vesting_info.starting_block
        ));
        assert_eq!(ModuleVesting::vesting(who), Option::Some(vesting_info));

        let refs_before = frame_system::Pallet::<Test>::providers(&who);
        ModuleVesting::on_initialize(2);

        assert!(ModuleVesting::vesting(who).is_none());
        assert_eq!(
            frame_system::Pallet::<Test>::providers(&who),
            refs_before - 1
        );
    });
}

#[test]
fn on_initialize_remove_correct_number_of_vesting_schedules() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(ModuleVesting::set_accounts_per_block_removed(
            RawOrigin::Root.into(),
            2
        ));

        let vesting_info = super::VestingInfo {
            locked: fx128!(10, 0).into_inner() as u128,
            per_block: fx128!(1, 0).into_inner() as u128,
            starting_block: 10,
        };
        let accounts: Vec<_> = (1..10).collect();
        let accounts_num = accounts.len() as u32;

        let refs_before: Vec<_> = accounts
            .iter()
            .map(|account| {
                assert_ok!(<ModuleVesting as frame_support::traits::VestingSchedule<
                    u64,
                >>::add_vesting_schedule(
                    account,
                    vesting_info.locked,
                    vesting_info.per_block,
                    vesting_info.starting_block
                ));
                assert!(ModuleVesting::vesting(account).is_some());
                frame_system::Pallet::<Test>::providers(account)
            })
            .collect();

        let removed_per_block = ModuleVesting::accounts_per_block();

        let iterations = (accounts_num + removed_per_block - 1) / removed_per_block;

        for iteration in 1..=iterations {
            ModuleVesting::on_initialize(2);
            let mut vestings_removed = 0;
            let mut vestings_left = accounts_num;
            for (index, account) in accounts.iter().enumerate() {
                if ModuleVesting::vesting(account).is_none() {
                    vestings_removed += 1;
                    vestings_left -= 1;
                    assert_eq!(
                        refs_before[index] - 1,
                        frame_system::Pallet::<Test>::providers(account)
                    )
                }
            }

            let should_be_removed = (removed_per_block * iteration).min(accounts_num);
            assert_eq!(vestings_removed, should_be_removed);
            assert_eq!(vestings_left, accounts_num - should_be_removed);
        }
    });
}
