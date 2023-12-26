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
    new_test_ext, ModuleBalances, ModuleQSwap, ModuleVesting1, ModuleVesting2, ModuleVesting3,
    RuntimeOrigin, Test, Vesting1AccountMock, Vesting2AccountMock, Vesting3AccountMock,
};
use crate::{QSwapConfigurations, SwapConfiguration, SwapConfigurationInput};
use eq_primitives::asset::{DOT, EQ, GENS};
use eq_primitives::mocks::TreasuryAccountMock;
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
                        mb_main_asset_q_price: Some(123u128),
                        mb_main_asset_q_discounted_price: Some(123u128),
                        mb_secondary_asset: Default::default(),
                        mb_secondary_asset_q_price: Default::default(),
                        mb_secondary_asset_q_discounted_price: Default::default(),
                        mb_instant_swap_share: Some(Percent::one()),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_main_vesting_starting_block: Some(10),
                        mb_main_vesting_duration_blocks: None,
                        mb_secondary_vesting_starting_block: Some(10),
                        mb_secondary_vesting_duration_blocks: None,
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
                        mb_main_asset_q_price: Some(123u128),
                        mb_main_asset_q_discounted_price: Some(123u128),
                        mb_secondary_asset: Default::default(),
                        mb_secondary_asset_q_price: Default::default(),
                        mb_secondary_asset_q_discounted_price: Default::default(),
                        mb_instant_swap_share: Some(Percent::one()),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_main_vesting_starting_block: Some(10),
                        mb_main_vesting_duration_blocks: None,
                        mb_secondary_vesting_starting_block: Some(10),
                        mb_secondary_vesting_duration_blocks: None,
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
                        mb_main_asset_q_price: Some(123u128),
                        mb_main_asset_q_discounted_price: Some(123u128),
                        mb_secondary_asset: Default::default(),
                        mb_secondary_asset_q_price: Default::default(),
                        mb_secondary_asset_q_discounted_price: Default::default(),
                        mb_instant_swap_share: Some(Percent::one()),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_main_vesting_starting_block: Some(10),
                        mb_main_vesting_duration_blocks: None,
                        mb_secondary_vesting_starting_block: Some(10),
                        mb_secondary_vesting_duration_blocks: None,
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
                        mb_main_asset_q_price: Some(123u128),
                        mb_main_asset_q_discounted_price: Some(123u128),
                        mb_secondary_asset: Default::default(),
                        mb_secondary_asset_q_price: Default::default(),
                        mb_secondary_asset_q_discounted_price: Default::default(),
                        mb_instant_swap_share: Some(Percent::one()),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_main_vesting_starting_block: Some(10),
                        mb_main_vesting_duration_blocks: Some(50),
                        mb_secondary_vesting_starting_block: Some(10),
                        mb_secondary_vesting_duration_blocks: Some(50),
                    }
                ),
                (
                    DOT,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(567),
                        mb_main_asset_q_price: Some(456u128),
                        mb_main_asset_q_discounted_price: Some(456u128),
                        mb_instant_swap_share: Some(Percent::from_percent(25)),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_secondary_asset: Default::default(),
                        mb_secondary_asset_q_price: Default::default(),
                        mb_secondary_asset_q_discounted_price: Default::default(),
                        mb_main_vesting_starting_block: Some(10),
                        mb_main_vesting_duration_blocks: Some(20),
                        mb_secondary_vesting_starting_block: Some(10),
                        mb_secondary_vesting_duration_blocks: Some(20),
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
                    mb_main_asset_q_price: Some(789u128),
                    mb_main_asset_q_discounted_price: Some(789u128),
                    mb_instant_swap_share: Some(Percent::from_percent(0)),
                    mb_main_vesting_number: Some(1),
                    mb_secondary_vesting_number: Some(2),
                    mb_secondary_asset: Default::default(),
                    mb_secondary_asset_q_price: Default::default(),
                    mb_secondary_asset_q_discounted_price: Default::default(),
                    mb_main_vesting_starting_block: Some(11),
                    mb_main_vesting_duration_blocks: Some(21),
                    mb_secondary_vesting_starting_block: Some(11),
                    mb_secondary_vesting_duration_blocks: Some(21),
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
                    main_asset_q_price: Default::default(),
                    main_asset_q_discounted_price: Default::default(),
                    secondary_asset: Default::default(),
                    secondary_asset_q_price: Default::default(),
                    secondary_asset_q_discounted_price: Default::default(),
                    instant_swap_share: Default::default(),
                    main_vesting_number: Default::default(),
                    secondary_vesting_number: Default::default(),
                    main_vesting_starting_block: Default::default(),
                    main_vesting_duration_blocks: Default::default(),
                    secondary_vesting_starting_block: Default::default(),
                    secondary_vesting_duration_blocks: Default::default(),
                },
                SwapConfiguration {
                    enabled: Default::default(),
                    min_amount: Default::default(),
                    main_asset_q_price: Default::default(),
                    main_asset_q_discounted_price: Default::default(),
                    secondary_asset: Default::default(),
                    secondary_asset_q_price: Default::default(),
                    secondary_asset_q_discounted_price: Default::default(),
                    instant_swap_share: Default::default(),
                    main_vesting_number: Default::default(),
                    secondary_vesting_number: Default::default(),
                    main_vesting_starting_block: Default::default(),
                    main_vesting_duration_blocks: Default::default(),
                    secondary_vesting_starting_block: Default::default(),
                    secondary_vesting_duration_blocks: Default::default(),
                },
                SwapConfiguration {
                    enabled: Default::default(),
                    min_amount: Default::default(),
                    main_asset_q_price: Default::default(),
                    main_asset_q_discounted_price: Default::default(),
                    secondary_asset: Default::default(),
                    secondary_asset_q_price: Default::default(),
                    secondary_asset_q_discounted_price: Default::default(),
                    instant_swap_share: Default::default(),
                    main_vesting_number: Default::default(),
                    secondary_vesting_number: Default::default(),
                    main_vesting_starting_block: Default::default(),
                    main_vesting_duration_blocks: Default::default(),
                    secondary_vesting_starting_block: Default::default(),
                    secondary_vesting_duration_blocks: Default::default(),
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
                    main_asset_q_price: 123u128,
                    main_asset_q_discounted_price: 123u128,
                    secondary_asset: Default::default(),
                    secondary_asset_q_price: Default::default(),
                    secondary_asset_q_discounted_price: Default::default(),
                    instant_swap_share: Percent::one(),
                    main_vesting_number: 1,
                    secondary_vesting_number: 2,
                    main_vesting_starting_block: 10u32,
                    main_vesting_duration_blocks: 50,
                    secondary_vesting_starting_block: 10u32,
                    secondary_vesting_duration_blocks: 50,
                },
                SwapConfiguration {
                    enabled: true,
                    min_amount: 567,
                    main_asset_q_price: 456u128,
                    main_asset_q_discounted_price: 456u128,
                    secondary_asset: Default::default(),
                    secondary_asset_q_price: Default::default(),
                    secondary_asset_q_discounted_price: Default::default(),
                    instant_swap_share: Percent::from_percent(25),
                    main_vesting_number: 1,
                    secondary_vesting_number: 2,
                    main_vesting_starting_block: 10u32,
                    main_vesting_duration_blocks: 20,
                    secondary_vesting_starting_block: 10u32,
                    secondary_vesting_duration_blocks: 20,
                },
                SwapConfiguration {
                    enabled: Default::default(),
                    min_amount: Default::default(),
                    main_asset_q_price: Default::default(),
                    main_asset_q_discounted_price: Default::default(),
                    secondary_asset: Default::default(),
                    secondary_asset_q_price: Default::default(),
                    secondary_asset_q_discounted_price: Default::default(),
                    instant_swap_share: Default::default(),
                    main_vesting_number: Default::default(),
                    secondary_vesting_number: Default::default(),
                    main_vesting_starting_block: Default::default(),
                    main_vesting_duration_blocks: Default::default(),
                    secondary_vesting_starting_block: Default::default(),
                    secondary_vesting_duration_blocks: Default::default(),
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
                    main_asset_q_price: 123u128,
                    main_asset_q_discounted_price: 123u128,
                    secondary_asset: Default::default(),
                    secondary_asset_q_price: Default::default(),
                    secondary_asset_q_discounted_price: Default::default(),
                    instant_swap_share: Percent::one(),
                    main_vesting_number: 1,
                    secondary_vesting_number: 2,
                    main_vesting_starting_block: 10u32,
                    main_vesting_duration_blocks: 50,
                    secondary_vesting_starting_block: 10u32,
                    secondary_vesting_duration_blocks: 50,
                },
                SwapConfiguration {
                    enabled: false,
                    min_amount: 667,
                    main_asset_q_price: 789u128,
                    main_asset_q_discounted_price: 789u128,
                    secondary_asset: Default::default(),
                    secondary_asset_q_price: Default::default(),
                    secondary_asset_q_discounted_price: Default::default(),
                    instant_swap_share: Percent::from_percent(0),
                    main_vesting_number: 1,
                    secondary_vesting_number: 2,
                    main_vesting_starting_block: 11u32,
                    main_vesting_duration_blocks: 21,
                    secondary_vesting_starting_block: 11u32,
                    secondary_vesting_duration_blocks: 21,
                },
                SwapConfiguration {
                    enabled: Default::default(),
                    min_amount: Default::default(),
                    main_asset_q_price: Default::default(),
                    main_asset_q_discounted_price: Default::default(),
                    secondary_asset: Default::default(),
                    secondary_asset_q_price: Default::default(),
                    secondary_asset_q_discounted_price: Default::default(),
                    instant_swap_share: Default::default(),
                    main_vesting_number: Default::default(),
                    secondary_vesting_number: Default::default(),
                    main_vesting_starting_block: Default::default(),
                    main_vesting_duration_blocks: Default::default(),
                    secondary_vesting_starting_block: Default::default(),
                    secondary_vesting_duration_blocks: Default::default(),
                }
            ]
        );

        assert_eq!(config_after_2_max_q_amount, 10);
    });
}

#[test]
fn swap() {
    new_test_ext().execute_with(|| {
        let account_1: u128 = 1;
        let account_2: u128 = 2;
        let account_3: u128 = 3;

        let vesting_1_account_id: u128 = Vesting1AccountMock::get();
        let vesting_2_account_id: u128 = Vesting2AccountMock::get();
        let vesting_3_account_id: u128 = Vesting3AccountMock::get();

        let treasury_acount_id: u128 = TreasuryAccountMock::get();

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
                    mb_main_asset_q_price: Some(1_700_000_000_000),
                    mb_main_asset_q_discounted_price: Some(502_960_000_000),
                    mb_secondary_asset: Default::default(),
                    mb_secondary_asset_q_price: Default::default(),
                    mb_secondary_asset_q_discounted_price: Default::default(),
                    mb_instant_swap_share: Some(Percent::from_percent(30)),
                    mb_main_vesting_number: Some(1),
                    mb_secondary_vesting_number: Some(2),
                    mb_main_vesting_starting_block: Some(10),
                    mb_main_vesting_duration_blocks: Some(20),
                    mb_secondary_vesting_starting_block: Some(10),
                    mb_secondary_vesting_duration_blocks: Some(20),
                }
            )])
        ));

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_1),
            EQ,
            1_005_920_000_000
        ));

        let account_1_vesting_1 = ModuleVesting1::vesting(account_1).unwrap();
        let account_1_vesting_2 = ModuleVesting2::vesting(account_1).unwrap();
        let account_1_q_received = QReceivedAmounts::<Test>::get(account_1);

        assert_balance!(&vesting_1_account_id, 414_202_353, 0, Q);
        assert_balance!(&vesting_2_account_id, 1_408_282_354, 0, Q);
        assert_balance!(&account_1, 8_994_080_000_000, 0, EQ);
        assert_balance!(&account_1, 177_515_293, 0, Q);
        assert_balance!(&treasury_acount_id, 1_005_920_000_000, 0, EQ);
        assert_balance!(&treasury_acount_id, 9_998_000_000_000, 0, Q);
        assert_eq!(
            account_1_vesting_1,
            VestingInfo {
                locked: 414_202_353,
                per_block: 20_710_117,
                starting_block: 10
            }
        );
        assert_eq!(
            account_1_vesting_2,
            VestingInfo {
                locked: 1_408_282_354,
                per_block: 70_414_117,
                starting_block: 10
            }
        );
        assert_eq!(account_1_q_received, 177_515_293);

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            Some(1000000000),
            Some(vec![
                (
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(100 * ONE_TOKEN),
                        mb_main_asset_q_price: Some(1_700_000_000_000),
                        mb_main_asset_q_discounted_price: Some(502_960_000_000),
                        mb_secondary_asset: Default::default(),
                        mb_secondary_asset_q_price: Default::default(),
                        mb_secondary_asset_q_discounted_price: Default::default(),
                        mb_instant_swap_share: Some(Percent::from_percent(30)),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_main_vesting_starting_block: Some(10),
                        mb_main_vesting_duration_blocks: Some(20),
                        mb_secondary_vesting_starting_block: Some(10),
                        mb_secondary_vesting_duration_blocks: Some(20),
                    }
                ),
                (
                    DOT,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(1_000_000),
                        mb_main_asset_q_price: Some(100_000_000),
                        mb_main_asset_q_discounted_price: Some(100_000_000),
                        mb_secondary_asset: Some(EQ),
                        mb_secondary_asset_q_price: Some(1000_000_000_000),
                        mb_secondary_asset_q_discounted_price: Some(295_860_000_000),
                        mb_instant_swap_share: Some(Percent::from_percent(50)),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_main_vesting_starting_block: Some(10),
                        mb_main_vesting_duration_blocks: Some(20),
                        mb_secondary_vesting_starting_block: Some(10),
                        mb_secondary_vesting_duration_blocks: Some(20),
                    }
                )
            ])
        ));

        assert_err!(
            ModuleQSwap::swap(RuntimeOrigin::signed(account_1), EQ, 99 * ONE_TOKEN),
            Error::<Test>::AmountTooSmall
        );

        assert_err!(
            ModuleQSwap::swap(RuntimeOrigin::signed(account_1), DOT, 990_000),
            Error::<Test>::AmountTooSmall
        );

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_1),
            DOT,
            150_000_000
        ));

        let account_1_vesting_1 = ModuleVesting1::vesting(account_1).unwrap();
        let account_1_vesting_2 = ModuleVesting2::vesting(account_1).unwrap();
        let account_1_q_received = QReceivedAmounts::<Test>::get(account_1);

        assert_balance!(&vesting_1_account_id, 636_097_353, 0, Q);
        assert_balance!(&vesting_2_account_id, 2_464_492_354, 0, Q);
        assert_balance!(&account_1, 8_550_290_000_000, 0, EQ);
        assert_balance!(&account_1, 9_999_850_000_000, 0, DOT);
        assert_balance!(&account_1, 399_410_293, 0, Q);
        assert_balance!(&treasury_acount_id, 1_449_710_000_000, 0, EQ);
        assert_balance!(&treasury_acount_id, 150_000_000, 0, DOT);
        assert_balance!(&treasury_acount_id, 9_996_500_000_000, 0, Q);
        assert_eq!(
            account_1_vesting_1,
            VestingInfo {
                locked: 636_097_353,
                per_block: 31_804_867,
                starting_block: 10
            }
        );
        assert_eq!(
            account_1_vesting_2,
            VestingInfo {
                locked: 2_464_492_354,
                per_block: 123_224_617,
                starting_block: 10
            }
        );
        assert_eq!(account_1_q_received, 399_410_293);

        assert_err!(
            ModuleQSwap::swap(RuntimeOrigin::signed(account_1), EQ, 0),
            Error::<Test>::AmountTooSmall
        );

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_2),
            DOT,
            150_000_000
        ));

        let account_1_vesting_1 = ModuleVesting1::vesting(account_1).unwrap();
        let account_1_vesting_2 = ModuleVesting2::vesting(account_1).unwrap();
        let account_1_q_received = QReceivedAmounts::<Test>::get(account_1);

        let account_2_vesting_1 = ModuleVesting1::vesting(account_2).unwrap();
        let account_2_vesting_2 = ModuleVesting2::vesting(account_2).unwrap();
        let account_2_q_received = QReceivedAmounts::<Test>::get(account_2);

        assert_balance!(&treasury_acount_id, 1_893_500_000_000, 0, EQ);
        assert_balance!(&treasury_acount_id, 300_000_000, 0, DOT);
        assert_balance!(&treasury_acount_id, 9_995_000_000_000, 0, Q);
        assert_balance!(&vesting_1_account_id, 857_992_353, 0, Q);
        assert_balance!(&vesting_2_account_id, 3_520_702_354, 0, Q);

        assert_balance!(&account_1, 8_550_290_000_000, 0, EQ);
        assert_balance!(&account_1, 9_999_850_000_000, 0, DOT);
        assert_balance!(&account_1, 399_410_293, 0, Q);
        assert_eq!(
            account_1_vesting_1,
            VestingInfo {
                locked: 636_097_353,
                per_block: 31_804_867,
                starting_block: 10
            }
        );
        assert_eq!(
            account_1_vesting_2,
            VestingInfo {
                locked: 2_464_492_354,
                per_block: 123_224_617,
                starting_block: 10
            }
        );
        assert_eq!(account_1_q_received, 399_410_293);

        assert_balance!(&account_2, 9_556_210_000_000, 0, EQ);
        assert_balance!(&account_2, 9_999_850_000_000, 0, DOT);
        assert_balance!(&account_2, 221_895_000, 0, Q);
        assert_eq!(
            account_2_vesting_1,
            VestingInfo {
                locked: 221_895_000,
                per_block: 11_094_750,
                starting_block: 10
            }
        );
        assert_eq!(
            account_2_vesting_2,
            VestingInfo {
                locked: 1_056_210_000,
                per_block: 52_810_500,
                starting_block: 10
            }
        );
        assert_eq!(account_2_q_received, 221_895_000);

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            Some(1000000000),
            Some(vec![(
                GENS,
                SwapConfigurationInput {
                    mb_enabled: Some(true),
                    mb_min_amount: Some(100 * ONE_TOKEN),
                    mb_main_asset_q_price: Some(4_000_000_000_000),
                    mb_main_asset_q_discounted_price: Some(4_000_000_000_000),
                    mb_secondary_asset: Default::default(),
                    mb_secondary_asset_q_price: Default::default(),
                    mb_secondary_asset_q_discounted_price: Default::default(),
                    mb_instant_swap_share: Some(Percent::from_percent(50)),
                    mb_main_vesting_number: Some(3),
                    mb_secondary_vesting_number: Default::default(),
                    mb_main_vesting_starting_block: Some(10),
                    mb_main_vesting_duration_blocks: Some(20),
                    mb_secondary_vesting_starting_block: Default::default(),
                    mb_secondary_vesting_duration_blocks: Default::default(),
                }
            )])
        ));

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_1),
            GENS,
            1000 * ONE_TOKEN
        ));

        let account_1_vesting_1 = ModuleVesting1::vesting(account_1).unwrap();
        let account_1_vesting_2 = ModuleVesting2::vesting(account_1).unwrap();
        let account_1_vesting_3 = ModuleVesting3::vesting(account_1).unwrap();
        let account_1_q_received = QReceivedAmounts::<Test>::get(account_1);

        assert_balance!(&treasury_acount_id, 1_893_500_000_000, 0, EQ);
        assert_balance!(&treasury_acount_id, 300_000_000, 0, DOT);
        assert_balance!(&treasury_acount_id, 1000_000_000_000, 0, GENS);
        assert_balance!(&treasury_acount_id, 9_994_750_000_000, 0, Q);
        assert_balance!(&vesting_1_account_id, 857_992_353, 0, Q);
        assert_balance!(&vesting_2_account_id, 3_520_702_354, 0, Q);
        assert_balance!(&vesting_3_account_id, 125_000_000, 0, Q);

        assert_balance!(&account_1, 8_550_290_000_000, 0, EQ);
        assert_balance!(&account_1, 9_999_850_000_000, 0, DOT);
        assert_balance!(&account_1, 9_000_000_000_000, 0, GENS);
        assert_balance!(&account_1, 524_410_293, 0, Q);
        assert_eq!(
            account_1_vesting_1,
            VestingInfo {
                locked: 636_097_353,
                per_block: 31_804_867,
                starting_block: 10
            }
        );
        assert_eq!(
            account_1_vesting_2,
            VestingInfo {
                locked: 2_464_492_354,
                per_block: 123_224_617,
                starting_block: 10
            }
        );
        assert_eq!(
            account_1_vesting_3,
            VestingInfo {
                locked: 125_000_000,
                per_block: 6_250_000,
                starting_block: 10
            }
        );
        assert_eq!(account_1_q_received, 524_410_293);

        assert_ok!(ModuleQSwap::set_config(
            RawOrigin::Root.into(),
            Some(100_000_000),
            Some(vec![(
                GENS,
                SwapConfigurationInput {
                    mb_enabled: Some(true),
                    mb_min_amount: Some(100 * ONE_TOKEN),
                    mb_main_asset_q_price: Some(4_000_000_000_000),
                    mb_main_asset_q_discounted_price: Some(2_000_000_000_000),
                    mb_secondary_asset: Default::default(),
                    mb_secondary_asset_q_price: Default::default(),
                    mb_secondary_asset_q_discounted_price: Default::default(),
                    mb_instant_swap_share: Some(Percent::from_percent(50)),
                    mb_main_vesting_number: Some(3),
                    mb_secondary_vesting_number: Some(2),
                    mb_main_vesting_starting_block: Some(10),
                    mb_main_vesting_duration_blocks: Some(20),
                    mb_secondary_vesting_starting_block: Some(50),
                    mb_secondary_vesting_duration_blocks: Some(100),
                }
            )])
        ));

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_3),
            GENS,
            1000 * ONE_TOKEN
        ));

        let account_3_vesting_2 = ModuleVesting2::vesting(account_3).unwrap();
        let account_3_vesting_3 = ModuleVesting3::vesting(account_3).unwrap();
        let account_3_q_received = QReceivedAmounts::<Test>::get(account_3);

        assert_balance!(&treasury_acount_id, 1_893_500_000_000, 0, EQ);
        assert_balance!(&treasury_acount_id, 300_000_000, 0, DOT);
        assert_balance!(&treasury_acount_id, 2000_000_000_000, 0, GENS);
        assert_balance!(&treasury_acount_id, 9_994_250_000_000, 0, Q);
        assert_balance!(&vesting_1_account_id, 857_992_353, 0, Q);
        assert_balance!(&vesting_2_account_id, 3_770_702_354, 0, Q);
        assert_balance!(&vesting_3_account_id, 275_000_000, 0, Q);

        assert_balance!(&account_3, 9_000_000_000_000, 0, GENS);
        assert_balance!(&account_3, 100_000_000, 0, Q);
        assert_eq!(
            account_3_vesting_2,
            VestingInfo {
                locked: 250_000_000,
                per_block: 2_500_000,
                starting_block: 50
            }
        );
        assert_eq!(
            account_3_vesting_3,
            VestingInfo {
                locked: 150_000_000,
                per_block: 7_500_000,
                starting_block: 10
            }
        );
        assert_eq!(account_3_q_received, 100_000_000);

        assert_ok!(ModuleQSwap::swap(
            RuntimeOrigin::signed(account_3),
            GENS,
            1000 * ONE_TOKEN
        ));

        let account_3_vesting_2 = ModuleVesting2::vesting(account_3).unwrap();
        let account_3_vesting_3 = ModuleVesting3::vesting(account_3).unwrap();
        let account_3_q_received = QReceivedAmounts::<Test>::get(account_3);

        assert_balance!(&treasury_acount_id, 1_893_500_000_000, 0, EQ);
        assert_balance!(&treasury_acount_id, 300_000_000, 0, DOT);
        assert_balance!(&treasury_acount_id, 3_000_000_000_000, 0, GENS);
        assert_balance!(&treasury_acount_id, 9_993_750_000_000, 0, Q);
        assert_balance!(&vesting_1_account_id, 857_992_353, 0, Q);
        assert_balance!(&vesting_2_account_id, 4_020_702_354, 0, Q);
        assert_balance!(&vesting_3_account_id, 525_000_000, 0, Q);

        assert_balance!(&account_3, 8_000_000_000_000, 0, GENS);
        assert_balance!(&account_3, 100_000_000, 0, Q);
        assert_eq!(
            account_3_vesting_2,
            VestingInfo {
                locked: 500_000_000,
                per_block: 5_000_000,
                starting_block: 50
            }
        );
        assert_eq!(
            account_3_vesting_3,
            VestingInfo {
                locked: 400_000_000,
                per_block: 20_000_000,
                starting_block: 10
            }
        );
        assert_eq!(account_3_q_received, 100_000_000);
    });
}

mod signed_extension {
    use super::*;
    use crate::mock::RuntimeCall;
    use frame_support::{dispatch::DispatchInfo, weights::Weight};

    pub fn info_from_weight(w: Weight) -> DispatchInfo {
        DispatchInfo {
            weight: w,
            ..Default::default()
        }
    }

    #[test]
    fn validate_should_skip_when_valid() {
        new_test_ext().execute_with(|| {
            let account_id = 1;

            assert_ok!(ModuleQSwap::set_config(
                RawOrigin::Root.into(),
                Some(1000 * ONE_TOKEN),
                Some(vec![(
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(100 * ONE_TOKEN),
                        mb_main_asset_q_price: Some(1_500_000_000),
                        mb_main_asset_q_discounted_price: Some(1_500_000_000),
                        mb_secondary_asset: Default::default(),
                        mb_secondary_asset_q_price: Default::default(),
                        mb_secondary_asset_q_discounted_price: Default::default(),
                        mb_instant_swap_share: Some(Percent::from_percent(50)),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_main_vesting_starting_block: Some(100),
                        mb_main_vesting_duration_blocks: Some(50),
                        mb_secondary_vesting_starting_block: Some(100),
                        mb_secondary_vesting_duration_blocks: Some(50),
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
            let account_id = 1;

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
            let account_id = 1;

            assert_ok!(ModuleQSwap::set_config(
                RawOrigin::Root.into(),
                Some(1000 * ONE_TOKEN),
                Some(vec![(
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(100 * ONE_TOKEN),
                        mb_main_asset_q_price: Some(1_500_000_000),
                        mb_main_asset_q_discounted_price: Some(1_500_000_000),
                        mb_secondary_asset_q_price: Default::default(),
                        mb_secondary_asset_q_discounted_price: Default::default(),
                        mb_secondary_asset: Default::default(),
                        mb_instant_swap_share: Some(Percent::from_percent(50)),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_main_vesting_starting_block: Some(100),
                        mb_main_vesting_duration_blocks: Some(50),
                        mb_secondary_vesting_starting_block: Some(100),
                        mb_secondary_vesting_duration_blocks: Some(50),
                    }
                )])
            ));

            let q_swap_call = RuntimeCall::QSwap(crate::Call::swap {
                asset: EQ,
                amount: 100_000 * ONE_TOKEN,
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
            let account_id = 1;

            assert_ok!(ModuleQSwap::set_config(
                RawOrigin::Root.into(),
                Some(1000 * ONE_TOKEN),
                Some(vec![(
                    EQ,
                    SwapConfigurationInput {
                        mb_enabled: Some(true),
                        mb_min_amount: Some(100 * ONE_TOKEN),
                        mb_main_asset_q_price: Some(1_500_000_000),
                        mb_main_asset_q_discounted_price: Some(1_500_000_000),
                        mb_secondary_asset: Default::default(),
                        mb_secondary_asset_q_price: Default::default(),
                        mb_secondary_asset_q_discounted_price: Default::default(),
                        mb_instant_swap_share: Some(Percent::from_percent(50)),
                        mb_main_vesting_number: Some(1),
                        mb_secondary_vesting_number: Some(2),
                        mb_main_vesting_starting_block: Some(100),
                        mb_main_vesting_duration_blocks: Some(50),
                        mb_secondary_vesting_starting_block: Some(100),
                        mb_secondary_vesting_duration_blocks: Some(50),
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
