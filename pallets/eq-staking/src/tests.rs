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

use core::convert::TryInto;

use crate::{mock::*, Error, Pallet, Rewards, Stake, StakePeriod, Stakes, STAKING_ID};
use eq_primitives::{
    asset,
    balance::{BalanceGetter, EqCurrency, LockGetter},
    SignedBalance,
};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use frame_system::RawOrigin;
use sp_runtime::traits::Zero;

#[test]
fn stake_ok() {
    new_test_ext().execute_with(|| {
        let accounts = [ACCOUNT_1, ACCOUNT_2, ACCOUNT_3];
        let periods = [
            StakePeriod::One,
            StakePeriod::Two,
            StakePeriod::Three,
            StakePeriod::Six,
            StakePeriod::Twelve,
            StakePeriod::Eighteen,
            StakePeriod::TwentyFour,
            StakePeriod::One,
            StakePeriod::Three,
            StakePeriod::Six,
        ];
        assert_eq!(
            periods.len(),
            MaxStakesCount::get() as usize,
            "Test configuration"
        );
        for account in accounts {
            assert_eq!(
                eq_balances::Pallet::<Test>::get_lock(account, STAKING_ID),
                0,
                "Test configuration"
            );
        }
        let stake = 500 * ONE_TOKEN;
        for i in 0..MaxStakesCount::get() as usize {
            for account in accounts {
                assert_ok!(Pallet::<Test>::stake(
                    RuntimeOrigin::signed(account),
                    stake,
                    periods[i]
                ));

                assert_eq!(Stakes::<Test>::get(account).len(), i + 1);
                assert_eq!(
                    eq_balances::Pallet::<Test>::get_lock(account, STAKING_ID),
                    (i + 1) as u128 * stake
                );
            }
        }

        let expected_stakes: Vec<Stake<Balance>> = periods
            .iter()
            .map(|&period| Stake {
                period,
                amount: stake,
                start: 0,
            })
            .collect();
        let expected_stakes: BoundedVec<Stake<Balance>, MaxStakesCount> =
            expected_stakes.try_into().unwrap();
        for account in accounts {
            assert_eq!(
                Stakes::<Test>::get(account)
                    .into_iter()
                    .fold(0, |acc, s| acc + s.amount),
                eq_balances::Pallet::<Test>::get_lock(account, STAKING_ID)
            );

            assert_eq!(Stakes::<Test>::get(account), expected_stakes);
        }
    });
}

#[test]
fn stake_max_number_err() {
    new_test_ext().execute_with(|| {
        let accounts = [ACCOUNT_1, ACCOUNT_2, ACCOUNT_3];
        let stake = 500 * ONE_TOKEN;
        for _ in 0..MaxStakesCount::get() as usize {
            for account in accounts {
                assert_ok!(Pallet::<Test>::stake(
                    RuntimeOrigin::signed(account),
                    stake,
                    StakePeriod::Two
                ));
            }
        }

        for account in accounts {
            assert_noop!(
                Pallet::<Test>::stake(RuntimeOrigin::signed(account), stake, StakePeriod::Two),
                Error::<Test>::MaxStakesNumberReached
            );
        }
    });
}

#[test]
fn stake_insufficient_funds_err() {
    new_test_ext().execute_with(|| {
        let account_with_no_balance = 1;
        assert_eq!(
            eq_balances::Pallet::<Test>::get_balance(&account_with_no_balance, &asset::EQ),
            SignedBalance::zero()
        );

        assert_noop!(
            Pallet::<Test>::stake(
                RuntimeOrigin::signed(account_with_no_balance),
                500 * ONE_TOKEN,
                StakePeriod::One
            ),
            Error::<Test>::InsufficientFunds
        );

        assert_ok!(Pallet::<Test>::stake(
            RuntimeOrigin::signed(ACCOUNT_1),
            BALANCE,
            StakePeriod::Three
        ));

        assert_noop!(
            Pallet::<Test>::stake(RuntimeOrigin::signed(ACCOUNT_1), 1, StakePeriod::Two),
            Error::<Test>::InsufficientFunds
        );
    });
}

#[test]
fn stake_has_no_effect_on_free_balance() {
    new_test_ext().execute_with(|| {
        let free_balance_before = eq_balances::Pallet::<Test>::free_balance(&ACCOUNT_1, asset::EQ);
        assert_eq!(free_balance_before, BALANCE);

        assert_ok!(Pallet::<Test>::stake(
            RuntimeOrigin::signed(ACCOUNT_1),
            BALANCE,
            StakePeriod::Six
        ));

        assert_eq!(
            eq_balances::Pallet::<Test>::free_balance(&ACCOUNT_1, asset::EQ),
            free_balance_before
        );
    });
}

#[test]
fn reward_ok() {
    new_test_ext().execute_with(|| {
        let periods = [StakePeriod::One, StakePeriod::Two, StakePeriod::Six];
        let stake = 1000 * ONE_TOKEN;
        let account_with_stake = ACCOUNT_1;
        let account_no_stake = ACCOUNT_2;
        for period in periods {
            assert_ok!(Pallet::<Test>::stake(
                RuntimeOrigin::signed(account_with_stake),
                stake,
                period
            ));
        }

        assert_eq!(
            Stakes::<Test>::get(account_no_stake).len(),
            0,
            "Test configuration"
        );

        let stakes_before = Stakes::<Test>::get(account_with_stake);

        let mut external_id_counter = EXTERNAL_ID;

        let reward = ONE_TOKEN;
        for _ in 0..10 {
            for acc in [account_with_stake, account_no_stake] {
                let balance = eq_balances::Pallet::<Test>::get_balance(&acc, &asset::EQ);
                let lock = eq_balances::Pallet::<Test>::get_lock(acc, STAKING_ID);
                let rewards_before = Rewards::<Test>::get(acc).unwrap_or(Stake {
                    start: 0,
                    amount: 0,
                    period: RewardsLockPeriod::get(),
                });
                assert_ok!(Pallet::<Test>::reward(
                    RawOrigin::Root.into(),
                    acc,
                    reward,
                    external_id_counter,
                ));
                external_id_counter += 1;
                assert_eq!(
                    eq_balances::Pallet::<Test>::get_balance(&acc, &asset::EQ),
                    balance.add_balance(&reward).unwrap()
                );
                assert_eq!(
                    eq_balances::Pallet::<Test>::get_lock(acc, STAKING_ID),
                    lock + reward
                );
                assert_eq!(
                    Rewards::<Test>::get(acc).unwrap(),
                    Stake {
                        amount: rewards_before.amount + reward,
                        start: rewards_before.start,
                        period: RewardsLockPeriod::get(),
                    }
                );
            }
        }

        assert_eq!(Stakes::<Test>::get(account_with_stake), stakes_before);
        assert_eq!(Stakes::<Test>::get(account_no_stake).len(), 0);

        pallet_timestamp::Pallet::<Test>::set_timestamp(RewardsLockPeriod::get().as_secs() * 1000);
        let now = pallet_timestamp::Pallet::<Test>::now();

        for acc in [account_with_stake, account_no_stake] {
            let balance = eq_balances::Pallet::<Test>::get_balance(&acc, &asset::EQ);
            let lock = eq_balances::Pallet::<Test>::get_lock(acc, STAKING_ID);
            let rewards_before = Rewards::<Test>::get(acc).unwrap();
            assert_ok!(Pallet::<Test>::reward(
                RawOrigin::Root.into(),
                acc,
                reward,
                external_id_counter,
            ));
            external_id_counter += 1;
            assert_eq!(
                eq_balances::Pallet::<Test>::get_balance(&acc, &asset::EQ),
                balance.add_balance(&reward).unwrap()
            );
            assert_eq!(
                eq_balances::Pallet::<Test>::get_lock(acc, STAKING_ID),
                lock - rewards_before.amount + reward
            );
            assert_eq!(
                Rewards::<Test>::get(acc).unwrap(),
                Stake {
                    amount: reward,
                    start: now / 1000,
                    period: RewardsLockPeriod::get(),
                }
            );
        }

        assert_eq!(Stakes::<Test>::get(account_with_stake), stakes_before);
        assert_eq!(Stakes::<Test>::get(account_no_stake).len(), 0);
    });
}

#[test]
fn reward_increases_free_balance() {
    new_test_ext().execute_with(|| {
        let free_balance_before = eq_balances::Pallet::<Test>::free_balance(&ACCOUNT_1, asset::EQ);
        let reward = ONE_TOKEN;
        assert_ok!(Pallet::<Test>::reward(
            RawOrigin::Root.into(),
            ACCOUNT_1,
            reward,
            EXTERNAL_ID,
        ));

        assert_eq!(
            eq_balances::Pallet::<Test>::free_balance(&ACCOUNT_1, asset::EQ),
            free_balance_before + reward
        );
    });
}

#[test]
fn unlock_stakes_ok() {
    new_test_ext().execute_with(|| {
        let periods = [
            StakePeriod::One,
            StakePeriod::Two,
            StakePeriod::Three,
            StakePeriod::Six,
            StakePeriod::Twelve,
            StakePeriod::Eighteen,
            StakePeriod::TwentyFour,
        ];
        let stake = 500 * ONE_TOKEN;

        for period in periods {
            assert_ok!(Pallet::<Test>::stake(
                RuntimeOrigin::signed(ACCOUNT_1),
                stake,
                period
            ));
        }

        let balance_before = eq_balances::Pallet::<Test>::get_balance(&ACCOUNT_1, &asset::EQ);

        for i in 0..periods.len() {
            let mut stake = Stakes::<Test>::get(ACCOUNT_1);
            let stake_lock_before = eq_balances::Pallet::<Test>::get_lock(ACCOUNT_1, STAKING_ID);
            pallet_timestamp::Pallet::<Test>::set_timestamp(periods[i].as_secs() * 1000);
            assert_ok!(Pallet::<Test>::unlock(
                RuntimeOrigin::signed(ACCOUNT_1),
                Some(0 as u32)
            ));
            assert_eq!(
                eq_balances::Pallet::<Test>::get_lock(ACCOUNT_1, STAKING_ID),
                stake_lock_before - stake[0].amount
            );
            stake.remove(0);
            assert_eq!(Stakes::<Test>::get(ACCOUNT_1), stake);
        }

        assert_eq!(
            balance_before,
            eq_balances::Pallet::<Test>::get_balance(&ACCOUNT_1, &asset::EQ)
        );
    });
}

#[test]
fn unlock_rewards_ok() {
    new_test_ext().execute_with(|| {
        let periods = [
            StakePeriod::One,
            StakePeriod::Two,
            StakePeriod::Three,
            StakePeriod::Six,
            StakePeriod::Twelve,
            StakePeriod::Eighteen,
            StakePeriod::TwentyFour,
        ];
        let stake = 500 * ONE_TOKEN;
        let reward = ONE_TOKEN;
        let account_with_stake = ACCOUNT_1;
        let account_no_stake = ACCOUNT_2;
        let accounts = [account_with_stake, account_no_stake];

        for period in periods {
            assert_ok!(Pallet::<Test>::stake(
                RuntimeOrigin::signed(account_with_stake),
                stake,
                period
            ));
        }

        for i in 0..accounts.len() {
            let acc = accounts[i];
            assert!(Rewards::<Test>::get(acc).is_none());
            assert_ok!(Pallet::<Test>::reward(
                RawOrigin::Root.into(),
                acc,
                reward,
                EXTERNAL_ID + i as u64,
            ));
            let reward_stake_lock_before = eq_balances::Pallet::<Test>::get_lock(acc, STAKING_ID);
            let balance_before = eq_balances::Pallet::<Test>::get_balance(&acc, &asset::EQ);
            let stakes_before = Stakes::<Test>::get(acc);
            let now = pallet_timestamp::Pallet::<Test>::now();
            pallet_timestamp::Pallet::<Test>::set_timestamp(
                now + RewardsLockPeriod::get().as_secs() * 1000,
            );
            assert_ok!(Pallet::<Test>::unlock(RuntimeOrigin::signed(acc), None));
            assert_eq!(
                eq_balances::Pallet::<Test>::get_lock(acc, STAKING_ID),
                reward_stake_lock_before - reward
            );
            assert!(Rewards::<Test>::get(acc).is_none());
            assert_eq!(
                balance_before,
                eq_balances::Pallet::<Test>::get_balance(&acc, &asset::EQ)
            );
            assert_eq!(stakes_before, Stakes::<Test>::get(acc));
        }
    });
}

#[test]
fn unlock_stakes_err() {
    new_test_ext().execute_with(|| {
        let periods = [StakePeriod::One, StakePeriod::Three, StakePeriod::Six];
        let stake = 500 * ONE_TOKEN;
        let accounts = [ACCOUNT_1, ACCOUNT_2, ACCOUNT_3];

        for i in 0..accounts.len() {
            let acc = accounts[i];
            for p in i..periods.len() {
                assert_noop!(
                    Pallet::<Test>::unlock(RuntimeOrigin::signed(acc), Some((p - i) as u32)),
                    Error::<Test>::StakeNotFound
                );
                assert_ok!(Pallet::<Test>::stake(
                    RuntimeOrigin::signed(acc),
                    stake,
                    periods[p]
                ));
            }
            assert_noop!(
                Pallet::<Test>::unlock(
                    RuntimeOrigin::signed(acc),
                    Some((periods.len() - i) as u32)
                ),
                Error::<Test>::StakeNotFound
            );
            for p in i..periods.len() {
                assert_noop!(
                    Pallet::<Test>::unlock(RuntimeOrigin::signed(acc), Some((p - i) as u32)),
                    Error::<Test>::LockPeriodNotEnded
                );
            }
        }
    });
}

#[test]
fn unlock_rewards_err() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            eq_balances::Pallet::<Test>::get_lock(ACCOUNT_1, STAKING_ID),
            0,
            "Test configuration"
        );
        assert_noop!(
            Pallet::<Test>::unlock(RuntimeOrigin::signed(ACCOUNT_1), None),
            Error::<Test>::StakeNotFound
        );
        let reward = ONE_TOKEN;
        assert_ok!(Pallet::<Test>::reward(
            RawOrigin::Root.into(),
            ACCOUNT_1,
            reward,
            EXTERNAL_ID,
        ));
        assert_noop!(
            Pallet::<Test>::unlock(RuntimeOrigin::signed(ACCOUNT_1), None),
            Error::<Test>::LockPeriodNotEnded
        );
        pallet_timestamp::Pallet::<Test>::set_timestamp(
            (RewardsLockPeriod::get().as_secs() - 1) * 1000,
        );
        assert_noop!(
            Pallet::<Test>::unlock(RuntimeOrigin::signed(ACCOUNT_1), None),
            Error::<Test>::LockPeriodNotEnded
        );
    });
}
