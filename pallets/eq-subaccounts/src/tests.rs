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

use crate::mock::{
    new_test_ext, AccountId, Balance, MarginCallManagerMock, ModuleAggregates, ModuleBalances,
    ModuleSubaccounts, Origin, Test,
};
use crate::{Error, SubAccType};
use eq_primitives::TransferReason;
use eq_primitives::{
    asset,
    asset::Asset,
    balance::{BalanceGetter, EqCurrency},
    subaccount::SubaccountsManager,
    Aggregates, MarginState, SignedBalance, TotalAggregates, UserGroup,
};
use eq_utils::ONE_TOKEN;
use frame_support::traits::ExistenceRequirement;
use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::RawOrigin;

// ----- Utilities ---------------------------------------------------------------------------------

fn create_subaccount(acc_id: &AccountId, subacc_type: SubAccType) -> AccountId {
    assert_ok!(ModuleSubaccounts::create_subaccount_inner(
        acc_id,
        &subacc_type,
    ));
    ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap()
}

fn check_subacc_in_user_group(
    subaccount: AccountId,
    subacc_type: SubAccType,
    is_in_group: bool,
    step_name: &str,
) {
    let expected_user_group = match subacc_type {
        SubAccType::Trader | SubAccType::Borrower => UserGroup::Borrowers,
        SubAccType::Bailsman => UserGroup::Bailsmen,
    };

    assert_eq!(
        ModuleAggregates::in_usergroup(&subaccount, expected_user_group),
        is_in_group,
        "Wrong subaccount '{:?}' user group state on test step: '{}'",
        subacc_type,
        step_name
    );
}

fn set_subacc_balance_directly(
    subaccount: AccountId,
    asset: Asset,
    balance: &SignedBalance<Balance>,
) {
    ModuleBalances::make_free_balance_be(&subaccount, asset, balance.clone());
    assert_eq!(
        ModuleBalances::get_balance(&subaccount, &asset),
        *balance,
        "Util function set_subacc_balance_directly did not set correct balance"
    );
}

fn create_bailsman_with_balance(
    main_acc: AccountId,
    asset: Asset,
    balance: Balance,
    ensure_is_bailsman: bool,
) -> AccountId {
    assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

    let subaccount = create_subaccount(&main_acc, SubAccType::Bailsman);

    let current_main_balance = ModuleBalances::get_balance(&main_acc, &asset);
    ModuleBalances::make_free_balance_be(
        &main_acc,
        asset,
        SignedBalance::Positive(balance + current_main_balance.abs()),
    );

    assert!(
        ModuleSubaccounts::transfer_to_subaccount(
            Origin::signed(main_acc),
            SubAccType::Bailsman,
            asset,
            balance,
        )
        .is_ok(),
        "Util function create_bailsman_with_balance: could not transfer to bailsman subacc. \
        Main acc balance before transfer: {:?}",
        current_main_balance
    );

    if ensure_is_bailsman {
        assert_eq!(
            ModuleAggregates::in_usergroup(&subaccount, UserGroup::Bailsmen),
            true,
            "Util function create_bailsman_with_balance: added subaccount is not a bailsman",
        );
    };
    subaccount
}

// ----- Tests -------------------------------------------------------------------------------------

#[test]
fn new_acc_does_not_have_subaccounts() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 1;

        for subacc_type in SubAccType::iterator() {
            assert_eq!(
                ModuleSubaccounts::has_subaccount(&acc_id, &subacc_type),
                false
            );
        }
    });
}

/// who, balance, debt, asset
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
fn add_trader_subaccount() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 1;
        let subacc_type = SubAccType::Trader;
        create_subaccount(&acc_id, subacc_type);

        assert!(ModuleSubaccounts::has_subaccount(&acc_id, &subacc_type));
        assert!(!ModuleSubaccounts::has_subaccount(
            &acc_id,
            &SubAccType::Bailsman
        ));

        // Checking subaccount storage
        let created_subacc = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();
        assert!(ModuleSubaccounts::subaccount(&acc_id, &SubAccType::Bailsman).is_none());

        // Checking owner account storage
        let owner_account = ModuleSubaccounts::owner_account(&created_subacc).unwrap();
        assert_eq!(owner_account.1, subacc_type);

        let _ = ModuleSubaccounts::try_set_usergroup(&created_subacc, &subacc_type);

        // Checking account has appropriate user group
        assert!(ModuleAggregates::in_usergroup(
            &created_subacc,
            UserGroup::Borrowers
        ));
    });
}

#[test]
fn add_borrower_subaccount() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 1;
        let subacc_type = SubAccType::Borrower;
        create_subaccount(&acc_id, subacc_type);

        assert!(ModuleSubaccounts::has_subaccount(&acc_id, &subacc_type));
        assert!(!ModuleSubaccounts::has_subaccount(
            &acc_id,
            &SubAccType::Bailsman
        ));

        // Checking subaccount storage
        let created_subacc = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();
        assert!(ModuleSubaccounts::subaccount(&acc_id, &SubAccType::Bailsman).is_none());

        // Checking owner account storage
        let owner_account = ModuleSubaccounts::owner_account(&created_subacc).unwrap();
        assert_eq!(owner_account.1, subacc_type);

        let _ = ModuleSubaccounts::try_set_usergroup(&created_subacc, &subacc_type);

        // Checking account has appropriate user group
        assert!(ModuleAggregates::in_usergroup(
            &created_subacc,
            UserGroup::Borrowers
        ));
    });
}

#[test]
fn add_all_subaccounts() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 6;

        for subacc_type in SubAccType::iterator() {
            create_subaccount(&acc_id, subacc_type);
            assert!(ModuleSubaccounts::has_subaccount(&acc_id, &subacc_type));

            let created_subacc = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();
            let owner_account = ModuleSubaccounts::owner_account(&created_subacc).unwrap();
            assert_eq!(owner_account.1, subacc_type);
        }
    });
}

#[test]
fn transfer_to_subacc_updates_aggregates() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));
        let acc_id: AccountId = 1;
        let asset = asset::ETH;
        let total_balance = 3_000_000_000; // Would be 3 of asset
        let transferred_amount = total_balance / 3;
        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id,
            asset,
            total_balance,
            true,
            None
        ));

        println!(
            "Aggregate {:?}",
            ModuleAggregates::total_user_groups(&UserGroup::Balances, asset)
        );

        // Checking all user groups aggregates are empty (except Balances)
        for user_group in UserGroup::iterator() {
            if user_group == UserGroup::Balances {
                continue;
            }

            assert_eq!(
                ModuleAggregates::total_user_groups(user_group, asset),
                TotalAggregates {
                    collateral: 0,
                    debt: 0
                },
                "Aggregates are empty before test transfer for user group: {:?}",
                user_group
            );
        }
        // use eq_bailsman::LtvChecker;
        for subacc_type in SubAccType::iterator() {
            // let subaccount = create_subaccount(&acc_id, &subacc_type);
            let ltv = ModuleBalances::get_balance(&acc_id, &asset);
            println!("subacc_type {:?} balance {:?}", subacc_type, ltv);
            // Transferring to subacc to update aggregates
            assert_ok!(ModuleSubaccounts::transfer_to_subaccount(
                Origin::signed(acc_id),
                subacc_type,
                asset,
                transferred_amount
            ));

            let new_total = TotalAggregates {
                collateral: transferred_amount,
                debt: 0,
            };

            // Checking user group aggregates updated
            match subacc_type {
                SubAccType::Trader => {
                    assert_eq!(
                        ModuleAggregates::total_user_groups(UserGroup::Borrowers, asset),
                        new_total,
                        "Wrong aggregates for subacc type: {:?}",
                        subacc_type
                    );
                }
                SubAccType::Borrower => {
                    assert_eq!(
                        ModuleAggregates::total_user_groups(UserGroup::Borrowers, asset),
                        TotalAggregates {
                            collateral: new_total.collateral * 2,
                            debt: new_total.debt
                        },
                        "Wrong aggregates for subacc type: {:?}",
                        subacc_type
                    );
                }
                // Not enough to reg as bailsman
                SubAccType::Bailsman => {
                    assert_eq!(
                        ModuleAggregates::total_user_groups(UserGroup::Bailsmen, asset),
                        TotalAggregates {
                            debt: 0,
                            collateral: 0
                        },
                        "Wrong aggregates for subacc type: {:?}",
                        subacc_type
                    );
                }
            }
        }
    });
}

#[test]
fn add_subaccount_when_exists() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 1;
        let subacc_type = SubAccType::Bailsman;

        ModuleSubaccounts::create_subaccount_inner(&acc_id, &subacc_type).unwrap();
        let expected_subacc = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();
        let expected_owner_acc = ModuleSubaccounts::owner_account(&expected_subacc).unwrap();

        assert_noop!(
            ModuleSubaccounts::create_subaccount_inner(&acc_id, &subacc_type),
            Error::<Test>::AlreadyHasSubaccount
        );

        let actual_subacc = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();
        let actual_owner_acc = ModuleSubaccounts::owner_account(&actual_subacc).unwrap();

        assert_eq!(expected_subacc, actual_subacc);
        assert_eq!(expected_owner_acc, actual_owner_acc);
    })
}

#[test]
fn delete_subaccount() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 77;
        let subacc_type = SubAccType::Trader;

        // Creating subaccount
        create_subaccount(&acc_id, subacc_type);
        assert!(ModuleSubaccounts::has_subaccount(&acc_id, &subacc_type));
        let created_subaccount = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();

        // Deleting subaccount
        assert_ok!(ModuleSubaccounts::delete_subaccount_inner(
            &acc_id,
            &subacc_type,
        ));

        // Checking storage
        assert!(ModuleSubaccounts::owner_account(&created_subaccount).is_none());
        for a_subacc_type in SubAccType::iterator() {
            assert!(!ModuleSubaccounts::has_subaccount(&acc_id, &a_subacc_type));
        }
    });
}

#[test]
fn delete_subaccount_removes_from_usergroup() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 94;

        for subacc_type in SubAccType::iterator() {
            create_subaccount(&acc_id, subacc_type);
            let created_subaccount = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();
            let _ = ModuleSubaccounts::try_set_usergroup(&created_subaccount, &subacc_type);

            // Checking correct UserGroup state
            let expected_status = if subacc_type == SubAccType::Bailsman {
                false
            } else {
                true
            };
            check_subacc_in_user_group(
                created_subaccount,
                subacc_type,
                expected_status,
                "after subaccount was created",
            );

            // Deleting subaccount
            assert_ok!(ModuleSubaccounts::delete_subaccount_inner(
                &acc_id,
                &subacc_type,
            ));

            // Checking all subaccounts are not in UserGroups
            check_subacc_in_user_group(
                created_subaccount,
                subacc_type,
                false,
                "after subaccount was deleted",
            );
        }
    });
}

#[test]
fn delete_subaccount_when_none() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 93;

        for a_subacc_type in SubAccType::iterator() {
            assert_noop!(
                ModuleSubaccounts::delete_subaccount_inner(&acc_id, &a_subacc_type),
                Error::<Test>::NoSubaccountOfThisType
            );
        }
    });
}

#[test]
fn subaccs_count() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 42;
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));
        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::BTC,
            SignedBalance::Positive(10_000_000_000_000),
        );

        assert_eq!(ModuleSubaccounts::get_subaccounts_amount(&acc_id), 0);

        // subaccounts not exist
        for subacc_type in SubAccType::iterator() {
            assert_ok!(ModuleSubaccounts::transfer_to_subaccount(
                Origin::signed(acc_id),
                subacc_type,
                asset::BTC,
                11_000_000_000,
            ));
            // check for creating subaccount
            assert!(ModuleSubaccounts::has_subaccount(&acc_id, &subacc_type));
            let created_subacc = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();
            let owner_account = ModuleSubaccounts::owner_account(&created_subacc).unwrap();
            assert_eq!(owner_account.1, subacc_type);
            // check for balances
            assert_balance!(created_subacc, 11_000_000_000, 0, asset::BTC);
            check_subacc_in_user_group(created_subacc, subacc_type, true, "after creation");
        }

        assert_eq!(ModuleSubaccounts::get_subaccounts_amount(&acc_id), 3);
    });
}

#[test]
fn transfer_to_subacc() {
    new_test_ext().execute_with(|| {
        let acc_id: AccountId = 42;
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));
        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::BTC,
            SignedBalance::Positive(10_000_000_000_000),
        );
        // subaccounts not exist
        for subacc_type in SubAccType::iterator() {
            assert_ok!(ModuleSubaccounts::transfer_to_subaccount(
                Origin::signed(acc_id),
                subacc_type,
                asset::BTC,
                11_000_000_000,
            ));
            // check for creating subaccount
            assert!(ModuleSubaccounts::has_subaccount(&acc_id, &subacc_type));
            let created_subacc = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();
            let owner_account = ModuleSubaccounts::owner_account(&created_subacc).unwrap();
            assert_eq!(owner_account.1, subacc_type);
            // check for balances
            assert_balance!(created_subacc, 11_000_000_000, 0, asset::BTC);
            check_subacc_in_user_group(created_subacc, subacc_type, true, "after creation");
        }
        // expected_value = 10_000 - 11 - 11 - 11 = 9_967
        assert_balance!(acc_id, 9_967_000_000_000, 0, asset::BTC);

        // subaccounts exist
        for subacc_type in SubAccType::iterator() {
            assert_ok!(ModuleSubaccounts::transfer_to_subaccount(
                Origin::signed(acc_id),
                subacc_type,
                asset::BTC,
                1_000_000_000,
            ));
            let subacc_id = ModuleSubaccounts::subaccount(&acc_id, &subacc_type).unwrap();
            // check for balances
            assert_balance!(subacc_id, 12_000_000_000, 0, asset::BTC);
        }
        //expected_value = 9_967 - 1 - 1 - 1 = 9_964
        assert_balance!(acc_id, 9_964_000_000_000, 0, asset::BTC);
    });
}

#[test]
fn transfer_to_bailsman() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        let acc_id: AccountId = 42;

        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::BTC,
            SignedBalance::Positive(10_000_000_000),
        );

        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::EQD,
            SignedBalance::Positive(102_000_000_000_000),
        );

        // check for no register as bailsman with transfer < MinCollateral

        assert_ok!(ModuleSubaccounts::transfer_to_subaccount(
            Origin::signed(acc_id),
            SubAccType::Bailsman,
            asset::EQD,
            1_000_000_000_000,
        ));

        let bails_subacc_id = ModuleSubaccounts::subaccount(&acc_id, SubAccType::Bailsman).unwrap();

        assert!(!ModuleAggregates::in_usergroup(
            &bails_subacc_id,
            UserGroup::Bailsmen
        ));

        // check for register as bailsman with subacc balance > MinCollateral

        assert_ok!(ModuleSubaccounts::transfer_to_subaccount(
            Origin::signed(acc_id),
            SubAccType::Bailsman,
            asset::EQD,
            100_000_000_000_000,
        ));

        assert!(ModuleAggregates::in_usergroup(
            &bails_subacc_id,
            UserGroup::Bailsmen
        ));
    });
}

#[test]
fn transfer_from_subaccount() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));
        let main_acc = 55;
        let test_asset = asset::EOS;
        let mut balance_inner: Balance = 88_005_553_535; // Subacc balance to set
        let mut withdrawn: Balance = 34_206_911_777; // Balance to withdraw from subacc
        let mut expected_balance_inner: Balance = 0; // Initial main acc balance

        for subacc_type in SubAccType::iterator() {
            let subacc = create_subaccount(&main_acc, subacc_type);
            let balance = SignedBalance::Positive(balance_inner);
            set_subacc_balance_directly(subacc, test_asset, &balance);
            expected_balance_inner = expected_balance_inner + withdrawn;

            assert_ok!(ModuleSubaccounts::transfer_from_subaccount(
                Origin::signed(main_acc),
                subacc_type,
                test_asset,
                withdrawn
            ));
            assert_eq!(
                ModuleBalances::get_balance(&subacc, &test_asset),
                SignedBalance::Positive(balance_inner - withdrawn),
                "Wrong balance of subaccount after transfer from it: {:?}",
                subacc_type
            );
            assert_eq!(
                ModuleBalances::get_balance(&main_acc, &test_asset),
                SignedBalance::Positive(expected_balance_inner),
                "Wrong balance of main acc after transfer from subacc: {:?}",
                subacc_type,
            );

            // Updating params to be different on next step
            balance_inner = balance_inner * 2;
            withdrawn = withdrawn * 2;
        }
    })
}

#[test]
fn transfer_from_non_existent_subacc() {
    new_test_ext().execute_with(|| {
        let main_acc: AccountId = 322;
        let test_asset = asset::BTC;
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));
        for subacc_type in SubAccType::iterator() {
            assert!(!ModuleSubaccounts::has_subaccount(&main_acc, &subacc_type));

            assert_err!(
                ModuleSubaccounts::transfer_from_subaccount(
                    Origin::signed(main_acc),
                    subacc_type,
                    test_asset,
                    11_987_654_321
                ),
                Error::<Test>::NoSubaccountOfThisType
            );
            assert_eq!(
                ModuleBalances::get_balance(&main_acc, &test_asset),
                SignedBalance::Positive(0),
                "Wrong balance of main account after failed transfer from subaccount: {:?}",
                subacc_type
            );
        }
    })
}

#[test]
fn transfer_from_subacc_without_balance() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));
        let main_acc: AccountId = 117;
        let test_asset = asset::DOT;
        let mut to_withdraw: Balance = 123_789;
        MarginCallManagerMock::set_margin_state(MarginState::SubGood);

        for subacc_type in SubAccType::iterator() {
            let subacc = create_subaccount(&main_acc, subacc_type);

            assert_err!(
                ModuleSubaccounts::transfer_from_subaccount(
                    Origin::signed(main_acc),
                    subacc_type,
                    test_asset,
                    to_withdraw
                ),
                eq_bailsman::Error::<Test>::WrongMargin,
            );
            assert_eq!(
                ModuleBalances::get_balance(&subacc, &test_asset),
                SignedBalance::Positive(0),
                "Wrong balance of subaccount after failed transfer from it: {:?}",
                subacc_type
            );
            assert_eq!(
                ModuleBalances::get_balance(&main_acc, &test_asset),
                SignedBalance::Positive(0),
                "Wrong balance of main acc after failed transfer from subacc: {:?}",
                subacc_type
            );

            to_withdraw = to_withdraw * 11;
        }
    })
}

#[test]
fn transfer_from_subacc_fails_low_balance() {
    // fails due to bailsman balance checker, bad ltv ratio

    new_test_ext().execute_with(|| {
        let main_acc: AccountId = 921;
        let test_asset = asset::EQD;
        let mut to_withdraw: Balance = 123_789;
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        MarginCallManagerMock::set_margin_state(MarginState::SubGood);

        for subacc_type in SubAccType::iterator() {
            let subacc = create_subaccount(&main_acc, subacc_type);
            let subacc_balance = to_withdraw / 2;
            set_subacc_balance_directly(
                subacc,
                test_asset,
                &SignedBalance::Positive(subacc_balance),
            );

            assert_err!(
                ModuleSubaccounts::transfer_from_subaccount(
                    Origin::signed(main_acc),
                    subacc_type,
                    test_asset,
                    to_withdraw
                ),
                eq_bailsman::Error::<Test>::WrongMargin,
            );
            assert_eq!(
                ModuleBalances::get_balance(&subacc, &test_asset),
                SignedBalance::Positive(subacc_balance),
                "Wrong balance of subaccount after failed transfer from it: {:?}",
                subacc_type
            );
            assert_eq!(
                ModuleBalances::get_balance(&main_acc, &test_asset),
                SignedBalance::Positive(0),
                "Wrong balance of main acc after failed transfer from subacc: {:?}",
                subacc_type
            );

            to_withdraw = to_withdraw * 11;
        }
    })
}

#[test]
fn transfer_from_will_unregister_bailsman() {
    new_test_ext().execute_with(|| {
        let main_acc_1: AccountId = 873; // Will transfer all funds from bailsman subacc
        let main_acc_2: AccountId = 495; // Will transfer just enough funds from bailsman subacc to unreg
        let main_acc_3: AccountId = 6; // Will not transfer enough to unreg bailsman
        let test_asset = asset::BTC; // Expected price = 10_000
        let initial_bailsman_balance: Balance = 11_000_000_000;

        // Creating bailsman subaccounts
        let bailsman_1 =
            create_bailsman_with_balance(main_acc_1, test_asset, initial_bailsman_balance, true);
        let bailsman_2 =
            create_bailsman_with_balance(main_acc_2, test_asset, initial_bailsman_balance, true);
        let bailsman_3 =
            create_bailsman_with_balance(main_acc_3, test_asset, initial_bailsman_balance, true);

        // Removing funds from subaccounts
        assert_ok!(ModuleSubaccounts::transfer_from_subaccount(
            Origin::signed(main_acc_1),
            SubAccType::Bailsman,
            test_asset,
            initial_bailsman_balance
        ));
        assert_ok!(ModuleSubaccounts::transfer_from_subaccount(
            Origin::signed(main_acc_2),
            SubAccType::Bailsman,
            test_asset,
            initial_bailsman_balance - 9_000_000_000
        ));
        assert_ok!(ModuleSubaccounts::transfer_from_subaccount(
            Origin::signed(main_acc_3),
            SubAccType::Bailsman,
            test_asset,
            999_999_999
        ));

        // Checking unreg
        assert_eq!(
            ModuleAggregates::in_usergroup(&bailsman_1, UserGroup::Bailsmen),
            false,
            "Bailsman did not unregister after all funds transferred from it's subaccount",
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&bailsman_2, UserGroup::Bailsmen),
            false,
            "Bailsman did not unregister after balance became less than required to be bailsman",
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&bailsman_3, UserGroup::Bailsmen),
            true,
            "Bailsman subaccount had enough asset on it's balance after transfer but \
            was unregistered",
        );
    })
}

#[test]
fn transfer_from_bailsman_with_debt() {
    new_test_ext().execute_with(|| {
        let main_acc_1: AccountId = 873; // Will need to unreg - should fail
        let main_acc_2: AccountId = 495; // Will not need to unreg - should be ok
        let test_asset = asset::ETH; // Expected price = 250
        let initial_bailsman_balance: Balance = 500 * 1_000_000_000;
        // MinimalCollateral is 100_000$

        // Creating bailsman subaccounts
        let bailsman_1 =
            create_bailsman_with_balance(main_acc_1, test_asset, initial_bailsman_balance, true);
        let bailsman_2 =
            create_bailsman_with_balance(main_acc_2, test_asset, initial_bailsman_balance, true);

        // Setting debts to subaccounts directly
        set_subacc_balance_directly(
            bailsman_1,
            asset::EOS, // price is 3
            &SignedBalance::Negative(333_000_000_000),
        );
        set_subacc_balance_directly(
            bailsman_2,
            asset::EOS,
            &SignedBalance::Negative(333_000_000_000),
        );

        // Should fail whole transfer because of debt
        assert_noop!(
            ModuleSubaccounts::transfer_from_subaccount(
                Origin::signed(main_acc_1),
                SubAccType::Bailsman,
                test_asset,
                101 * 1_000_000_000
            ),
            eq_bailsman::Error::<Test>::BailsmanHasDebt
        );

        // Should be ok, because no need to reinit
        assert_ok!(ModuleSubaccounts::transfer_from_subaccount(
            Origin::signed(main_acc_2),
            SubAccType::Bailsman,
            test_asset,
            89 * 1_000_000_000
        ));

        // Checking unreg din not happen
        assert_eq!(
            ModuleAggregates::in_usergroup(&bailsman_1, UserGroup::Bailsmen),
            true,
            "Bailsman 1 unregistered unexpectedly",
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&bailsman_2, UserGroup::Bailsmen),
            true,
            "Bailsman 2 unregistered unexpectedly",
        );

        // Checking balances (when unreg fails transfer should not happen)
        assert_eq!(
            ModuleBalances::get_balance(&bailsman_1, &test_asset),
            SignedBalance::Positive(initial_bailsman_balance),
            "Wrong balance of bailsman subacc after failed transfer from it"
        );
        assert_eq!(
            ModuleBalances::get_balance(&bailsman_2, &test_asset),
            SignedBalance::Positive((500 - 89) * 1_000_000_000),
            "Wrong balance of bailsman subacc after transfer from it"
        );
    })
}

#[test]
fn transfer_negative_non_borrower_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        let main_acc: AccountId = 921;
        let collat_asset = asset::BTC;
        let test_asset = asset::EQD;
        let mut to_withdraw: Balance = 123_789;

        for subacc_type in SubAccType::iterator() {
            if subacc_type == SubAccType::Trader || subacc_type == SubAccType::Borrower {
                continue;
            }
            let subacc = create_subaccount(&main_acc, subacc_type);
            let subacc_balance = to_withdraw * 2;
            set_subacc_balance_directly(
                subacc,
                collat_asset,
                &SignedBalance::Positive(subacc_balance),
            );

            assert_err!(
                ModuleSubaccounts::transfer_from_subaccount(
                    Origin::signed(main_acc),
                    subacc_type,
                    test_asset,
                    to_withdraw
                ),
                Error::<Test>::Debt,
            );
            assert_eq!(
                ModuleBalances::get_balance(&subacc, &collat_asset),
                SignedBalance::Positive(subacc_balance),
                "Wrong balance of subaccount after failed transfer from it: {:?}",
                subacc_type
            );
            assert_eq!(
                ModuleBalances::get_balance(&main_acc, &test_asset),
                SignedBalance::Positive(0),
                "Wrong balance of main acc after failed transfer from subacc: {:?}",
                subacc_type
            );

            to_withdraw = to_withdraw * 11;
        }
    })
}

#[test]
fn borrower_transfer_negative() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));
        let main_acc: AccountId = 921;
        let collat_asset = asset::BTC;
        let test_asset = asset::EQD;
        let to_withdraw: Balance = 123_789;

        let subacc_type = SubAccType::Trader;
        let subacc = create_subaccount(&main_acc, subacc_type);
        ModuleSubaccounts::try_set_usergroup(&subacc, &subacc_type).unwrap();
        let subacc_balance = to_withdraw * 2;
        set_subacc_balance_directly(
            subacc,
            collat_asset,
            &SignedBalance::Positive(subacc_balance),
        );

        assert_ok!(ModuleSubaccounts::transfer_from_subaccount(
            Origin::signed(main_acc),
            subacc_type,
            test_asset,
            to_withdraw
        ));
        assert_eq!(
            ModuleBalances::get_balance(&subacc, &collat_asset),
            SignedBalance::Positive(subacc_balance),
            "Wrong balance of subaccount after transfer from it: {:?}",
            subacc_type
        );
        assert_eq!(
            ModuleBalances::get_balance(&subacc, &test_asset),
            SignedBalance::Negative(to_withdraw),
            "Wrong balance of subaccount after transfer from it: {:?}",
            subacc_type
        );
        assert_eq!(
            ModuleBalances::get_balance(&main_acc, &test_asset),
            SignedBalance::Positive(to_withdraw),
            "Wrong balance of main acc after transfer from subacc: {:?}",
            subacc_type
        );
    })
}

#[test]
fn transfer_from_borrower_to_account() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        let source_acc: AccountId = 921;
        let collat_asset = asset::BTC;
        let test_asset = asset::EQD;
        let to_transfer = 123_789;

        let subacc_type = SubAccType::Trader;
        let subacc = create_subaccount(&source_acc, subacc_type);
        ModuleSubaccounts::try_set_usergroup(&subacc, &subacc_type).unwrap();

        let subacc_balance = to_transfer * 2;
        set_subacc_balance_directly(
            subacc,
            collat_asset,
            &SignedBalance::Positive(subacc_balance),
        );

        let dest_acc: AccountId = 900;

        assert_ok!(ModuleSubaccounts::transfer(
            Origin::signed(source_acc),
            subacc_type,
            dest_acc.clone(),
            test_asset,
            to_transfer
        ));

        assert_eq!(
            ModuleBalances::get_balance(&subacc, &test_asset),
            SignedBalance::Negative(to_transfer),
            "Wrong balance of main acc after transfer from subacc: {:?}",
            subacc_type
        );

        assert_eq!(
            ModuleBalances::get_balance(&dest_acc, &test_asset),
            SignedBalance::Positive(to_transfer),
            "Wrong balance of destination acc after transfer from subacc: {:?}",
            subacc_type
        );
    });
}

#[test]
fn transfer_from_bailsman_to_account() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        let source_acc: AccountId = 921;
        let collat_asset = asset::EQD;
        let test_asset = asset::BTC;
        let collat_balance = 200_000 * ONE_TOKEN;
        let to_transfer = 123_789 * ONE_TOKEN;

        let subacc_type = SubAccType::Bailsman;
        let subacc = create_subaccount(&source_acc, subacc_type);

        let subacc_balance = to_transfer * 2;
        set_subacc_balance_directly(
            subacc,
            collat_asset,
            &SignedBalance::Positive(collat_balance),
        );
        set_subacc_balance_directly(subacc, test_asset, &SignedBalance::Positive(subacc_balance));

        ModuleSubaccounts::try_set_usergroup(&subacc, &subacc_type).unwrap();

        let dest_acc: AccountId = 900;

        assert_ok!(ModuleSubaccounts::transfer(
            Origin::signed(source_acc),
            subacc_type,
            dest_acc.clone(),
            test_asset,
            to_transfer
        ));

        assert_eq!(
            ModuleBalances::get_balance(&subacc, &test_asset),
            SignedBalance::Positive(to_transfer),
            "Wrong balance of main acc after transfer from subacc: {:?}",
            subacc_type
        );

        assert_eq!(
            ModuleBalances::get_balance(&dest_acc, &test_asset),
            SignedBalance::Positive(to_transfer),
            "Wrong balance of destination acc after transfer from subacc: {:?}",
            subacc_type
        );
    });
}

#[test]
fn trasnsfer_from_bailsman_should_fail_when_not_enough_balance() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));

        let source_acc: AccountId = 921;
        let collat_asset = asset::EQD;
        let test_asset = asset::BTC;
        let collat_balance = 200_000 * ONE_TOKEN;
        let to_transfer = 123_789 * ONE_TOKEN;

        let subacc_type = SubAccType::Bailsman;
        let subacc = create_subaccount(&source_acc, subacc_type);

        set_subacc_balance_directly(
            subacc,
            collat_asset,
            &SignedBalance::Positive(collat_balance),
        );
        ModuleSubaccounts::try_set_usergroup(&subacc, &subacc_type).unwrap();

        let dest_acc: AccountId = 900;

        assert_err!(
            ModuleSubaccounts::transfer(
                Origin::signed(source_acc),
                subacc_type,
                dest_acc.clone(),
                test_asset,
                to_transfer
            ),
            Error::<Test>::Debt
        );
    });
}

#[test]
fn transfer_from_subaccount_to_subaccount_err() {
    new_test_ext().execute_with(|| {
        assert_ok!(ModuleBalances::enable_transfers(RawOrigin::Root.into()));
        let source_acc: u64 = 921;
        let collat_asset = asset::BTC;
        let to_transfer = 123_789;

        let subacc_type = SubAccType::Trader;
        let subacc = create_subaccount(&source_acc, subacc_type);
        ModuleSubaccounts::try_set_usergroup(&subacc, &subacc_type).unwrap();

        let subacc_balance = to_transfer * 2;
        set_subacc_balance_directly(
            subacc,
            collat_asset,
            &SignedBalance::Positive(subacc_balance),
        );

        let dest_acc: AccountId = 900;
        let dest_subacc_type = SubAccType::Trader;
        let dest_subacc = create_subaccount(&dest_acc, dest_subacc_type);
        ModuleSubaccounts::try_set_usergroup(&subacc, &subacc_type).unwrap();

        assert_noop!(
            ModuleSubaccounts::transfer(
                Origin::signed(source_acc),
                subacc_type,
                dest_subacc.clone(),
                collat_asset,
                to_transfer
            ),
            Error::<Test>::AccountIsNotMaster
        );

        assert_eq!(
            ModuleBalances::get_balance(&subacc, &collat_asset),
            SignedBalance::Positive(subacc_balance),
            "Wrong balance of main acc after transfer from subacc: {:?}",
            subacc_type
        );

        assert_eq!(
            ModuleBalances::get_balance(&dest_subacc, &collat_asset),
            SignedBalance::Positive(0),
            "Wrong balance of destination acc after transfer from subacc: {:?}",
            subacc_type
        );
    });
}

#[test]
fn transfer_from_bailsman_new_balance_neg_change_pos_ok() {
    new_test_ext().execute_with(|| {
        let master = 1;
        let transactor = 2;
        let asset = asset::BTC;
        let subacc = create_subaccount(&master, SubAccType::Bailsman);
        let subacc_balance = 2;
        let transactor_balance = 3;
        let to_transfer = 1;
        assert!(
            subacc_balance > to_transfer,
            "Abs subacc balance should be less then transfer"
        );

        ModuleBalances::make_free_balance_be(
            &subacc,
            asset,
            SignedBalance::Negative(subacc_balance),
        );
        ModuleBalances::make_free_balance_be(
            &transactor,
            asset,
            SignedBalance::Positive(transactor_balance),
        );

        assert_eq!(
            ModuleBalances::get_balance(&subacc, &asset),
            SignedBalance::Negative(subacc_balance),
            "Make free balance error"
        );

        assert_eq!(
            ModuleBalances::get_balance(&transactor, &asset),
            SignedBalance::Positive(transactor_balance),
            "Make free balance error"
        );

        assert_ok!(ModuleBalances::currency_transfer(
            &transactor,
            &subacc,
            asset,
            to_transfer,
            ExistenceRequirement::AllowDeath,
            TransferReason::Common,
            true
        ));

        assert_eq!(
            ModuleBalances::get_balance(&subacc, &asset),
            SignedBalance::Negative(subacc_balance - to_transfer),
            "Make free balance error"
        );
    });
}

#[test]
fn transfer_from_bailsman_new_balance_neg_change_neg_err() {
    new_test_ext().execute_with(|| {
        let master = 1;
        let dest = 2;
        let asset = asset::BTC;
        let subacc = create_subaccount(&master, SubAccType::Bailsman);
        let subacc_balance = 2;
        let to_transfer = 1;

        ModuleBalances::make_free_balance_be(
            &subacc,
            asset,
            SignedBalance::Negative(subacc_balance),
        );
        ModuleBalances::make_free_balance_be(&dest, asset, SignedBalance::Positive(subacc_balance));

        assert_eq!(
            ModuleBalances::get_balance(&subacc, &asset),
            SignedBalance::Negative(subacc_balance),
            "Make free balance error"
        );

        assert_noop!(
            ModuleBalances::currency_transfer(
                &subacc,
                &dest,
                asset,
                to_transfer,
                ExistenceRequirement::AllowDeath,
                TransferReason::Common,
                true
            ),
            Error::<Test>::Debt
        );

        assert_eq!(
            ModuleBalances::get_balance(&dest, &asset),
            SignedBalance::Positive(subacc_balance),
            "Make free balance error"
        );

        assert_eq!(
            ModuleBalances::get_balance(&subacc, &asset),
            SignedBalance::Negative(subacc_balance),
            "Make free balance error"
        );
    });
}

#[test]
fn create_subaccount_inner_inc_provider() {
    new_test_ext().execute_with(|| {
        let borrower = 1;
        let bailsman = 2;
        assert_ok!(ModuleSubaccounts::create_subaccount_inner(
            &borrower,
            &SubAccType::Trader,
        ));

        assert_ok!(ModuleSubaccounts::create_subaccount_inner(
            &bailsman,
            &SubAccType::Bailsman,
        ));

        assert_eq!(frame_system::Pallet::<Test>::account(borrower).providers, 1);
        assert_eq!(frame_system::Pallet::<Test>::account(bailsman).providers, 1);
    });
}
