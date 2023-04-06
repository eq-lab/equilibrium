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

use std::collections::HashMap;

use super::*;
use crate::mock::*;

use eq_primitives::asset;
use eq_primitives::balance::EqCurrency;

use eq_utils::ONE_TOKEN;
use frame_support::dispatch::DispatchError::BadOrigin;
use frame_support::traits::OffchainWorker;
use frame_support::{assert_noop, assert_ok};
use sp_core::offchain::{
    testing::{TestOffchainExt, TestTransactionPoolExt},
    OffchainDbExt, OffchainWorkerExt, TransactionPoolExt,
};
use sp_runtime::testing::UintAuthorityId;

#[test]
fn reinit_on_debt() {
    let mut ext = new_test_ext();
    let (offchain, _) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        ModuleSystem::set_block_number(0);
        ModuleTimestamp::set_timestamp(2000);
        ModuleRate::set_last_update(&1);

        // given
        let block = 1;
        ModuleSystem::set_block_number(block);
        ModuleTimestamp::set_timestamp(24 * 60 * 60 * 1_000); // 1 day

        UintAuthorityId::set_all_keys(vec![11]);

        ModuleBalances::make_free_balance_be(
            &1,
            eq_primitives::asset::BTC,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &1,
            eq_primitives::asset::EQD,
            SignedBalance::<Balance>::Negative(100000 * ONE_TOKEN),
        );

        ModuleRate::offchain_worker(block);

        assert_eq!(state.read().transactions.len(), 1);

        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let reinit = match ex.call {
            crate::mock::Call::EqRate(crate::Call::reinit { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(reinit.account, Some(1));
        assert_eq!(reinit.authority_index, 0);
        assert_eq!(reinit.validators_len, 5);
        assert_eq!(reinit.block_num, 1);
    });
}

#[test]
fn reinit_on_debt_wrong_client() {
    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        use codec::Decode;

        ModuleSystem::set_block_number(0);
        ModuleTimestamp::set_timestamp(2000);
        vec![0, 1, 2, 3, 4, 5]
            .iter()
            .for_each(ModuleRate::set_last_update);

        // given
        let block = 1;
        ModuleSystem::set_block_number(block);
        ModuleTimestamp::set_timestamp(24 * 60 * 60 * 1_000);

        UintAuthorityId::set_all_keys(vec![11, 21, 101, 41, 51]);

        for i in 0..5 {
            ModuleBalances::make_free_balance_be(
                &i,
                asset::BTC,
                SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
            );
            ModuleBalances::make_free_balance_be(
                &i,
                asset::EQD,
                SignedBalance::<Balance>::Negative(100000 * ONE_TOKEN),
            );
            println!(
                "{:?} {:?} {:?}",
                i,
                ModuleBalances::get_balance(&i, &asset::BTC),
                ModuleBalances::get_balance(&i, &asset::EQD)
            );
        }

        ModuleRate::offchain_worker(block);

        let exts = state
            .write()
            .transactions
            .drain(..)
            .map(|enc_ext| <Extrinsic as Decode>::decode(&mut &enc_ext[..]).ok())
            .collect::<Option<Vec<_>>>()
            .unwrap();

        //  val_id  |  [acc_id]
        //  11      |  [0, 1]
        //  21      |  [0, 1, 2]
        //  41      |  [2, 3, 4]
        //  51      |  [3, 4]
        assert_eq!(exts.len(), 10);

        for ext in exts {
            let reinit = match ext.call {
                crate::mock::Call::EqRate(crate::Call::reinit { request, .. }) => request,
                e => panic!("Unexpected call: {:?}", e),
            };
            if reinit.higher_priority {
                assert_ne!(reinit.account, Some(2));
                assert_eq!(reinit.account, Some(reinit.authority_index as u64));
            }
            assert_eq!(reinit.validators_len, 5);
            assert_eq!(reinit.block_num, 1);
        }

        ModuleBalances::make_free_balance_be(
            &6,
            asset::BTC,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &6,
            asset::EQD,
            SignedBalance::<Balance>::Negative(1000000 * ONE_TOKEN),
        );

        ModuleRate::offchain_worker(block);

        let exts = state
            .write()
            .transactions
            .drain(..)
            .map(|enc_ext| <Extrinsic as Decode>::decode(&mut &enc_ext[..]).ok())
            .collect::<Option<Vec<_>>>()
            .unwrap();

        //  val_id  |  [acc_id]
        //  11      |  [0, 1, 6]
        //  21      |  [0, 1, 2, 6]
        //  41      |  [2, 3, 4]
        //  51      |  [3, 4]
        assert_eq!(exts.len(), 12);

        let mut to_validate = HashMap::new();
        to_validate.extend([
            (0, false),
            (1, false),
            (2, false),
            (3, false),
            (4, false),
            (6, false),
        ]);

        for ext in exts {
            let reinit = match ext.call {
                crate::mock::Call::EqRate(crate::Call::reinit { request, .. }) => request,
                e => panic!("Unexpected call: {:?}", e),
            };

            *to_validate.get_mut(&reinit.account.unwrap()).unwrap() = true;

            if reinit.higher_priority {
                assert_ne!(reinit.account, Some(2));
                assert_eq!(reinit.account.unwrap() % 6, reinit.authority_index as u64);
            }
            assert_eq!(reinit.validators_len, 5);
            assert_eq!(reinit.block_num, 1);
        }

        // All accounts are reinited
        assert!(to_validate.values().all(|is| *is));
    });
}

#[test]
fn reinit_on_margincall() {
    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));
    set_fee(0);

    ext.execute_with(|| {
        let block = 0;
        ModuleSystem::set_block_number(block);
        ModuleTimestamp::set_timestamp(2000);
        vec![0, 1, 2, 3, 4, 5]
            .iter()
            .for_each(ModuleRate::set_last_update);

        UintAuthorityId::set_all_keys(vec![11, 21, 31, 41, 51]);
        for i in 0..5 {
            ModuleBalances::make_free_balance_be(
                &i,
                asset::BTC,
                SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
            );
            ModuleBalances::make_free_balance_be(
                &i,
                asset::EQD,
                SignedBalance::<Balance>::Negative(941380 * ONE_TOKEN),
            );
        }

        ModuleRate::offchain_worker(block);

        assert_eq!(state.read().transactions.len(), 0);

        for i in 0..5 {
            ModuleBalances::make_free_balance_be(
                &i,
                asset::BTC,
                SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
            );
            ModuleBalances::make_free_balance_be(
                &i,
                asset::EQD,
                SignedBalance::<Balance>::Negative(975380 * ONE_TOKEN),
            );
        }

        ModuleRate::offchain_worker(block);

        let exts = state
            .write()
            .transactions
            .drain(..)
            .map(|enc_ext| <Extrinsic as Decode>::decode(&mut &enc_ext[..]).ok())
            .collect::<Option<Vec<_>>>()
            .unwrap();
        //  val_id  |  [acc_id]
        //  11      |  [0, 1]
        //  21      |  [0, 1, 2]
        //  31      |  [1, 2, 3]
        //  41      |  [2, 3, 4]
        //  51      |  [3, 4]
        assert_eq!(exts.len(), 13);

        for ext in exts {
            let reinit = match ext.call {
                crate::mock::Call::EqRate(crate::Call::reinit { request, .. }) => request,
                e => panic!("Unexpected call: {:?}", e),
            };

            if reinit.higher_priority {
                assert_eq!(reinit.account, Some(reinit.authority_index as u64));
            }
            assert_eq!(reinit.validators_len, 5);
            assert_eq!(reinit.block_num, 0);
        }
    });
}

#[test]
#[allow(unused_must_use)]
fn reinit_sufficient_eq_no_buyout() {
    new_test_ext().execute_with(|| {
        let acc_id = 1;
        let request = OperationRequest::<AccountId, u64> {
            account: Some(acc_id),
            authority_index: 0,
            validators_len: 0,
            block_num: 0,
            higher_priority: false,
        };

        let id: UintAuthorityId = UintAuthorityId::from(acc_id);
        let signature = id.sign(&request.encode()).unwrap();

        let initial_eq_balance = 20_000 * ONE_TOKEN;

        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::EQ,
            SignedBalance::<Balance>::Positive(initial_eq_balance),
        );
        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::BTC,
            SignedBalance::<Balance>::Negative(1 * ONE_TOKEN),
        );

        clear_eq_buyout_args();

        ModuleTimestamp::set_timestamp(24 * 60 * 60 * 1_000); // 1 day
        ModuleRate::reinit(system::RawOrigin::None.into(), request, signature);

        let expected_fee = 1232032852u128;
        assert_eq!(get_eq_buyout_args(), None);
        assert_eq!(
            ModuleBalances::get_balance(&acc_id, &asset::EQ),
            SignedBalance::<Balance>::Positive(initial_eq_balance - expected_fee)
        );
    });
}

#[test]
#[allow(unused_must_use)]
fn reinit_less_than_debt_eq_partial_buyout() {
    new_test_ext().execute_with(|| {
        let acc_id = 1;
        let request = OperationRequest::<AccountId, u64> {
            account: Some(acc_id),
            authority_index: 0,
            validators_len: 0,
            block_num: 0,
            higher_priority: false,
        };

        let id: UintAuthorityId = UintAuthorityId::from(acc_id);
        let signature = id.sign(&request.encode()).unwrap();

        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::EQ,
            SignedBalance::<Balance>::Positive(ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::BTC,
            SignedBalance::<Balance>::Positive(20 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::EQD,
            SignedBalance::<Balance>::Negative(100_000 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::ETH,
            SignedBalance::<Balance>::Negative(100 * ONE_TOKEN),
        );

        clear_eq_buyout_args();

        ModuleTimestamp::set_timestamp(36525 * 24 * 60 * 60 * 1_000 / 100); // 1 year

        /* Expected values
        r_prime = 2% for unit test
        Coeff = 100_000 EQD + 25_000 EQD (ETH weight in EQD) = 125_000
        treasury_fee = 125_000 * 1% = 1250
        bailsman_fee = 1250 + 1750 + 600 = 3600
            base = 125_000 * 1% = 1250
            insurance = 125_000 * (1 - 0.3) * 2% = 1750
            synthetic = 125_000 * 100_000/125_000 * 2% * 0.3 = 600

        lender_fee = 125_000 * 25_000/125_000  * (0.5% + 0.3 * 2%) = 275
        total_fee = 1250 + 3600 + 275 = 5125 EQ

        eq_bayout = 1_EQ - 5125_EQ = -5124_EQ
        */

        ModuleRate::reinit(system::RawOrigin::None.into(), request, signature);

        assert_eq!(get_eq_buyout_args(), Some((acc_id, 5124 * ONE_TOKEN)));
    });
}

#[test]
#[allow(unused_must_use)]
fn reinit_zero_eq_full_buyout() {
    new_test_ext().execute_with(|| {
        let acc_id = 1;
        let request = OperationRequest::<AccountId, u64> {
            account: Some(acc_id),
            authority_index: 0,
            validators_len: 0,
            block_num: 0,
            higher_priority: false,
        };

        let id: UintAuthorityId = UintAuthorityId::from(acc_id);
        let signature = id.sign(&request.encode()).unwrap();

        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::BTC,
            SignedBalance::<Balance>::Positive(10 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::ETH,
            SignedBalance::<Balance>::Negative(50 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &acc_id,
            asset::EQD,
            SignedBalance::<Balance>::Negative(5_000 * ONE_TOKEN),
        );

        clear_eq_buyout_args();

        /* Expected buouyt value
            r_prime = 2% for unit test
            debt_usd = 50 * 250 + 5000 = 17500
            coeff = 17_500
            treasury_fee = 17_500 * 1% = 175
            bailsman_fee = 175 + 245 + 30 = 450
                base = 17_500 * 1% = 175
                insurance = 17_500 * (1 - 0.3) * 2% = 245
                synthetic = 17_500 * 5_000/17_500 * 1 * 0.02* 0.3 = 30
            lender_fee = 17_500 * 12_500/17_500 (0.005 + 0.3 * 0.02) = 137,5
            -------------------------------------------------------
            total_fee = 175 + 450 + 137,5 = 762,5

            eq_bayout = 0_EQ - 817.5_EQ = -762,5_EQ
        */

        ModuleTimestamp::set_timestamp(36525 * 24 * 60 * 60 * 1_000 / 100); // 1 year
        ModuleRate::reinit(system::RawOrigin::None.into(), request, signature);

        assert_eq!(get_eq_buyout_args(), Some((acc_id, 762499999998))); //~762.5 EQ
    });
}

#[test]
fn acc_delete_offchain() {
    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        ModuleBalances::make_free_balance_be(
            &987,
            asset::EQD, // can_be_deleted mock returns true, mock fee is 0
            SignedBalance::<Balance>::Positive(150_000),
        );

        ModuleSystem::set_block_number(1);
        ModuleTimestamp::set_timestamp(6000);
        ModuleRate::set_last_update(&987);

        UintAuthorityId::set_all_keys(vec![11]);

        ModuleRate::offchain_worker(1);

        assert_eq!(state.read().transactions.len(), 1);
        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let del_request = match ex.call {
            crate::mock::Call::EqRate(crate::Call::delete_account { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(del_request.account, Some(987));
        assert_eq!(del_request.authority_index, 0);
        assert_eq!(del_request.validators_len, 5);
        assert_eq!(del_request.block_num, 1);
    });
}

#[test]
fn acc_delete_offchain_no_trx() {
    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        ModuleBalances::make_free_balance_be(
            &321,
            asset::EQD, // can_be_deleted mock returns false, mock fee is 0
            SignedBalance::<Balance>::Positive(150_000),
        );

        ModuleSystem::set_block_number(1);
        ModuleTimestamp::set_timestamp(6000);
        ModuleRate::set_last_update(&321);

        UintAuthorityId::set_all_keys(vec![11]);

        ModuleRate::offchain_worker(1);

        assert_eq!(state.read().transactions.len(), 0);
    });
}

#[test]
fn offchain_worker_turn_off() {
    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        // Copy of 'reinit on debt' test init
        ModuleSystem::set_block_number(0);
        ModuleTimestamp::set_timestamp(2000);
        ModuleRate::set_last_update(&1);

        // Turning offchain worker off
        assert_ok!(ModuleRate::set_auto_reinit_enabled(
            system::RawOrigin::Root.into(),
            false
        ));
        assert_noop!(
            ModuleRate::set_auto_reinit_enabled(Origin::signed(99), false),
            BadOrigin
        );
        assert_eq!(
            ModuleRate::auto_reinit_enabled(),
            false,
            "Offchain worker did not change status after status set to false"
        );

        let block = 1;
        ModuleSystem::set_block_number(block);
        ModuleTimestamp::set_timestamp(5 * 60 * 60 * 1_000); // 5 hour

        UintAuthorityId::set_all_keys(vec![11]);

        ModuleBalances::make_free_balance_be(
            &1,
            asset::BTC,
            SignedBalance::<Balance>::Positive(100 * ONE_TOKEN),
        );
        ModuleBalances::make_free_balance_be(
            &1,
            asset::EQD,
            SignedBalance::<Balance>::Negative(100_000 * ONE_TOKEN),
        );

        // Asserting offchain worker did not work
        ModuleRate::offchain_worker(block);
        assert_eq!(state.read().transactions.len(), 0);

        // Turning offchain worker back on

        assert_ok!(ModuleRate::set_auto_reinit_enabled(
            system::RawOrigin::Root.into(),
            true
        ));

        // Asserting offchain worker is now working ok
        ModuleRate::offchain_worker(block);
        assert_eq!(state.read().transactions.len(), 1);

        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let reinit = match ex.call {
            crate::mock::Call::EqRate(crate::Call::reinit { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(reinit.account, Some(1));
        assert_eq!(reinit.authority_index, 0);
        assert_eq!(reinit.validators_len, 5);
        assert_eq!(reinit.block_num, 1);
    });
}

#[test]
fn reinit_on_bailsman_do_distribution_first() {
    new_test_ext().execute_with(|| {
        #[allow(unused_imports)]
        use frame_support::traits::OnInitialize;

        let borrower = 0;
        let bailsman = 1234;

        for acc in [borrower, bailsman] {
            ModuleBalances::make_free_balance_be(
                &acc,
                eq_primitives::asset::BTC,
                SignedBalance::<Balance>::Positive(ONE_TOKEN),
            );
        }

        assert_ok!(ModuleBailsman::register_bailsman(&bailsman));

        for acc in [borrower, bailsman] {
            ModuleBalances::make_free_balance_be(
                &acc,
                eq_primitives::asset::EQ,
                SignedBalance::<Balance>::Negative(ONE_TOKEN * 1000),
            );
        }

        ModuleBalances::make_free_balance_be(
            &BailsmanModuleId::get().into_account_truncating(),
            eq_primitives::asset::BTC,
            SignedBalance::<Balance>::Positive(ONE_TOKEN),
        );

        ModuleBailsman::on_initialize(1);

        let queue = eq_bailsman::DistributionQueue::<Test>::get();
        let distribution_before = queue.1.get(&1).expect("Non empty queue");

        assert_eq!(distribution_before.remaining_bailsmen, 1);

        assert_ok!(ModuleRate::reinit_external(Origin::signed(1), borrower));

        assert_eq!(
            eq_bailsman::DistributionQueue::<Test>::get()
                .1
                .get(&1)
                .expect("Non empty queue"),
            distribution_before
        );

        assert_ok!(ModuleRate::reinit_external(Origin::signed(1), bailsman));

        let queue = eq_bailsman::DistributionQueue::<Test>::get();
        let distribution_after = queue.1.get(&1).expect("Non empty queue");

        assert_eq!(distribution_after.remaining_bailsmen, 0);
    });
}

#[test]
fn need_to_reinit_bailsman() {
    new_test_ext().execute_with(|| {
        #[allow(unused_imports)]
        use frame_support::traits::OnInitialize;

        let borrower = 0;
        let bailsman = 1234;
        let offset = 2 * 30 * 24 * 60 * 60 * 1000;

        for acc in [borrower, bailsman] {
            ModuleBalances::make_free_balance_be(
                &acc,
                eq_primitives::asset::BTC,
                SignedBalance::<Balance>::Positive(ONE_TOKEN),
            );
            ModuleBalances::make_free_balance_be(
                &acc,
                eq_primitives::asset::DOT,
                SignedBalance::<Balance>::Positive(ONE_TOKEN),
            );
        }

        assert_ok!(ModuleBailsman::register_bailsman(&bailsman));

        assert!(!ModuleRate::need_to_reinit(&borrower));
        assert!(!ModuleRate::need_to_reinit(&bailsman));

        for acc in [borrower, bailsman] {
            ModuleBalances::make_free_balance_be(
                &acc,
                eq_primitives::asset::BTC,
                SignedBalance::<Balance>::Negative(ONE_TOKEN),
            );
        }

        let now = ModuleRate::now().as_secs();
        ModuleTimestamp::set_timestamp(now * 1000 + offset);

        assert!(ModuleRate::need_to_reinit(&borrower));
        assert!(ModuleRate::need_to_reinit(&bailsman));
        // balance, queue
        ModuleBalances::make_free_balance_be(
            &bailsman,
            eq_primitives::asset::BTC,
            SignedBalance::<Balance>::Positive(ONE_TOKEN / 2),
        );

        ModuleBalances::make_free_balance_be(
            &BailsmanModuleId::get().into_account_truncating(),
            eq_primitives::asset::BTC,
            SignedBalance::<Balance>::Positive(ONE_TOKEN),
        );

        ModuleBailsman::on_initialize(1);

        let queue = eq_bailsman::DistributionQueue::<Test>::get();
        let distribution = queue.1.get(&1).expect("Non empty queue");
        assert_eq!(distribution.remaining_bailsmen, 1);

        assert!(!ModuleRate::need_to_reinit(&bailsman));

        ModuleBalances::make_free_balance_be(
            &BailsmanModuleId::get().into_account_truncating(),
            eq_primitives::asset::BTC,
            SignedBalance::<Balance>::Negative(2 * ONE_TOKEN),
        );

        ModuleBailsman::on_initialize(1);

        let queue = eq_bailsman::DistributionQueue::<Test>::get();
        let distribution = queue.1.get(&2).expect("Non empty queue");
        assert_eq!(distribution.remaining_bailsmen, 1);

        assert!(ModuleRate::need_to_reinit(&bailsman));
    });
}

#[test]
fn asset_removal_non_zero_collateral() {
    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));
    ext.execute_with(|| {
        assert_ok!(ModuleRate::set_auto_reinit_enabled(
            system::RawOrigin::Root.into(),
            true
        ));

        UintAuthorityId::set_all_keys(vec![11]);

        ModuleBalances::make_free_balance_be(
            &322,
            asset::BTC,
            eq_primitives::balance_adapter::Positive(100),
        );

        assert_eq!(
            ModuleAggregates::get_total(eq_primitives::UserGroup::Balances, asset::BTC),
            eq_primitives::TotalAggregates::<u128> {
                collateral: 100,
                debt: 0,
            }
        );

        assert_ok!(EqAssets::remove_asset(
            system::RawOrigin::Root.into(),
            asset::BTC
        ));

        assert_eq!(EqAssets::assets_to_remove(), Some(vec![asset::BTC]));

        ModuleRate::offchain_worker(1);
        assert_eq!(state.read().transactions.len(), 1);

        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let withdraw = match ex.call {
            crate::mock::Call::EqRate(crate::Call::withdraw { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(withdraw.account, 322);
        assert_eq!(withdraw.asset, asset::BTC);
        assert_eq!(withdraw.amount, 100);
        assert_eq!(withdraw.authority_index, 0);
        assert_eq!(withdraw.validators_len, 5);
        assert_eq!(withdraw.block_num, 1);
    });
}

#[test]
fn asset_removal_non_zero_debt() {
    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));
    ext.execute_with(|| {
        assert_ok!(ModuleRate::set_auto_reinit_enabled(
            system::RawOrigin::Root.into(),
            true
        ));

        UintAuthorityId::set_all_keys(vec![11]);

        ModuleBalances::make_free_balance_be(
            &322,
            asset::ETH,
            eq_primitives::balance_adapter::Positive(100_000_000_000_000),
        );

        ModuleBalances::make_free_balance_be(
            &322,
            asset::BTC,
            eq_primitives::balance_adapter::Negative(100),
        );

        assert_eq!(
            ModuleAggregates::get_total(eq_primitives::UserGroup::Balances, asset::BTC),
            eq_primitives::TotalAggregates::<u128> {
                collateral: 0,
                debt: 100,
            }
        );

        assert_ok!(EqAssets::remove_asset(
            system::RawOrigin::Root.into(),
            asset::BTC
        ));

        assert_eq!(EqAssets::assets_to_remove(), Some(vec![asset::BTC]));

        ModuleRate::offchain_worker(1);
        assert_eq!(state.read().transactions.len(), 1);

        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let deposit = match ex.call {
            crate::mock::Call::EqRate(crate::Call::deposit { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(deposit.account, 322);
        assert_eq!(deposit.asset, asset::BTC);
        assert_eq!(deposit.amount, 100);
        assert_eq!(deposit.authority_index, 0);
        assert_eq!(deposit.validators_len, 5);
        assert_eq!(deposit.block_num, 1);
    });
}

#[test]
fn asset_removal_non_zero_collateral_and_debt() {
    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));
    ext.execute_with(|| {
        assert_ok!(ModuleRate::set_auto_reinit_enabled(
            system::RawOrigin::Root.into(),
            true
        ));

        UintAuthorityId::set_all_keys(vec![11]);

        ModuleBalances::make_free_balance_be(
            &321,
            asset::BTC,
            eq_primitives::balance_adapter::Positive(100_000_000_000_000),
        );

        ModuleBalances::make_free_balance_be(
            &322,
            asset::ETH,
            eq_primitives::balance_adapter::Positive(100_000_000_000_000),
        );

        ModuleBalances::make_free_balance_be(
            &322,
            asset::BTC,
            eq_primitives::balance_adapter::Negative(100),
        );

        assert_eq!(
            ModuleAggregates::get_total(eq_primitives::UserGroup::Balances, asset::BTC),
            eq_primitives::TotalAggregates::<u128> {
                collateral: 100_000_000_000_000,
                debt: 100,
            }
        );

        assert_ok!(EqAssets::remove_asset(
            system::RawOrigin::Root.into(),
            asset::BTC
        ));

        assert_eq!(EqAssets::assets_to_remove(), Some(vec![asset::BTC]));

        ModuleRate::offchain_worker(1);
        assert_eq!(state.read().transactions.len(), 1);

        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let deposit = match ex.call {
            crate::mock::Call::EqRate(crate::Call::deposit { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(deposit.account, 322);
        assert_eq!(deposit.asset, asset::BTC);
        assert_eq!(deposit.amount, 100);
        assert_eq!(deposit.authority_index, 0);
        assert_eq!(deposit.validators_len, 5);
        assert_eq!(deposit.block_num, 1);
    });
}
