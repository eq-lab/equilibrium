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

use asset::AssetGetter;
use eq_primitives::PriceSetter;
use eq_primitives::{asset, SignedBalance};
use frame_benchmarking::{account, benchmarks};
use frame_support::pallet_prelude::Hooks;
use frame_support::PalletId;
use frame_system::RawOrigin;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_runtime::traits::{AccountIdConversion, One, Zero};
use sp_runtime::FixedI64;

const SEED: u32 = 0;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    eq_balances::Config
    + eq_whitelists::Config
    + eq_oracle::Config
    + eq_bailsman::Config
    + eq_assets::Config
    + crate::Config
{
}

fn init<T: Config>() {
    eq_balances::Pallet::<T>::enable_transfers(RawOrigin::Root.into()).unwrap();

    let price_setter: T::AccountId = account("price_setter", 0, SEED);
    eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
        .unwrap();

    for asset in eq_assets::Pallet::<T>::get_assets_with_usd() {
        <eq_oracle::Pallet<T> as PriceSetter<T::AccountId>>::set_price(
            price_setter.clone(),
            asset,
            FixedI64::one(),
        )
        .unwrap();
    }
}

fn init_account_balance<T: Config>(caller: &T::AccountId) {
    <T as crate::Config>::EqCurrency::deposit_creating(
        caller,
        asset::EQ,
        From::<u128>::from(9_000_000_000_000 as u128),
        true,
        None,
    )
    .unwrap();
    <T as crate::Config>::EqCurrency::deposit_creating(
        caller,
        asset::BTC,
        From::<u128>::from(30_000_000_000_000u128),
        true,
        None,
    )
    .unwrap();
}

fn prepare_temp_balances<T: Config>() {
    let temp_balance_acc_id = PalletId(*b"eq/bails").into_account_truncating();
    let to_distribute: <T as eq_bailsman::Config>::Balance = 1_000_000_000_000u128.into();

    let assets = <T as eq_bailsman::Config>::AssetGetter::get_assets();
    for asset in assets {
        <T as eq_bailsman::Config>::EqCurrency::make_free_balance_be(
            &temp_balance_acc_id,
            asset,
            SignedBalance::Positive(to_distribute),
        );
    }
}

fn prepare_distribution_queue<T: Config>(count: u32) {
    let block_number = T::BlockNumber::zero();
    for _ in 0..count {
        prepare_temp_balances::<T>();
        eq_bailsman::pallet::Pallet::<T>::on_initialize(block_number);
    }
}

benchmarks! {
    //_ { }

    transfer_to_bailsman_register {
        let caller: T::AccountId = account("caller", 0, SEED);
        init::<T>();
        init_account_balance::<T>(&caller);
    }: transfer_to_subaccount(RawOrigin::Signed(caller), SubAccType::Bailsman, asset::BTC, 20_000_000_000_000u128.unique_saturated_into())

    transfer_to_borrower_register {
        let caller: T::AccountId = account("caller", 0, SEED);
        init::<T>();
        init_account_balance::<T>(&caller);
    }: transfer_to_subaccount(RawOrigin::Signed(caller), SubAccType::Trader, asset::BTC, 20_000_000_000_000u128.unique_saturated_into())

    transfer_to_bailsman_and_redistribute {
        let r in 0..50;

        let caller: T::AccountId = account("caller", 0, SEED);
        init::<T>();
        init_account_balance::<T>(&caller);
        <T as crate::Config>::BailsmenManager::register_bailsman(&caller).unwrap();
        prepare_distribution_queue::<T>(r);
    }: transfer_to_subaccount(RawOrigin::Signed(caller), SubAccType::Bailsman, asset::BTC, 20_000_000_000_000u128.unique_saturated_into())

    // Same as transfer to existed Borrower
    transfer_to_subaccount {
        let caller: T::AccountId = account("caller", 0, SEED);
        init::<T>();
        init_account_balance::<T>(&caller);
        <T as crate::Config>::BailsmenManager::register_bailsman(&caller).unwrap();
    }: transfer_to_subaccount(RawOrigin::Signed(caller), SubAccType::Bailsman, asset::BTC, 20_000_000_000_000u128.unique_saturated_into())

    transfer_from_subaccount {
        let caller: T::AccountId = account("caller", 0, SEED);
        init::<T>();
        init_account_balance::<T>(&caller);
        crate::Pallet::<T>::transfer_to_subaccount(RawOrigin::Signed(caller.clone()).into(), SubAccType::Bailsman, asset::BTC, 20_000_000_000_000u128.unique_saturated_into())?;
    }: _(RawOrigin::Signed(caller), SubAccType::Bailsman, asset::BTC, 20_000_000_000_000u64.unique_saturated_into())

    transfer_from_subaccount_redistribute {
        let r in 0..50;
        let caller: T::AccountId = account("caller", 0, SEED);
        init::<T>();
        init_account_balance::<T>(&caller);
        crate::Pallet::<T>::transfer_to_subaccount(RawOrigin::Signed(caller.clone()).into(), SubAccType::Bailsman, asset::BTC, 20_000_000_000_000u128.unique_saturated_into())?;
        prepare_distribution_queue::<T>(r);
    }: transfer_from_subaccount(RawOrigin::Signed(caller), SubAccType::Bailsman, asset::BTC, 20_000_000_000_000u64.unique_saturated_into())
}
