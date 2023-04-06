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

//! # Equilibrium Lockdrop Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use core::convert::TryInto;
use eq_assets;
use eq_balances;
use eq_primitives::{asset::AssetGetter, PriceSetter};
use eq_rate;
use eq_whitelists;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::{dispatch::UnfilteredDispatchable, unsigned::ValidateUnsigned, PalletId};
use frame_system::{Pallet as System, RawOrigin};
use sp_runtime::traits::One;
use sp_runtime::FixedI64;
use sp_std::vec;
use timestamp;

const SEED: u32 = 0;
const BUDGET: u128 = 100_000_000_000_000;
const MILLISECS_PER_SEC: u64 = 1000;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    timestamp::Config
    + eq_whitelists::Config
    + eq_oracle::Config
    + eq_assets::Config
    + eq_balances::Config
    + eq_rate::Config
    + crate::Config
{
}

benchmarks! {
    lock{
        let user: T::AccountId = account("user", 0, SEED);
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone()).unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one()).unwrap();
        }
        let amount = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&user, asset::EQ, amount, true, None).unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&user, asset::DOT, amount, true, None).unwrap();

        let balance: <T as eq_rate::Config>::Balance = (BUDGET).try_into().map_err(|_|"balance conversion error").unwrap();
    }: _(RawOrigin::Signed(user.clone()), balance)
    verify{
        let user_lock = Locks::<T>::get(user);
        let balance: <T as eq_rate::Config>::Balance = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        assert_eq!(user_lock, balance);
    }

    unlock{
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone()).unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one()).unwrap();
        }

        let user: T::AccountId = account("user", 0, SEED);
        let balance: <T as eq_rate::Config>::Balance = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        let lock_start = 10u64;
        let lock_period: u64 = <T as crate::Config>::LockPeriod::get();
        let _ = Locks::<T>::mutate(user.clone(), |value| *value = balance);
        let _ = LockStart::<T>::mutate(|start| *start = Some(lock_start));

        let lockdrop_acc_id = PalletId(*b"eq/lkdrp").into_account_truncating();
        let amount = (5*BUDGET).try_into().map_err(|_|"balance conversion error").unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&lockdrop_acc_id, asset::EQ, amount, true, None).unwrap();

        let move_time = (lock_period + lock_start + 10) * MILLISECS_PER_SEC;
        eq_rate::pallet::Pallet::<T>::set_now_millis_offset(RawOrigin::Root.into(), move_time).unwrap();
        System::<T>::set_block_number(1u32.into());

        let request = OperationRequest::<T::AccountId, T::BlockNumber> {
            account: user,
            authority_index: 0,
            validators_len: 1,
            block_num: T::BlockNumber::default(),
        };
        let key = <T as eq_rate::Config>::AuthorityId::generate_pair(None);
        let signature = key.sign(&request.encode()).unwrap();
    }: _(RawOrigin::None, request, signature)
    verify {
        let user: T::AccountId = account("user", 0, SEED);
        let caller_lock: <T as eq_rate::Config>::Balance = Locks::<T>::get(user);
        let default = <T as eq_rate::Config>::Balance::default();
        assert_eq!(caller_lock, default);
    }

    unlock_external{
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone()).unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one()).unwrap();
        }

        let caller: T::AccountId = whitelisted_caller();
        let balance: <T as eq_rate::Config>::Balance = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        let lock_start = 10u64;
        let lock_period: u64 = <T as crate::Config>::LockPeriod::get();
        let _ = Locks::<T>::mutate(caller.clone(), |value| *value = balance);
        let _ = LockStart::<T>::mutate(|start| *start = Some(lock_start));

        let lockdrop_acc_id = PalletId(*b"eq/lkdrp").into_account_truncating(); //: AccountId32
        let amount = (5*BUDGET).try_into().map_err(|_|"balance conversion error").unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&lockdrop_acc_id, asset::EQ, amount, true, None).unwrap();

        let move_time = (lock_period + lock_start + 10) * MILLISECS_PER_SEC;
        eq_rate::pallet::Pallet::<T>::set_now_millis_offset(RawOrigin::Root.into(), move_time).unwrap();
        System::<T>::set_block_number(1u32.into());
    }: _(RawOrigin::Signed(caller))
    verify {
        let caller: T::AccountId = whitelisted_caller();
        let caller_lock: <T as eq_rate::Config>::Balance = Locks::<T>::get(caller);
        let default = <T as eq_rate::Config>::Balance::default();
        assert_eq!(caller_lock, default);
    }

    set_lock_start{
        let start_value = 10u64;

        pallet::Pallet::<T>::clear_lock_start(RawOrigin::Root.into()).unwrap();
        assert!(LockStart::<T>::get().is_none());
    }: _(RawOrigin::Root, start_value)
    verify {
        let start_value = 10u64;
        assert_eq!(LockStart::<T>::get(), Some(start_value));
    }

    clear_lock_start{
        let start_value = 10u64;
        let _ = LockStart::<T>::mutate(|start| *start = Some(start_value));

        assert!(LockStart::<T>::get().is_some());
        assert_eq!(LockStart::<T>::get().unwrap(), start_value);

    }: _(RawOrigin::Root)
    verify {
        assert!(LockStart::<T>::get().is_none());
    }

    set_auto_unlock{
        let enabled = false;
        assert_eq!(AutoUnlockEnabled::<T>::get(), true);
    }: _(RawOrigin::Root, enabled)
    verify {
        let enabled = false;
        assert_eq!(AutoUnlockEnabled::<T>::get(), enabled);
    }

    validate_unsigned {
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone()).unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one()).unwrap();
        }

        let user: T::AccountId = account("user", 0, SEED);
        let balance: <T as eq_rate::Config>::Balance = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        let lock_start = 10u64;
        let lock_period: u64 = <T as crate::Config>::LockPeriod::get();
        let _ = Locks::<T>::mutate(user.clone(), |value| *value = balance);
        let _ = LockStart::<T>::mutate(|start| *start = Some(lock_start));

        let lockdrop_acc_id = PalletId(*b"eq/lkdrp").into_account_truncating(); //: AccountId32
        let amount = (5*BUDGET).try_into().map_err(|_|"balance conversion error").unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&lockdrop_acc_id, asset::EQ, amount, true, None).unwrap();

        let move_time = (lock_period + lock_start + 10) * MILLISECS_PER_SEC;
        eq_rate::pallet::Pallet::<T>::set_now_millis_offset(RawOrigin::Root.into(), move_time).unwrap();
        System::<T>::set_block_number(1u32.into());

        let request = OperationRequest::<T::AccountId, T::BlockNumber> {
            account: user,
            authority_index: 0,
            validators_len: 1,
            block_num: T::BlockNumber::default(),
        };
        let validator = <T as eq_rate::Config>::AuthorityId::generate_pair(None);
        eq_rate::Keys::<T>::set(vec![validator.clone()]);
        let signature = validator.sign(&request.encode()).expect("validator failed to sign request");
        let call = crate::Call::unlock{request, signature};
        let source = sp_runtime::transaction_validity::TransactionSource::External;
    }: {
        super::Pallet::<T>::validate_unsigned(source, &call).unwrap();
        call.dispatch_bypass_filter(RawOrigin::None.into()).unwrap();
    }
    verify {
        let user: T::AccountId = account("user", 0, SEED);
        let caller_lock: <T as eq_rate::Config>::Balance = Locks::<T>::get(user);
        let default = <T as eq_rate::Config>::Balance::default();
        assert_eq!(caller_lock, default);
    }
}
