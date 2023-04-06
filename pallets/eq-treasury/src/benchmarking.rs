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
use eq_assets;
use eq_primitives::balance::EqCurrency;
use eq_primitives::{asset, PriceSetter, SignedBalance};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::FixedI64;
use sp_std::prelude::*;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    eq_whitelists::Config + eq_oracle::Config + eq_assets::Config + eq_rate::Config + crate::Config
{
}

fn init_prices<T: Config>() {
    let price_setter: T::AccountId = account("price_setter", 0, 0);
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

benchmarks! {
    buyout {
        let move_time = BUYOUT_LIMIT_PERIOD_IN_SEC * 1000 * 2;
        eq_rate::pallet::Pallet::<T>::set_now_millis_offset(RawOrigin::Root.into(), move_time).unwrap();

        init_prices::<T>();

        let caller: T::AccountId = whitelisted_caller();
        let treas_acc = crate::Pallet::<T>::account_id();
        let basic_asset = <T as pallet::Config>::AssetGetter::get_main_asset();

         <T as pallet::Config>::EqCurrency::make_free_balance_be(
            &treas_acc,
            basic_asset,
            SignedBalance::Positive((1000u128 * 1_000_000_000u128).try_into().unwrap_or_default())
        );

        <T as pallet::Config>::EqCurrency::make_free_balance_be(
            &caller,
            asset::DOT,
            SignedBalance::Positive((1000u128 * 1_000_000_000u128).try_into().unwrap_or_default())
        );
        let limit: <T as crate::Config>::Balance = 100_000_000_000u128.try_into().unwrap_or_default();
        BuyoutLimit::<T>::put(limit);
        //set prev limit
        Buyouts::<T>::insert(caller.clone(), (<T as crate::Config>::Balance::default(), 0));

    }: _(RawOrigin::Signed(caller.clone()), asset::DOT, Amount::Buyout(100_000_000_000u128.try_into().unwrap_or_default()))
    verify{
        assert_eq!(
             <T as pallet::Config>::EqCurrency::total_balance(&caller, asset::EQ),
            100_000_000_000u128.try_into().unwrap_or_default()
        );
    }

    update_buyout_limit {
    }: _(RawOrigin::Root, Some(100_000_000_000u128.try_into().unwrap_or_default()))
}
