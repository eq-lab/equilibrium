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
    new_test_ext, ModuleBalances, ModuleCrowdloanDots, SubaccountsManagerMock, Test,
};
use crate::{AllowedCrowdloanDotsSwap, CrowdloanDotAsset};
use eq_primitives::{SignedBalance, ONE_TOKEN};
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

        assert_ok!(ModuleCrowdloanDots::allow_crowdloan_swap(
            RawOrigin::Root.into(),
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
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

        assert_ok!(ModuleCrowdloanDots::swap_crowdloan_dots(
            Some(account_1).into(),
            None,
            vec![
                CrowdloanDotAsset::XDOT,
                CrowdloanDotAsset::XDOT3,
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
