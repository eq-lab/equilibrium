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
    new_test_ext, Balance, ModuleBalances, ModuleEqToQSwap, ModuleVesting, RuntimeOrigin, Test,
};
use crate::{EqSwapConfiguration, SwapConfiguration};
use eq_primitives::mocks::{TreasuryAccountMock, VestingAccountMock};
use eq_primitives::ONE_TOKEN;
use eq_vesting::VestingInfo;
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;

macro_rules! assert_balance {
    ($who:expr, $balance:expr, $debt:expr, $asset:expr) => {
        assert_eq!(
            ModuleBalances::total_balance(&$who, $asset),
            $balance,
            "assert balance failed"
        );
        assert_eq!(
            ModuleBalances::debt(&$who, $asset),
            $debt,
            "assert debt failed"
        );
    };
}

#[test]
fn set_config() {
    new_test_ext().execute_with(|| {
        let config_initial = EqSwapConfiguration::<Test>::get();

        assert_err!(
            ModuleEqToQSwap::set_config(
                RawOrigin::Root.into(),
                Some(true),
                Some(123),
                Some(123),
                Some(123u128),
                Some(Percent::one()),
                Some(10),
                None
            ),
            Error::<Test>::InvalidConfiguration
        );

        assert_err!(
            ModuleEqToQSwap::set_config(
                RawOrigin::Root.into(),
                Some(true),
                Some(0),
                Some(123),
                Some(123u128),
                Some(Percent::one()),
                Some(10),
                None
            ),
            Error::<Test>::InvalidConfiguration
        );

        assert_err!(
            ModuleEqToQSwap::set_config(
                RawOrigin::Root.into(),
                Some(true),
                Some(123),
                Some(0),
                Some(123u128),
                Some(Percent::one()),
                Some(10),
                None
            ),
            Error::<Test>::InvalidConfiguration
        );

        assert_ok!(ModuleEqToQSwap::set_config(
            RawOrigin::Root.into(),
            Some(true),
            Some(123),
            Some(234),
            Some(123u128),
            Some(Percent::one()),
            Some(10),
            Some(20)
        ));

        let config_after_1 = EqSwapConfiguration::<Test>::get();

        assert_ok!(ModuleEqToQSwap::set_config(
            RawOrigin::Root.into(),
            Some(false),
            Some(0),
            Some(123),
            Some(123u128),
            Some(Percent::one()),
            None,
            Some(0)
        ));

        assert_ok!(ModuleEqToQSwap::set_config(
            RawOrigin::Root.into(),
            Some(true),
            Some(12),
            Some(15),
            Some(200u128),
            Some(Percent::from_percent(80)),
            Some(20),
            Some(30)
        ));

        let config_after_2 = EqSwapConfiguration::<Test>::get();

        assert_eq!(
            config_initial,
            SwapConfiguration {
                enabled: false,
                min_amount: Balance::zero(),
                max_q_amount: Balance::zero(),
                eq_to_q_ratio: 0u128,
                vesting_share: Percent::default(),
                vesting_starting_block: 0u32,
                vesting_duration_blocks: Balance::zero()
            }
        );

        assert_eq!(
            config_after_1,
            SwapConfiguration {
                enabled: true,
                min_amount: Balance::from(123u128),
                max_q_amount: Balance::from(234u128),
                eq_to_q_ratio: 123u128,
                vesting_share: Percent::one(),
                vesting_starting_block: 10u32,
                vesting_duration_blocks: Balance::from(20u128)
            }
        );

        assert_eq!(
            config_after_2,
            SwapConfiguration {
                enabled: true,
                min_amount: Balance::from(12u128),
                max_q_amount: Balance::from(15u128),
                eq_to_q_ratio: 200u128,
                vesting_share: Percent::from_percent(80),
                vesting_starting_block: 20u32,
                vesting_duration_blocks: Balance::from(30u128)
            }
        );
    });
}

#[test]
fn swap_eq_to_q() {
    new_test_ext().execute_with(|| {
        let account_1: u64 = 1;
        let account_2: u64 = 2;
        let vesting_account_id = VestingAccountMock::get();
        let q_holder_account_id = TreasuryAccountMock::get();

        assert_err!(
            ModuleEqToQSwap::swap_eq_to_q(RuntimeOrigin::signed(account_1), 1000 * ONE_TOKEN),
            Error::<Test>::SwapsAreDisabled
        );

        assert_ok!(ModuleEqToQSwap::set_config(
            RawOrigin::Root.into(),
            Some(true),
            Some(100 * ONE_TOKEN),
            Some(1000 * ONE_TOKEN),
            Some(800_000_000),
            Some(Percent::from_percent(20)),
            Some(10),
            Some(20)
        ));

        assert_err!(
            ModuleEqToQSwap::swap_eq_to_q(RuntimeOrigin::signed(account_1), 99 * ONE_TOKEN),
            Error::<Test>::AmountTooSmall
        );

        assert_ok!(ModuleEqToQSwap::swap_eq_to_q(
            RuntimeOrigin::signed(account_1),
            800 * ONE_TOKEN
        ));

        let account_1_vesting = ModuleVesting::vesting(account_1).unwrap();
        let account_1_q_received = QReceivedAmounts::<Test>::get(account_1);

        assert_balance!(&vesting_account_id, 128 * ONE_TOKEN, 0, Q);
        assert_balance!(&account_1, 200 * ONE_TOKEN, 0, EQ);
        assert_balance!(&account_1, 512 * ONE_TOKEN, 0, Q);
        assert_balance!(&q_holder_account_id, (10_000 - 512 - 128) * ONE_TOKEN, 0, Q);
        assert_eq!(
            account_1_vesting,
            VestingInfo {
                locked: 128 * ONE_TOKEN,
                per_block: 6_400_000_000,
                starting_block: 10
            }
        );
        assert_eq!(account_1_q_received, 512 * ONE_TOKEN);

        assert_ok!(ModuleEqToQSwap::swap_eq_to_q(
            RuntimeOrigin::signed(account_1),
            200 * ONE_TOKEN
        ));

        let account_1_vesting = ModuleVesting::vesting(account_1).unwrap();
        let account_1_q_received = QReceivedAmounts::<Test>::get(account_1);

        assert_balance!(&vesting_account_id, (128 + 32) * ONE_TOKEN, 0, Q);
        assert_balance!(&account_1, 0, 0, EQ);
        assert_balance!(&account_1, (512 + 128) * ONE_TOKEN, 0, Q);
        assert_balance!(
            &q_holder_account_id,
            (10_000 - 512 - 128 - 128 - 32) * ONE_TOKEN,
            0,
            Q
        );
        assert_eq!(
            account_1_vesting,
            VestingInfo {
                locked: (128 + 32) * ONE_TOKEN,
                per_block: 8_000_000_000,
                starting_block: 10
            }
        );
        assert_eq!(account_1_q_received, (512 + 128) * ONE_TOKEN);

        assert_err!(
            ModuleEqToQSwap::swap_eq_to_q(RuntimeOrigin::signed(account_1), 0),
            Error::<Test>::AmountTooSmall
        );

        assert_ok!(ModuleEqToQSwap::set_config(
            RawOrigin::Root.into(),
            Some(true),
            Some(100 * ONE_TOKEN),
            Some(1000 * ONE_TOKEN),
            Some(1_500_000_000),
            Some(Percent::from_percent(0)),
            Some(10),
            Some(20)
        ));

        assert_ok!(ModuleEqToQSwap::swap_eq_to_q(
            RuntimeOrigin::signed(account_2),
            200 * ONE_TOKEN
        ));

        let account_2_vesting = ModuleVesting::vesting(account_2);
        let account_2_q_received = QReceivedAmounts::<Test>::get(account_2);

        assert_balance!(&vesting_account_id, (128 + 32) * ONE_TOKEN, 0, Q);
        assert_balance!(&account_2, 800 * ONE_TOKEN, 0, EQ);
        assert_balance!(&account_2, 300 * ONE_TOKEN, 0, Q);
        assert_balance!(
            &q_holder_account_id,
            (10_000 - 512 - 128 - 128 - 32 - 300) * ONE_TOKEN,
            0,
            Q
        );
        assert_eq!(account_2_vesting, None);
        assert_eq!(account_2_q_received, 300 * ONE_TOKEN);

        assert_ok!(ModuleEqToQSwap::set_config(
            RawOrigin::Root.into(),
            Some(true),
            Some(100 * ONE_TOKEN),
            Some(400 * ONE_TOKEN),
            Some(1_500_000_000),
            Some(Percent::from_percent(50)),
            Some(100),
            Some(50)
        ));

        assert_ok!(ModuleEqToQSwap::swap_eq_to_q(
            RuntimeOrigin::signed(account_2),
            100 * ONE_TOKEN
        ));

        let account_2_vesting = ModuleVesting::vesting(account_2).unwrap();
        let account_2_q_received = QReceivedAmounts::<Test>::get(account_2);

        assert_balance!(&vesting_account_id, (128 + 32 + 75) * ONE_TOKEN, 0, Q);
        assert_balance!(&account_2, 700 * ONE_TOKEN, 0, EQ);
        assert_balance!(&account_2, (300 + 75) * ONE_TOKEN, 0, Q);
        assert_balance!(
            &q_holder_account_id,
            (10_000 - 512 - 128 - 128 - 32 - 300 - 75 - 75) * ONE_TOKEN,
            0,
            Q
        );
        assert_eq!(
            account_2_vesting,
            VestingInfo {
                locked: 75 * ONE_TOKEN,
                per_block: 1_500_000_000,
                starting_block: 100
            }
        );
        assert_eq!(account_2_q_received, (300 + 75) * ONE_TOKEN);

        assert_err!(
            ModuleEqToQSwap::swap_eq_to_q(RuntimeOrigin::signed(account_2), 300 * ONE_TOKEN),
            Error::<Test>::QAmountExceeded
        );

        assert_ok!(ModuleEqToQSwap::set_config(
            RawOrigin::Root.into(),
            Some(false),
            None,
            None,
            None,
            None,
            None,
            None
        ));

        assert_err!(
            ModuleEqToQSwap::swap_eq_to_q(RuntimeOrigin::signed(account_2), 100 * ONE_TOKEN),
            Error::<Test>::SwapsAreDisabled
        );
    });
}

mod signed_extension {
    use super::*;
    use crate::mock::RuntimeCall;
    use frame_support::dispatch::DispatchInfo;

    pub fn info_from_weight(w: Weight) -> DispatchInfo {
        DispatchInfo {
            weight: w,
            ..Default::default()
        }
    }

    #[test]
    fn validate_should_skip_when_valid() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;

            assert_ok!(ModuleEqToQSwap::set_config(
                RawOrigin::Root.into(),
                Some(true),
                Some(100 * ONE_TOKEN),
                Some(1000 * ONE_TOKEN),
                Some(1_500_000_000),
                Some(Percent::from_percent(50)),
                Some(100),
                Some(50)
            ));

            let eq_to_q_swap_call = RuntimeCall::EqToQSwap(crate::Call::swap_eq_to_q {
                amount: 100 * ONE_TOKEN,
            });

            let check = CheckEqToQSwap::<Test>::new();
            let info = info_from_weight(Weight::zero());
            assert_ok!(check.validate(&account_id, &eq_to_q_swap_call, &info, 0));
        });
    }

    #[test]
    fn validate_should_fail_when_swap_disabled() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;

            let eq_to_q_swap_call = RuntimeCall::EqToQSwap(crate::Call::swap_eq_to_q {
                amount: 100 * ONE_TOKEN,
            });

            let check = CheckEqToQSwap::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &eq_to_q_swap_call, &info, 1),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    ValidityError::SwapsAreDisabled.into()
                ))
            );
        });
    }

    #[test]
    fn validate_should_fail_when_not_enough_balance() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;

            assert_ok!(ModuleEqToQSwap::set_config(
                RawOrigin::Root.into(),
                Some(true),
                Some(100 * ONE_TOKEN),
                Some(1000 * ONE_TOKEN),
                Some(1_500_000_000),
                Some(Percent::from_percent(50)),
                Some(100),
                Some(50)
            ));

            let eq_to_q_swap_call = RuntimeCall::EqToQSwap(crate::Call::swap_eq_to_q {
                amount: 10_000 * ONE_TOKEN,
            });

            let check = CheckEqToQSwap::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &eq_to_q_swap_call, &info, 1),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    ValidityError::NotEnoughBalance.into()
                ))
            );
        });
    }

    #[test]
    fn validate_should_fail_when_less_then_min_amount() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;

            assert_ok!(ModuleEqToQSwap::set_config(
                RawOrigin::Root.into(),
                Some(true),
                Some(100 * ONE_TOKEN),
                Some(1000 * ONE_TOKEN),
                Some(1_500_000_000),
                Some(Percent::from_percent(50)),
                Some(100),
                Some(50)
            ));

            let eq_to_q_swap_call = RuntimeCall::EqToQSwap(crate::Call::swap_eq_to_q {
                amount: 99 * ONE_TOKEN,
            });

            let check = CheckEqToQSwap::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &eq_to_q_swap_call, &info, 1),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    ValidityError::AmountTooSmall.into()
                ))
            );
        });
    }

    #[test]
    fn validate_should_fail_when_q_received_amount_exceeded() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;

            assert_ok!(ModuleEqToQSwap::set_config(
                RawOrigin::Root.into(),
                Some(true),
                Some(100 * ONE_TOKEN),
                Some(200 * ONE_TOKEN),
                Some(1_000_000_000),
                Some(Percent::from_percent(50)),
                Some(100),
                Some(50)
            ));

            let eq_to_q_swap_call_1 = RuntimeCall::EqToQSwap(crate::Call::swap_eq_to_q {
                amount: 200 * ONE_TOKEN,
            });

            let check = CheckEqToQSwap::<Test>::new();
            let info = info_from_weight(Weight::zero());
            assert_ok!(check.validate(&account_id, &eq_to_q_swap_call_1, &info, 0));

            let eq_to_q_swap_call_2 = RuntimeCall::EqToQSwap(crate::Call::swap_eq_to_q {
                amount: 210 * ONE_TOKEN,
            });

            assert_err!(
                check.validate(&account_id, &eq_to_q_swap_call_2, &info, 1),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    ValidityError::QAmountExceeded.into()
                ))
            );
        });
    }
}
