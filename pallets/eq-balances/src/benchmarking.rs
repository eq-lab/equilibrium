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

//! # Equilibrium Balances Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use eq_primitives::asset;
use eq_primitives::PriceSetter;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
// use frame_support::traits::Hooks;
use frame_system::RawOrigin;
use sp_runtime::traits::One;
use sp_runtime::Percent;
use sp_runtime::{FixedI64, Permill};
use sp_std::vec;
use xcm::latest::{Junction::*, Junctions::*, MultiLocation, NetworkId};

const SEED: u32 = 0;
const BUDGET: u128 = 10_000_000_000_000;
const TRANSFER: u128 = 2_000_000_000_000;
// const REMAINING_BALANCE: u128 = 1_899_000_000_000;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    eq_whitelists::Config + eq_oracle::Config + eq_assets::Config + crate::Config
{
}

benchmarks! {

    enable_transfers {
    }: _(RawOrigin::Root)
    verify {
        assert!(IsTransfersEnabled::<T>::get());
    }

    disable_transfers {
    }: _(RawOrigin::Root)
    verify {
        assert!(!IsTransfersEnabled::<T>::get());
    }

    transfer {
        crate::Pallet::<T>::enable_transfers(RawOrigin::Root.into())
            .unwrap();

        let _ = eq_assets::Pallet::<T>::do_add_asset(
            asset::BTC,
            EqFixedU128::zero(),
            FixedI64::zero(),
            Permill::zero(),
            Permill::zero(),
            eq_primitives::asset::AssetXcmData::None,
            Permill::zero(),
            0,
            eq_primitives::asset::AssetType::Physical,
            true,
            Percent::zero(),
            Permill::one(),
            vec![],
        );

        let price_setter: T::AccountId = account("price_setter", 0, SEED);

        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();

        crate::Pallet::<T>::deposit_creating(
            &price_setter,
            asset::EQ,
            900_000_000_000_u128.try_into()
                .map_err(|_| "balance conversion error")
                .unwrap(),
            true,
            None
        ).unwrap();

        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<T::AccountId>>::set_price(
                price_setter.clone(),
                curr,
                FixedI64::one()
            ).unwrap();
        }

        let caller = whitelisted_caller();

        crate::Pallet::<T>::deposit_creating(
            &caller,
            asset::EQ,
            BUDGET.try_into()
                .map_err(|_| "balance conversion error")
                .unwrap(),
            true,
            None
        ).unwrap();
        crate::Pallet::<T>::deposit_creating(
            &caller,
            asset::BTC,
            BUDGET.try_into()
                .map_err(|_| "balance conversion error")
                .unwrap(),
            true,
            None
        ).unwrap();

        let to = account("to", 0, SEED);
    }: _(RawOrigin::Signed(caller), asset::BTC, to, TRANSFER.try_into()
                .map_err(|_| "balance conversion error")
                .unwrap())
    verify {
        let acc = account("to", 0, SEED);
        // Got a lot less because of the treasury buyout
        assert_eq!(crate::Pallet::<T>::free_balance(&acc, asset::BTC), TRANSFER.try_into()
                .map_err(|_| "balance conversion error")
                .unwrap());
    }

    allow_xcm_transfers_native_for {
        let z in 1..100;

        let mut accounts = vec![];
        for i in 0..z{
            accounts.push(account("account", i, SEED))
        }

    }: _(RawOrigin::Root, accounts)
    verify {
        assert_eq!(XcmNativeTransfers::<T>::iter_values().count(), z as usize);
    }

    forbid_xcm_transfers_native_for {
        let z in 1..100;

        let mut accounts = vec![];
        for i in 0..z{
            let account = account("account", i, SEED);
            XcmNativeTransfers::<T>::insert(&account,(<T as pallet::Config>::Balance::zero(),0));
            accounts.push(account)
        }

    }: _(RawOrigin::Root, accounts)
    verify{
        assert_eq!(XcmNativeTransfers::<T>::iter_values().count(), 0 as usize);
    }

    update_xcm_transfer_native_limit {
        let limit = (10_000 * eq_utils::ONE_TOKEN).try_into().unwrap_or_default();
    }: _(RawOrigin::Root, Some(limit))
    verify{
        assert_eq!(DailyXcmLimit::<T>::get(), Some(limit));
    }

    xcm_transfer_native {
        IsXcmTransfersEnabled::<T>::put(XcmMode::Xcm(true));
        let account_id = account("account", 0, SEED);
        let recepient: [u8; 32] = [0;32];

        crate::Pallet::<T>::deposit_creating(
            &account_id,
            asset::EQ,
            BUDGET.try_into()
                .map_err(|_| "balance conversion error")
                .unwrap(),
            true,
            None
        ).unwrap();
        crate::Pallet::<T>::deposit_creating(
            &account_id,
            asset::DOT,
            (200_000_000_000u128).try_into().unwrap_or_default(),
            true,
            None
        ).unwrap();
        let amount = (50_000_000_000u128).try_into().unwrap_or_default();
    }: _(RawOrigin::Signed(account_id), asset::DOT, amount, AccountType::Id32(recepient), XcmTransferDealWithFee::SovereignAccWillPay)

    xcm_transfer {
        IsXcmTransfersEnabled::<T>::put(XcmMode::Xcm(true));
        let account_id = account("account", 0, SEED);
        let recepient: [u8; 32] = [0;32];
        let to = MultiLocation {
            parents: 1,
            interior: X2(
                Parachain(2000),
                AccountId32 {
                    id: recepient,
                    network: NetworkId::Any,
                },
            ),
        };

        crate::Pallet::<T>::deposit_creating(
            &account_id,
            asset::EQ,
            BUDGET.try_into().unwrap_or_default(),
            true,
            None
        ).unwrap();
        crate::Pallet::<T>::deposit_creating(
            &account_id,
            asset::ACA,
            (200_000_000_000u128).try_into().unwrap_or_default(),
            true,
            None
        ).unwrap();
        let amount = (50_000_000_000u128).try_into().unwrap_or_default();
    }: _(RawOrigin::Signed(account_id), asset::ACA, amount, to, XcmTransferDealWithFee::SovereignAccWillPay)
}
