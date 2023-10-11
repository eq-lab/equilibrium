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
use eq_primitives::asset::AssetGetter;
use eq_primitives::{PriceSetter, SignedBalance};
use eq_utils::ONE_TOKEN;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Hooks;
use frame_system::RawOrigin;
use sp_std::prelude::*;

const SEED: u32 = 0;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    eq_whitelists::Config + eq_oracle::Config + eq_assets::Config + crate::Config
{
}

fn init_wrapped_dot_supply<T: Config>() {
    let account_id = account("some_user", 0, SEED);
    let balance = (200 * ONE_TOKEN)
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();

    T::EqCurrency::make_free_balance_be(
        &account_id,
        asset::EQDOT,
        SignedBalance::Positive(balance),
    );

    let transferable = (100 * ONE_TOKEN)
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();

    let staked = (100 * ONE_TOKEN)
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();

    CurrentBalance::<T>::put(StakingBalance {
        transferable,
        staked,
    });
}

fn prepare_relay_staking_info<T: Config>() {
    let stash_account = account("stash", 0, SEED);
    let mut total = Default::default();
    let active = 10_000_000_000u64.into();

    let value: <RelayRuntime as RelaySystemConfig>::Balance = 10000_0_000_000_000u64.into();

    let mut unlocking = vec![];
    for i in 0..6 {
        let chunk = UnlockChunk { value, era: i + 1 };
        unlocking.push(chunk);
        total += value;
    }

    let ledger = StakingLedger::<RelayRuntime> {
        stash: stash_account,
        total,
        active,
        unlocking: unlocking.try_into().unwrap(),
        claimed_rewards: vec![],
    };

    RelayStakingInfo::<T>::put((1, ledger));
}

fn prepare_withdraw_queue<T: Config>(count: u32) {
    let to_burn: <T as crate::Config>::Balance = (10 * ONE_TOKEN)
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();

    let amount_to_withdraw: <T as crate::Config>::Balance = (10 * ONE_TOKEN)
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();

    let initial_eqdot: <T as crate::Config>::Balance = ((count as u128) * 10 * ONE_TOKEN)
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();

    let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();
    init_account::<T>(&pallet_account);

    T::EqCurrency::make_free_balance_be(
        &pallet_account,
        asset::EQDOT,
        SignedBalance::Positive(initial_eqdot),
    );

    let mut withdraw_queue: Vec<(_, _, _)> = vec![];

    for i in 0..count {
        let beneficiary: T::AccountId = account("beneficiary", i, SEED);
        init_account::<T>(&beneficiary);

        T::EqCurrency::make_free_balance_be(
            &beneficiary,
            asset::EQDOT,
            SignedBalance::Positive(initial_eqdot),
        );

        withdraw_queue.push((beneficiary, amount_to_withdraw, to_burn))
    }

    WithdrawQueue::<T>::put(withdraw_queue);
}

fn prepare_staking_rebalance<T: Config>() {
    let transferable = (250 * ONE_TOKEN)
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();
    let staked = (750 * ONE_TOKEN)
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();

    CurrentBalance::<T>::put(StakingBalance {
        transferable,
        staked,
    });
}

fn init_asset_prices<T: Config>() {
    let price_setter: T::AccountId = account("price_setter", 0, 0);
    eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
        .unwrap();

    for asset in eq_assets::pallet::Pallet::<T>::get_assets_with_usd() {
        <eq_oracle::Pallet<T> as PriceSetter<T::AccountId>>::set_price(
            price_setter.clone(),
            asset,
            FixedI64::one(),
        )
        .unwrap();
    }
}

fn init_treasury<T: Config>() {
    let sender: T::AccountId = PalletId(*b"eq/trsry").into_account_truncating();
    let budget: <T as pallet::Config>::Balance = (1000_000_000_000 as u128)
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();
    let native_asset = eq_assets::pallet::Pallet::<T>::get_main_asset();
    T::EqCurrency::deposit_creating(&sender, native_asset, budget, false, None).unwrap();
}

fn init<T: Config>() {
    init_asset_prices::<T>();
    init_treasury::<T>();
    init_wrapped_dot_supply::<T>();
}

fn init_account<T: Config>(account_id: &T::AccountId) {
    let basic_aaset_amount = 100_000_000_000u128
        .try_into()
        .map_err(|_| "balance conversion error")
        .unwrap();

    T::EqCurrency::make_free_balance_be(
        account_id,
        asset::EQ,
        SignedBalance::Positive(basic_aaset_amount),
    );
}

benchmarks! {
    deposit{
        init::<T>();

        let caller = account("user", 0, SEED);
        init_account::<T>(&caller);

        let deposit_amount = (10 * ONE_TOKEN)
                .try_into()
                .map_err(|_| "balance conversion error")
                .unwrap();

        T::EqCurrency::deposit_creating(
            &caller,
            asset::DOT,
            deposit_amount,
            true,
            None,
        ).unwrap();
    }:_(RawOrigin::Signed(caller.clone()), deposit_amount)
    verify{
        assert_eq!(
            T::EqCurrency::total_balance(&caller, asset::DOT),
            <T as pallet::Config>::Balance::zero()
        )
    }

    withdraw{
        init::<T>();
        let caller = account("user", 0, SEED);
        init_account::<T>(&caller);

        let withdraw_amount = (50 * ONE_TOKEN)
                .try_into()
                .map_err(|_| "balance conversion error")
                .unwrap();

        let initial_eqdot_amount = (100 * ONE_TOKEN)
                .try_into()
                .map_err(|_| "balance conversion error")
                .unwrap();

        T::EqCurrency::make_free_balance_be(
            &caller,
            asset::EQDOT,
            SignedBalance::Positive(initial_eqdot_amount),
        );
    }:_(RawOrigin::Signed(caller.clone()), WithdrawAmount::Dot(withdraw_amount))
    verify{
        assert_eq!(
            T::EqCurrency::total_balance(&caller, asset::DOT),
            withdraw_amount
        );
    }

    withdraw_unbond{
        init::<T>();

        let caller = account("user", 0, SEED);
        init_account::<T>(&caller);

        let withdraw_amount = (101 * ONE_TOKEN)
                .try_into()
                .map_err(|_| "balance conversion error")
                .unwrap();

        let initial_eqdot_amount = (250 * ONE_TOKEN)
                .try_into()
                .map_err(|_| "balance conversion error")
                .unwrap();

        T::EqCurrency::make_free_balance_be(
            &caller,
            asset::EQDOT,
            SignedBalance::Positive(initial_eqdot_amount),
        );

        assert_eq!(
            WithdrawQueue::<T>::get().len(),
            0
        );

    }: withdraw(RawOrigin::Signed(caller.clone()), WithdrawAmount::Dot(withdraw_amount))
    verify{
        assert_eq!(
            WithdrawQueue::<T>::get().len(),
            1
        );
    }

    on_initialize{
        let c in 1..50;

        init::<T>();

        prepare_relay_staking_info::<T>();
        prepare_withdraw_queue::<T>(c);
        prepare_staking_rebalance::<T>();

        let block_number = BlockNumberFor::<T>::zero();
    }:{
        pallet::Pallet::<T>::on_initialize(block_number);
    }

    on_finalize{
        init::<T>();
        let block_number = BlockNumberFor::<T>::zero();
    }:{
        pallet::Pallet::<T>::on_finalize(block_number);
    }
}
