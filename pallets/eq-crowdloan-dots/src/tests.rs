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
    new_test_ext, LendingModuleId, ModuleBalances, ModuleCrowdloanDots, ModuleLending,
    RuntimeOrigin, SubaccountsManagerMock, Test,
};
use crate::{AllowedCrowdloanDotsSwap, CrowdloanDotAsset};
use eq_primitives::asset::EQ;
use eq_primitives::{SignedBalance, ONE_TOKEN};
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::traits::AccountIdConversion;

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
fn allow_crowdloan_dots_swap() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleCrowdloanDots::allow_crowdloan_swap(
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

        assert_ok!(ModuleCrowdloanDots::allow_crowdloan_swap(
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
        let lending_pool_account = LendingModuleId::get().into_account_truncating();
        let account_1: u64 = 1;
        let account_2: u64 = 2;
        let account_1_sub_account =
            SubaccountsManagerMock::create_subaccount_inner(&account_1, &SubAccType::Borrower)
                .unwrap();

        // deposit and lends tokens main account_1
        ModuleBalances::make_free_balance_be(
            &account_1,
            XDOT,
            SignedBalance::Positive(16 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &account_1,
            CDOT613,
            SignedBalance::Positive(118 * ONE_TOKEN),
        );
        assert_ok!(ModuleLending::deposit(
            RuntimeOrigin::signed(account_1),
            XDOT,
            16 * ONE_TOKEN
        ));
        assert_ok!(ModuleLending::deposit(
            RuntimeOrigin::signed(account_1),
            CDOT613,
            118 * ONE_TOKEN
        ));

        // deposit and lend tokens main account_2
        ModuleBalances::make_free_balance_be(
            &account_2,
            XDOT,
            SignedBalance::Positive(18 * ONE_TOKEN),
        );
        assert_ok!(ModuleLending::deposit(
            RuntimeOrigin::signed(account_2),
            XDOT,
            18 * ONE_TOKEN
        ));

        // set lending rewards for the lent tokens
        assert_ok!(ModuleLending::add_reward(CDOT613, 1000));
        assert_ok!(ModuleLending::add_reward(XDOT, 1000));

        let lender_1_xdot = ModuleLending::lender(&account_1, XDOT).unwrap();
        let lender_2_xdot = ModuleLending::lender(&account_2, XDOT).unwrap();
        let lender_1_cdot613 = ModuleLending::lender(&account_1, CDOT613).unwrap();

        // main account_1 tokens after lend and before swap
        assert_balance!(&account_1, 0, 0, XDOT);
        assert_balance!(&account_1, 0, 0, CDOT613);

        // main account_2 tokens after lend and before swap
        assert_balance!(&account_2, 0, 0, XDOT);

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

        assert_ok!(ModuleCrowdloanDots::allow_crowdloan_swap(
            RawOrigin::Root.into(),
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
                CrowdloanDotAsset::CDOT613,
                CrowdloanDotAsset::CDOT714,
                CrowdloanDotAsset::CDOT815
            ],
        ));

        assert_err!(
            ModuleCrowdloanDots::swap_crowdloan_dots(
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

        // main account_1 lending positions before swap
        assert_eq!(lender_1_xdot.value, 16 * ONE_TOKEN);
        assert_eq!(lender_1_cdot613.value, 118 * ONE_TOKEN);

        // main account_2 lending positions before swap
        assert_eq!(lender_2_xdot.value, 18 * ONE_TOKEN);

        // main account_1 initial EQ balance before swap
        assert_balance!(&account_1, ONE_TOKEN, 0, EQ);

        // main account_2 initial EQ balance before swap
        assert_balance!(&account_2, 0, 0, EQ);

        // lending_pool_account balances before swap
        assert_balance!(&lending_pool_account, (16 + 18) * ONE_TOKEN, 0, XDOT);
        assert_balance!(&lending_pool_account, 118 * ONE_TOKEN, 0, CDOT613);

        // lending pool totals before swap
        assert_eq!(ModuleLending::aggregates(XDOT), (16 + 18) * ONE_TOKEN);
        assert_eq!(ModuleLending::aggregates(CDOT613), 118 * ONE_TOKEN);

        assert_ok!(ModuleCrowdloanDots::swap_crowdloan_dots(
            Some(account_1).into(),
            None,
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
                CrowdloanDotAsset::CDOT613,
                CrowdloanDotAsset::CDOT714,
                CrowdloanDotAsset::CDOT815
            ]
        ));

        assert_ok!(ModuleCrowdloanDots::swap_crowdloan_dots(
            Some(account_1).into(),
            Some(account_2),
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
                CrowdloanDotAsset::CDOT613,
                CrowdloanDotAsset::CDOT714,
                CrowdloanDotAsset::CDOT815
            ]
        ));

        let lender_1_xdot = ModuleLending::lender(&account_1, XDOT);
        let lender_2_xdot = ModuleLending::lender(&account_2, XDOT);
        let lender_1_cdot613 = ModuleLending::lender(&account_1, CDOT613);
        let lender_1_dot = ModuleLending::lender(&account_1, DOT).unwrap();
        let lender_2_dot = ModuleLending::lender(&account_2, DOT).unwrap();

        // main account_1 Crowdloan DOT balances after swap
        assert_balance!(&account_1, 0, 0, XDOT);
        assert_balance!(&account_1, 100 * ONE_TOKEN, 0, XDOT2);
        assert_balance!(&account_1, 0, 0, XDOT3);
        assert_balance!(&account_1, 0, 0, CDOT714);
        assert_balance!(&account_1, 0, 0, CDOT815);
        assert_balance!(&account_1, (40 + 215 + 327 + 427) * ONE_TOKEN, 0, DOT);

        // main account_2 Crowdloan DOT balances after swap
        assert_balance!(&account_2, 0, 0, XDOT);
        assert_balance!(&account_1, 0, 0, CDOT613);
        assert_balance!(&account_2, 0, 0, CDOT714);
        assert_balance!(&account_2, 0, 0, CDOT815);
        assert_balance!(&account_2, (40 + 50 + 60) * ONE_TOKEN, 0, DOT);

        // account_1_subaccount Crowdloan DOT balances after swap
        assert_balance!(&account_1_sub_account, 0, 0, XDOT);
        assert_balance!(&account_1_sub_account, 0, 782 * ONE_TOKEN, XDOT2);
        assert_balance!(&account_1_sub_account, 0, 0, XDOT3);
        assert_balance!(&account_1_sub_account, 0, 0, CDOT613);
        assert_balance!(&account_1_sub_account, 0, 0, CDOT815);
        assert_balance!(&account_1_sub_account, (782) * ONE_TOKEN, 0, DOT);

        // main account_1 lending positions after swap
        assert!(lender_1_xdot.is_none());
        assert!(lender_1_cdot613.is_none());
        assert_eq!(lender_1_dot.value, (118 + 16) * ONE_TOKEN);

        // main account_2 lending positions after swap
        assert!(lender_2_xdot.is_none());
        assert_eq!(lender_2_dot.value, 18 * ONE_TOKEN);

        // main account_1 initial EQ balance after swap
        assert_balance!(&account_1, 1408 + ONE_TOKEN, 0, EQ);

        // main account_2 initial EQ balance after swap
        assert_balance!(&account_2, 522, 0, EQ);

        // lending_pool_account balances after swap
        assert_balance!(&lending_pool_account, 0, 0, XDOT);
        assert_balance!(&lending_pool_account, 0, 0, CDOT613);
        assert_balance!(&lending_pool_account, (16 + 18 + 118) * ONE_TOKEN, 0, DOT);

        // lending pool totals after swap
        assert_eq!(ModuleLending::aggregates(XDOT), 0);
        assert_eq!(ModuleLending::aggregates(CDOT613), 0);
        assert_eq!(ModuleLending::aggregates(DOT), (16 + 18 + 118) * ONE_TOKEN);
    });
}
