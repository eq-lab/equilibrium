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

//! # Equilibrium MarginCall Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use eq_assets;
use eq_primitives::{asset, balance::EqCurrency, PriceSetter, SignedBalance};
use eq_utils::ONE_TOKEN;
use eq_whitelists;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_runtime::{traits::One, FixedI64};

const SEED: u32 = 0;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    eq_whitelists::Config + eq_oracle::Config + eq_assets::Config + eq_balances::Config + crate::Config
{
}

benchmarks! {
    try_margincall_external{
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<T::AccountId>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let acc: T::AccountId = account("account", 0, SEED);
        <eq_balances::Pallet::<T> as EqCurrency<_, _>>::make_free_balance_be(
            &acc,
            asset::EQD,
            SignedBalance::Negative((962380 * ONE_TOKEN).try_into().map_err(|_| "balance conversion error").unwrap()),
        );
        <eq_balances::Pallet::<T> as EqCurrency<_, _>>::make_free_balance_be(
            &acc,
            asset::BTC,
            SignedBalance::Positive((200 * ONE_TOKEN).try_into().map_err(|_| "balance conversion error").unwrap())
        );

        let margin_state = crate::Pallet::<T>::check_margin(&acc)?;
        assert_eq!(margin_state, MarginState::SubCritical);
    }: _(RawOrigin::Signed(acc.clone()), acc.clone())
    verify{
        let acc: T::AccountId = account("account", 0, SEED);
        assert!(eq_balances::Pallet::<T>::get_balance(&acc, &asset::EQD).is_zero());
        assert!(eq_balances::Pallet::<T>::get_balance(&acc, &asset::BTC).is_zero());
    }
}
