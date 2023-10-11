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
use crate::Call;
use crate::Pallet as EqBailsman;
use eq_primitives::asset;
use eq_primitives::asset::AssetXcmData;
use eq_primitives::PriceSetter;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::dispatch::UnfilteredDispatchable;
use frame_support::traits::Hooks;
use frame_support::unsigned::ValidateUnsigned;
use frame_system::RawOrigin;
use sp_arithmetic::Permill;
use sp_runtime::{FixedI64, Percent};
use sp_std::prelude::*;
use sp_std::vec;

const SEED: u32 = 0;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    eq_whitelists::Config + eq_oracle::Config + eq_assets::Config + crate::Config
{
}

fn init<T: Config>() {
    prepare_assets::<T>();

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

fn prepare_assets<T: Config>() {
    let assets = [
        asset::BTC,
        asset::EQD,
        asset::USDT,
        asset::USDC,
        asset::ACA,
        asset::DOT,
        asset::CRV,
        asset::FRAX,
        asset::ASTR,
        asset::DAI,
        asset::GENS,
        asset::KSM,
        asset::AUSD,
        asset::BNB,
        asset::BUSD,
        asset::EOS,
    ];

    for asset in assets {
        let exists = <T as pallet::Config>::AssetGetter::exists(asset);
        if exists {
            continue;
        }

        eq_assets::Pallet::<T>::do_add_asset(
            asset,
            EqFixedU128::zero(),
            FixedI64::zero(),
            Permill::zero(),
            Permill::zero(),
            AssetXcmData::None,
            Permill::from_rational(2u32, 5u32),
            0,
            eq_primitives::asset::AssetType::Physical,
            true,
            Percent::one(),
            Permill::one(),
            vec![],
        )
        .unwrap();
    }
}

fn prepare_temp_balances<T: Config>() {
    let temp_balance_acc_id = PalletId(*b"eq/bails").into_account_truncating();
    let to_distribute: <T as pallet::Config>::Balance = 1_000_000_000_000u128.into();

    let assets = <T as pallet::Config>::AssetGetter::get_assets();
    for asset in assets {
        T::EqCurrency::make_free_balance_be(
            &temp_balance_acc_id,
            asset,
            SignedBalance::Positive(to_distribute),
        );
    }
}

fn register_bailsmans<T: Config>(count: u32) -> Vec<T::AccountId> {
    let mut bailsmans = Vec::with_capacity(count as usize);
    for i in 0..count {
        let bails: T::AccountId = account("bails", i, SEED);
        bailsmans.push(bails.clone());
        T::EqCurrency::deposit_creating(
            &bails,
            asset::EQ,
            From::<u128>::from(9_000_000_000_000u128),
            true,
            None,
        )
        .unwrap();
        T::EqCurrency::deposit_creating(
            &bails,
            asset::BTC,
            From::<u128>::from(10_000_000_000_000u128),
            true,
            None,
        )
        .unwrap();
        T::EqCurrency::deposit_creating(
            &bails,
            asset::EQD,
            From::<u128>::from(10_000_000_000_000u128),
            true,
            None,
        )
        .unwrap();
        crate::Pallet::<T>::register_bailsman(&bails).unwrap();
    }

    bailsmans
}

fn prepare_distribution_queue<T: Config>(count: u32) {
    let block_number = BlockNumberFor::<T>::zero();
    for _ in 0..count {
        prepare_temp_balances::<T>();
        EqBailsman::<T>::on_initialize(block_number);
    }
}

benchmarks! {
    toggle_auto_redistribution{
        init::<T>();
        let enabled = false; // true by default
    }:_(RawOrigin::Root, enabled)
    verify {
        assert!(!AutoRedistributionEnabled::<T>::get())
    }

    redistribute_unsigned{
        // 50 - max value of queue after ddos tests
        let z in 1..50;

        init::<T>();
        let bailsmans = register_bailsmans::<T>(2);
        prepare_distribution_queue::<T>(z);

        let caller: T::AccountId = whitelisted_caller();
        let account_id = bailsmans[0].clone();

        let block_number = BlockNumberFor::<T>::zero();
        let request =
            DistributionRequest {
                bailsman: account_id.clone(),
                block_number,
                last_distr_id: 0,
                auth_idx: 0,
                curr_distr_id: 0,
                higher_priority: false,
                queue_len: z,
                val_len: 1,
            };

        let validator = <T as pallet::Config>::AuthorityId::generate_pair(None);
        T::ValidatorOffchainBatcher::set_local_authority_keys(vec![validator.clone()]);
        let signature = validator.sign(&request.encode()).expect("validator failed to sign request");
        let call = crate::Call::redistribute_unsigned{request, signature};
        let source = sp_runtime::transaction_validity::TransactionSource::External;

    }: {
        crate::Pallet::<T>::validate_unsigned(source, &call).unwrap();
        call.dispatch_bypass_filter(RawOrigin::None.into()).unwrap();
    }
    verify {
        assert_eq!(crate::Pallet::<T>::bailsmen_count(), 2);
        let (distr_id,_) = crate::Pallet::<T>::distribution_queue();
        assert_eq!(distr_id, z);

        let last_distr_id = LastDistribution::<T>::get(account_id).unwrap();
        assert_eq!(distr_id, last_distr_id);
    }

    redistribute{
        let z in 1..50;

        init::<T>();
        let bailsmans = register_bailsmans::<T>(2);
        prepare_distribution_queue::<T>(z);

        let caller: T::AccountId = whitelisted_caller();
        let account_id = bailsmans[0].clone();
    }:_(RawOrigin::Signed(caller), account_id.clone())
    verify{
        assert_eq!(crate::Pallet::<T>::bailsmen_count(), 2);
        let (distr_id,_) = crate::Pallet::<T>::distribution_queue();
        assert_eq!(distr_id, z);

        let last_distr_id = LastDistribution::<T>::get(account_id).unwrap();
        assert_eq!(distr_id, last_distr_id);
    }

    on_initialize{
        init::<T>();
        prepare_assets::<T>();
        prepare_temp_balances::<T>();
        let _ = register_bailsmans::<T>(10);

        let block_number = BlockNumberFor::<T>::zero();
    }:{
        EqBailsman::<T>::on_initialize(block_number);
    }
    verify{
        assert_eq!(crate::Pallet::<T>::bailsmen_count(), 10);

        let (distr_id, queue) = crate::Pallet::<T>::distribution_queue();
        assert_eq!(distr_id, 1);
        assert_eq!(queue.len(), 1);
    }

    on_finalize{
        let z in 1..50;

        init::<T>();
        let bailsmans = register_bailsmans::<T>(2);
        prepare_distribution_queue::<T>(z);

        let caller: T::AccountId = whitelisted_caller();
        let account_id = bailsmans[0].clone();

        for account_id in bailsmans{
            let _ = crate::Pallet::<T>::redistribute(RawOrigin::Signed(caller.clone()).into(), account_id);
        }

        let block_number = BlockNumberFor::<T>::zero();
    }:{
        EqBailsman::<T>::on_finalize(block_number);
    }
}
