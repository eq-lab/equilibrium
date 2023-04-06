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
use eq_primitives::{
    asset::Asset,
    balance::EqCurrency,
    subaccount::{SubAccType, SubaccountsManager},
    PriceSetter,
};
use eq_utils::ONE_TOKEN;
use frame_benchmarking::{account, benchmarks};
use frame_support::unsigned::ValidateUnsigned;
use frame_system::RawOrigin;
use sp_runtime::FixedI64;
use test_utils::financial_config;

use crate::Pallet as Rate;
use core::time::Duration;
use eq_subaccounts::Pallet as Subaccounts;
use financial_pallet::Module as Financial;

const SEED: u32 = 0;

pub struct Pallet<T: Config>(crate::Pallet<T>);
pub trait Config:
    crate::Config
    + eq_whitelists::Config
    + eq_oracle::Config
    + eq_subaccounts::Config
    + eq_balances::Config
    + financial_pallet::Config<
        Asset = Asset,
        Price = substrate_fixed::types::I64F64,
        FixedNumberBits = i128,
    >
{
}

/// Initialize all finance metrics for interest fee calculation
fn initialize_financial_data<T: Config>(period_start: Duration) {
    <T as crate::Config>::AssetGetter::get_assets_with_usd()
        .iter()
        .for_each(|asset| {
            Financial::<T>::set_per_asset_metrics(
                RawOrigin::Root.into(),
                *asset,
                financial_config::get_per_asset_metrics(*asset, period_start),
            )
            .unwrap();
        });

    Financial::<T>::set_metrics(
        RawOrigin::Root.into(),
        financial_config::get_metrics(period_start),
    )
    .unwrap();
}

fn basic_asset<T: Config>() -> Asset {
    <T as crate::Config>::AssetGetter::get_main_asset()
}

/// Initialize all prices to prevent price errors
fn initialize_prices<T: Config>(index: u32) {
    let price_setter: T::AccountId = account("price_setter", index, SEED);
    let _ =
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone());
    for asset in <T as crate::Config>::AssetGetter::get_assets_with_usd() {
        <eq_oracle::Pallet<T> as PriceSetter<_>>::set_price(
            price_setter.clone(),
            asset,
            FixedI64::one(),
        )
        .unwrap();
    }
}

/// Transactor
fn initialize_owner<T: Config>() -> T::AccountId {
    let basic_asset = basic_asset::<T>();
    let owner: T::AccountId = account("owner", 0, SEED);
    <T as crate::Config>::EqCurrency::deposit_creating(
        &owner,
        basic_asset,
        (10u128 * ONE_TOKEN)
            .try_into()
            .map_err(|_| "conversion error")
            .unwrap(),
        false,
        None,
    )
    .unwrap();

    owner
}

/// Main account and borrower subaccount with debt and collateral
fn generate_borrower_with_debt<T: Config>(index: u32) -> T::AccountId {
    let main_account: T::AccountId = account("borrower_owner", index, SEED);

    let assets = <T as crate::Config>::AssetGetter::get_assets_with_usd();
    for asset in assets.iter() {
        <T as crate::Config>::EqCurrency::deposit_creating(
            &main_account,
            *asset,
            (2_000u128 * ONE_TOKEN)
                .try_into()
                .map_err(|_| "conversion error")
                .unwrap(),
            true,
            None,
        )
        .unwrap();
    }

    for asset in assets.iter() {
        Subaccounts::<T>::transfer_to_subaccount(
            RawOrigin::Signed(main_account.clone()).into(),
            SubAccType::Trader,
            *asset,
            From::<u128>::from(1_000 * ONE_TOKEN),
        )
        .unwrap();
    }

    Subaccounts::<T>::transfer_from_subaccount(
        RawOrigin::Signed(main_account.clone()).into(),
        SubAccType::Trader,
        eq_primitives::asset::EQD,
        From::<u128>::from(8_000 * ONE_TOKEN),
    )
    .unwrap();

    <T as pallet::Config>::SubaccountsManager::get_subaccount_id(&main_account, &SubAccType::Trader)
        .unwrap()
}

/// Main account and bailsman subaccount with collateral
fn generate_bailsman<T: Config>(index: u32) {
    eq_balances::Pallet::<T>::enable_transfers(RawOrigin::Root.into()).unwrap();

    let assets = <T as crate::Config>::AssetGetter::get_assets_with_usd();

    let main_account: T::AccountId = account("bailsman_owner", index, SEED);
    for asset in assets.iter() {
        <T as crate::Config>::EqCurrency::deposit_creating(
            &main_account,
            *asset,
            (100_000 * ONE_TOKEN)
                .try_into()
                .map_err(|_| "conversion error")
                .unwrap(),
            true,
            None,
        )
        .unwrap();
    }

    for asset in assets.iter() {
        Subaccounts::<T>::transfer_to_subaccount(
            RawOrigin::Signed(main_account.clone()).into(),
            SubAccType::Bailsman,
            *asset,
            From::<u128>::from(10_000 * ONE_TOKEN),
        )
        .unwrap();
    }
}

/// Sign request for offchain calls
fn sign_request<T: crate::Config>(
    request: &OperationRequest<T::AccountId, T::BlockNumber>,
) -> <T::AuthorityId as RuntimeAppPublic>::Signature {
    let validator = <T as crate::Config>::AuthorityId::generate_pair(None);
    crate::Keys::<T>::set(vec![validator.clone()]);
    validator.sign(&request.encode()).unwrap()
}

benchmarks! {
    reinit{
        initialize_financial_data::<T>(Duration::from_secs(0));
        initialize_prices::<T>(0);
        generate_bailsman::<T>(0);

        let borrower = generate_borrower_with_debt::<T>(0);

        let request = OperationRequest::<T::AccountId, T::BlockNumber> {
            authority_index: 0,
            validators_len: 1,
            block_num: T::BlockNumber::default(),
            account: Some(borrower),
            higher_priority: false
        };

        let signature = sign_request::<T>(&request);

        let offset_millis: u64  = 365 * 24 * 60 * 60 * 1000; // 1 year
        let _ = Rate::<T>::set_now_millis_offset(RawOrigin::Root.into(), offset_millis);

        //to prevent price timeout update prices
        initialize_prices::<T>(1);
        let source = sp_runtime::transaction_validity::TransactionSource::External;
        let call = crate::Call::<T>::reinit{request: request.clone(), signature: signature.clone()};
    }: {
        super::Pallet::<T>::validate_unsigned(source, &call).unwrap();
        super::Pallet::<T>::reinit(RawOrigin::None.into(), request, signature).unwrap();
    }

    reinit_external{
        initialize_financial_data::<T>(Duration::from_secs(0));
        initialize_prices::<T>(0);
        generate_bailsman::<T>(0);

        let owner = initialize_owner::<T>();
        let borrower: T::AccountId = generate_borrower_with_debt::<T>(0);


        let offset_millis: u64  = 365 * 24 * 60 * 60 * 1000; // 1 year
        let _ = Rate::<T>::set_now_millis_offset(RawOrigin::Root.into(), offset_millis);
        initialize_prices::<T>(1);

    }: _(RawOrigin::Signed(owner.clone()), borrower)

    delete_account{
        initialize_prices::<T>(0);
        let owner = initialize_owner::<T>();

        let account_to_delete: T::AccountId = account("account_to_delete", 0, SEED);
        let _ = <T as crate::Config>::EqCurrency::deposit_creating(
            &account_to_delete,
            basic_asset::<T>(),
            (50_000_000u128).try_into()
                .map_err(|_| "conversion error")
                .unwrap(),
            true,
            None
        ); //0.05 NATIVE

        let request = OperationRequest::<T::AccountId, T::BlockNumber> {
            authority_index: 0,
            validators_len: 1,
            block_num: T::BlockNumber::default(),
            account: Some(account_to_delete),
            higher_priority: false
        };

        let signature = sign_request::<T>(&request);
        let source = sp_runtime::transaction_validity::TransactionSource::External;
        let call = crate::Call::<T>::delete_account{ request: request.clone(), signature: signature.clone()};
    }: {
        super::Pallet::<T>::validate_unsigned(source, &call).unwrap();
        super::Pallet::<T>::delete_account(RawOrigin::None.into(), request, signature).unwrap();
    }

    delete_account_external{
        let validator = <T as crate::Config>::AuthorityId::generate_pair(None);
        initialize_prices::<T>(0);
        let owner = initialize_owner::<T>();

        let account_to_delete: T::AccountId = account("account_to_delete", 0, SEED);
        <T as crate::Config>::EqCurrency::deposit_creating(
            &account_to_delete,
            basic_asset::<T>(),
            (50_000_000u128)
                .try_into()
                .map_err(|_| "conversion error")
                .unwrap(),
            true,
            None
        ).unwrap(); //0.05 NATIVE

    }: _(RawOrigin::Signed(owner.clone()), account_to_delete)

    set_auto_reinit_enabled{

    }: _(RawOrigin::Root, true)
    verify{
        assert_eq!(AutoReinitEnabled::<T>::get(), true);
    }
}
