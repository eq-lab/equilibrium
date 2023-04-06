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

//! # Equilibrium Eq-Eth Bridge Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::Pallet as EqBridge;
use eq_primitives::balance::EqCurrency;
use eq_primitives::{asset, PriceSetter, SignedBalance};
use frame_benchmarking::{account, benchmarks};
use frame_support::sp_runtime::traits::Hash;
use frame_support::PalletId;
use frame_system::RawOrigin;
use sp_arithmetic::{traits::One, FixedI64};
use sp_runtime::traits::AccountIdConversion;

const SEED: u32 = 0;
const BUDGET: u128 = 20_000_000_000_000; // 20_000
const TRANSFER: u128 = 1000_000_000; // 0.1
pub const MAX: u32 = <u32>::MAX;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config: eq_whitelists::Config + eq_oracle::Config + crate::Config {}

fn init_prices<T: Config>() {
    let price_setter: T::AccountId = account("price_setter", 0, SEED);
    eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
        .unwrap();
    <eq_oracle::Pallet<T> as PriceSetter<T::AccountId>>::set_price(
        price_setter.clone(),
        asset::EQ,
        FixedI64::one(),
    )
    .unwrap();
    <eq_oracle::Pallet<T> as PriceSetter<T::AccountId>>::set_price(
        price_setter.clone(),
        asset::ETH,
        FixedI64::from(1500),
    )
    .unwrap();
}

fn init_treasury<T: Config>() {
    let sender: T::AccountId = PalletId(*b"eq/trsry").into_account_truncating();
    let budget: <T as chainbridge::Config>::Balance = (BUDGET as u32).into();
    let native_asset = <T as pallet::Config>::AssetGetter::get_main_asset();
    T::EqCurrency::deposit_creating(&sender, native_asset, budget, false, None).unwrap();
}

benchmarks! {
    transfer_native {
        let account: T::AccountId = account("from", 0, SEED);
        let amount: <T as chainbridge::Config>::Balance = (TRANSFER as u32).into();
        let budget: <T as chainbridge::Config>::Balance = MAX.into();
        let recipient = vec![99];
        let dest_id = 0;
        let resource_id = chainbridge::derive_resource_id(1, b"hash");

        EqBridge::<T>::enable_withdrawals(RawOrigin::Root.into(), resource_id, dest_id).unwrap();

        EqBridge::<T>::set_resource(
            RawOrigin::Root.into(),
            resource_id,
            asset::ETH
        ).expect("set_resource unexpected panic");

        <chainbridge::Pallet<T>>::whitelist_chain(RawOrigin::Root.into(), 0, 0u32.into()).unwrap();
        <T as chainbridge::Config>::Currency::make_free_balance_be(&account.clone(), (BUDGET).try_into().map_err(|_| "balance conversion error").unwrap());

        T::EqCurrency::make_free_balance_be(&account.clone(), asset::ETH, SignedBalance::Positive(budget));

    }: _(RawOrigin::Signed(account.clone()), amount, recipient, dest_id, resource_id)
    verify {
        assert!(true);
    }

    transfer {
        init_prices::<T>();
        init_treasury::<T>();
        let bridge_id = <chainbridge::Pallet<T>>::account_id();
        let to = account("to", 0, SEED);
        let amount: <T as chainbridge::Config>::Balance = (TRANSFER as u32).into();
        let budget: <T as chainbridge::Config>::Balance = (BUDGET as u32).into();
        let resource_id = chainbridge::derive_resource_id(1, b"hash");

        EqBridge::<T>::set_resource(
            RawOrigin::Root.into(),
            resource_id,
            asset::ETH
        ).expect("set_resource unexpected panic");

        <T as chainbridge::Config>::Currency::make_free_balance_be(&bridge_id, (BUDGET).try_into().map_err(|_| "balance conversion error").unwrap());
        T::EqCurrency::make_free_balance_be(&bridge_id, asset::ETH, SignedBalance::Positive(budget));
        // T::EqCurrency::make_free_balance_be(&to, T::MainAsset::get(), SignedBalance::Positive(budget));

    }: _(RawOrigin::Signed(bridge_id.clone()), to, amount, resource_id)

    transfer_basic {
        let bridge_id = <chainbridge::Pallet<T>>::account_id();
        let to = account("to", 0, SEED);
        let amount: <T as chainbridge::Config>::Balance = (TRANSFER as u32).into();
        let budget: <T as chainbridge::Config>::Balance = (BUDGET as u32).into();
        let resource_id = chainbridge::derive_resource_id(1, b"hash");

        EqBridge::<T>::set_resource(
            RawOrigin::Root.into(),
            resource_id,
            T::MainAsset::get()
        ).expect("set_resource unexpected panic");

        <T as chainbridge::Config>::Currency::make_free_balance_be(&<chainbridge::Pallet<T>>::account_id(), (BUDGET).try_into().map_err(|_| "balance conversion error").unwrap());
        T::EqCurrency::make_free_balance_be(&<chainbridge::Pallet<T>>::account_id(),T::MainAsset::get(), SignedBalance::Positive(budget));

    }: transfer(RawOrigin::Signed(bridge_id.clone()), to, amount, resource_id)

    remark {
        let bridge_id = <chainbridge::Pallet<T>>::account_id();
        let hash: T::Hash = <T as frame_system::Config>::Hashing::hash_of(&vec![10]);
    }: _(RawOrigin::Signed(bridge_id.clone()), hash)
    verify {
        assert!(true);
    }

    set_resource{
        let resource_id = chainbridge::derive_resource_id(1, b"hash");
    }: _(RawOrigin::Root, resource_id, T::MainAsset::get())

    enable_withdrawals{
        let chain_id = 0;
        let resource_id = chainbridge::derive_resource_id(1, b"hash");
    }: _(RawOrigin::Root, resource_id, chain_id)

    disable_withdrawals{
        let chain_id = 0;
        let resource_id = chainbridge::derive_resource_id(1, b"hash");

        EqBridge::<T>::enable_withdrawals(RawOrigin::Root.into(), resource_id, chain_id).unwrap();

    }: _(RawOrigin::Root, resource_id, chain_id)

    set_minimum_transfer_amount{
        let chain_id = 0;
        let resource_id = chainbridge::derive_resource_id(1, b"hash");

        EqBridge::<T>::set_resource(
            RawOrigin::Root.into(),
            resource_id,
            asset::ETH
        ).expect("set_resource unexpected panic");

        chainbridge::pallet::Pallet::<T>::whitelist_chain(RawOrigin::Root.into(), chain_id, 0u32.into()).unwrap();
    }: _(RawOrigin::Root, chain_id, resource_id, 1000u128.try_into()
                .map_err(|_| "balance conversion error")
                .unwrap())
}
