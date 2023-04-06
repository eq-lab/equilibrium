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
use crate::Pallet as EqLending;
use eq_primitives::PriceSetter;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_arithmetic::FixedI64;
use sp_runtime::traits::One;

const SEED: u32 = 0;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    eq_whitelists::Config + eq_oracle::Config + eq_assets::Config + crate::Config
{
}

fn init_prices<T: Config>() {
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

fn init_lending_pool<T: Config>() {
    OnlyBailsmanTill::<T>::put(0);
    let amount: <T as pallet::Config>::Balance = 1_000_000_000_000u128
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();

    for i in 0..10 {
        let account = account("account", i, SEED);
        T::EqCurrency::make_free_balance_be(&account, asset::EQ, SignedBalance::Positive(amount));

        for asset in eq_assets::Pallet::<T>::get_assets_data()
            .iter()
            .filter(|a| a.asset_type == AssetType::Physical)
            .map(|a| a.id)
        {
            T::EqCurrency::make_free_balance_be(&account, asset, SignedBalance::Positive(amount));

            EqLending::<T>::deposit(RawOrigin::Signed(account.clone()).into(), asset, amount)
                .unwrap();
        }
    }
}

fn add_reward<T: Config>(asset: Asset, amount: <T as pallet::Config>::Balance) {
    let lending_account = T::ModuleId::get().into_account_truncating();
    T::EqCurrency::make_free_balance_be(
        &lending_account,
        asset::EQ,
        SignedBalance::Positive(amount),
    );

    EqLending::<T>::do_add_reward(asset, amount).unwrap();
}

fn init_account<T: Config>(account: &T::AccountId) {
    T::EqCurrency::make_free_balance_be(
        account,
        asset::EQ,
        SignedBalance::Positive(
            1000_000_000_000u128
                .try_into()
                .map_err(|_| "balance conversion error")
                .unwrap(),
        ),
    );

    T::EqCurrency::make_free_balance_be(
        account,
        asset::ETH,
        SignedBalance::Positive(
            1000_000_000_000u128
                .try_into()
                .map_err(|_| "balance conversion error")
                .unwrap(),
        ),
    );

    T::EqCurrency::make_free_balance_be(
        account,
        asset::BTC,
        SignedBalance::Positive(
            1000_000_000_000u128
                .try_into()
                .map_err(|_| "balance conversion error")
                .unwrap(),
        ),
    );
}

benchmarks! {
    deposit{
        let caller: T::AccountId = whitelisted_caller();
        init_prices::<T>();
        init_account::<T>(&caller);
        init_lending_pool::<T>();

        let amount: <T as pallet::Config>::Balance = 100_000_000_000u128.try_into().map_err(|_| "balance conversion error").unwrap();
        EqLending::<T>::deposit(RawOrigin::Signed(caller.clone()).into(), asset::ETH, amount).unwrap();
        add_reward::<T>(asset::ETH, 1_000_000_000u128.try_into().map_err(|_| "balance conversion error").unwrap());
        let aggregates_before = LendersAggregates::<T>::get(asset::ETH);
        let balance_before = T::EqCurrency::free_balance(&caller, asset::ETH);
    }:_(RawOrigin::Signed(caller.clone()), asset::ETH, amount)
    verify{
        assert_eq!(T::EqCurrency::free_balance(&caller, asset::ETH), balance_before - 100_000_000_000u128.try_into().map_err(|_| "balance conversion error").unwrap());
        assert_eq!(LendersAggregates::<T>::get(asset::ETH), aggregates_before + amount);
    }

    withdraw{
        let caller: T::AccountId = whitelisted_caller();
        init_prices::<T>();
        init_account::<T>(&caller);
        init_lending_pool::<T>();

        let amount: <T as pallet::Config>::Balance = 100_000_000_000u128.try_into().map_err(|_| "balance conversion error").unwrap();
        EqLending::<T>::deposit(RawOrigin::Signed(caller.clone()).into(), asset::ETH, amount).unwrap();
        add_reward::<T>(asset::ETH, 1_000_000_000u128.try_into().map_err(|_| "balance conversion error").unwrap());
    }:_(RawOrigin::Signed(caller.clone()), asset::ETH, amount)

    payout {
        let caller: T::AccountId = whitelisted_caller();
        init_prices::<T>();
        init_account::<T>(&caller);
        init_lending_pool::<T>();

        let amount: <T as pallet::Config>::Balance = 100_000_000_000u128.try_into().map_err(|_| "balance conversion error").unwrap();
        EqLending::<T>::deposit(RawOrigin::Signed(caller.clone()).into(), asset::ETH, amount).unwrap();
        add_reward::<T>(asset::ETH, 1_000_000_000u128.try_into().map_err(|_| "balance conversion error").unwrap());

        assert!(!EqLending::<T>::is_only_bailsmen_period());
    }:_(RawOrigin::Signed(caller.clone()), asset::ETH, caller.clone())
}
