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

//! # Equilibrium Distribution Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use asset::AssetGetter;
use eq_assets;
use eq_primitives::asset;
use eq_primitives::balance::EqCurrency;
use eq_primitives::PriceSetter;
use frame_benchmarking::{account, benchmarks_instance};
use frame_support::traits::Instance;
use frame_system::RawOrigin;
use sp_runtime::traits::One;
use sp_runtime::FixedI64;
use sp_std::prelude::*;

const SEED: u32 = 0;
const BLOCK: u32 = 0;
const BUDGET: u128 = 10_000_000_000_000;
const TRANSFER: u32 = 10_000_000;
const RECEIVED_BALANCE: u32 = 10_000_000;
const VESTED_BALANCE: u32 = 100_000;

pub struct Pallet<T: Config<I>, I: 'static>(crate::Pallet<T, I>);

pub trait Config<I: 'static>:
    eq_whitelists::Config
    + eq_oracle::Config
    + eq_assets::Config
    + eq_balances::Config
    + crate::Config<I>
{
}

benchmarks_instance! {

    transfer {
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for asset in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<T::AccountId>>::set_price(price_setter.clone(), asset, FixedI64::one())
                .unwrap();
        }

        let basic_asset = eq_assets::Pallet::<T>::get_main_asset();

        let sender: T::AccountId = PalletId(*b"eq/trsry").into_account_truncating();
        let budget: BalanceOf::<T, I> = (BUDGET as u32).into();
        CurrencyOf::<T, I>::deposit_creating(
            &sender,
            budget
        );
        <eq_balances::Pallet<T>>::deposit_creating(&sender, asset::BTC, (BUDGET).try_into().map_err(|_| "balance conversion error").unwrap(), false, None)
            .unwrap();
        <eq_balances::Pallet<T>>::deposit_creating(&sender, asset::EQD, (BUDGET).try_into().map_err(|_| "balance conversion error").unwrap(), false, None)
            .unwrap();

        let target = account("target", 0, SEED);
        let amount: BalanceOf::<T, I> = TRANSFER.into();
    }: _(RawOrigin::Root, basic_asset, target, amount)
    verify {
        let acc = account("target", 0, SEED);
        assert_eq!(<eq_balances::Pallet<T>>::free_balance(&acc, basic_asset), RECEIVED_BALANCE.into());
    }

    vested_transfer {
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for asset in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<T::AccountId>>::set_price(price_setter.clone(), asset, FixedI64::one())
                .unwrap();
        }

        let basic_asset = eq_assets::Pallet::<T>::get_main_asset();

        let amount_to_deposit = BUDGET.try_into()
                .map_err(|_| "balance conversion error")
                .unwrap();

        let sender: T::AccountId = PalletId(*b"eq/trsry").into_account_truncating();
        <eq_balances::Pallet<T>>::deposit_creating(&sender, basic_asset, amount_to_deposit, false, None)
            .unwrap();
        <eq_balances::Pallet<T>>::deposit_creating(&sender, asset::BTC, amount_to_deposit, false, None)
            .unwrap();
        <eq_balances::Pallet<T>>::deposit_creating(&sender, asset::EQD, amount_to_deposit, false, None)
            .unwrap();

        let target = account("target", 0, SEED);
        let locked: BalanceOf::<T, I> = TRANSFER.into();
        let per_block: BalanceOf::<T, I> = (TRANSFER / 100).into();
        let starting_block: T::BlockNumber = BLOCK.into();
        let schedule = (locked, per_block, starting_block);
    }: _(RawOrigin::Root, target, schedule)
    verify {
        let acc = account("target", 0, SEED);
        assert_eq!(<eq_balances::Pallet<T>>::free_balance(&acc, basic_asset), VESTED_BALANCE.into());
    }
}
