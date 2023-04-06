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

//! # Equilibrium Dex Pallet Benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::{Order, OrderSide, OrderType}; // BestPrice
use eq_assets;
use eq_balances;
use eq_oracle;
use eq_primitives::map;
use eq_primitives::{asset, PriceSetter};
use eq_rate;
use eq_subaccounts;
use eq_utils::fixed::{
    eq_fixedu128_from_fixedi64, fixedi64_from_fixedu128, fixedu128_from_fixedi64,
};
use eq_whitelists;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::PalletId;
use frame_support::{dispatch::UnfilteredDispatchable, unsigned::ValidateUnsigned};
use frame_system::RawOrigin;
use sp_runtime::{traits::One, FixedI64, FixedPointNumber, FixedU128};
use sp_std::vec;

const SEED: u32 = 0;
const BUDGET: u128 = 100_000_000_000_000;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    eq_whitelists::Config
    + eq_oracle::Config
    + eq_assets::Config
    + eq_subaccounts::Config
    + eq_balances::Config
    + eq_rate::Config
    + crate::Config
{
}

benchmarks! {
    create_limit_order {
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let user = account("user", 0, SEED);
        let borrower_id = eq_subaccounts::Pallet::<T>::create_subaccount_inner(&user, &SubAccType::Trader).unwrap();

        let amount = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();

        eq_balances::Pallet::<T>::deposit_creating(&borrower_id, asset::EQ, amount, true, None)
            .unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&borrower_id, asset::DOT, amount, true, None)
            .unwrap();

        let asset = asset::DOT;
        let asset_data = eq_assets::Pallet::<T>::get_asset_data(&asset).unwrap();
        let amount = asset_data.lot;
        let price = FixedI64::one();
        let expiration_time = 100u64;
        let order_type = OrderType::Limit{price, expiration_time};
        let side = OrderSide::Buy;
    }: create_order(RawOrigin::Signed(user), asset, order_type, side, amount)
    verify {
        let asset = asset::DOT;
        let asset_data = eq_assets::Pallet::<T>::get_asset_data(&asset).unwrap();
        let price = FixedI64::one();
        let order_id = 1;
        let amount = asset_data.lot;
        let order_type = OrderType::Limit{price, expiration_time};
        let side = OrderSide::Buy;

        let stored_order = crate::Pallet::<T>::find_order(&asset, order_id, price);
        assert!(stored_order.is_some());
        let stored_order = stored_order.unwrap();
        assert_eq!(side, stored_order.side);
        assert_eq!(amount, stored_order.amount);
        assert_eq!(expiration_time, stored_order.expiration_time);
    }

    create_market_order{
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let amount = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        let user = account("user", 0, SEED);
        let borrower_id = eq_subaccounts::Pallet::<T>::create_subaccount_inner(&user, &SubAccType::Trader).unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&borrower_id, asset::EQ, amount, true, None)
            .unwrap();
        eq_balances::Pallet::<T>::deposit_creating(&borrower_id, asset::DOT, amount, true, None)
            .unwrap();

        let asset = asset::DOT;
        let asset_data = eq_assets::Pallet::<T>::get_asset_data(&asset).unwrap();
        let amount = asset_data.lot;

        // panic at 'not yet implemented'
        // uncomment this block side after implementation
        // let order_type = OrderType::Market;
        let price = FixedI64::one();
        let expiration_time = 100u64;
        let order_type = OrderType::Limit{price, expiration_time};
        // remove block above with price after implementation
        let side = OrderSide::Buy;

        // uncomment this block side after implementation
        // let best_asset_price = BestPrice {
        //     ask: Some(FixedI64::from(15)),
        //     bid: Some(FixedI64::from(10))
        // };
        // let _ = BestPriceByAsset::<T>::mutate(asset, |best_price| *best_price = best_asset_price);
    }: create_order(RawOrigin::Signed(user), asset, order_type, side, amount)
    verify {
        let asset = asset::DOT;
        let asset_data = eq_assets::Pallet::<T>::get_asset_data(&asset).unwrap();
        let price = FixedI64::one();
        let order_id = 1;
        let amount = asset_data.lot;
        let expiration_time = 100u64;
        let order_type = OrderType::Limit {price, expiration_time};

        let side = OrderSide::Buy;

        let stored_order = crate::Pallet::<T>::find_order(&asset, order_id, price);
        assert!(stored_order.is_some());
        let stored_order = stored_order.unwrap();
        assert_eq!(side, stored_order.side);
        assert_eq!(amount, stored_order.amount);
        assert_eq!(expiration_time, stored_order.expiration_time);
    }

    delete_order {
        eq_balances::Pallet::<T>::deposit_creating(
            &PalletId(*b"eq/trsry").into_account_truncating(),
            asset::EQ,
            BUDGET.try_into().map_err(|_|"balance conversion error").unwrap(),
            true,
            None
        ).unwrap();
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let user = account("user", 0, SEED);
        let borrower_id = eq_subaccounts::Pallet::<T>::create_subaccount_inner(&user, &SubAccType::Trader).unwrap();
        let asset = asset::DOT;
        let asset_data = eq_assets::Pallet::<T>::get_asset_data(&asset).unwrap();
        let amount = asset_data.lot;
        let order_id = 1;
        let price = FixedI64::one();
        let side = OrderSide::Buy;
        let expiration_time = 100u64;
        let created_at = 10u64;

        let asset_price_step = fixedu128_from_fixedi64(asset_data.price_step).unwrap();
        let price_step_count = FixedU128::saturating_from_integer(T::PriceStepCount::get());
        let denominator = price_step_count * asset_price_step;
        let fixed_chunk_key = price / fixedi64_from_fixedu128(denominator).unwrap();
        let chunk_key: u64 = (fixed_chunk_key.into_inner() / FixedI64::accuracy()) as u64;

        let order = Order {
            order_id,
            account_id: borrower_id.clone(),
            amount,
            created_at,
            side,
            price,
            expiration_time,
        };
        let _ = OrdersByAssetAndChunkKey::<T>::mutate(asset, chunk_key, |orders| *orders = vec![order]);
        let _ = ActualChunksByAsset::<T>::mutate(asset, |chunks| *chunks = vec![chunk_key]);
        let _ = AssetWeightByAccountId::<T>::mutate(
            borrower_id.clone(), |asset_weights| {
                let aggr = OrderAggregateBySide::new(amount, eq_fixedu128_from_fixedi64(price).unwrap(), side).unwrap();
                *asset_weights = map![asset => aggr];
            }
        );

        let buyout: <T as eq_rate::Config>::Balance = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        let request = OperationRequestDexDeleteOrder::<T::BlockNumber, T::AccountId, <T as eq_rate::Config>::Balance> {
            asset,
            order_id,
            price,
            who: user,
            buyout: Some(buyout),
            authority_index: 0,
            validators_len: 1,
            block_num: T::BlockNumber::default(),
            reason: DeleteOrderReason::OutOfCorridor,
        };
        //assert!(request.should_operate_in_block()); TODO: !!!!
        let key = <T as eq_rate::Config>::AuthorityId::generate_pair(None);
        let signature = key.sign(&request.encode()).unwrap();
    }: _(RawOrigin::None, request, signature)
    verify {
        let user = account("user", 0, SEED);
        let borrower_id = eq_subaccounts::Pallet::<T>::get_subaccount_id(&user, &SubAccType::Trader).unwrap();
        let asset = asset::DOT;
        let chunk_key = 20;

        let maybe_orders = OrdersByAssetAndChunkKey::<T>::get(asset, chunk_key);
        let maybe_chunks = ActualChunksByAsset::<T>::get(asset);
        let maybe_asset_weight = AssetWeightByAccountId::<T>::get(borrower_id.clone());

        assert!(maybe_orders.is_empty());
        assert!(maybe_chunks.is_empty());
        assert!(maybe_asset_weight.is_empty());
    }

    delete_order_external {
        let caller = whitelisted_caller();

        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let borrower_id = eq_subaccounts::Pallet::<T>::create_subaccount_inner(&caller, &SubAccType::Trader).unwrap();
        let asset = asset::DOT;
        let asset_data = eq_assets::Pallet::<T>::get_asset_data(&asset).unwrap();
        let amount = asset_data.lot;
        let order_id = 1;
        let price = FixedI64::one();
        let side = OrderSide::Buy;
        let expiration_time = 100u64;
        let created_at = 10u64;

        let asset_price_step = asset_data.price_step;
        let price_step_count = FixedU128::saturating_from_integer(T::PriceStepCount::get());
        let denominator = price_step_count * fixedu128_from_fixedi64(asset_price_step).unwrap();
        let fixed_chunk_key = price / fixedi64_from_fixedu128(denominator).unwrap();
        let chunk_key: u64 = (fixed_chunk_key.into_inner() / FixedI64::accuracy()) as u64;

        let order = Order {
            order_id,
            account_id: borrower_id.clone(),
            amount,
            created_at,
            side,
            price,
            expiration_time,
        };
        let _ = OrdersByAssetAndChunkKey::<T>::mutate(asset, chunk_key, |orders| *orders = vec![order]);
        let _ = ActualChunksByAsset::<T>::mutate(asset, |chunks| *chunks = vec![chunk_key]);
        let _ = AssetWeightByAccountId::<T>::mutate(
            borrower_id.clone(), |asset_weights| *asset_weights = map![asset => OrderAggregateBySide::new(amount, eq_fixedu128_from_fixedi64(price).unwrap(),side).unwrap()]
        );

    }: _(RawOrigin::Signed(caller), asset, order_id, price)
    verify {
        let caller = whitelisted_caller();
        let borrower_id = eq_subaccounts::Pallet::<T>::get_subaccount_id(&caller, &SubAccType::Trader).unwrap();
        let asset = asset::DOT;
        let chunk_key = 20;

        let maybe_orders = OrdersByAssetAndChunkKey::<T>::get(asset, chunk_key);
        let maybe_chunks = ActualChunksByAsset::<T>::get(asset);
        let maybe_asset_weight = AssetWeightByAccountId::<T>::get(borrower_id.clone());

        assert!(maybe_orders.is_empty());
        assert!(maybe_chunks.is_empty());
        assert!(maybe_asset_weight.is_empty());
    }

    update_asset_corridor {
        let asset = asset::DOT;
        let new_value: u32 = 10;
        assert_eq!(ChunkCorridorByAsset::<T>::get(asset), 5);
    }: _(RawOrigin::Root, asset, new_value)
    verify {
        let asset = asset::DOT;
        assert_eq!(ChunkCorridorByAsset::<T>::get(asset), 10);
    }

    validate_unsigned {
        eq_balances::Pallet::<T>::deposit_creating(
            &PalletId(*b"eq/trsry").into_account_truncating(),
            asset::EQ,
            BUDGET.try_into().map_err(|_|"balance conversion error").unwrap(),
            true,
            None
        ).unwrap();
        let price_setter: T::AccountId = account("price_setter", 0, SEED);
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        for curr in eq_assets::Pallet::<T>::get_assets_with_usd() {
            <eq_oracle::Pallet::<T> as PriceSetter<_>>::set_price(price_setter.clone(), curr, FixedI64::one())
                .unwrap();
        }

        let user = account("user", 0, SEED);
        let borrower_id = eq_subaccounts::Pallet::<T>::create_subaccount_inner(&user, &SubAccType::Trader).unwrap();
        let asset = asset::DOT;
        let asset_data = eq_assets::Pallet::<T>::get_asset_data(&asset).unwrap();
        let amount = asset_data.lot;
        let order_id = 1;
        let price = FixedI64::one();
        let side = OrderSide::Buy;
        let expiration_time = 100u64;
        let created_at = 10u64;

        let asset_price_step = fixedu128_from_fixedi64(asset_data.price_step).unwrap();
        let price_step_count = FixedU128::saturating_from_integer(T::PriceStepCount::get());
        let denominator = price_step_count * asset_price_step;
        let fixed_chunk_key = price / fixedi64_from_fixedu128(denominator).unwrap();
        let chunk_key: u64 = (fixed_chunk_key.into_inner() / FixedI64::accuracy()) as u64;

        let order = Order {
            order_id,
            account_id: borrower_id.clone(),
            amount,
            created_at,
            side,
            price,
            expiration_time,
        };
        let _ = OrdersByAssetAndChunkKey::<T>::mutate(asset, chunk_key, |orders| *orders = vec![order]);
        let _ = ActualChunksByAsset::<T>::mutate(asset, |chunks| *chunks = vec![chunk_key]);
        let _ = AssetWeightByAccountId::<T>::mutate(
            borrower_id.clone(), |asset_weights| *asset_weights = map![asset => OrderAggregateBySide::new(amount, eq_fixedu128_from_fixedi64(price).unwrap(),side).unwrap()]
        );

        let buyout: <T as eq_rate::Config>::Balance = BUDGET.try_into().map_err(|_|"balance conversion error").unwrap();
        let request = OperationRequestDexDeleteOrder::<T::BlockNumber, T::AccountId, <T as eq_rate::Config>::Balance> {
            asset,
            order_id,
            price,
            who: user,
            buyout: Some(buyout),
            authority_index: 0,
            validators_len: 1,
            block_num: T::BlockNumber::default(),
            reason: DeleteOrderReason::Cancel
        };

        let validator = <T as eq_rate::Config>::AuthorityId::generate_pair(None);
        eq_rate::Keys::<T>::set(vec![validator.clone()]);
        let signature = validator.sign(&request.encode()).expect("validator failed to sign request");
        let call = crate::Call::delete_order{request, signature};
        let source = sp_runtime::transaction_validity::TransactionSource::External;
    }: {
        super::Pallet::<T>::validate_unsigned(source, &call).unwrap();
        call.dispatch_bypass_filter(RawOrigin::None.into()).unwrap();
    }
}
