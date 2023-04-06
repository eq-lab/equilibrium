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

//! # Equilibrium Market Maker Pools Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use eq_primitives::{
    asset::{self, AssetGetter},
    balance::EqCurrency,
    PriceSetter,
};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::{traits::One, FixedI64};
use sp_std::prelude::*;

pub trait Config:
    eq_whitelists::Config + eq_oracle::Config + eq_assets::Config + eq_balances::Config + crate::Config
{
}
pub struct Pallet<T: Config>(crate::Pallet<T>);

const SEED: u32 = 0;
const BUDGET: u128 = 100_000_000_000_000;

benchmarks! {
    create_pool {}: _(RawOrigin::Root, asset::ETH, 100u128.into())

    change_min_amount {
        crate::Pallet::<T>::create_pool(RawOrigin::Root.into(), asset::ETH, 100u128.into())?;
    }: _(RawOrigin::Root, asset::ETH, 200u128.into())

    set_epoch_duration {}: _(RawOrigin::Root, 700_000)

    add_manager {
        let borrower: T::AccountId = whitelisted_caller();
    }: _(RawOrigin::Root, borrower.clone(), 0)

    set_allocations {
        let b in 0 .. 13;

        let allocations: Vec<_> = eq_assets::Pallet::<T>::get_assets_with_usd()
            .into_iter()
            .take(b as usize)
            .map(|asset| (asset, Perbill::from_percent(50)))
            .collect();
        assert_eq!(allocations.len(), b as usize);
    }: _(RawOrigin::Root, 0, allocations)

    borrow {
        crate::Pallet::<T>::create_pool(RawOrigin::Root.into(), asset::ETH, 100u128.into())?;

        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let lender: T::AccountId = whitelisted_caller();
        let amount = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&lender, asset::ETH, amount, true, None)?;
        crate::Pallet::<T>::deposit(RawOrigin::Signed(lender.clone()).into(), 200u128.into(), asset::ETH)?;

        let borrower: T::AccountId = whitelisted_caller();
        crate::Pallet::<T>::add_manager(RawOrigin::Root.into(), borrower.clone(), 0)?;
        crate::Pallet::<T>::set_allocations(RawOrigin::Root.into(), 0, vec![(asset::ETH, Perbill::from_percent(100))])?;
    }: _(RawOrigin::Signed(borrower.clone()), 100u128.into(), asset::ETH)

    repay {
        crate::Pallet::<T>::create_pool(RawOrigin::Root.into(), asset::ETH, 100u128.into())?;

        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let lender: T::AccountId = whitelisted_caller();
        let amount = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&lender, asset::ETH, amount, true, None)?;
        crate::Pallet::<T>::deposit(RawOrigin::Signed(lender.clone()).into(), 200u128.into(), asset::ETH)?;

        let borrower: T::AccountId = whitelisted_caller();
        crate::Pallet::<T>::add_manager(RawOrigin::Root.into(), borrower.clone(), 0)?;
        crate::Pallet::<T>::set_allocations(RawOrigin::Root.into(), 0, vec![(asset::ETH, Perbill::from_percent(100))])?;
        crate::Pallet::<T>::borrow(RawOrigin::Signed(borrower.clone()).into(), 100u128.into(), asset::ETH)?;
    }: _(RawOrigin::Signed(borrower.clone()), 100u128.into(), asset::ETH)

    deposit {
        crate::Pallet::<T>::create_pool(RawOrigin::Root.into(), asset::ETH, 100u128.into())?;

        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let caller: T::AccountId = whitelisted_caller();
        let amount = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&caller, asset::ETH, amount, true, None)?;
    }: _(RawOrigin::Signed(caller.clone()), 200u128.into(), asset::ETH)

    request_withdrawal {
        crate::Pallet::<T>::create_pool(RawOrigin::Root.into(), asset::ETH, 100u128.into())?;

        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let caller: T::AccountId = whitelisted_caller();
        let amount = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&caller, asset::ETH, amount, true, None)?;
        crate::Pallet::<T>::deposit(RawOrigin::Signed(caller.clone()).into(), 200u128.into(), asset::ETH)?;
    }: _(RawOrigin::Signed(caller.clone()), 200u128.into(), asset::ETH)

    withdraw {
        crate::Pallet::<T>::create_pool(RawOrigin::Root.into(), asset::ETH, 100u128.into())?;

        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let caller: T::AccountId = whitelisted_caller();
        let amount = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&caller, asset::ETH,  amount, true, None)?;
        crate::Pallet::<T>::deposit(RawOrigin::Signed(caller.clone()).into(), 200u128.into(), asset::ETH)?;
        crate::Pallet::<T>::request_withdrawal(RawOrigin::Signed(caller.clone()).into(), 200u128.into(), asset::ETH)?;

        Epoch::<T>::mutate(|e| e.counter += 2);
    }: _(RawOrigin::Signed(caller.clone()), asset::ETH)

    global_advance_epoch {
        let b in 0 .. 13;

        let assets = eq_assets::Pallet::<T>::get_assets_with_usd();

        for i in 0..b {
            crate::Pallet::<T>::create_pool(RawOrigin::Root.into(), assets[i as usize], 100u128.into())?;
        }

        let epoch = crate::EpochInfo {
            counter: 0,
            started_at: 0,
            duration: 100,
            new_duration: Some(200),
        };
    }: {
        crate::Pallet::<T>::global_advance_epoch(epoch);
    }
}
