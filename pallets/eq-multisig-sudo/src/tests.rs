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
#![deny(warnings)]
#![allow(dead_code)]
use super::*;

use crate::mock::{new_test_ext, Call, LoggerCall, ModuleMultisigSudo, Origin, Test};

use codec::Encode;
use frame_support::{assert_noop, assert_ok, weights::Weight};
use frame_system::RawOrigin;
use sp_io::hashing::blake2_256;

// ---------------------- TESTING ACCOUNTS AND PARAMETERS ------------------------------
const THRESHOLD: u32 = 3;
const ALICE_ID: u64 = 1;
const BOB_ID: u64 = 2;
const CHARLIE_ID: u64 = 3;
const DAVE_ID: u64 = 4;
const ERNIE_ID: u64 = 5;

fn get_call_hash(who: u64, call_data: OpaqueCall) -> [u8; 32] {
    return (
        b"CALLHASH",
        who.clone(),
        &call_data[..],
        <frame_system::Pallet<Test>>::block_number(),
    )
        .using_encoded(blake2_256);
}

#[test]
fn test_setup_works() {
    new_test_ext(vec![1u64, 2u64, 3u64], THRESHOLD).execute_with(|| {
        assert_eq!(ModuleMultisigSudo::keys(&1u64), true);
    });
}

#[test]
fn add_key() {
    new_test_ext(vec![1u64, 2u64, 3u64], THRESHOLD).execute_with(|| {
        //Adding Dave from Root
        assert_ok!(ModuleMultisigSudo::add_key(RawOrigin::Root.into(), DAVE_ID));
        //now it is stored
        assert_eq!(ModuleMultisigSudo::keys(DAVE_ID), true);
        //Can't add Dave again
        assert_noop!(
            ModuleMultisigSudo::add_key(RawOrigin::Root.into(), DAVE_ID),
            Error::<Test>::AlreadyInKeyList
        );
    })
}

#[test]
fn remove_key() {
    new_test_ext(vec![1u64, 2u64, 3u64], THRESHOLD).execute_with(|| {
        //Adding Dave from Root
        assert_ok!(ModuleMultisigSudo::add_key(RawOrigin::Root.into(), DAVE_ID));
        //Removing Charlie from Root
        assert_ok!(ModuleMultisigSudo::remove_key(
            RawOrigin::Root.into(),
            DAVE_ID
        ));
        //Can't remove Charlie again
        assert_noop!(
            ModuleMultisigSudo::remove_key(RawOrigin::Root.into(), DAVE_ID),
            Error::<Test>::NotInKeyList
        );
    })
}

#[test]
fn modify_threshold() {
    new_test_ext(vec![1u64, 2u64, 3u64], THRESHOLD).execute_with(|| {
        //Modify threshold to 4
        assert_ok!(ModuleMultisigSudo::modify_threshold(
            RawOrigin::Root.into(),
            2
        ));
        //Can't modify threshold to 0
        assert_noop!(
            ModuleMultisigSudo::modify_threshold(RawOrigin::Root.into(), 0),
            Error::<Test>::InvalidThresholdValue
        );
        //Can't modify threshold to 5 (max 4)
        assert_noop!(
            ModuleMultisigSudo::modify_threshold(RawOrigin::Root.into(), 5),
            Error::<Test>::InvalidThresholdValue
        );
    })
}

#[test]
fn propose() {
    new_test_ext(vec![1u64, 2u64, 3u64], THRESHOLD).execute_with(|| {
        // propose a call
        let call = Box::new(Call::Logger(LoggerCall::privileged_i32_log {
            i: 42,
            weight: Weight::from_ref_time(1_000),
        }));
        let call_data: OpaqueCall = Encode::encode(&call);
        let call_hash = get_call_hash(ALICE_ID, call_data);
        assert_ok!(ModuleMultisigSudo::propose(Origin::signed(ALICE_ID), call));
        assert_eq!(<MultisigProposals<Test>>::contains_key(&call_hash), true);
        // call not accepted
        let call = Box::new(Call::Logger(LoggerCall::privileged_i32_log {
            i: 42,
            weight: Weight::from_ref_time(1_000),
        }));
        assert_noop!(
            ModuleMultisigSudo::propose(Origin::signed(DAVE_ID), call),
            Error::<Test>::NotInKeyList
        );
    })
}

#[test]
fn cancel_proposal() {
    new_test_ext(vec![1u64, 2u64, 3u64], THRESHOLD).execute_with(|| {
        let call = Box::new(Call::Logger(LoggerCall::privileged_i32_log {
            i: 42,
            weight: Weight::from_ref_time(1_000),
        }));
        let call_data: OpaqueCall = Encode::encode(&call);
        let call_hash = get_call_hash(ALICE_ID, call_data);
        assert_ok!(ModuleMultisigSudo::propose(Origin::signed(ALICE_ID), call));
        assert_ok!(ModuleMultisigSudo::modify_threshold(
            RawOrigin::Root.into(),
            2
        ));
        //Bob tries to remove a proposal and succeeds
        assert_ok!(ModuleMultisigSudo::cancel_proposal(
            Origin::signed(BOB_ID),
            call_hash
        ));
        assert_noop!(
            ModuleMultisigSudo::cancel_proposal(Origin::signed(BOB_ID), call_hash),
            Error::<Test>::AlreadyCancelled
        );
        //Alice tries and succeeds
        assert_ok!(ModuleMultisigSudo::cancel_proposal(
            Origin::signed(ALICE_ID),
            call_hash
        ));
        //But no more
        assert_noop!(
            ModuleMultisigSudo::cancel_proposal(Origin::signed(ALICE_ID), call_hash),
            Error::<Test>::ProposalNotFound
        );
    })
}

#[test]
fn approve() {
    new_test_ext(vec![1u64, 2u64, 3u64], THRESHOLD).execute_with(|| {
        assert_ok!(ModuleMultisigSudo::modify_threshold(
            RawOrigin::Root.into(),
            3
        ));
        let call = Box::new(Call::Logger(LoggerCall::privileged_i32_log {
            i: 42,
            weight: Weight::from_ref_time(1_000),
        }));
        let call_data: OpaqueCall = Encode::encode(&call);
        let call_hash = get_call_hash(ALICE_ID, call_data);
        assert_ok!(ModuleMultisigSudo::propose(Origin::signed(ALICE_ID), call));
        //Bob approves
        assert_ok!(ModuleMultisigSudo::approve(
            Origin::signed(BOB_ID),
            call_hash
        ));
        //His account is in .approvals
        let ms = <MultisigProposals<Test>>::get(&call_hash).unwrap();
        assert_eq!(ms.approvals.len(), 2);
        assert_eq!(ms.approvals.contains(&BOB_ID), true);
        //Bob cannot approve again
        assert_noop!(
            ModuleMultisigSudo::approve(Origin::signed(BOB_ID), call_hash),
            Error::<Test>::AlreadyApproved
        );
        //Dave cannot approve as he is not on the list
        assert_noop!(
            ModuleMultisigSudo::approve(Origin::signed(DAVE_ID), call_hash),
            Error::<Test>::NotInKeyList
        );
    })
}

#[test]
fn check_all_sudo() {
    new_test_ext(vec![1u64, 2u64, 3u64], THRESHOLD).execute_with(|| {
        assert_ok!(ModuleMultisigSudo::modify_threshold(
            RawOrigin::Root.into(),
            3
        ));
        let call = Box::new(Call::Logger(LoggerCall::privileged_i32_log {
            i: 42,
            weight: Weight::from_ref_time(1_000),
        }));
        let call_data: OpaqueCall = Encode::encode(&call);
        let call_hash = get_call_hash(ALICE_ID, call_data);
        //propose
        assert_ok!(ModuleMultisigSudo::propose(Origin::signed(ALICE_ID), call));
        //approve
        assert_ok!(ModuleMultisigSudo::approve(
            Origin::signed(BOB_ID),
            call_hash
        ));
        //approve
        assert_ok!(ModuleMultisigSudo::approve(
            Origin::signed(CHARLIE_ID),
            call_hash
        ));

        //check if call_hash is out of the MultisigProposalsMap -> Sudid
        assert_eq!(<MultisigProposals<Test>>::contains_key(&call_hash), false);
    })
}
