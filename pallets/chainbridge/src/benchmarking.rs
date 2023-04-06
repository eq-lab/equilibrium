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

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

const SEED: u32 = 0;

benchmarks! {

    set_threshold {
    }: _(RawOrigin::Root, 42u32)
    verify {
        assert_eq!(RelayerThreshold::<T>::get(), 42u32);
    }

    set_resource {
        let id: ResourceId = [1; 32];
        let method = "Pallet.do_something".as_bytes().to_vec();

    }: _(RawOrigin::Root, id, method)
    verify {
        let method = "Pallet.do_something".as_bytes().to_vec();
        assert_eq!(Pallet::<T>::resources(id), Some(method));
    }

    remove_resource {
        let id: ResourceId = [1; 32];
        let method = "Pallet.do_something".as_bytes().to_vec();
        Pallet::<T>::set_resource(RawOrigin::Root.into(), id, method).unwrap();

    }: _(RawOrigin::Root, id)
    verify {
        let method = "Pallet.do_something".as_bytes().to_vec();
        assert_eq!(Pallet::<T>::resources(id), None);
    }

    whitelist_chain {
    }: _(RawOrigin::Root, 0u8, 0u32.into())
    verify {
        assert_eq!(ChainNonces::<T>::get(0u8), Some(0));
    }

    add_relayer {
        let account: T::AccountId = whitelisted_caller();
    }: _(RawOrigin::Root, account.clone())
    verify {
        assert_eq!(Relayers::<T>::get(account), true);
    }

    remove_relayer {
        let account: T::AccountId = whitelisted_caller();
        Relayers::<T>::insert(account.clone(), true);
    }: _(RawOrigin::Root, account.clone())
    verify {
        assert_eq!(Relayers::<T>::get(account), false);
    }

    acknowledge_proposal {
        let account: T::AccountId = whitelisted_caller();
        let proposal:<T as Config>::Proposal = frame_system::Call::<T>::remark{ remark: vec![]}.into();
        // we will add weights for transfer later.
        Pallet::<T>::set_threshold(RawOrigin::Root.into(), 2).unwrap();
        Pallet::<T>::add_relayer(RawOrigin::Root.into(), account.clone()).unwrap();
        Pallet::<T>::whitelist_chain(RawOrigin::Root.into(), 0, 0u32.into()).unwrap();
        Pallet::<T>::set_resource(RawOrigin::Root.into(), [1; 32], b"Test.test".to_vec()).unwrap();
    } : _(RawOrigin::Signed(account.clone()), 0, 0, [1; 32], Box::new(proposal.clone()))
    verify {
        assert_eq!(Pallet::<T>::votes(0, (0, proposal.clone())).unwrap().votes_for,
            vec![account]);
    }

    reject_proposal {
        let account: T::AccountId = whitelisted_caller();
        let proposal:<T as Config>::Proposal = frame_system::Call::<T>::remark{ remark: vec![]}.into();
        // we will add weights for transfer later.
        Pallet::<T>::set_threshold(RawOrigin::Root.into(), 2).unwrap();
        Pallet::<T>::add_relayer(RawOrigin::Root.into(), account.clone()).unwrap();
        Pallet::<T>::whitelist_chain(RawOrigin::Root.into(), 0, 0u32.into()).unwrap();
        Pallet::<T>::set_resource(RawOrigin::Root.into(), [1; 32], b"Test.test".to_vec()).unwrap();
    } : _(RawOrigin::Signed(account.clone()), 0, 0, [1; 32], Box::new(proposal.clone()))
    verify {
        assert_eq!(Pallet::<T>::votes(0, (0, proposal.clone())).unwrap().votes_against,
            vec![account]);
    }

    eval_vote_state {
        let account: T::AccountId = whitelisted_caller();
        let proposal:<T as Config>::Proposal = frame_system::Call::<T>::remark{ remark: vec![]}.into();
        // we will add weights for transfer later.
        Pallet::<T>::set_threshold(RawOrigin::Root.into(), 2).unwrap();
        Pallet::<T>::add_relayer(RawOrigin::Root.into(), account.clone()).unwrap();
        Pallet::<T>::whitelist_chain(RawOrigin::Root.into(), 0, 0u32.into()).unwrap();
        Pallet::<T>::set_resource(RawOrigin::Root.into(), [1; 32], b"Test.test".to_vec()).unwrap();
        Pallet::<T>::reject_proposal(RawOrigin::Signed(account.clone()).into(), 0, 0, [1; 32], Box::new(proposal.clone())).unwrap();
    } : _(RawOrigin::Signed(account.clone()), 0, 0, Box::new(proposal.clone()))
    verify {
        assert_eq!(Pallet::<T>::votes(0, (0, proposal.clone())).unwrap().votes_against,
            vec![account]);
    }

    redistribute_fees {
        let z in 1..50;

        // adding relayers
        for i in 0..z {
            let account: T::AccountId = account("relayer", i, SEED);
            Relayers::<T>::insert(account, true);
            RelayerCount::<T>::mutate(|i| *i += 1);
        }

        T::Currency::make_free_balance_be(&Pallet::<T>::fee_account_id(), (z * 1000).into());
        let caller: T::AccountId = whitelisted_caller();
    } : _(RawOrigin::Signed(caller.clone()))
    verify {
        let acc: T::AccountId = account("relayer", 0, SEED);
        assert_eq!(T::Currency::free_balance(&acc), 1000u32.into());
    }
}
