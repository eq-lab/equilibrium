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
use crate::mock::{new_test_ext, ModuleBalances, ModuleQSwap, ModuleVesting, RuntimeOrigin, Test};
use crate::{QSwapConfigurations, SwapConfiguration, SwapConfigurationInput};
use eq_primitives::asset::{DOT, EQ, GENS};
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
        let config_initial_eq = QSwapConfigurations::<Test>::get(EQ);
        let config_initial_gens = QSwapConfigurations::<Test>::get(GENS);
        let config_initial_dot = QSwapConfigurations::<Test>::get(DOT);
        let config_initial_max_q_amount = QReceivingThreshold::<Test>::get();

        assert_err!(
            ModuleQSwap::set_config(
                RawOrigin::Root.into(),
                Some(123),
                Some(vec![(
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(123),
                        mb_q_ratio: Some(123u128),
                        mb_vesting_share: Some(Percent::one()),
                        mb_vesting_starting_block: Some(10),
                        mb_vesting_duration_blocks: None
                    }
                )])
            ),
            Error::<Test>::InvalidConfiguration
        );

        assert_err!(
            ModuleQSwap::set_config(
                RawOrigin::Root.into(),
                Some(123),
                Some(vec![(
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(0),
                        mb_q_ratio: Some(123u128),
                        mb_vesting_share: Some(Percent::one()),
                        mb_vesting_starting_block: Some(10),
                        mb_vesting_duration_blocks: None
                    }
                )])
            ),
            Error::<Test>::InvalidConfiguration
        );

        assert_err!(
            ModuleQSwap::set_config(
                RawOrigin::Root.into(),
                Some(0),
                Some(vec![(
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(123),
                        mb_q_ratio: Some(123u128),
                        mb_vesting_share: Some(Percent::one()),
                        mb_vesting_starting_block: Some(10),
                        mb_vesting_duration_blocks: None
                    }
                )])
            ),
            Error::<Test>::InvalidConfiguration
        );

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            Some(5),
            Some(vec![
                (
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(123),
                        mb_q_ratio: Some(123u128),
                        mb_vesting_share: Some(Percent::one()),
                        mb_vesting_starting_block: Some(10),
                        mb_vesting_duration_blocks: Some(50)
                    }
                ),
                (
                    DOT,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(567),
                        mb_q_ratio: Some(456u128),
                        mb_vesting_share: Some(Percent::from_percent(25)),
                        mb_vesting_starting_block: Some(10),
                        mb_vesting_duration_blocks: Some(20)
                    }
                )
            ])
        ));

        let config_after_1_eq = QSwapConfigurations::<Test>::get(EQ);
        let config_after_1_gens = QSwapConfigurations::<Test>::get(GENS);
        let config_after_1_dot = QSwapConfigurations::<Test>::get(DOT);
        let config_after_1_max_q_amount = QReceivingThreshold::<Test>::get();

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            Some(10),
            Some(vec![(
                DOT,
                SwapConfigurationInput {
                    mb_enabled: Some(false),
                    mb_min_amount: Some(667),
                    mb_q_ratio: Some(789u128),
                    mb_vesting_share: Some(Percent::from_percent(0)),
                    mb_vesting_starting_block: Some(11),
                    mb_vesting_duration_blocks: Some(21)
                }
            )])
        ));

        let config_after_2_eq = QSwapConfigurations::<Test>::get(EQ);
        let config_after_2_gens = QSwapConfigurations::<Test>::get(GENS);
        let config_after_2_dot = QSwapConfigurations::<Test>::get(DOT);
        let config_after_2_max_q_amount = QReceivingThreshold::<Test>::get();

        assert_eq!(
            vec![config_initial_eq, config_initial_dot, config_initial_gens],
            vec![
                SwapConfiguration {
                    enabled: Default::default(),
                    min_amount: Default::default(),
                    q_ratio: Default::default(),
                    vesting_share: Default::default(),
                    vesting_starting_block: Default::default(),
                    vesting_duration_blocks: Default::default()
                },
                SwapConfiguration {
                    enabled: Default::default(),
                    min_amount: Default::default(),
                    q_ratio: Default::default(),
                    vesting_share: Default::default(),
                    vesting_starting_block: Default::default(),
                    vesting_duration_blocks: Default::default()
                },
                SwapConfiguration {
                    enabled: Default::default(),
                    min_amount: Default::default(),
                    q_ratio: Default::default(),
                    vesting_share: Default::default(),
                    vesting_starting_block: Default::default(),
                    vesting_duration_blocks: Default::default()
                }
            ]
        );

        assert_eq!(config_initial_max_q_amount, 0);

        assert_eq!(
            vec![config_after_1_eq, config_after_1_dot, config_after_1_gens],
            vec![
                SwapConfiguration {
                    enabled: true,
                    min_amount: 123,
                    q_ratio: 123u128,
                    vesting_share: Percent::one(),
                    vesting_starting_block: 10u32,
                    vesting_duration_blocks: 50
                },
                SwapConfiguration {
                    enabled: true,
                    min_amount: 567,
                    q_ratio: 456u128,
                    vesting_share: Percent::from_percent(25),
                    vesting_starting_block: 10u32,
                    vesting_duration_blocks: 20
                },
                SwapConfiguration {
                    enabled: Default::default(),
                    min_amount: Default::default(),
                    q_ratio: Default::default(),
                    vesting_share: Default::default(),
                    vesting_starting_block: Default::default(),
                    vesting_duration_blocks: Default::default()
                }
            ]
        );

        assert_eq!(config_after_1_max_q_amount, 5);

        assert_eq!(
            vec![config_after_2_eq, config_after_2_dot, config_after_2_gens],
            vec![
                SwapConfiguration {
                    enabled: true,
                    min_amount: 123,
                    q_ratio: 123u128,
                    vesting_share: Percent::one(),
                    vesting_starting_block: 10u32,
                    vesting_duration_blocks: 50
                },
                SwapConfiguration {
                    enabled: false,
                    min_amount: 667,
                    q_ratio: 789u128,
                    vesting_share: Percent::from_percent(0),
                    vesting_starting_block: 11u32,
                    vesting_duration_blocks: 21
                },
                SwapConfiguration {
                    enabled: Default::default(),
                    min_amount: Default::default(),
                    q_ratio: Default::default(),
                    vesting_share: Default::default(),
                    vesting_starting_block: Default::default(),
                    vesting_duration_blocks: Default::default()
                }
            ]
        );

        assert_eq!(config_after_2_max_q_amount, 10);
    });
}

#[test]
fn swap() {
    new_test_ext().execute_with(|| {
        let account_1: u64 = 1;
        let account_2: u64 = 2;
        let vesting_account_id = VestingAccountMock::get();
        let treasury_acount_id = TreasuryAccountMock::get();

        assert_err!(
            ModuleQSwap::swap(RuntimeOrigin::signed(account_1), EQ, 1000 * ONE_TOKEN),
            Error::<Test>::SwapsAreDisabled
        );

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            Some(1000 * ONE_TOKEN),
            Some(vec![(
                EQ,
                SwapConfigurationInput {
                    mb_enabled: Some(true),
                    mb_min_amount: Some(100 * ONE_TOKEN),
                    mb_q_ratio: Some(800_000_000),
                    mb_vesting_share: Some(Percent::from_percent(20)),
                    mb_vesting_starting_block: Some(10),
                    mb_vesting_duration_blocks: Some(20)
                }
            )])
        ));

        assert_err!(
            ModuleQSwap::swap(RuntimeOrigin::signed(account_1), EQ, 99 * ONE_TOKEN),
            Error::<Test>::AmountTooSmall
        );

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_1),
            EQ,
            800 * ONE_TOKEN
        ));

        let account_1_vesting = ModuleVesting::vesting(account_1).unwrap();
        let account_1_q_received = QReceivedAmounts::<Test>::get(account_1);

        assert_balance!(&vesting_account_id, 128 * ONE_TOKEN, 0, Q);
        assert_balance!(&account_1, 200 * ONE_TOKEN, 0, EQ);
        assert_balance!(&account_1, 512 * ONE_TOKEN, 0, Q);
        assert_balance!(&treasury_acount_id, 800 * ONE_TOKEN, 0, EQ);
        assert_balance!(&treasury_acount_id, (10_000 - 512 - 128) * ONE_TOKEN, 0, Q);
        assert_eq!(
            account_1_vesting,
            VestingInfo {
                locked: 128 * ONE_TOKEN,
                per_block: 6_400_000_000,
                starting_block: 10
            }
        );
        assert_eq!(account_1_q_received, 512 * ONE_TOKEN);

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_1),
            EQ,
            200 * ONE_TOKEN
        ));

        let account_1_vesting = ModuleVesting::vesting(account_1).unwrap();
        let account_1_q_received = QReceivedAmounts::<Test>::get(account_1);

        assert_balance!(&vesting_account_id, (128 + 32) * ONE_TOKEN, 0, Q);
        assert_balance!(&account_1, 0, 0, EQ);
        assert_balance!(&account_1, (512 + 128) * ONE_TOKEN, 0, Q);
        assert_balance!(&treasury_acount_id, 1000 * ONE_TOKEN, 0, EQ);
        assert_balance!(
            &treasury_acount_id,
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
            ModuleQSwap::swap(RuntimeOrigin::signed(account_1), EQ, 0),
            Error::<Test>::AmountTooSmall
        );

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            Some(1000 * ONE_TOKEN),
            Some(vec![(
                EQ,
                SwapConfigurationInput {
                    mb_enabled: Some(true),
                    mb_min_amount: Some(100 * ONE_TOKEN),
                    mb_q_ratio: Some(1_500_000_000),
                    mb_vesting_share: Some(Percent::from_percent(0)),
                    mb_vesting_starting_block: Some(10),
                    mb_vesting_duration_blocks: Some(20)
                }
            )])
        ));

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_2),
            EQ,
            200 * ONE_TOKEN
        ));

        let account_2_vesting = ModuleVesting::vesting(account_2);
        let account_2_q_received = QReceivedAmounts::<Test>::get(account_2);

        assert_balance!(&vesting_account_id, (128 + 32) * ONE_TOKEN, 0, Q);
        assert_balance!(&account_2, 800 * ONE_TOKEN, 0, EQ);
        assert_balance!(&account_2, 300 * ONE_TOKEN, 0, Q);
        assert_balance!(&treasury_acount_id, 1_200 * ONE_TOKEN, 0, EQ);
        assert_balance!(
            &treasury_acount_id,
            (10_000 - 512 - 128 - 128 - 32 - 300) * ONE_TOKEN,
            0,
            Q
        );
        assert_eq!(account_2_vesting, None);
        assert_eq!(account_2_q_received, 300 * ONE_TOKEN);

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            Some(400 * ONE_TOKEN),
            Some(vec![(
                EQ,
                SwapConfigurationInput {
                    mb_enabled: Some(true),
                    mb_min_amount: Some(100 * ONE_TOKEN),
                    mb_q_ratio: Some(1_500_000_000),
                    mb_vesting_share: Some(Percent::from_percent(50)),
                    mb_vesting_starting_block: Some(100),
                    mb_vesting_duration_blocks: Some(50)
                }
            )])
        ));

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_2),
            EQ,
            100 * ONE_TOKEN
        ));

        let account_2_vesting = ModuleVesting::vesting(account_2).unwrap();
        let account_2_q_received = QReceivedAmounts::<Test>::get(account_2);

        assert_balance!(&vesting_account_id, (128 + 32 + 75) * ONE_TOKEN, 0, Q);
        assert_balance!(&account_2, 700 * ONE_TOKEN, 0, EQ);
        assert_balance!(&account_2, (300 + 75) * ONE_TOKEN, 0, Q);
        assert_balance!(&treasury_acount_id, 1_300 * ONE_TOKEN, 0, EQ);
        assert_balance!(
            &treasury_acount_id,
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

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_2),
            EQ,
            300 * ONE_TOKEN
        ));

        let account_2_vesting = ModuleVesting::vesting(account_2).unwrap();
        let account_2_q_received = QReceivedAmounts::<Test>::get(account_2);

        assert_balance!(&vesting_account_id, (128 + 32 + 75 + 425) * ONE_TOKEN, 0, Q);
        assert_balance!(&account_2, (700 - 300) * ONE_TOKEN, 0, EQ);
        assert_balance!(&account_2, (300 + 75 + 25) * ONE_TOKEN, 0, Q);
        assert_balance!(&treasury_acount_id, 1_600 * ONE_TOKEN, 0, EQ);
        assert_balance!(
            &treasury_acount_id,
            (10_000 - 512 - 128 - 128 - 32 - 300 - 75 - 75 - 425 - 25) * ONE_TOKEN,
            0,
            Q
        );
        assert_eq!(
            account_2_vesting,
            VestingInfo {
                locked: (75 + 425) * ONE_TOKEN,
                per_block: 10_000_000_000,
                starting_block: 100
            }
        );
        assert_eq!(account_2_q_received, (300 + 75 + 25) * ONE_TOKEN);

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_2),
            EQ,
            100 * ONE_TOKEN
        ));

        let account_2_vesting = ModuleVesting::vesting(account_2).unwrap();
        let account_2_q_received = QReceivedAmounts::<Test>::get(account_2);

        assert_balance!(
            &vesting_account_id,
            (128 + 32 + 75 + 425 + 150) * ONE_TOKEN,
            0,
            Q
        );
        assert_balance!(&account_2, (700 - 300 - 100) * ONE_TOKEN, 0, EQ);
        assert_balance!(&account_2, (300 + 75 + 25) * ONE_TOKEN, 0, Q);
        assert_balance!(
            &treasury_acount_id,
            (10_000 - 512 - 128 - 128 - 32 - 300 - 75 - 75 - 425 - 25 - 150) * ONE_TOKEN,
            0,
            Q
        );
        assert_balance!(&treasury_acount_id, 1_700 * ONE_TOKEN, 0, EQ);
        assert_eq!(
            account_2_vesting,
            VestingInfo {
                locked: (75 + 425 + 150) * ONE_TOKEN,
                per_block: 13_000_000_000,
                starting_block: 100
            }
        );
        assert_eq!(account_2_q_received, (300 + 75 + 25) * ONE_TOKEN);

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            None,
            Some(vec![(
                EQ,
                SwapConfigurationInput {
                    mb_enabled: Some(false),
                    mb_min_amount: None,
                    mb_q_ratio: None,
                    mb_vesting_share: None,
                    mb_vesting_starting_block: None,
                    mb_vesting_duration_blocks: None
                }
            )])
        ));

        assert_err!(
            ModuleQSwap::swap(RuntimeOrigin::signed(account_2), EQ, 100 * ONE_TOKEN),
            Error::<Test>::SwapsAreDisabled
        );

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            Some(1000 * ONE_TOKEN),
            Some(vec![(
                DOT,
                SwapConfigurationInput {
                    mb_enabled: Some(true),
                    mb_min_amount: Some(100 * ONE_TOKEN),
                    mb_q_ratio: Some(1_000_000_000),
                    mb_vesting_share: Some(Percent::from_percent(50)),
                    mb_vesting_starting_block: Some(100),
                    mb_vesting_duration_blocks: Some(10)
                }
            )])
        ));

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_2),
            DOT,
            200 * ONE_TOKEN
        ));

        let account_2_vesting = ModuleVesting::vesting(account_2).unwrap();
        let account_2_q_received = QReceivedAmounts::<Test>::get(account_2);

        assert_balance!(
            &vesting_account_id,
            (128 + 32 + 75 + 425 + 150 + 100) * ONE_TOKEN,
            0,
            Q
        );
        assert_balance!(&account_2, 800 * ONE_TOKEN, 0, DOT);
        assert_balance!(&account_2, (300 + 75 + 25 + 100) * ONE_TOKEN, 0, Q);
        assert_balance!(
            &treasury_acount_id,
            (10_000 - 512 - 128 - 128 - 32 - 300 - 75 - 75 - 425 - 25 - 150 - 200) * ONE_TOKEN,
            0,
            Q
        );
        assert_balance!(&treasury_acount_id, 200 * ONE_TOKEN, 0, DOT);
        assert_eq!(
            account_2_vesting,
            VestingInfo {
                locked: (75 + 425 + 150 + 100) * ONE_TOKEN,
                per_block: 75_000_000_000,
                starting_block: 100
            }
        );
        assert_eq!(account_2_q_received, (300 + 75 + 25 + 100) * ONE_TOKEN);
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

            assert_ok!(ModuleQSwap::set_config(
                RawOrigin::Root.into(),
                Some(1000 * ONE_TOKEN),
                Some(vec![(
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(100 * ONE_TOKEN),
                        mb_q_ratio: Some(1_500_000_000),
                        mb_vesting_share: Some(Percent::from_percent(50)),
                        mb_vesting_starting_block: Some(100),
                        mb_vesting_duration_blocks: Some(50)
                    }
                )])
            ));

            let q_swap_call = RuntimeCall::QSwap(crate::Call::swap {
                asset: EQ,
                amount: 100 * ONE_TOKEN,
            });

            let check = CheckQSwap::<Test>::new();
            let info = info_from_weight(Weight::zero());
            assert_ok!(check.validate(&account_id, &q_swap_call, &info, 0));
        });
    }

    #[test]
    fn validate_should_fail_when_swap_disabled() {
        new_test_ext().execute_with(|| {
            let account_id = 1u64;

            let q_swap_call = RuntimeCall::QSwap(crate::Call::swap {
                asset: EQ,
                amount: 100 * ONE_TOKEN,
            });

            let check = CheckQSwap::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &q_swap_call, &info, 1),
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

            assert_ok!(ModuleQSwap::set_config(
                RawOrigin::Root.into(),
                Some(1000 * ONE_TOKEN),
                Some(vec![(
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(100 * ONE_TOKEN),
                        mb_q_ratio: Some(1_500_000_000),
                        mb_vesting_share: Some(Percent::from_percent(50)),
                        mb_vesting_starting_block: Some(100),
                        mb_vesting_duration_blocks: Some(50)
                    }
                )])
            ));

            let q_swap_call = RuntimeCall::QSwap(crate::Call::swap {
                asset: EQ,
                amount: 10_000 * ONE_TOKEN,
            });

            let check = CheckQSwap::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &q_swap_call, &info, 1),
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

            assert_ok!(ModuleQSwap::set_config(
                RawOrigin::Root.into(),
                Some(1000 * ONE_TOKEN),
                Some(vec![(
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(100 * ONE_TOKEN),
                        mb_q_ratio: Some(1_500_000_000),
                        mb_vesting_share: Some(Percent::from_percent(50)),
                        mb_vesting_starting_block: Some(100),
                        mb_vesting_duration_blocks: Some(50)
                    }
                )])
            ));

            let q_swap_call = RuntimeCall::QSwap(crate::Call::swap {
                asset: EQ,
                amount: 99 * ONE_TOKEN,
            });

            let check = CheckQSwap::<Test>::new();
            let info = info_from_weight(Weight::zero());

            assert_err!(
                check.validate(&account_id, &q_swap_call, &info, 1),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    ValidityError::AmountTooSmall.into()
                ))
            );
        });
    }
}
