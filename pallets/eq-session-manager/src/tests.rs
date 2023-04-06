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

use crate::mock::{self, Test};
use crate::mock::{
    force_new_session, initialize_block, new_test_ext, session_changed, validators,
    ErrorSessionManager, MockSessionKeys, ModuleSessionManager, Origin, Session,
};
use frame_support::assert_err;
use sp_runtime::testing::UintAuthorityId;

fn sorted<T: Clone + Ord>(v: Vec<T>) -> Vec<T> {
    let mut w = v.clone();
    w.sort();
    w
}

fn register_validator(id: u64) {
    frame_system::Pallet::<Test>::inc_providers(&id);
    Session::set_keys(
        Origin::signed(id),
        MockSessionKeys::from(UintAuthorityId::from(id)),
        vec![],
    )
    .unwrap();
}

#[test]
fn initial_validators() {
    new_test_ext().execute_with(|| {
        let actual = <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(0)
            .map(|x| sorted(x));
        let expected = Some(mock::initial_validators());
        assert_eq!(expected, actual);
    });
}

#[test]
fn add_validators() {
    new_test_ext().execute_with(|| {
        <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(0);
        let account_id_1 = 333;
        let account_id_2 = 444;
        let refs_before_1 = frame_system::Pallet::<Test>::providers(&account_id_1);
        let refs_before_2 = frame_system::Pallet::<Test>::providers(&account_id_2);
        register_validator(account_id_1);
        register_validator(account_id_2);
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), account_id_1)
            .unwrap();
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), account_id_2)
            .unwrap();
        let actual = <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(1)
            .map(|x| sorted(x));
        let expected = Some(vec![111, 222, account_id_1, account_id_2]);
        assert_eq!(expected, actual);
        // refs counter incremented twice: in set_keys and in add_validator
        // we need to call purge_keys before deleting validator account
        assert!(frame_system::Pallet::<Test>::providers(&account_id_1) == refs_before_1 + 2);
        assert!(frame_system::Pallet::<Test>::providers(&account_id_2) == refs_before_2 + 2);
    });
}

#[test]
fn remove_validators() {
    new_test_ext().execute_with(|| {
        <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(0);
        let acc_to_remove = 111;
        ModuleSessionManager::remove_validator(frame_system::RawOrigin::Root.into(), acc_to_remove)
            .unwrap();
        let actual = <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(1)
            .map(|x| sorted(x));
        let expected = Some(vec![222]);
        assert_eq!(expected, actual);
        // check refcounter
        <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(2);
        let new_added = 333;
        register_validator(new_added);
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), new_added)
            .unwrap();
        let refs_before = frame_system::Pallet::<Test>::providers(&new_added);
        <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(3);
        ModuleSessionManager::remove_validator(frame_system::RawOrigin::Root.into(), new_added)
            .unwrap();
        let actual = <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(4)
            .map(|x| sorted(x));
        let expected = Some(vec![222]);
        assert_eq!(expected, actual);
        assert!(frame_system::Pallet::<Test>::providers(&new_added) == refs_before - 1);
    });
}

#[test]
fn validators_stay_unchanged() {
    new_test_ext().execute_with(|| {
        <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(0);
        let actual = <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(1)
            .map(|x| sorted(x));
        let expected = None;
        assert_eq!(expected, actual);
    });
}

#[test]
fn several_sessions() {
    new_test_ext().execute_with(|| {
        <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(0);

        register_validator(333);
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 333).unwrap();
        let actual = <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(1)
            .map(|x| sorted(x));
        let expected = Some(vec![111, 222, 333]);
        assert_eq!(expected, actual);

        let actual = <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(2)
            .map(|x| sorted(x));
        let expected = None;
        assert_eq!(expected, actual);

        ModuleSessionManager::remove_validator(frame_system::RawOrigin::Root.into(), 111).unwrap();
        let actual = <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(3)
            .map(|x| sorted(x));
        let expected = Some(vec![222, 333]);
        assert_eq!(expected, actual);

        let actual = <ModuleSessionManager as pallet_session::SessionManager<u64>>::new_session(4)
            .map(|x| sorted(x));
        let expected = None;
        assert_eq!(expected, actual);
    });
}

#[test]
fn first_session_validators_from_genesis() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);

        let actual: Vec<u64> = sorted(validators());
        let expected: Vec<u64> = mock::initial_validators();
        assert_eq!(expected, actual);
    });
}

#[test]
fn second_session_validators_from_config() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);

        force_new_session();
        initialize_block(2);

        let actual: Vec<u64> = sorted(validators());
        let expected: Vec<u64> = mock::initial_validators();
        assert_eq!(expected, actual);
    });
}

#[test]
fn session_no_validator_changes() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);

        force_new_session();
        initialize_block(2);

        force_new_session();
        initialize_block(3);

        let actual: Vec<u64> = sorted(validators());
        let expected: Vec<u64> = mock::initial_validators();
        assert_eq!(expected, actual);
    });
}

#[test]
fn session_validators_added() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);

        register_validator(333);
        register_validator(444);
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 333).unwrap();
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 444).unwrap();

        force_new_session();
        initialize_block(2);

        let actual: Vec<u64> = sorted(validators());
        let expected: Vec<u64> = mock::initial_validators();
        assert_eq!(expected, actual);

        force_new_session();
        initialize_block(3);

        let actual: Vec<u64> = sorted(validators());
        let expected: Vec<u64> = vec![111, 222, 333, 444];
        assert_eq!(expected, actual);
    });
}

#[test]
fn session_validator_removed() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);

        ModuleSessionManager::remove_validator(frame_system::RawOrigin::Root.into(), 111).unwrap();

        force_new_session();
        initialize_block(2);

        let actual: Vec<u64> = sorted(validators());
        let expected: Vec<u64> = mock::initial_validators();
        assert_eq!(expected, actual);

        force_new_session();
        initialize_block(3);

        let actual: Vec<u64> = sorted(validators());
        let expected: Vec<u64> = vec![222];
        assert_eq!(expected, actual);
    });
}

#[test]
fn session_several_sessions() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);
        register_validator(333);
        register_validator(444);
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 333).unwrap();
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 444).unwrap();

        force_new_session();
        initialize_block(2);
        let actual: Vec<u64> = sorted(validators());
        let expected: Vec<u64> = mock::initial_validators();
        assert_eq!(expected, actual);

        force_new_session();
        initialize_block(3);
        let a: Vec<u64> = sorted(validators());
        assert_eq!(vec![111, 222, 333, 444], a);

        force_new_session();
        initialize_block(4);
        let a: Vec<u64> = sorted(validators());
        assert_eq!(vec![111, 222, 333, 444], a);
        ModuleSessionManager::remove_validator(frame_system::RawOrigin::Root.into(), 333).unwrap();

        force_new_session();
        initialize_block(5);
        let a: Vec<u64> = sorted(validators());
        assert_eq!(vec![111, 222, 333, 444], a);
        register_validator(555);
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 555).unwrap();
        ModuleSessionManager::remove_validator(frame_system::RawOrigin::Root.into(), 222).unwrap();

        force_new_session();
        initialize_block(6);
        let a: Vec<u64> = sorted(validators());
        assert_eq!(vec![111, 222, 444], a);

        force_new_session();
        initialize_block(6);
        let a: Vec<u64> = sorted(validators());
        assert_eq!(vec![111, 444, 555], a);

        force_new_session();
        initialize_block(7);
        let a: Vec<u64> = sorted(validators());
        assert_eq!(vec![111, 444, 555], a);
    });
}

#[test]
fn session_existing_validator_added() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);

        register_validator(333);
        register_validator(444);
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 333).unwrap();
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 444).unwrap();
        let actual = ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 333);
        let expected = ErrorSessionManager::AlreadyAdded;
        assert_err!(actual, expected);
    });
}

#[test]
fn session_nonexistent_validator_removed() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);

        let actual =
            ModuleSessionManager::remove_validator(frame_system::RawOrigin::Root.into(), 999);
        let expected = ErrorSessionManager::AlreadyRemoved;
        assert_err!(actual, expected);
    });
}

#[test]
fn session_change_flag() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);

        force_new_session();
        initialize_block(2);

        force_new_session();
        initialize_block(3);
        register_validator(333);
        register_validator(444);
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 333).unwrap();
        ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 444).unwrap();
        assert_eq!(false, session_changed());

        force_new_session();
        initialize_block(4);
        assert_eq!(false, session_changed());

        force_new_session();
        initialize_block(5);
        assert_eq!(true, session_changed());

        force_new_session();
        initialize_block(6);
        assert_eq!(false, session_changed());
    });
}

#[test]
fn session_unregistered_validator_added() {
    new_test_ext().execute_with(|| {
        force_new_session();
        initialize_block(1);

        let actual = ModuleSessionManager::add_validator(frame_system::RawOrigin::Root.into(), 333);
        let expected = ErrorSessionManager::NotRegistered;
        assert_err!(actual, expected);
    });
}
