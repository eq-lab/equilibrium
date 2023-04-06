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

use crate::{mock::*, Error};
use codec::Decode;
use eq_primitives::{
    asset::EQ as Eq,
    balance::{BalanceGetter, EqCurrency},
    SignedBalance,
};
use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::{testing::UintAuthorityId, DispatchError};

const MILLISECS_PER_SEC: u64 = 1000;

#[test]
fn lock_in_out_of_period_err() {
    new_test_ext().execute_with(|| {
        let acc_id = 1;
        let initial_balance = 100;

        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id,
            Eq,
            initial_balance,
            true,
            None
        ));

        let now = 2;
        let lock_start = now + 1;
        ModuleTimestamp::set_timestamp(now * MILLISECS_PER_SEC);

        assert_ok!(ModuleLockdrop::do_set_lock_start(lock_start));

        // Before StartLock
        let lock_1 = 42;
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id), lock_1));

        assert_eq!(
            ModuleBalances::get_balance(&acc_id, &Eq),
            SignedBalance::<u128>::Positive(initial_balance - lock_1)
        );

        assert_eq!(
            ModuleBalances::get_balance(&ModuleLockdrop::get_account_id(), &Eq),
            SignedBalance::<u128>::Positive(lock_1)
        );

        assert_eq!(ModuleLockdrop::locks(acc_id), lock_1);

        // move in period
        let now = lock_start + LockPeriod::get() - 1;
        ModuleTimestamp::set_timestamp(now * MILLISECS_PER_SEC);

        assert_err!(
            EqLockdrop::lock(Origin::signed(acc_id), 42),
            Error::<Test>::OutOfLockPeriod
        );

        // move time
        let now = lock_start + LockPeriod::get() + 1;
        ModuleTimestamp::set_timestamp(now * MILLISECS_PER_SEC);

        // After EndLock
        let lock_2 = 42;
        assert_noop!(
            EqLockdrop::lock(Origin::signed(acc_id), lock_2),
            Error::<Test>::OutOfLockPeriod
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id, &Eq),
            SignedBalance::<u128>::Positive(initial_balance - lock_1)
        );

        assert_eq!(
            ModuleBalances::get_balance(&ModuleLockdrop::get_account_id(), &Eq),
            SignedBalance::<u128>::Positive(lock_1)
        );

        assert_eq!(ModuleLockdrop::locks(acc_id), lock_1);
    });
}

#[test]
fn lock_in_period_ok() {
    new_test_ext().execute_with(|| {
        let acc_id_1 = 1;
        let acc_id_2 = 2;
        let acc_id_3 = 3;
        let initial_balance = 50;

        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id_1,
            Eq,
            initial_balance,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id_2,
            Eq,
            initial_balance,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id_3,
            Eq,
            initial_balance,
            true,
            None
        ));

        let lock_1 = 1;

        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id_1), lock_1));

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_1, &Eq),
            SignedBalance::<u128>::Positive(initial_balance - lock_1)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_2, &Eq),
            SignedBalance::<u128>::Positive(initial_balance)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_3, &Eq),
            SignedBalance::<u128>::Positive(initial_balance)
        );

        assert_eq!(
            ModuleBalances::get_balance(&ModuleLockdrop::get_account_id(), &Eq),
            SignedBalance::<u128>::Positive(lock_1)
        );

        assert_eq!(ModuleLockdrop::locks(acc_id_1), lock_1);

        assert_eq!(ModuleLockdrop::locks(acc_id_2), 0);

        assert_eq!(ModuleLockdrop::locks(acc_id_3), 0);

        let now = 2;
        let lock_start = now + 1;
        ModuleTimestamp::set_timestamp(now * MILLISECS_PER_SEC);

        assert_ok!(ModuleLockdrop::do_set_lock_start(lock_start));

        let lock_2 = 2;
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id_1), lock_2));
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id_2), lock_2));

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_1, &Eq),
            SignedBalance::<u128>::Positive(initial_balance - lock_1 - lock_2)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_2, &Eq),
            SignedBalance::<u128>::Positive(initial_balance - lock_2)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_3, &Eq),
            SignedBalance::<u128>::Positive(initial_balance)
        );

        assert_eq!(
            ModuleBalances::get_balance(&ModuleLockdrop::get_account_id(), &Eq),
            SignedBalance::<u128>::Positive(lock_1 + lock_2 + lock_2)
        );

        assert_eq!(ModuleLockdrop::locks(acc_id_1), lock_1 + lock_2);

        assert_eq!(ModuleLockdrop::locks(acc_id_2), lock_2);

        assert_eq!(ModuleLockdrop::locks(acc_id_3), 0);
    });
}

#[test]
fn unlock_in_period_err() {
    new_test_ext().execute_with(|| {
        let acc_id_1 = 1;
        let initial_balance = 50;

        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id_1,
            Eq,
            initial_balance,
            true,
            None
        ));

        assert_noop!(
            ModuleLockdrop::unlock_external(Origin::signed(acc_id_1)),
            Error::<Test>::LockPeriodNotOver
        );

        // move time 1
        let now = 2;
        let lock_start = now;
        ModuleTimestamp::set_timestamp(now * MILLISECS_PER_SEC);
        assert_ok!(ModuleLockdrop::do_set_lock_start(lock_start));

        let lock_1 = 1;
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id_1), lock_1));

        assert_noop!(
            ModuleLockdrop::unlock_external(Origin::signed(acc_id_1)),
            Error::<Test>::LockPeriodNotOver
        );

        // move time 2
        let now = lock_start + 1;
        ModuleTimestamp::set_timestamp(now * MILLISECS_PER_SEC);

        assert_noop!(
            ModuleLockdrop::unlock_external(Origin::signed(acc_id_1)),
            Error::<Test>::LockPeriodNotOver
        );

        // assert balances
        assert_eq!(
            ModuleBalances::get_balance(&acc_id_1, &Eq),
            SignedBalance::<u128>::Positive(initial_balance - lock_1)
        );

        assert_eq!(
            ModuleBalances::get_balance(&ModuleLockdrop::get_account_id(), &Eq),
            SignedBalance::<u128>::Positive(lock_1)
        );

        assert_eq!(ModuleLockdrop::locks(acc_id_1), lock_1);
    });
}

#[test]
fn unlock_ok() {
    new_test_ext().execute_with(|| {
        let acc_id_1 = 1;
        let acc_id_2 = 2;
        let acc_id_3 = 3;
        let initial_balance = 50;

        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id_1,
            Eq,
            initial_balance,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id_2,
            Eq,
            initial_balance,
            true,
            None
        ));

        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id_3,
            Eq,
            initial_balance,
            true,
            None
        ));

        assert_noop!(
            ModuleLockdrop::unlock_external(Origin::signed(acc_id_1)),
            Error::<Test>::LockPeriodNotOver
        );

        // move time 1
        let now = 2;
        let lock_start = now;
        ModuleTimestamp::set_timestamp(now * MILLISECS_PER_SEC);
        assert_ok!(ModuleLockdrop::do_set_lock_start(lock_start));

        let lock_1 = 1;
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id_1), lock_1));
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id_2), lock_1));
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id_3), lock_1));

        // move time 4
        let now = lock_start + LockPeriod::get() + 1;
        ModuleTimestamp::set_timestamp(now * MILLISECS_PER_SEC);

        // unlock acc_id_1
        assert_ok!(ModuleLockdrop::unlock_external(Origin::signed(acc_id_1)));

        // assert balances
        assert_eq!(
            ModuleBalances::get_balance(&acc_id_1, &Eq),
            SignedBalance::<u128>::Positive(initial_balance)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_2, &Eq),
            SignedBalance::<u128>::Positive(initial_balance - lock_1)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_3, &Eq),
            SignedBalance::<u128>::Positive(initial_balance - lock_1)
        );

        assert_eq!(
            ModuleBalances::get_balance(&ModuleLockdrop::get_account_id(), &Eq),
            SignedBalance::<u128>::Positive((lock_1) * 2)
        );

        assert_eq!(ModuleLockdrop::locks(acc_id_1), 0);
        assert_eq!(ModuleLockdrop::locks(acc_id_2), lock_1);
        assert_eq!(ModuleLockdrop::locks(acc_id_3), lock_1);

        // unlock acc_id_2
        assert_ok!(ModuleLockdrop::unlock_external(Origin::signed(acc_id_2)));

        // assert balances
        assert_eq!(
            ModuleBalances::get_balance(&acc_id_1, &Eq),
            SignedBalance::<u128>::Positive(initial_balance)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_2, &Eq),
            SignedBalance::<u128>::Positive(initial_balance)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_3, &Eq),
            SignedBalance::<u128>::Positive(initial_balance - lock_1)
        );

        assert_eq!(
            ModuleBalances::get_balance(&ModuleLockdrop::get_account_id(), &Eq),
            SignedBalance::<u128>::Positive(lock_1)
        );

        assert_eq!(ModuleLockdrop::locks(acc_id_1), 0);
        assert_eq!(ModuleLockdrop::locks(acc_id_2), 0);
        assert_eq!(ModuleLockdrop::locks(acc_id_3), lock_1);

        // unlock acc_id_3
        assert_ok!(ModuleLockdrop::unlock_external(Origin::signed(acc_id_3)));

        // assert balances
        assert_eq!(
            ModuleBalances::get_balance(&acc_id_1, &Eq),
            SignedBalance::<u128>::Positive(initial_balance)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_2, &Eq),
            SignedBalance::<u128>::Positive(initial_balance)
        );

        assert_eq!(
            ModuleBalances::get_balance(&acc_id_3, &Eq),
            SignedBalance::<u128>::Positive(initial_balance)
        );

        assert_eq!(
            ModuleBalances::get_balance(&ModuleLockdrop::get_account_id(), &Eq),
            SignedBalance::<u128>::Positive(0)
        );

        assert_eq!(ModuleLockdrop::locks(acc_id_1), 0);
        assert_eq!(ModuleLockdrop::locks(acc_id_2), 0);
        assert_eq!(ModuleLockdrop::locks(acc_id_3), 0);
    });
}

#[test]
fn block_multiple_transfers_with_vesting() {
    new_test_ext().execute_with(|| {
        let block = 1;

        let lock_start = 1;
        assert_ok!(ModuleLockdrop::set_lock_start(
            RawOrigin::Root.into(),
            lock_start
        ));
        ModuleTimestamp::set_timestamp(lock_start * MILLISECS_PER_SEC + 1);
        let lock = 3;
        let acc_id = 1;

        let initial_balance = 50;

        assert_ok!(ModuleBalances::deposit_creating(
            &acc_id,
            Eq,
            initial_balance,
            true,
            None
        ));

        assert_ok!(ModuleVesting::force_vested_transfer(
            Origin::root(),
            acc_id,
            acc_id,
            eq_vesting::VestingInfo {
                starting_block: block,
                locked: 12,
                per_block: 1,
            }
        ));

        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id), lock));
        // Second transfer with vesting!
        assert_err!(
            EqLockdrop::lock(Origin::signed(acc_id), lock),
            Error::<Test>::MultipleTransferWithVesting
        );
    });
}

#[test]
fn set_clear_lock_start() {
    new_test_ext().execute_with(|| {
        let lock_start_1 = 1;
        let lock_start_2 = 2;
        // no root
        assert_noop!(
            ModuleLockdrop::set_lock_start(Origin::signed(1), lock_start_1),
            DispatchError::BadOrigin
        );

        // set
        assert_ok!(ModuleLockdrop::set_lock_start(
            RawOrigin::Root.into(),
            lock_start_1
        ));
        // check storage
        assert_eq!(ModuleLockdrop::lock_start(), Some(lock_start_1));

        // set non empty
        assert_noop!(
            ModuleLockdrop::set_lock_start(RawOrigin::Root.into(), lock_start_2),
            Error::<Test>::LockStartNotEmpty
        );

        // clear no root
        assert_noop!(
            ModuleLockdrop::clear_lock_start(Origin::signed(1)),
            DispatchError::BadOrigin
        );

        // clear
        assert_ok!(ModuleLockdrop::clear_lock_start(RawOrigin::Root.into()));
        // check storage
        assert_eq!(ModuleLockdrop::lock_start(), None);

        // clear 2
        assert_ok!(ModuleLockdrop::clear_lock_start(RawOrigin::Root.into()));
        assert_eq!(ModuleLockdrop::lock_start(), None);

        // set
        assert_ok!(ModuleLockdrop::set_lock_start(
            RawOrigin::Root.into(),
            lock_start_2
        ));
        // check storage
        assert_eq!(ModuleLockdrop::lock_start(), Some(lock_start_2));
    });
}

use frame_support::traits::OffchainWorker;
use sp_core::offchain::testing::{TestOffchainExt, TestTransactionPoolExt};
use sp_core::offchain::{OffchainDbExt, OffchainWorkerExt, TransactionPoolExt};

#[test]
fn unlock_from_offchain_worker() {
    let origin = frame_system::RawOrigin::Root;

    let mut ext = new_test_ext();
    let (offchain, _) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        UintAuthorityId::set_all_keys(vec![11, 21, 31, 41, 51]);
        let block = 1;

        let lock_start = 1;
        assert_ok!(ModuleLockdrop::set_lock_start(
            RawOrigin::Root.into(),
            lock_start
        ));
        ModuleTimestamp::set_timestamp(lock_start * MILLISECS_PER_SEC + 1);
        let lock = 3;
        let acc_id = 1;
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id), lock));
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id), lock));
        assert_ok!(EqLockdrop::lock(Origin::signed(acc_id), lock));

        let now = lock_start + LockPeriod::get() + 1;
        ModuleTimestamp::set_timestamp(now * MILLISECS_PER_SEC);

        ModuleSystem::set_block_number(block);
        ModuleLockdrop::offchain_worker(block);
        assert_eq!(state.read().transactions.len(), 1);
        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let req = match ex.call {
            crate::mock::Call::EqLockdrop(crate::Call::unlock {
                request: r,
                signature: _,
            }) => r,
            e => panic!("Unexpected call: {:?}", e),
        };
        assert_eq!(req.authority_index, 0);
        assert_eq!(req.validators_len, 5);
        assert_eq!(req.block_num, 1);

        // test disabled worker
        let block = 0;
        ModuleSystem::set_block_number(block);
        let r = ModuleLockdrop::set_auto_unlock(origin.clone().into(), false);
        assert!(r.is_ok());
        ModuleLockdrop::offchain_worker(block);
        assert_eq!(state.read().transactions.len(), 0);

        // enable the worker again
        let r = ModuleLockdrop::set_auto_unlock(origin.into(), true);
        assert!(r.is_ok());
        let block = 1;
        ModuleSystem::set_block_number(block);
        ModuleLockdrop::offchain_worker(block);
        assert_eq!(state.read().transactions.len(), 1);
        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let req = match ex.call {
            crate::mock::Call::EqLockdrop(crate::Call::unlock {
                request: r,
                signature: _,
            }) => r,
            e => panic!("Unexpected call: {:?}", e),
        };
        assert_eq!(req.authority_index, 0);
        assert_eq!(req.validators_len, 5);
        assert_eq!(req.block_num, 1);
    });
}
