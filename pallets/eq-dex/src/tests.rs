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

#![cfg(test)]

use super::*;
use crate::mock::*;
use eq_primitives::asset::{AssetType, BTC};
use eq_primitives::{
    asset::{Asset, DAI, EQD, ETH},
    balance::BalanceGetter,
    Aggregates, OrderAggregate, PriceSetter, SignedBalance, UserGroup,
};
use frame_support::{assert_err, assert_noop, assert_ok, dispatch::DispatchError};
use frame_system::RawOrigin;
use sp_arithmetic::traits::{Bounded, CheckedAdd};
use sp_arithmetic::Permill;
use sp_core::offchain::{
    testing::{TestOffchainExt, TestTransactionPoolExt},
    TransactionPoolExt,
};
use sp_runtime::offchain::{OffchainDbExt, OffchainWorkerExt};
use sp_runtime::traits::One;
use sp_runtime::Percent;
use sp_runtime::{testing::UintAuthorityId, FixedPointNumber};

fn convert_to_prices(prices: &[i32]) -> Vec<FixedI64> {
    prices.iter().map(|&i| FixedI64::from(i as i64)).collect()
}

fn create_orders(who: &u64, asset: Asset, side: OrderSide, prices: &[Price]) -> Vec<OrderId> {
    let amount = EqFixedU128::from(1);
    println!("amount {:?}", amount);
    let expiration_time = 100u64;
    let borrower_id = SubaccountsManagerMock::get_subaccount_id(who, &SubAccType::Trader)
        .expect("Borrower subaccount");
    let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");

    prices
        .iter()
        .map(|&price| {
            assert_ok!(ModuleDex::create_limit_order(
                borrower_id,
                asset,
                price,
                side,
                amount,
                expiration_time,
                &asset_data
            ));

            OrderIdCounter::<Test>::get()
        })
        .collect()
}

#[test]
fn offchain_worker_delete_orders_of_dex_disabled_assets() {
    use frame_support::traits::OffchainWorker;

    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        UintAuthorityId::set_all_keys(vec![11, 21, 31, 41, 51]);
        let acc_id = 1;
        let asset = ETH;
        let mut price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;
        let block = 1;

        let root_origin: Origin = RawOrigin::Root.into();
        let new_asset_corridor: u32 = 5;
        assert_ok!(ModuleDex::update_asset_corridor(
            root_origin,
            asset,
            new_asset_corridor
        ));

        (0..3).for_each(|_| {
            assert_ok!(ModuleDex::create_order(
                Origin::signed(acc_id),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ));
            price = price + FixedI64::from(5);
        });

        assert_eq!(EqAssets::get_asset_data(&ETH).unwrap().is_dex_enabled, true);

        //disable dex for asset
        assert_ok!(EqAssets::update_asset(
            RawOrigin::Root.into(),
            ETH,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(false),
            None,
            None
        ));

        assert_eq!(
            EqAssets::get_asset_data(&ETH).unwrap().is_dex_enabled,
            false
        );

        ModuleDex::offchain_worker(block);
        ModuleSystem::set_block_number(block);

        assert_eq!(state.read().transactions.len(), 3);

        for _i in 0..3 {
            let transaction = state.write().transactions.pop().unwrap();
            let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
            let delete_order = match ex.call {
                crate::mock::Call::EqDex(crate::Call::delete_order { request, .. }) => request,
                e => panic!("Unexpected call: {:?}", e),
            };

            assert_eq!(delete_order.asset, asset);
        }
    });
}

#[test]
fn offchain_worker_delete_off_corridor() {
    use frame_support::traits::OffchainWorker;

    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        UintAuthorityId::set_all_keys(vec![11, 21, 31, 41, 51]);

        let acc_id = 1;
        let asset = ETH;
        let mut price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;
        let block = 1;

        let root_origin: Origin = RawOrigin::Root.into();
        let new_asset_corridor: u32 = 5;
        assert_ok!(ModuleDex::update_asset_corridor(
            root_origin,
            asset,
            new_asset_corridor
        ));

        (0..8).for_each(|_| {
            assert_ok!(ModuleDex::create_order(
                Origin::signed(acc_id),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ));
            price = price + FixedI64::from(5);
        });

        let chunks = ActualChunksByAsset::<Test>::get(asset);
        let expected_chunk = 50u64;
        for n in 0..8 {
            assert_eq!(chunks[n], expected_chunk + n as u64);
        }

        ModuleDex::offchain_worker(block);
        ModuleSystem::set_block_number(block);

        assert_eq!(state.read().transactions.len(), 2);

        let mut transaction = state.write().transactions.pop().unwrap();
        let mut ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let mut delete_order = match ex.call {
            crate::mock::Call::EqDex(crate::Call::delete_order { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(delete_order.asset, asset);
        assert_eq!(delete_order.order_id, 2);
        assert_eq!(delete_order.price, FixedI64::from(255));
        assert_eq!(delete_order.authority_index, 2);
        assert_eq!(delete_order.validators_len, 5);
        assert_eq!(delete_order.block_num, 1);

        transaction = state.write().transactions.pop().unwrap();
        ex = Decode::decode(&mut &*transaction).unwrap();
        delete_order = match ex.call {
            crate::mock::Call::EqDex(crate::Call::delete_order { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(delete_order.asset, asset);
        assert_eq!(delete_order.order_id, 1);
        assert_eq!(delete_order.price, FixedI64::from(250));
        assert_eq!(delete_order.authority_index, 1);
        assert_eq!(delete_order.validators_len, 5);
        assert_eq!(delete_order.block_num, 1);
    });
}

#[test]
fn offchain_delete_orders_out_of_corridor_when_oracle_price_changed_to_lower() {
    use frame_support::traits::OffchainWorker;

    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        UintAuthorityId::set_all_keys(vec![11, 21, 31, 41, 51]);

        let acc_id = 1;
        let asset = BTC;
        let ask_price = FixedI64::from(20_000);
        let bid_price = FixedI64::from(19_000);
        let expiration_time = 100u64;
        let block = 1;

        let root_origin: Origin = RawOrigin::Root.into();
        let new_asset_corridor: u32 = 200;
        assert_ok!(ModuleDex::update_asset_corridor(
            root_origin,
            asset,
            new_asset_corridor
        ));

        OracleMock::set_price(1u64, asset, ask_price).unwrap();

        assert_ok!(ModuleDex::create_order(
            Origin::signed(acc_id),
            asset,
            Limit {
                price: ask_price,
                expiration_time
            },
            Sell,
            EqFixedU128::from(1),
        ));

        assert_ok!(ModuleDex::create_order(
            Origin::signed(acc_id),
            asset,
            Limit {
                price: bid_price,
                expiration_time
            },
            Buy,
            EqFixedU128::from(1),
        ));

        ModuleDex::offchain_worker(block);
        ModuleSystem::set_block_number(block);

        assert_eq!(state.read().transactions.len(), 0);

        //change oracle price to 18_000
        //we expect that sell order with price 20_000 should be deleted
        //mid_price = min(ask,oracle) + max(bid,oracle) / 2 = (18_000 + 19_000)/ 2 = 18_500
        //corridor should be [17_500, 19_500]

        OracleMock::set_price(1u64, asset, FixedI64::from(18_000)).unwrap();

        let block = 2;
        ModuleDex::offchain_worker(block);
        ModuleSystem::set_block_number(block);

        assert_eq!(state.read().transactions.len(), 1);

        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let delete_order = match ex.call {
            crate::mock::Call::EqDex(crate::Call::delete_order { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(delete_order.asset, BTC);
        assert_eq!(delete_order.order_id, 1);
        assert_eq!(delete_order.price, ask_price);
        assert_eq!(delete_order.authority_index, 1);
        assert_eq!(delete_order.validators_len, 5);
        assert_eq!(delete_order.block_num, 2);
    });
}

#[test]
fn offchain_delete_orders_out_of_corridor_when_oracle_price_changed_to_greater() {
    use frame_support::traits::OffchainWorker;

    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        UintAuthorityId::set_all_keys(vec![11, 21, 31, 41, 51]);

        let acc_id = 1;
        let asset = BTC;
        let ask_price = FixedI64::from(20_000);
        let bid_price = FixedI64::from(19_000);
        let expiration_time = 100u64;
        let block = 1;

        let root_origin: Origin = RawOrigin::Root.into();
        let new_asset_corridor: u32 = 200;
        assert_ok!(ModuleDex::update_asset_corridor(
            root_origin,
            asset,
            new_asset_corridor
        ));

        OracleMock::set_price(1u64, asset, ask_price).unwrap();

        assert_ok!(ModuleDex::create_order(
            Origin::signed(acc_id),
            asset,
            Limit {
                price: ask_price,
                expiration_time
            },
            Sell,
            EqFixedU128::from(1),
        ));

        assert_ok!(ModuleDex::create_order(
            Origin::signed(acc_id),
            asset,
            Limit {
                price: bid_price,
                expiration_time
            },
            Buy,
            EqFixedU128::from(1),
        ));

        ModuleDex::offchain_worker(block);
        ModuleSystem::set_block_number(block);

        assert_eq!(state.read().transactions.len(), 0);

        //change oracle price to 21_000
        //we expect that sell order with price 20_000 should be deleted
        //mid_price = min(ask,oracle) + max(bid,oracle) / 2 = (20_000 + 21_000)/ 2 = 20_500
        //corridor should be [19_500, 21_500]

        OracleMock::set_price(1u64, asset, FixedI64::from(21_000)).unwrap();

        let block = 2;
        ModuleDex::offchain_worker(block);
        ModuleSystem::set_block_number(block);

        assert_eq!(state.read().transactions.len(), 1);

        let transaction = state.write().transactions.pop().unwrap();
        let ex: Extrinsic = Decode::decode(&mut &*transaction).unwrap();
        let delete_order = match ex.call {
            crate::mock::Call::EqDex(crate::Call::delete_order { request, .. }) => request,
            e => panic!("Unexpected call: {:?}", e),
        };

        assert_eq!(delete_order.asset, BTC);
        assert_eq!(delete_order.order_id, 2);
        assert_eq!(delete_order.price, bid_price);
        assert_eq!(delete_order.authority_index, 2);
        assert_eq!(delete_order.validators_len, 5);
        assert_eq!(delete_order.block_num, 2);
    });
}

#[test]
fn cannot_set_corridor_not_from_root() {
    new_test_ext().execute_with(|| {
        let account_id = 1;
        let origin = Origin::signed(account_id);
        let asset = ETH;
        let new_asset_corridor: u32 = 5;

        assert_noop!(
            ModuleDex::update_asset_corridor(origin, asset, new_asset_corridor),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn cannot_create_order_if_price_not_in_corridor() {
    new_test_ext().execute_with(|| {
        let account_id = 1;
        let origin = Origin::signed(account_id);
        let asset = ETH;
        let mut price = FixedI64::from(275);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;
        let root_origin: Origin = RawOrigin::Root.into();
        let new_asset_corridor: u32 = 5;

        OracleMock::set_price(account_id, ETH, price).unwrap();

        let asset_data = AssetData {
            asset_type: AssetType::Physical,
            id: eq_primitives::asset::ETH,
            is_dex_enabled: true,
            price_step: FixedI64::saturating_from_rational(1, 10),
            buyout_priority: 1,
            lot: EqFixedU128::from(1),
            maker_fee: Permill::from_rational(5u32, 10_000u32),
            debt_weight: Permill::zero(),
            taker_fee: Permill::from_rational(1u32, 1000u32),
            asset_xcm_data: eq_primitives::asset::AssetXcmData::None,
            collateral_discount: Percent::one(),
            lending_debt_weight: Permill::one(),
        };

        AssetGetterMock::set_asset_data(asset_data);

        assert_ok!(ModuleDex::update_asset_corridor(
            root_origin,
            asset,
            new_asset_corridor
        ));

        (0..6).for_each(|_| {
            assert_ok!(ModuleDex::create_order(
                origin.clone(),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ));
            price = price - FixedI64::from(5) * FixedI64::saturating_from_rational(1, 10);
        });

        assert_eq!(
            ModuleDex::create_order(
                origin.clone(),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ),
            Err(Error::<Test>::OrderPriceShouldBeInCorridor.into())
        );
    });
}

#[test]
fn create_order_with_zero_corridor() {
    let mut ext = new_test_ext();
    let (offchain, _) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        UintAuthorityId::set_all_keys(vec![11, 21, 31, 41, 51]);

        use frame_support::traits::OffchainWorker;
        let account_id = 1;
        let origin = Origin::signed(account_id);
        let asset = ETH;
        let mut price = FixedI64::from(275);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        OracleMock::set_price(account_id, ETH, price).unwrap();

        let root_origin: Origin = RawOrigin::Root.into();
        let new_asset_corridor: u32 = 0;
        assert_ok!(ModuleDex::update_asset_corridor(
            root_origin,
            asset,
            new_asset_corridor
        ));

        (0..5).for_each(|_| {
            assert_ok!(ModuleDex::create_order(
                origin.clone(),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ));
            price = price + FixedI64::from(1);
        });

        assert_eq!(
            ModuleDex::create_order(
                origin.clone(),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ),
            Err(Error::<Test>::OrderPriceShouldBeInCorridor.into())
        );

        let block = 1;
        ModuleSystem::set_block_number(block);
        ModuleDex::offchain_worker(block);

        // Offchain doesn't delete orders
        assert_eq!(state.read().transactions.len(), 0);
    });
}

#[test]
fn offchain_worker_when_account_with_bad_margin_should_delete_all_account_orders() {
    use frame_support::traits::OffchainWorker;

    let mut ext = new_test_ext();
    let (offchain, _) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        UintAuthorityId::set_all_keys(vec![11, 21, 31, 41, 51]);
        let block_number = 1;
        let prices = &convert_to_prices(&[10, 20, 30, 40]);

        let account_orders: Vec<(u64, Vec<OrderId>)> = (1..100u64)
            .map(|account_id| {
                let borrower_id = SubaccountsManagerMock::create_subaccount_inner(
                    &account_id,
                    &SubAccType::Trader,
                )
                .unwrap();
                assert_ok!(AggregatesMock::set_usergroup(
                    &borrower_id,
                    UserGroup::Borrowers,
                    true
                ));

                let side = if account_id % 2 == 0 { Buy } else { Sell };
                let order_ids = create_orders(&account_id, ETH, side, prices);

                (borrower_id, order_ids)
            })
            .collect();

        let subgood_margin_account = account_orders[0].0;
        MarginCallManagerMock::set_margin_state(
            subgood_margin_account,
            MarginState::SubGood,
            false,
        );

        //make bad margin
        let bad_margin_account_1 = account_orders[10].0;
        MarginCallManagerMock::set_margin_state(
            bad_margin_account_1,
            MarginState::MaintenanceStart,
            false,
        );

        let bad_margin_account_2 = account_orders[11].0;
        MarginCallManagerMock::set_margin_state(
            bad_margin_account_2,
            MarginState::MaintenanceIsGoing,
            false,
        );

        let bad_margin_account_3 = account_orders[12].0;
        MarginCallManagerMock::set_margin_state(
            bad_margin_account_3,
            MarginState::MaintenanceTimeOver,
            false,
        );

        let bad_margin_account_4 = account_orders[13].0;
        MarginCallManagerMock::set_margin_state(
            bad_margin_account_4,
            MarginState::MaintenanceEnd,
            false,
        );

        let bad_margin_account_5 = account_orders[14].0;
        MarginCallManagerMock::set_margin_state(
            bad_margin_account_5,
            MarginState::SubCritical,
            false,
        );

        let mut expected_ids: Vec<OrderId> = (10usize..15usize)
            .flat_map(|index| account_orders[index].1.iter().copied())
            .collect();
        expected_ids.sort();

        ModuleSystem::set_block_number(block_number);
        ModuleDex::offchain_worker(block_number);

        //check that pool contains transactions of bad margin accounts
        assert_eq!(state.read().transactions.len(), expected_ids.len());

        let mut ids: Vec<OrderId> = state
            .read()
            .transactions
            .iter()
            .map(|t| -> OrderId {
                let transaction = &mut &*t.clone();
                let ex: Extrinsic = Decode::decode(transaction).unwrap();
                match ex.call {
                    crate::mock::Call::EqDex(crate::Call::delete_order { request, .. }) => {
                        request.order_id
                    }
                    e => panic!("Unexpected call: {:?}", e),
                }
            })
            .collect();
        ids.sort();

        assert_eq!(ids.cmp(&expected_ids), core::cmp::Ordering::Equal);
    });
}

#[test]
fn offchain_worker_when_bad_margin_order_and_out_of_corridor_order() {
    //offchain should submit only unique transactions

    use frame_support::traits::OffchainWorker;

    let mut ext = new_test_ext();
    let (offchain, _state) = TestOffchainExt::new();
    let (pool, state) = TestTransactionPoolExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        UintAuthorityId::set_all_keys(vec![11, 21, 31, 41, 51]);

        let acc_id = 1;
        let asset = ETH;
        let mut price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;
        let block = 1;

        let _order_ids: Vec<OrderId> = (0..7)
            .map(|_| {
                assert_ok!(ModuleDex::create_order(
                    Origin::signed(acc_id),
                    asset,
                    Limit {
                        price,
                        expiration_time
                    },
                    side,
                    amount,
                ));
                price = price + FixedI64::from(5);

                OrderIdCounter::<Test>::get()
            })
            .collect();

        let acc_2 = 2;
        let borrower_id_2 =
            SubaccountsManagerMock::create_subaccount_inner(&acc_2, &SubAccType::Trader).unwrap();
        assert_ok!(ModuleDex::create_order(
            Origin::signed(acc_2),
            asset,
            Limit {
                price: FixedI64::from(250),
                expiration_time
            },
            side,
            amount,
        ));

        let root_origin: Origin = RawOrigin::Root.into();
        let new_asset_corridor: u32 = 5;
        assert_ok!(ModuleDex::update_asset_corridor(
            root_origin,
            asset,
            new_asset_corridor
        ));

        MarginCallManagerMock::set_margin_state(
            borrower_id_2,
            MarginState::MaintenanceStart,
            false,
        );

        assert_ok!(AggregatesMock::set_usergroup(
            &borrower_id_2,
            UserGroup::Borrowers,
            true
        ));

        ModuleSystem::set_block_number(block);
        ModuleDex::offchain_worker(block);

        assert_eq!(state.read().transactions.len(), 2);
    });
}

#[test]
fn create_order() {
    new_test_ext().execute_with(|| {
        let account_id = 1;

        let origin = Origin::signed(account_id);
        let borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&account_id, &SubAccType::Trader)
                .unwrap();

        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;
        let price_step = FixedI64::from(1);

        assert_ok!(ModuleDex::create_order(
            origin,
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));

        let order_id = OrderIdCounter::<Test>::get();
        let maybe_order = ModuleDex::find_order(&asset, order_id, price);
        assert!(maybe_order.is_some());
        let order = maybe_order.unwrap();

        assert_eq!(order.account_id, borrower_id);
        assert_eq!(order.price, price);
        assert_eq!(order.side, side);
        assert_eq!(order.amount, amount);
        assert_eq!(order.expiration_time, expiration_time);
        assert_eq!(order.order_id, order_id);

        let chunk_key = ModuleDex::get_chunk_key(price, price_step).unwrap();
        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert_eq!(chunks[0], chunk_key);
    });
}

#[test]
fn create_order_when_orders_has_same_price_chunks_should_be_sorted_by_create_time() {
    new_test_ext().execute_with(|| {
        let account_1 = 1u64;
        let origin_1 = Origin::signed(account_1);
        SubaccountsManagerMock::create_subaccount_inner(&account_1, &SubAccType::Trader).unwrap();

        let account_2 = 2u64;
        let origin_2 = Origin::signed(account_2);
        SubaccountsManagerMock::create_subaccount_inner(&account_2, &SubAccType::Trader).unwrap();

        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;
        let price_step = FixedI64::from(1);

        let first_order_create_time = ModuleTimestamp::get();
        (0..5).for_each(|_| {
            assert_ok!(ModuleDex::create_order(
                origin_1.clone(),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ));
        });

        ModuleSystem::set_block_number(2000);
        ModuleTimestamp::set_timestamp(first_order_create_time + 2000);

        println!("{:?}", ModuleTimestamp::now());

        let second_order_create_time = ModuleTimestamp::get();
        assert!(first_order_create_time < second_order_create_time);

        assert!(ModuleDex::create_order(
            origin_2,
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        )
        .is_ok());
        let last_order_id = OrderIdCounter::<Test>::get();

        let chunk_key = ModuleDex::get_chunk_key(price, price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert_eq!(orders.len(), 6);
        assert_eq!(orders[orders.len() - 1].order_id, last_order_id);
    });
}

#[test]
fn create_order_when_orders_with_same_price_and_create_time_should_be_sorted_by_order_id() {
    new_test_ext().execute_with(|| {
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;
        let steps = 5u64;

        (0..steps).for_each(|_| {
            assert_ok!(ModuleDex::create_order(
                Origin::signed(1),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ));
        });

        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert_eq!(chunks.len(), 1);

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunks[0]);

        (0..steps).for_each(|i| {
            let expected_order_id = i + 1;
            assert_eq!(orders[i as usize].order_id, expected_order_id);
        })
    });
}

#[test]
fn create_order_chunks_should_be_ordered_by_price() {
    new_test_ext().execute_with(|| {
        let who = 101u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        let mut price = FixedI64::from(250);
        let mut side: OrderSide;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        for i in 0..100 {
            if i % 2 == 0 {
                side = Buy;
            } else {
                side = Sell;
                price = price + FixedI64::from(1);
            };

            assert_ok!(ModuleDex::create_limit_order(
                who,
                asset,
                price,
                side,
                amount,
                expiration_time,
                &asset_data
            ));
        }

        let chunks = ActualChunksByAsset::<Test>::get(asset);
        for chunk_key in chunks.into_iter() {
            let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
            let mut prev_order = orders[0].clone();
            orders.into_iter().for_each(|o| {
                assert!(o.price >= prev_order.price);
                prev_order = o.clone();
            });
        }
    });
}

#[test]
fn create_order_should_update_best_price() {
    new_test_ext().execute_with(|| {
        let asset = ETH;
        let prices: Vec<FixedI64> =
            convert_to_prices(&[12, 17, 1, 3, 34, 3, 5, 7, 19, 100, 250, 50, 17, 17, 25]);
        let account_id = 1u64;

        let best_price = BestPriceByAsset::<Test>::get(asset);
        assert_eq!(best_price, Default::default());

        let min_price = *prices.iter().min().unwrap();
        let max_price = *prices.iter().max().unwrap();

        create_orders(&account_id, asset, Sell, &prices);

        let best_price = BestPriceByAsset::<Test>::get(asset);
        assert_eq!(best_price.ask, Some(min_price));

        create_orders(&account_id, asset, Buy, &prices);
        let best_price = BestPriceByAsset::<Test>::get(asset);
        assert_eq!(
            best_price,
            BestPrice {
                ask: Some(min_price),
                bid: Some(max_price)
            }
        );
    });
}

#[test]
fn create_order_when_account_is_not_borrower_should_fail() {
    new_test_ext().execute_with(|| {
        let not_borrower_origin = Origin::signed(0u64);
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        assert_eq!(
            ModuleDex::create_order(
                not_borrower_origin,
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ),
            Err(Error::<Test>::AccountIsNotTrader.into())
        );
    });
}

#[test]
fn create_order_when_negative_or_zero_price_should_fail() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let asset = ETH;
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        assert_eq!(
            ModuleDex::create_order(
                origin.clone(),
                asset,
                Limit {
                    price: FixedI64::from(0),
                    expiration_time
                },
                side,
                amount,
            ),
            Err(Error::<Test>::OrderPriceShouldBePositive.into())
        );

        assert_eq!(
            ModuleDex::create_order(
                origin.clone(),
                asset,
                Limit {
                    price: FixedI64::from(-1),
                    expiration_time
                },
                side,
                amount,
            ),
            Err(Error::<Test>::OrderPriceShouldBePositive.into())
        );
    })
}

#[test]
fn create_order_when_asset_not_exists_should_fail() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let not_exists_asset = DAI;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        assert_eq!(
            ModuleDex::create_order(
                origin,
                not_exists_asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ),
            Err(eq_assets::Error::<Test>::AssetNotExists.into())
        );
    });
}

#[test]
fn create_order_when_market_and_no_best_price_should_fail() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let asset = ETH;
        let side = Buy;
        let amount = EqFixedU128::from(1);

        assert_err!(
            ModuleDex::create_order(origin, asset, OrderType::Market, side, amount),
            Error::<Test>::NoBestPriceForMarketOrder
        );
    })
}

#[test]
fn create_order_when_bad_margin_should_fail() {
    new_test_ext().execute_with(|| {
        let account_id = 21u64;
        let origin = Origin::signed(21u64);
        let borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&account_id, &SubAccType::Trader)
                .unwrap();

        let asset = ETH;
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        MarginCallManagerMock::set_margin_state(borrower_id, MarginState::SubGood, false);

        assert_err!(
            ModuleDex::create_order(
                origin,
                asset,
                Limit {
                    price: FixedI64::from(10),
                    expiration_time
                },
                side,
                amount,
            ),
            Error::<Test>::BadMargin
        );
    })
}

#[test]
fn create_order_when_check_margin_fail_should_fail() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(22u64);
        let asset = ETH;
        let side = Buy;
        let amount = EqFixedU128::saturating_from_rational(5, 100);
        let expiration_time = 100u64;

        assert!(ModuleDex::create_order(
            origin,
            asset,
            Limit {
                price: FixedI64::from(10),
                expiration_time
            },
            side,
            amount,
        )
        .is_err());
    })
}

#[test]
fn create_order_when_amount_is_zero_should_fail() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::zero();
        let expiration_time = 100u64;

        assert_eq!(
            ModuleDex::create_order(
                origin,
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ),
            Err(Error::<Test>::OrderAmountShouldBePositive.into())
        );
    })
}

#[test]
fn create_order_best_price_should_match_best_price_in_chunks() {
    new_test_ext().execute_with(|| {
        let who = 1u64;
        let asset = ETH;

        let sell_prices = convert_to_prices(&[229, 250, 251, 230, 245, 246, 247, 248, 249]);
        let buy_prices = convert_to_prices(&[228, 227, 226, 225, 224, 210, 207, 200, 198]);

        create_orders(&who, asset, Sell, &sell_prices);
        create_orders(&who, asset, Buy, &buy_prices);

        let chunks = ActualChunksByAsset::<Test>::get(asset);

        let best_price_by_chunks = chunks
            .iter()
            .flat_map(|&c| OrdersByAssetAndChunkKey::<Test>::get(asset, c))
            .fold(BestPrice::default(), |mut acc, o| {
                match o.side {
                    Buy => acc.bid = acc.bid.max(Some(o.price)),
                    Sell => acc.ask = acc.ask.map_or(Some(o.price), |a| Some(a.min(o.price))),
                }

                acc
            });

        let best_price = BestPriceByAsset::<Test>::get(asset);

        assert_eq!(best_price.ask, best_price_by_chunks.ask);
        assert_eq!(best_price.bid, best_price_by_chunks.bid);
    });
}

#[test]
fn create_order_when_amount_not_satisfies_lot_should_fail() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::saturating_from_rational(9, 10);
        let expiration_time = 100u64;

        assert_err!(
            ModuleDex::create_order(
                origin.clone(),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ),
            Error::<Test>::OrderAmountShouldSatisfyLot
        );

        let mut asset_data = ASSET_DATA.with(|v| v.borrow().clone());
        asset_data.lot = EqFixedU128::saturating_from_rational(1, 10);
        AssetGetterMock::set_asset_data(asset_data);

        assert_ok!(ModuleDex::create_order(
            origin,
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));
    });
}

#[test]
fn create_order_when_price_not_satisfies_price_step_should_fail() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        let origin = Origin::signed(account_id);
        let asset = ETH;
        let mut price = FixedI64::saturating_from_rational(500005, 10);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        OracleMock::set_price(account_id, ETH, FixedI64::from(50000)).unwrap();

        assert_err!(
            ModuleDex::create_order(
                origin.clone(),
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ),
            Error::<Test>::OrderPriceShouldSatisfyPriceStep
        );

        price = FixedI64::from(50000);

        assert_ok!(ModuleDex::create_order(
            origin,
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));
    });
}

#[test]
fn create_order_when_dex_is_disabled_should_fail() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        let mut asset_data = ASSET_DATA.with(|v| v.borrow().clone());
        asset_data.is_dex_enabled = false;
        AssetGetterMock::set_asset_data(asset_data);

        assert_err!(
            ModuleDex::create_order(
                origin,
                asset,
                Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ),
            Error::<Test>::DexIsDisabledForAsset
        );
    });
}

#[test]
fn delete_order() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;
        let price_step = FixedI64::from(1);

        assert_ok!(ModuleDex::create_order(
            origin.clone(),
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));
        let order_id = OrderIdCounter::<Test>::get();
        let order = ModuleDex::find_order(&asset, order_id, price);
        assert!(order.is_some());
        let order = order.unwrap();

        assert_ok!(ModuleDex::delete_order_external(
            origin.clone(),
            asset,
            order.order_id,
            price
        ));

        //check that order not exists in storage after delete
        let order = ModuleDex::find_order(&asset, order_id, price);
        assert!(order.is_none());

        let chunk_key = ModuleDex::get_chunk_key(price, price_step).unwrap();
        let chunk = ActualChunksByAsset::<Test>::get(asset);
        assert_eq!(chunk.len(), 0);
        assert!(!OrdersByAssetAndChunkKey::<Test>::contains_key(
            asset, chunk_key
        ));
    });
}

#[test]
fn delete_order_when_account_is_not_owner_should_fail() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        assert_ok!(ModuleDex::create_order(
            origin,
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));
        let order_id = OrderIdCounter::<Test>::get();

        let not_owner_account_id = 12u64;
        let not_owner_origin = Origin::signed(not_owner_account_id);
        SubaccountsManagerMock::create_subaccount_inner(&not_owner_account_id, &SubAccType::Trader)
            .unwrap();

        assert_err!(
            ModuleDex::delete_order_external(not_owner_origin, asset, order_id, price),
            Error::<Test>::OnlyOwnerCanRemoveOrder
        );
    });
}

#[test]
fn delete_order_by_root_should_success() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        assert!(ModuleDex::create_order(
            origin.clone(),
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        )
        .is_ok());
        let order_id = OrderIdCounter::<Test>::get();

        let root_origin: Origin = RawOrigin::Root.into();
        assert!(ModuleDex::delete_order_external(root_origin, asset, order_id, price).is_ok());
    });
}

#[test]
fn delete_order_should_update_best_prices() {
    fn assert_best_price(asset: Asset, side: OrderSide, expected: Option<FixedI64>) {
        let best_prices = BestPriceByAsset::<Test>::get(asset);
        assert_eq!(
            if side == Sell {
                best_prices.ask
            } else {
                best_prices.bid
            },
            expected
        );
    }

    /// iterate by ids and prices, delete order and check best price
    fn assert_after_delete_order(
        origin: &Origin,
        asset: Asset,
        side: OrderSide,
        ids: &[u64],
        prices: &[Price],
    ) {
        let (last_order_id, last_price) = ids.iter().zip(prices).rev().fold(
            (Option::<u64>::None, Option::<FixedI64>::None),
            |prev, (&order_id, &price)| match prev {
                (Some(prev_order_id), Some(prev_price)) => {
                    assert!(ModuleDex::delete_order_external(
                        origin.clone(),
                        asset,
                        prev_order_id,
                        prev_price
                    )
                    .is_ok());
                    assert_best_price(asset, side, Some(price));

                    (Some(order_id), Some(price))
                }
                _ => (Some(order_id), Some(price)),
            },
        );

        assert_ok!(ModuleDex::delete_order_external(
            origin.clone(),
            asset,
            last_order_id.unwrap(),
            last_price.unwrap()
        ));
        assert_best_price(asset, side, None);
    }

    new_test_ext().execute_with(|| {
        //ask and bid prices should be sorted

        let ask_prices = convert_to_prices(&[
            1000, 251, 250, 249, 248, 230, 230, 229, 100, 53, 25, 24, 11, 5,
        ]);
        let best_ask = ask_prices.iter().min().unwrap();
        let bid_prices = convert_to_prices(&[
            1, 12, 16, 33, 45, 225, 226, 227, 228, 229, 231, 231, 500, 501, 553, 575,
        ]);
        let best_bid = bid_prices.iter().max().unwrap();

        let who = 1u64;
        let origin = Origin::signed(who);
        let asset = ETH;

        let sell_orders = create_orders(&who, asset, Sell, &ask_prices);
        let buy_orders = create_orders(&who, asset, Buy, &bid_prices);
        assert_best_price(asset, Sell, Some(*best_ask));
        assert_best_price(asset, Buy, Some(*best_bid));

        assert_after_delete_order(&origin, asset, Sell, &sell_orders, &ask_prices);
        assert_after_delete_order(&origin, asset, Buy, &buy_orders, &bid_prices);
    });
}

#[test]
fn delete_order_should_update_chunks() {
    new_test_ext().execute_with(|| {
        let sell_prices = convert_to_prices(&[
            1000, 500, 300, 251, 250, 249, 248, 230, 230, 229, 100, 53, 25, 24, 11, 5,
        ]);
        let buy_prices = convert_to_prices(&[
            1, 12, 16, 33, 45, 225, 226, 227, 228, 229, 231, 231, 500, 501, 553, 575,
        ]);

        let who = 1u64;
        let origin = Origin::signed(who);
        let asset = ETH;

        create_orders(&who, asset, Sell, &sell_prices);
        create_orders(&who, asset, Buy, &buy_prices);

        let chunks = ActualChunksByAsset::<Test>::get(asset);
        chunks.iter().for_each(|&c| {
            OrdersByAssetAndChunkKey::<Test>::get(asset, c)
                .iter()
                .for_each(|o| {
                    assert_ok!(ModuleDex::delete_order_external(
                        origin.clone(),
                        asset,
                        o.order_id,
                        o.price
                    ));
                });

            assert!(!OrdersByAssetAndChunkKey::<Test>::contains_key(asset, c));
            assert!(!ActualChunksByAsset::<Test>::get(asset)
                .iter()
                .any(|&ac| ac == c));
        });
    });
}

#[test]
fn delete_order_when_order_not_found_should_fail() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1u64);
        let asset = ETH;
        let price = FixedI64::from(250);
        let order_id = 1;

        assert_err!(
            ModuleDex::delete_order_external(origin, asset, order_id, price),
            Error::<Test>::OrderNotFound
        );
    });
}

#[test]
fn get_order_should_return_order_if_exists() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1);
        let asset = ETH;
        let price = FixedI64::from(250);

        assert_ok!(ModuleDex::create_order(
            origin,
            asset,
            Limit {
                price,
                expiration_time: 100u64
            },
            Buy,
            EqFixedU128::from(1),
        ));

        let order_id = OrderIdCounter::<Test>::get();
        let maybe_order = ModuleDex::find_order(&asset, order_id, price);
        assert!(maybe_order.is_some());

        let wrong_price = price
            .checked_add(&FixedI64::saturating_from_rational(1, 100))
            .unwrap();
        let maybe_order = ModuleDex::find_order(&asset, order_id, wrong_price);
        assert!(maybe_order.is_none());

        let wrong_order_id = order_id + 1;
        let maybe_order = ModuleDex::find_order(&asset, wrong_order_id, price);
        assert!(maybe_order.is_none());

        let wrong_asset = EQD;
        let maybe_order = ModuleDex::find_order(&wrong_asset, order_id, price);
        assert!(maybe_order.is_none());
    });
}

#[test]
fn get_chunk_key_should_return_chunk_key() {
    new_test_ext().execute_with(|| {
        let price_step = FixedI64::from(2);
        // 250 / (5*2) = 10
        assert_eq!(
            ModuleDex::get_chunk_key(FixedI64::from(250), price_step),
            Ok(25u64)
        );

        // 25 / (5 * 2) = 2
        assert_eq!(
            ModuleDex::get_chunk_key(FixedI64::from(25), price_step),
            Ok(2u64)
        );
        assert_eq!(
            ModuleDex::get_chunk_key(FixedI64::from(26), price_step),
            Ok(2u64)
        );
        assert_eq!(
            ModuleDex::get_chunk_key(FixedI64::from(27), price_step),
            Ok(2u64)
        );
        assert_eq!(
            ModuleDex::get_chunk_key(FixedI64::from(1), price_step),
            Ok(0u64)
        );
        assert_eq!(
            ModuleDex::get_chunk_key(FixedI64::saturating_from_rational(2, 3), price_step),
            Ok(0u64)
        );
        assert_eq!(
            ModuleDex::get_chunk_key(FixedI64::saturating_from_rational(251, 10), price_step),
            Ok(2u64)
        );
    })
}

#[test]
fn get_chunk_when_price_step_or_price_step_count_is_zero_should_fail() {
    new_test_ext().execute_with(|| {
        let price_step = FixedI64::zero();

        assert_err!(
            ModuleDex::get_chunk_key(FixedI64::from(250), price_step),
            Error::<Test>::PriceStepShouldBePositive
        );
    });
}

#[test]
fn get_order_id_should_increment_order_counter() {
    new_test_ext().execute_with(|| {
        let current_order_id = OrderIdCounter::<Test>::get();
        (0..5).for_each(|_| {
            ModuleDex::get_order_id();
        });
        assert_eq!(current_order_id + 5, OrderIdCounter::<Test>::get());
    });
}

#[test]
fn create_order_should_increase_asset_weight() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        let origin = Origin::signed(account_id);
        let borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&account_id, &SubAccType::Trader)
                .unwrap();
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        let weight_before = AssetWeightByAccountId::<Test>::get(borrower_id);
        assert_eq!(weight_before.len(), 0);

        assert_ok!(ModuleDex::create_order(
            origin.clone(),
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));

        let weight_after = AssetWeightByAccountId::<Test>::get(borrower_id);
        assert_eq!(weight_after.len(), 1);
        let price = price.try_into().expect("Positive");
        let expected_aggregate = OrderAggregateBySide::new(amount, price, side).unwrap();

        assert_eq!(weight_after.get(&asset).unwrap(), &expected_aggregate);

        //create another order with same asset, but different price
        let amount_2 = EqFixedU128::one();
        let price_2 = FixedI64::from(255);

        assert_ok!(ModuleDex::create_order(
            origin,
            asset,
            Limit {
                price: price_2,
                expiration_time
            },
            side,
            amount_2,
        ));

        let weight_after = AssetWeightByAccountId::<Test>::get(borrower_id);
        assert_eq!(weight_after.len(), 1);

        let price_2 = price_2.try_into().expect("Positive");
        let mut expected_aggregate = OrderAggregateBySide::new(amount, price, side).unwrap();
        expected_aggregate.add(amount_2, price_2, side);

        assert_eq!(weight_after.get(&asset).unwrap(), &expected_aggregate);
    });
}

#[test]
fn delete_order_should_decrease_asset_weight() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        let origin = Origin::signed(account_id);
        let borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&account_id, &SubAccType::Trader)
                .unwrap();
        let asset = ETH;
        let price_1 = FixedI64::from(250);
        let side = Buy;
        let amount_1 = EqFixedU128::from(1);
        let expiration_time = 100u64;

        assert_ok!(ModuleDex::create_order(
            origin.clone(),
            asset,
            Limit {
                price: price_1,
                expiration_time
            },
            side,
            amount_1,
        ));
        let order_id_1 = OrderIdCounter::<Test>::get();

        let price_2 = FixedI64::from(250);
        let amount_2 = EqFixedU128::from(1);

        assert_ok!(ModuleDex::create_order(
            origin.clone(),
            asset,
            Limit {
                price: price_2,
                expiration_time
            },
            side,
            amount_2,
        ));
        let order_id_2 = OrderIdCounter::<Test>::get();

        let weights = AssetWeightByAccountId::<Test>::get(borrower_id);
        assert_eq!(weights.len(), 1);
        assert_eq!(
            weights.get(&asset).unwrap(),
            &OrderAggregateBySide {
                buy: OrderAggregate {
                    amount: amount_1 + amount_2,
                    amount_by_price: amount_1 * price_1.try_into().expect("Positive")
                        + amount_2 * price_2.try_into().expect("Positive"),
                },
                sell: Default::default()
            }
        );

        assert_ok!(ModuleDex::delete_order_external(
            origin.clone(),
            asset,
            order_id_1,
            price_1,
        ));

        let weights = AssetWeightByAccountId::<Test>::get(borrower_id);
        assert_eq!(weights.len(), 1);
        assert_eq!(
            weights.get(&asset).unwrap(),
            &OrderAggregateBySide::new(amount_2, price_2.try_into().expect("Positive"), side)
                .unwrap()
        );

        assert_ok!(ModuleDex::delete_order_external(
            origin.clone(),
            asset,
            order_id_2,
            price_2,
        ));

        let weights = AssetWeightByAccountId::<Test>::get(borrower_id);
        assert_eq!(weights.len(), 0);
    });
}

#[test]
fn update_asset_weight() {
    new_test_ext().execute_with(|| {
        let subaccount_id = 1u64;
        let asset = ETH;
        let amount = EqFixedU128::one();
        let price = FixedI64::one();

        assert_ok!(ModuleDex::update_asset_weight(
            subaccount_id,
            asset,
            amount,
            price,
            OrderSide::Sell,
            Operation::Increase
        ));

        let asset_weights = AssetWeightByAccountId::<Test>::get(subaccount_id);
        assert_eq!(asset_weights.len(), 1);
        assert_eq!(
            asset_weights.get(&asset).unwrap(),
            &OrderAggregateBySide::new(
                amount,
                price.try_into().expect("Positive"),
                OrderSide::Sell
            )
            .unwrap()
        );

        assert_ok!(ModuleDex::update_asset_weight(
            subaccount_id,
            asset,
            amount,
            price,
            OrderSide::Sell,
            Operation::Decrease
        ));

        let asset_weights = AssetWeightByAccountId::<Test>::get(subaccount_id);
        assert_eq!(asset_weights.len(), 0);
    });
}

#[test]
fn update_asset_weight_when_overflow_should_fail() {
    new_test_ext().execute_with(|| {
        let subaccount_id = 1u64;
        let asset = ETH;
        let amount = EqFixedU128::max_value();
        let price = FixedI64::max_value();

        assert_err!(
            ModuleDex::update_asset_weight(
                subaccount_id,
                asset,
                amount,
                price,
                OrderSide::Sell,
                Operation::Increase
            ),
            ArithmeticError::Overflow
        );
    });
}

#[test]
fn update_asset_weight_when_price_is_negative_should_fail() {
    new_test_ext().execute_with(|| {
        let subaccount_id = 1u64;
        let asset = ETH;
        let amount = EqFixedU128::one();
        let price = FixedI64::from(-1);

        assert_err!(
            ModuleDex::update_asset_weight(
                subaccount_id,
                asset,
                amount,
                price,
                OrderSide::Sell,
                Operation::Increase
            ),
            Error::<Test>::OrderPriceShouldBePositive
        );
    });
}

#[test]
fn update_asset_weight_when_decrease_not_exists_asset_should_fail() {
    new_test_ext().execute_with(|| {
        let subaccount_id = 1u64;
        let asset = ETH;
        let amount = EqFixedU128::one();
        let price = FixedI64::one();

        assert_err!(
            ModuleDex::update_asset_weight(
                subaccount_id,
                asset,
                amount,
                price,
                OrderSide::Sell,
                Operation::Decrease
            ),
            ArithmeticError::Underflow
        );
    });
}

#[test]
fn get_asset_weights() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;
        let origin = Origin::signed(account_id);
        let borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&account_id, &SubAccType::Trader)
                .unwrap();
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        let weight_before = AssetWeightByAccountId::<Test>::get(borrower_id);
        assert_eq!(weight_before.len(), 0);

        assert_ok!(ModuleDex::create_order(
            origin.clone(),
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));

        let weight_after = AssetWeightByAccountId::<Test>::get(borrower_id);
        assert_eq!(weight_after.len(), 1);

        let order_weights = ModuleDex::get_asset_weights(&borrower_id);
        assert!(!order_weights.is_empty());
    });
}

#[test]
fn get_asset_weights_when_account_doesnt_have_borrower_should_return_none() {
    new_test_ext().execute_with(|| {
        let not_a_borrower = 0u64;

        let order_weights = ModuleDex::get_asset_weights(&not_a_borrower);
        assert!(order_weights.is_empty());
    });
}

#[test]
fn get_asset_weights_when_asset_weights_not_exists_should_return_none() {
    new_test_ext().execute_with(|| {
        let account_id = 1u64;

        let weight_after = AssetWeightByAccountId::<Test>::get(account_id);
        assert_eq!(weight_after.len(), 0);

        let order_weights = ModuleDex::get_asset_weights(&account_id);
        assert!(order_weights.is_empty());
    });
}

#[test]
fn find_order() {
    new_test_ext().execute_with(|| {
        let account_id = 1;

        let origin = Origin::signed(account_id);
        let asset = ETH;
        let price = FixedI64::from(250);
        let side = Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        assert_ok!(ModuleDex::create_order(
            origin.clone(),
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));

        assert_ok!(ModuleDex::create_order(
            origin,
            asset,
            Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));

        let order_id = OrderIdCounter::<Test>::get();

        assert!(ModuleDex::find_order(&asset, order_id, price).is_some());
        assert!(ModuleDex::find_order(&EQD, order_id, price).is_none());
        assert!(ModuleDex::find_order(&asset, order_id, price + FixedI64::from(1)).is_none());
        assert!(ModuleDex::find_order(&asset, order_id + 1, price).is_none());
    });
}

#[test]
fn two_orders_match_total_with_no_rest_taker_buy() {
    new_test_ext().execute_with(|| {
        let maker = 101_u64;
        let taker = 102_u64;

        let asset = ETH;
        let maker_asset_balance: Balance = 250_000_000_000;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        assert_ok!(ModuleBalances::deposit_creating(
            &maker,
            asset,
            maker_asset_balance,
            true,
            None
        ));

        let taker_usd_balance: Balance = 260_000_000_000;
        assert_ok!(ModuleBalances::deposit_creating(
            &taker,
            EQD,
            taker_usd_balance,
            true,
            None
        ));

        let maker_price = FixedI64::saturating_from_integer(250);
        let side = OrderSide::Sell;
        let maker_amount = EqFixedU128::saturating_from_integer(1);
        let expiration_time = 100u64;
        assert_ok!(ModuleDex::create_limit_order(
            maker,
            asset,
            maker_price,
            side,
            maker_amount,
            expiration_time,
            &asset_data
        ));

        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 1);
        let maker_order = &orders[0];

        let taker_amount = maker_amount;
        let taker_price = maker_price + FixedI64::one();
        let taker_side = OrderSide::Buy;

        assert_eq!(
            ModuleDex::match_two_orders(
                &taker,
                taker_amount,
                Limit {
                    price: taker_price,
                    expiration_time: 0
                },
                taker_side,
                maker_order,
                &asset
            ),
            Ok(taker_amount)
        );
        // maker
        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);
        // taker
        let chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);
        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert!(chunks.len() == 0);

        assert_eq!(
            ModuleBalances::get_balance(&maker, &asset),
            SignedBalance::Positive(
                maker_asset_balance - balance_from_eq_fixedu128::<Balance>(maker_amount).unwrap()
            )
        );

        assert_eq!(
            ModuleBalances::get_balance(&taker, &asset),
            SignedBalance::Positive(balance_from_eq_fixedu128(maker_amount).unwrap())
        );

        let expected_maker_usd = maker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() - asset_data.maker_fee.into());
        let expected_maker_usd: Balance = balance_from_eq_fixedu128(expected_maker_usd).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&maker, &EQD),
            SignedBalance::Positive(expected_maker_usd)
        );

        let expected_taker_usd_pay = maker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() + asset_data.taker_fee.into());
        let expected_taker_usd_pay: Balance =
            balance_from_eq_fixedu128(expected_taker_usd_pay).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&taker, &EQD),
            SignedBalance::Positive(taker_usd_balance - expected_taker_usd_pay)
        );
    });
}

#[test]
fn two_orders_match_total_with_no_rest_taker_sell() {
    new_test_ext().execute_with(|| {
        let maker = 101_u64;
        let taker = 102_u64;

        let asset = ETH;
        let taker_asset_balance: Balance = 250_000_000_000;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        assert_ok!(ModuleBalances::deposit_creating(
            &taker,
            asset,
            taker_asset_balance,
            true,
            None
        ));

        let maker_usd_balance: Balance = 260_000_000_000;
        assert_ok!(ModuleBalances::deposit_creating(
            &maker,
            EQD,
            maker_usd_balance,
            true,
            None
        ));

        let maker_price = FixedI64::saturating_from_integer(250);
        let side = OrderSide::Buy;
        let maker_amount = EqFixedU128::saturating_from_integer(1);
        let expiration_time = 100u64;
        assert_ok!(ModuleDex::create_limit_order(
            maker,
            asset,
            maker_price,
            side,
            maker_amount,
            expiration_time,
            &asset_data
        ));

        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 1);
        let maker_order = &orders[0];

        let taker_amount = maker_amount;
        let taker_price = maker_price - FixedI64::one();
        let taker_side = OrderSide::Sell;

        assert_eq!(
            ModuleDex::match_two_orders(
                &taker,
                taker_amount,
                Limit {
                    price: taker_price,
                    expiration_time: 0
                },
                taker_side,
                maker_order,
                &asset
            ),
            Ok(taker_amount)
        );
        // maker
        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);
        // taker
        let chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);
        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert!(chunks.len() == 0);

        assert_eq!(
            ModuleBalances::get_balance(&taker, &asset),
            SignedBalance::Positive(
                taker_asset_balance - balance_from_eq_fixedu128::<Balance>(maker_amount).unwrap()
            )
        );

        assert_eq!(
            ModuleBalances::get_balance(&maker, &asset),
            SignedBalance::Positive(balance_from_eq_fixedu128(maker_amount).unwrap())
        );

        let expected_taker_usd = maker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() - asset_data.taker_fee.into());
        let expected_taker_usd = balance_from_eq_fixedu128(expected_taker_usd).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&taker, &EQD),
            SignedBalance::Positive(expected_taker_usd)
        );

        let expected_maker_usd_pay = maker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() + asset_data.maker_fee.into());
        let expected_maker_usd_pay =
            balance_from_eq_fixedu128::<Balance>(expected_maker_usd_pay).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&maker, &EQD),
            SignedBalance::Positive(maker_usd_balance - expected_maker_usd_pay)
        );
    });
}

#[test]
fn two_orders_match_successful_with_taker_rest_taker_buy() {
    new_test_ext().execute_with(|| {
        let maker = 101_u64;
        let taker = 102_u64;

        let asset = ETH;
        let maker_asset_balance: Balance = 250_000_000_000;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        assert_ok!(ModuleBalances::deposit_creating(
            &maker,
            asset,
            maker_asset_balance,
            true,
            None
        ));

        let taker_usd_balance: Balance = 510_000_000_000;
        assert_ok!(ModuleBalances::deposit_creating(
            &taker,
            EQD,
            taker_usd_balance,
            true,
            None
        ));

        let maker_price = FixedI64::saturating_from_integer(250);
        let side = OrderSide::Sell;
        let maker_amount = EqFixedU128::saturating_from_integer(1);
        let expiration_time = 100u64;
        assert_ok!(ModuleDex::create_limit_order(
            maker,
            asset,
            maker_price,
            side,
            maker_amount,
            expiration_time,
            &asset_data
        ));

        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 1);
        let maker_order = &orders[0];

        let taker_amount = maker_amount * EqFixedU128::saturating_from_integer(2);
        let taker_price = maker_price + FixedI64::one();
        let taker_side = OrderSide::Buy;
        let expected_amount = maker_amount;
        assert_eq!(
            ModuleDex::match_two_orders(
                &taker,
                taker_amount,
                Limit {
                    price: taker_price,
                    expiration_time: 0
                },
                taker_side,
                maker_order,
                &asset
            ),
            Ok(expected_amount)
        );
        // maker order
        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        let order_idx = orders.binary_search_by(|o| o.order_id.cmp(&maker_order.order_id));
        assert!(order_idx.is_err());
        assert_err!(order_idx, 0_usize);
        // taker order
        let chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);

        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert!(chunks.len() == 0);

        assert_eq!(
            ModuleBalances::get_balance(&maker, &asset),
            SignedBalance::Positive(
                maker_asset_balance - balance_from_eq_fixedu128::<Balance>(maker_amount).unwrap()
            )
        );

        assert_eq!(
            ModuleBalances::get_balance(&taker, &asset),
            SignedBalance::Positive(balance_from_eq_fixedu128(maker_amount).unwrap())
        );

        let expected_maker_usd = maker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() - asset_data.maker_fee.into());
        let expected_maker_usd: Balance = balance_from_eq_fixedu128(expected_maker_usd).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&maker, &EQD),
            SignedBalance::Positive(expected_maker_usd)
        );

        let expected_taker_usd_pay = maker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() + asset_data.taker_fee.into());
        let expected_taker_usd_pay: Balance =
            balance_from_eq_fixedu128(expected_taker_usd_pay).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&taker, &EQD),
            SignedBalance::Positive(taker_usd_balance - expected_taker_usd_pay)
        );
    });
}

#[test]
fn two_orders_match_successful_with_taker_rest_taker_sell() {
    new_test_ext().execute_with(|| {
        let maker = 101_u64;
        let taker = 102_u64;

        let asset = ETH;
        let taker_asset_balance: Balance = 250_000_000_000;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        assert_ok!(ModuleBalances::deposit_creating(
            &taker,
            asset,
            taker_asset_balance,
            true,
            None
        ));

        let maker_usd_balance: Balance = 510_000_000_000;
        assert_ok!(ModuleBalances::deposit_creating(
            &maker,
            EQD,
            maker_usd_balance,
            true,
            None
        ));

        let maker_price = FixedI64::saturating_from_integer(250);
        let side = OrderSide::Buy;
        let maker_amount = EqFixedU128::saturating_from_integer(1);
        let expiration_time = 100u64;
        assert_ok!(ModuleDex::create_limit_order(
            maker,
            asset,
            maker_price,
            side,
            maker_amount,
            expiration_time,
            &asset_data
        ));

        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 1);
        let maker_order = &orders[0];

        let taker_amount = maker_amount * EqFixedU128::saturating_from_integer(2);
        let taker_price = maker_price - FixedI64::one();
        let taker_side = OrderSide::Sell;
        let expected_amount = maker_amount;
        assert_eq!(
            ModuleDex::match_two_orders(
                &taker,
                taker_amount,
                Limit {
                    price: taker_price,
                    expiration_time: 0
                },
                taker_side,
                maker_order,
                &asset
            ),
            Ok(expected_amount)
        );
        // maker order
        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        let order_idx = orders.binary_search_by(|o| o.order_id.cmp(&maker_order.order_id));
        assert!(order_idx.is_err());
        assert_err!(order_idx, 0_usize);
        // taker order
        let chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);

        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert!(chunks.len() == 0);

        assert_eq!(
            ModuleBalances::get_balance(&taker, &asset),
            SignedBalance::Positive(
                taker_asset_balance - balance_from_eq_fixedu128::<Balance>(maker_amount).unwrap()
            )
        );

        assert_eq!(
            ModuleBalances::get_balance(&maker, &asset),
            SignedBalance::Positive(balance_from_eq_fixedu128(maker_amount).unwrap())
        );

        let expected_taker_usd = maker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() - asset_data.taker_fee.into());
        let expected_taker_usd: Balance = balance_from_eq_fixedu128(expected_taker_usd).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&taker, &EQD),
            SignedBalance::Positive(expected_taker_usd)
        );

        let expected_maker_usd_pay = maker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() + asset_data.maker_fee.into());
        let expected_maker_usd_pay: Balance =
            balance_from_eq_fixedu128(expected_maker_usd_pay).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&maker, &EQD),
            SignedBalance::Positive(maker_usd_balance - expected_maker_usd_pay)
        );
    });
}

#[test]
fn two_orders_match_successful_with_maker_rest_taker_buy() {
    new_test_ext().execute_with(|| {
        let maker = 101_u64;
        let taker = 102_u64;

        let asset = ETH;
        let maker_asset_balance: Balance = 500_000_000_000;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        assert_ok!(ModuleBalances::deposit_creating(
            &maker,
            asset,
            maker_asset_balance,
            true,
            None
        ));

        let taker_usd_balance: Balance = 260_000_000_000;
        assert_ok!(ModuleBalances::deposit_creating(
            &taker,
            EQD,
            taker_usd_balance,
            true,
            None
        ));

        let maker_price = FixedI64::saturating_from_integer(250);
        let side = OrderSide::Sell;
        let maker_amount = EqFixedU128::saturating_from_integer(2);
        let expiration_time = 100u64;
        assert_ok!(ModuleDex::create_limit_order(
            maker,
            asset,
            maker_price,
            side,
            maker_amount,
            expiration_time,
            &asset_data
        ));

        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 1);
        let maker_order = &orders[0];

        let taker_amount = maker_amount / EqFixedU128::saturating_from_integer(2);
        let taker_price = maker_price + FixedI64::one();
        let taker_side = OrderSide::Buy;
        let expected_amount = taker_amount;
        assert_eq!(
            ModuleDex::match_two_orders(
                &taker,
                taker_amount,
                Limit {
                    price: taker_price,
                    expiration_time: 0
                },
                taker_side,
                maker_order,
                &asset
            ),
            Ok(expected_amount)
        );
        // maker order
        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        let order_idx = orders.binary_search_by(|o| o.order_id.cmp(&maker_order.order_id));
        assert_eq!(orders.len(), 1_usize);
        assert_eq!(order_idx, Ok(0_usize));
        let maker_rest_order = &orders[order_idx.expect("Order exists")];
        assert_eq!(maker_rest_order.amount, maker_amount - taker_amount);
        assert_eq!(maker_rest_order.price, maker_price);
        assert_eq!(maker_rest_order.order_id, maker_order.order_id);
        assert_eq!(
            maker_rest_order.expiration_time,
            maker_order.expiration_time
        );
        assert_eq!(maker_rest_order.account_id, maker_order.account_id);
        assert_eq!(maker_rest_order.side, maker_order.side);
        assert_eq!(maker_rest_order.created_at, maker_order.created_at);

        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert!(chunks.len() != 0);

        assert_eq!(
            ModuleBalances::get_balance(&maker, &asset),
            SignedBalance::Positive(
                maker_asset_balance - balance_from_eq_fixedu128::<Balance>(taker_amount).unwrap()
            )
        );

        assert_eq!(
            ModuleBalances::get_balance(&taker, &asset),
            SignedBalance::Positive(balance_from_eq_fixedu128(taker_amount).unwrap())
        );

        let expected_maker_usd = taker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() - asset_data.maker_fee.into());
        let expected_maker_usd: Balance = balance_from_eq_fixedu128(expected_maker_usd).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&maker, &EQD),
            SignedBalance::Positive(expected_maker_usd)
        );

        let expected_taker_usd_pay = taker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() + asset_data.taker_fee.into());
        let expected_taker_usd_pay: Balance =
            balance_from_eq_fixedu128(expected_taker_usd_pay).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&taker, &EQD),
            SignedBalance::Positive(taker_usd_balance - expected_taker_usd_pay)
        );
    });
}

#[test]
fn two_orders_match_successful_with_maker_rest_taker_sell() {
    new_test_ext().execute_with(|| {
        let maker = 101_u64;
        let taker = 102_u64;

        let asset = ETH;
        let taker_asset_balance: Balance = 250_000_000_000;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        assert_ok!(ModuleBalances::deposit_creating(
            &taker,
            asset,
            taker_asset_balance,
            true,
            None
        ));

        let maker_usd_balance: Balance = 500_000_000_000;
        assert_ok!(ModuleBalances::deposit_creating(
            &maker,
            EQD,
            maker_usd_balance,
            true,
            None
        ));

        let maker_price = FixedI64::saturating_from_integer(250);
        let side = OrderSide::Buy;
        let maker_amount = EqFixedU128::saturating_from_integer(2);
        let expiration_time = 100u64;
        assert_ok!(ModuleDex::create_limit_order(
            maker,
            asset,
            maker_price,
            side,
            maker_amount,
            expiration_time,
            &asset_data
        ));

        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 1);
        let maker_order = &orders[0];

        let taker_amount = maker_amount / EqFixedU128::saturating_from_integer(2);
        let taker_price = maker_price - FixedI64::one();
        let taker_side = OrderSide::Sell;
        let expected_amount = taker_amount;
        assert_eq!(
            ModuleDex::match_two_orders(
                &taker,
                taker_amount,
                Limit {
                    price: taker_price,
                    expiration_time: 0
                },
                taker_side,
                maker_order,
                &asset
            ),
            Ok(expected_amount)
        );
        // maker order
        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert_eq!(orders.len(), 1_usize);
        let order_idx = orders.binary_search_by(|o| o.order_id.cmp(&maker_order.order_id));
        assert_eq!(order_idx, Ok(0_usize));
        let maker_rest_order = &orders[order_idx.expect("Order exists")];
        assert_eq!(maker_rest_order.amount, maker_amount - taker_amount);
        assert_eq!(maker_rest_order.price, maker_price);
        assert_eq!(maker_rest_order.order_id, maker_order.order_id);
        assert_eq!(
            maker_rest_order.expiration_time,
            maker_order.expiration_time
        );
        assert_eq!(maker_rest_order.account_id, maker_order.account_id);
        assert_eq!(maker_rest_order.side, maker_order.side);
        assert_eq!(maker_rest_order.created_at, maker_order.created_at);

        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert!(chunks.len() == 1);

        assert_eq!(
            ModuleBalances::get_balance(&taker, &asset),
            SignedBalance::Positive(
                taker_asset_balance - balance_from_eq_fixedu128::<Balance>(taker_amount).unwrap()
            )
        );

        assert_eq!(
            ModuleBalances::get_balance(&maker, &asset),
            SignedBalance::Positive(balance_from_eq_fixedu128(taker_amount).unwrap())
        );

        let expected_taker_usd = taker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() - asset_data.taker_fee.into());
        let expected_taker_usd: Balance = balance_from_eq_fixedu128(expected_taker_usd).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&taker, &EQD),
            SignedBalance::Positive(expected_taker_usd)
        );

        let expected_maker_usd_pay = taker_amount
            * maker_price.try_into().expect("Positive")
            * (EqFixedU128::one() + asset_data.maker_fee.into());
        let expected_maker_usd_pay: Balance =
            balance_from_eq_fixedu128(expected_maker_usd_pay).unwrap();
        assert_eq!(
            ModuleBalances::get_balance(&maker, &EQD),
            SignedBalance::Positive(maker_usd_balance - expected_maker_usd_pay)
        );
    });
}

#[test]
fn match_buy_order_single_chunk_without_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 250..254;
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        // amount = FixedU128::from(1) for all makers orders
        makers.clone().zip(prices.clone()).for_each(|(m, p)| {
            let _ = SubaccountsManagerMock::create_subaccount_inner(&m, &SubAccType::Trader)
                .expect("Create borrower subaccount");
            create_orders(&m, asset, Sell, &vec![FixedI64::saturating_from_integer(p)]);
        });

        let taker_side = Buy;
        let taker_price = FixedI64::saturating_from_integer(prices.end);
        let taker_amount = EqFixedU128::from((makers.end - makers.start) as u128);
        let _ = SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
            .expect("Create borrower subaccount");

        // check that taker order will be in the same chunk as makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        prices.for_each(|p| {
            assert_eq!(
                ModuleDex::get_chunk_key(
                    FixedI64::saturating_from_integer(p),
                    asset_data.price_step
                )
                .unwrap(),
                taker_chunk_key
            );
        });
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), makers.end as usize - makers.start as usize);

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time: 999_000_000_000
            },
            taker_side,
            taker_amount,
        ));

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);
    });
}

#[test]
fn match_sell_order_single_chunk_without_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 251..255;
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        // amount = FixedU128::from(1) for all makers orders
        makers.clone().zip(prices.clone()).for_each(|(m, p)| {
            let _ = SubaccountsManagerMock::create_subaccount_inner(&m, &SubAccType::Trader)
                .expect("Create borrower subaccount");
            create_orders(&m, asset, Buy, &vec![FixedI64::saturating_from_integer(p)]);
        });

        let taker_side = Sell;
        let taker_price = FixedI64::saturating_from_integer(prices.start - 1);
        let taker_amount = EqFixedU128::from((makers.end - makers.start) as u128);
        let _ = SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
            .expect("Create borrower subaccount");

        // check that taker order will be in the same chunk as makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        prices.for_each(|p| {
            assert_eq!(
                ModuleDex::get_chunk_key(
                    FixedI64::saturating_from_integer(p),
                    asset_data.price_step
                )
                .unwrap(),
                taker_chunk_key
            );
        });
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), makers.end as usize - makers.start as usize);

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time: 999_000_000_000
            },
            taker_side,
            taker_amount,
        ));

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);
    });
}

#[test]
fn match_buy_order_single_chunk_with_taker_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 250..254;
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        let makers_orders = makers
            .clone()
            .zip(prices.clone())
            .map(|(m, p)| {
                let _ = SubaccountsManagerMock::create_subaccount_inner(&m, &SubAccType::Trader)
                    .expect("Create borrower subaccount");
                create_orders(&m, asset, Sell, &vec![FixedI64::saturating_from_integer(p)])[0]
            })
            .collect::<Vec<u64>>();

        let taker_side = Buy;
        let taker_price = FixedI64::saturating_from_integer(prices.end);
        // amount = FixedU128::from(1) for all makers orders
        let taker_amount = EqFixedU128::from((makers.end - makers.start + 1) as u128);
        let taker_borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
                .expect("Create borrower subaccount");
        let expiration_time = 999_000_000_000;

        // check that taker order will be in the same chunk as makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        prices.for_each(|p| {
            assert_eq!(
                ModuleDex::get_chunk_key(
                    FixedI64::saturating_from_integer(p),
                    asset_data.price_step
                )
                .unwrap(),
                taker_chunk_key
            );
        });
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), makers.end as usize - makers.start as usize);

        let expected_taker_order_id = makers_orders[makers_orders.len() - 1] + 1;

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            taker_side,
            taker_amount,
        ));

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 1);
        let order_idx = orders.binary_search_by(|o| o.order_id.cmp(&expected_taker_order_id));
        assert_eq!(order_idx, Ok(0_usize));
        let taker_rest_order = &orders[order_idx.expect("Order exists")];
        assert_eq!(
            taker_rest_order.amount,
            taker_amount - EqFixedU128::from((makers.end - makers.start) as u128)
        );
        assert_eq!(taker_rest_order.price, taker_price);
        assert_eq!(taker_rest_order.order_id, expected_taker_order_id);
        assert_eq!(taker_rest_order.expiration_time, expiration_time);
        assert_eq!(taker_rest_order.account_id, taker_borrower_id);
        assert_eq!(taker_rest_order.side, taker_side);
    });
}

#[test]
fn match_sell_order_single_chunk_with_taker_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 251..255;
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        let makers_orders = makers
            .clone()
            .zip(prices.clone())
            .map(|(m, p)| {
                let _ = SubaccountsManagerMock::create_subaccount_inner(&m, &SubAccType::Trader)
                    .expect("Create borrower subaccount");
                create_orders(&m, asset, Buy, &vec![FixedI64::saturating_from_integer(p)])[0]
            })
            .collect::<Vec<u64>>();

        let taker_side = Sell;
        let taker_price = FixedI64::saturating_from_integer(prices.start - 1);
        // amount = FixedU128::from(1) for all makers orders
        let taker_amount = EqFixedU128::from((makers.end - makers.start + 1) as u128);
        let taker_borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
                .expect("Create borrower subaccount");
        let expiration_time = 999_000_000_000;

        // check that taker order will be in the same chunk as makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        prices.for_each(|p| {
            assert_eq!(
                ModuleDex::get_chunk_key(
                    FixedI64::saturating_from_integer(p),
                    asset_data.price_step
                )
                .unwrap(),
                taker_chunk_key
            );
        });
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), makers.end as usize - makers.start as usize);

        let expected_taker_order_id = makers_orders[makers_orders.len() - 1] + 1;

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            taker_side,
            taker_amount,
        ));

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 1);
        let order_idx = orders.binary_search_by(|o| o.order_id.cmp(&expected_taker_order_id));
        assert_eq!(order_idx, Ok(0_usize));
        let taker_rest_order = &orders[order_idx.expect("Order exists")];
        assert_eq!(
            taker_rest_order.amount,
            taker_amount - EqFixedU128::from((makers.end - makers.start) as u128)
        );
        assert_eq!(taker_rest_order.price, taker_price);
        assert_eq!(taker_rest_order.order_id, expected_taker_order_id);
        assert_eq!(taker_rest_order.expiration_time, expiration_time);
        assert_eq!(taker_rest_order.account_id, taker_borrower_id);
        assert_eq!(taker_rest_order.side, taker_side);
    });
}

#[test]
fn sell_order_single_chunk_bid_ask() {
    new_test_ext().execute_with(|| {
        let maker = 1_u64;
        let taker = 105_u64;
        let asset = ETH;
        create_orders(
            &maker,
            asset,
            Sell,
            &vec![FixedI64::saturating_from_integer(252u64)],
        );
        create_orders(
            &maker,
            asset,
            Sell,
            &vec![FixedI64::saturating_from_integer(252u64)],
        );
        create_orders(
            &maker,
            asset,
            Buy,
            &vec![FixedI64::saturating_from_integer(251u64)],
        );

        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        let taker_chunk_key = ModuleDex::get_chunk_key(
            FixedI64::saturating_from_integer(252u64),
            asset_data.price_step,
        )
        .unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        println!("{:?}", orders);

        let taker_side = Buy;
        let taker_price = FixedI64::saturating_from_integer(300u64);
        // amount = FixedU128::from(1) for all makers orders
        let taker_amount = EqFixedU128::saturating_from_integer(1u128);
        let _taker_borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
                .expect("Create borrower subaccount");
        let expiration_time = 999_000_000_000;

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            taker_side,
            taker_amount,
        ));

        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        let maker_chunk_key = ModuleDex::get_chunk_key(
            FixedI64::saturating_from_integer(252u64),
            asset_data.price_step,
        )
        .unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, maker_chunk_key);

        // match
        let _new_bid = BestPriceByAsset::<Test>::get(asset).bid.unwrap();
        assert_eq!(orders.len(), 2);

        // no new orders
        let taker_chunk_key = ModuleDex::get_chunk_key(
            FixedI64::saturating_from_integer(300u64),
            asset_data.price_step,
        )
        .unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);
    });
}

#[test]
fn match_buy_order_multiple_chunks_without_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 250..254;
        assert_eq!(makers.end - makers.start, prices.end - prices.start);
        // place prices in different chunks
        let prices_fixed = prices.clone().enumerate().map(|(i, p)| {
            FixedI64::saturating_from_integer(p + (i as u32 * PriceStepCount::get()) as u64)
        });
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        makers.clone().zip(prices_fixed.clone()).for_each(|(m, p)| {
            let _ = SubaccountsManagerMock::create_subaccount_inner(&m, &SubAccType::Trader)
                .expect("Create borrower subaccount");
            create_orders(&m, asset, Sell, &vec![p]);
        });

        let taker_side = Buy;
        let taker_price = FixedI64::saturating_from_integer(
            prices.end + (prices.end - prices.start) as u64 * PriceStepCount::get() as u64,
        );
        // amount = FixedU128::from(1) for all makers orders
        let taker_amount = EqFixedU128::from((makers.end - makers.start) as u128);
        let _ = SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
            .expect("Create borrower subaccount");

        // check that taker order not in any chunk with makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let maker_chunks_keys = prices_fixed.clone().map(|p| {
            assert_ne!(
                ModuleDex::get_chunk_key(p, asset_data.price_step).unwrap(),
                taker_chunk_key
            );
            ModuleDex::get_chunk_key(p, asset_data.price_step).unwrap()
        });

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time: 999_000_000_000
            },
            taker_side,
            taker_amount,
        ));

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);
        maker_chunks_keys.for_each(|chunk_key| {
            let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
            assert_eq!(orders.len(), 0);
        });
    });
}

#[test]
fn match_sell_order_multiple_chunks_without_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 251..255;
        assert_eq!(makers.end - makers.start, prices.end - prices.start);
        // place prices in different chunks
        let prices_fixed = prices.clone().enumerate().map(|(i, p)| {
            FixedI64::saturating_from_integer(p - (i as u32 * PriceStepCount::get()) as u64)
        });
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        makers.clone().zip(prices_fixed.clone()).for_each(|(m, p)| {
            let _ = SubaccountsManagerMock::create_subaccount_inner(&m, &SubAccType::Trader)
                .expect("Create borrower subaccount");
            create_orders(&m, asset, Buy, &vec![p]);
        });

        let taker_side = Sell;
        let taker_price = FixedI64::saturating_from_integer(
            prices.start - 1 - (prices.end - prices.start) as u64 * PriceStepCount::get() as u64,
        );
        // amount = FixedU128::from(1) for all makers orders
        let taker_amount = EqFixedU128::from((makers.end - makers.start) as u128);
        let _ = SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
            .expect("Create borrower subaccount");

        // check that taker order not in any chunk with makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let maker_chunks_keys = prices_fixed.clone().map(|p| {
            assert_ne!(
                ModuleDex::get_chunk_key(p, asset_data.price_step).unwrap(),
                taker_chunk_key
            );
            ModuleDex::get_chunk_key(p, asset_data.price_step).unwrap()
        });

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time: 999_000_000_000
            },
            taker_side,
            taker_amount,
        ));

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);
        maker_chunks_keys.for_each(|chunk_key| {
            let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
            assert_eq!(orders.len(), 0);
        });
    });
}

#[test]
fn match_buy_order_multiple_chunks_with_taker_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 250..254;
        assert_eq!(makers.end - makers.start, prices.end - prices.start);
        // place prices in different chunks
        let prices_fixed = prices.clone().enumerate().map(|(i, p)| {
            FixedI64::saturating_from_integer(p + (i as u32 * PriceStepCount::get()) as u64)
        });
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        let makers_order_ids = makers
            .clone()
            .zip(prices_fixed.clone())
            .map(|(m, p)| {
                let _ = SubaccountsManagerMock::create_subaccount_inner(&m, &SubAccType::Trader)
                    .expect("Create borrower subaccount");
                create_orders(&m, asset, Sell, &vec![p])[0]
            })
            .collect::<Vec<u64>>();
        let expected_taker_order_id = makers_order_ids[makers_order_ids.len() - 1] + 1;

        let taker_side = Buy;
        let taker_price = FixedI64::saturating_from_integer(
            prices.end + (prices.end - prices.start) as u64 * PriceStepCount::get() as u64,
        );
        // amount = FixedU128::from(1) for all makers orders
        let taker_amount = EqFixedU128::from((makers.end - makers.start + 1) as u128);
        let expiration_time = 999_000_000_000;
        let taker_borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
                .expect("Create borrower subaccount");

        // check that taker order not in any chunk with makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let maker_chunks_keys = prices_fixed.clone().map(|p| {
            assert_ne!(
                ModuleDex::get_chunk_key(p, asset_data.price_step).unwrap(),
                taker_chunk_key
            );
            ModuleDex::get_chunk_key(p, asset_data.price_step).unwrap()
        });

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            taker_side,
            taker_amount,
        ));

        maker_chunks_keys.for_each(|chunk_key| {
            let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
            assert_eq!(orders.len(), 0);
        });

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 1);
        let taker_rest_order = &orders[0];
        assert_eq!(
            taker_rest_order.amount,
            taker_amount - EqFixedU128::from((makers.end - makers.start) as u128)
        );
        assert_eq!(taker_rest_order.price, taker_price);
        assert_eq!(taker_rest_order.order_id, expected_taker_order_id);
        assert_eq!(taker_rest_order.expiration_time, expiration_time);
        assert_eq!(taker_rest_order.account_id, taker_borrower_id);
        assert_eq!(taker_rest_order.side, taker_side);
    });
}

#[test]
fn match_sell_order_multiple_chunks_with_taker_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 251..255;
        assert_eq!(makers.end - makers.start, prices.end - prices.start);
        // place prices in different chunks
        let prices_fixed = prices.clone().enumerate().map(|(i, p)| {
            FixedI64::saturating_from_integer(p - (i as u32 * PriceStepCount::get()) as u64)
        });
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        let makers_order_ids = makers
            .clone()
            .zip(prices_fixed.clone())
            .map(|(m, p)| {
                let _ = SubaccountsManagerMock::create_subaccount_inner(&m, &SubAccType::Trader)
                    .expect("Create borrower subaccount");
                create_orders(&m, asset, Buy, &vec![p])[0]
            })
            .collect::<Vec<u64>>();
        let expected_taker_order_id = makers_order_ids[makers_order_ids.len() - 1] + 1;

        let taker_side = Sell;
        let taker_price = FixedI64::saturating_from_integer(
            prices.start - 1 - (prices.end - prices.start) as u64 * PriceStepCount::get() as u64,
        );
        // amount = FixedU128::from(1) for all makers orders
        let taker_amount = EqFixedU128::from((makers.end - makers.start + 1) as u128);
        let expiration_time = 999_000_000_000;
        let taker_borrower_id =
            SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
                .expect("Create borrower subaccount");

        // check that taker order not in any chunk with makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let maker_chunks_keys = prices_fixed.clone().map(|p| {
            assert_ne!(
                ModuleDex::get_chunk_key(p, asset_data.price_step).unwrap(),
                taker_chunk_key
            );
            ModuleDex::get_chunk_key(p, asset_data.price_step).unwrap()
        });

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            taker_side,
            taker_amount,
        ));

        maker_chunks_keys.for_each(|chunk_key| {
            let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
            assert_eq!(orders.len(), 0);
        });

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 1);
        let taker_rest_order = &orders[0];
        assert_eq!(
            taker_rest_order.amount,
            taker_amount - EqFixedU128::from((makers.end - makers.start) as u128)
        );
        assert_eq!(taker_rest_order.price, taker_price);
        assert_eq!(taker_rest_order.order_id, expected_taker_order_id);
        assert_eq!(taker_rest_order.expiration_time, expiration_time);
        assert_eq!(taker_rest_order.account_id, taker_borrower_id);
        assert_eq!(taker_rest_order.side, taker_side);
    });
}

#[test]
fn match_buy_order_multiple_chunks_with_maker_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 250..254;
        assert_eq!(makers.end - makers.start, prices.end - prices.start);
        // place prices in different chunks
        let prices_fixed = prices
            .clone()
            .enumerate()
            .map(|(i, p)| {
                FixedI64::saturating_from_integer(p + (i as u32 * PriceStepCount::get()) as u64)
            })
            .collect::<Vec<_>>();
        let maker_with_rest_price = prices_fixed[prices_fixed.len() - 1];
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        let maker_expiration_time = 1111111111;
        let default_maker_amount = EqFixedU128::from(1);
        let maker_amount_for_rest = default_maker_amount * EqFixedU128::saturating_from_integer(2);
        let makers_order_ids = makers
            .clone()
            .zip(prices_fixed.iter())
            .map(|(m, p)| {
                assert_ok!(ModuleDex::create_limit_order(
                    m,
                    asset,
                    *p,
                    Sell,
                    if m == makers.end - 1 {
                        maker_amount_for_rest
                    } else {
                        default_maker_amount
                    },
                    maker_expiration_time,
                    &asset_data
                ));
                OrderIdCounter::<Test>::get()
            })
            .collect::<Vec<_>>();
        let maker_order_with_rest_id = makers_order_ids[makers_order_ids.len() - 1];

        let taker_side = Buy;
        let taker_price = FixedI64::saturating_from_integer(
            prices.end + (prices.end - prices.start) as u64 * PriceStepCount::get() as u64,
        );
        // amount = FixedU128::from(1) for all makers orders except last: he has FixedU128::from(2)
        let taker_amount = EqFixedU128::from((makers.end - makers.start) as u128);
        let expiration_time = 999_000_000_000;
        let _ = SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
            .expect("Create borrower subaccount");

        // check that taker order not in any chunk with makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let maker_chunks_keys = prices_fixed
            .iter()
            .map(|p| {
                assert_ne!(
                    ModuleDex::get_chunk_key(*p, asset_data.price_step).unwrap(),
                    taker_chunk_key
                );
                ModuleDex::get_chunk_key(*p, asset_data.price_step).unwrap()
            })
            .collect::<Vec<_>>();

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            taker_side,
            taker_amount,
        ));

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);

        let chunk_for_maker_with_rest = maker_chunks_keys[maker_chunks_keys.len() - 1];
        maker_chunks_keys.iter().for_each(|chunk_key| {
            let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
            if chunk_key == &chunk_for_maker_with_rest {
                assert_eq!(orders.len(), 1);
            } else {
                assert_eq!(orders.len(), 0);
            }
        });

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_for_maker_with_rest);
        let maker_rest_order = &orders[0];
        assert_eq!(
            maker_rest_order.amount,
            taker_amount - EqFixedU128::from((makers.end - makers.start - 1) as u128)
        );
        assert_eq!(maker_rest_order.price, maker_with_rest_price);
        assert_eq!(maker_rest_order.order_id, maker_order_with_rest_id);
        assert_eq!(maker_rest_order.expiration_time, maker_expiration_time);
        assert_eq!(maker_rest_order.account_id, makers.end - 1);
        assert_eq!(maker_rest_order.side, Sell);
    });
}

#[test]
fn match_sell_order_multiple_chunks_with_maker_rest() {
    new_test_ext().execute_with(|| {
        let makers = 1_u64..5;
        let prices = 251..255;
        assert_eq!(makers.end - makers.start, prices.end - prices.start);
        // place prices in different chunks
        let prices_fixed = prices
            .clone()
            .enumerate()
            .map(|(i, p)| {
                FixedI64::saturating_from_integer(p - (i as u32 * PriceStepCount::get()) as u64)
            })
            .collect::<Vec<_>>();
        let maker_with_rest_price = prices_fixed[prices_fixed.len() - 1];
        let taker = 105_u64;
        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");
        let maker_expiration_time = 1111111111;
        let default_maker_amount = EqFixedU128::from(1);
        let maker_amount_for_rest = default_maker_amount * EqFixedU128::saturating_from_integer(2);
        let makers_order_ids = makers
            .clone()
            .zip(prices_fixed.iter())
            .map(|(m, p)| {
                assert_ok!(ModuleDex::create_limit_order(
                    m,
                    asset,
                    *p,
                    Buy,
                    if m == makers.end - 1 {
                        maker_amount_for_rest
                    } else {
                        default_maker_amount
                    },
                    maker_expiration_time,
                    &asset_data
                ));
                OrderIdCounter::<Test>::get()
            })
            .collect::<Vec<_>>();
        let maker_order_with_rest_id = makers_order_ids[makers_order_ids.len() - 1];

        let taker_side = Sell;
        let taker_price = FixedI64::saturating_from_integer(
            prices.start - 1 - (prices.end - prices.start) as u64 * PriceStepCount::get() as u64,
        );
        // amount = FixedU128::from(1) for all makers orders except last: he has FixedU128::from(2)
        let taker_amount = EqFixedU128::from((makers.end - makers.start) as u128);
        let expiration_time = 999_000_000_000;
        let _ = SubaccountsManagerMock::create_subaccount_inner(&taker, &SubAccType::Trader)
            .expect("Create borrower subaccount");

        // check that taker order not in any chunk with makers orders
        let taker_chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let maker_chunks_keys = prices_fixed
            .iter()
            .map(|p| {
                assert_ne!(
                    ModuleDex::get_chunk_key(*p, asset_data.price_step).unwrap(),
                    taker_chunk_key
                );
                ModuleDex::get_chunk_key(*p, asset_data.price_step).unwrap()
            })
            .collect::<Vec<_>>();

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);

        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            taker_side,
            taker_amount,
        ));

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, taker_chunk_key);
        assert_eq!(orders.len(), 0);

        let chunk_for_maker_with_rest = maker_chunks_keys[maker_chunks_keys.len() - 1];
        maker_chunks_keys.iter().for_each(|chunk_key| {
            let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
            if chunk_key == &chunk_for_maker_with_rest {
                assert_eq!(orders.len(), 1);
            } else {
                assert_eq!(orders.len(), 0);
            }
        });

        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_for_maker_with_rest);
        let maker_rest_order = &orders[0];
        assert_eq!(
            maker_rest_order.amount,
            taker_amount - EqFixedU128::from((makers.end - makers.start - 1) as u128)
        );
        assert_eq!(maker_rest_order.price, maker_with_rest_price);
        assert_eq!(maker_rest_order.order_id, maker_order_with_rest_id);
        assert_eq!(maker_rest_order.expiration_time, maker_expiration_time);
        assert_eq!(maker_rest_order.account_id, makers.end - 1);
        assert_eq!(maker_rest_order.side, Buy);
    });
}

#[test]
fn two_orders_match_maker_sell_fail() {
    new_test_ext().execute_with(|| {
        let maker = FAIL_ACC;
        let taker = 102_u64;

        // TODO change to balances module deposit
        frame_system::Pallet::<Test>::inc_providers(&maker);
        frame_system::Pallet::<Test>::inc_providers(&taker);

        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");

        let maker_price = FixedI64::saturating_from_integer(250);
        let side = OrderSide::Sell;
        let maker_amount = EqFixedU128::saturating_from_integer(1);
        let expiration_time = 100u64;
        assert_ok!(ModuleDex::create_limit_order(
            maker,
            asset,
            maker_price,
            side,
            maker_amount,
            expiration_time,
            &asset_data
        ));

        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 1);
        let maker_order = &orders[0];

        let taker_amount = maker_amount;
        let taker_price = maker_price + FixedI64::one();
        let taker_side = OrderSide::Buy;

        assert_eq!(
            ModuleDex::match_two_orders(
                &taker,
                taker_amount,
                Limit {
                    price: taker_price,
                    expiration_time: 0
                },
                taker_side,
                maker_order,
                &asset
            ),
            Ok(EqFixedU128::zero())
        );
        // maker
        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);
        // taker
        let chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);
        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert!(chunks.len() == 0);
    });
}

#[test]
fn two_orders_match_maker_buy_fail() {
    new_test_ext().execute_with(|| {
        let maker = FAIL_ACC;
        let taker = 102_u64;

        // TODO change to balances module deposit
        frame_system::Pallet::<Test>::inc_providers(&maker);
        frame_system::Pallet::<Test>::inc_providers(&taker);

        let asset = ETH;
        let asset_data = AssetGetterMock::get_asset_data(&asset).expect("Asset exists");

        let maker_price = FixedI64::saturating_from_integer(250);
        let side = OrderSide::Buy;
        let maker_amount = EqFixedU128::saturating_from_integer(1);
        let expiration_time = 100u64;
        assert_ok!(ModuleDex::create_limit_order(
            maker,
            asset,
            maker_price,
            side,
            maker_amount,
            expiration_time,
            &asset_data
        ));

        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 1);
        let maker_order = &orders[0];

        let taker_amount = maker_amount;
        let taker_price = maker_price - FixedI64::one();
        let taker_side = OrderSide::Sell;

        assert_eq!(
            ModuleDex::match_two_orders(
                &taker,
                taker_amount,
                Limit {
                    price: taker_price,
                    expiration_time: 0
                },
                taker_side,
                maker_order,
                &asset
            ),
            Ok(EqFixedU128::zero())
        );
        // maker
        let chunk_key = ModuleDex::get_chunk_key(maker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);
        // taker
        let chunk_key = ModuleDex::get_chunk_key(taker_price, asset_data.price_step).unwrap();
        let orders = OrdersByAssetAndChunkKey::<Test>::get(asset, chunk_key);
        assert!(orders.len() == 0);
        let chunks = ActualChunksByAsset::<Test>::get(asset);
        assert!(chunks.len() == 0);
    });
}

#[test]
fn match_order_maker_failure_on_exchange() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::inc_providers(&101);
        frame_system::Pallet::<Test>::inc_providers(&102);
        frame_system::Pallet::<Test>::inc_providers(&FAIL_SUBACC);

        let asset = ETH;
        let mut price = FixedI64::from(5);
        let amount = EqFixedU128::from(100);
        let expiration_time = 100u64;

        let price_setter = 1u64;
        OracleMock::set_price(price_setter, ETH, price).unwrap();

        let new_asset_corridor: u32 = 20;
        assert_ok!(ModuleDex::update_asset_corridor(
            RawOrigin::Root.into(),
            asset,
            new_asset_corridor
        ));

        for i in 0..8 {
            assert_ok!(<ModuleDex as OrderManagement>::create_order(
                if i == 4 { FAIL_ACC } else { 1 },
                asset,
                Limit {
                    price,
                    expiration_time
                },
                Buy,
                amount,
            ));
            price = price + FixedI64::from(5);
        }

        assert_eq!(all_orders(asset, Buy).len(), 8);

        let taker = 2;
        let taker_price = FixedI64::from(5);
        let taker_amount = EqFixedU128::from(200);
        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            Sell,
            taker_amount,
        ));

        assert_eq!(all_orders(asset, Buy).len(), 6);

        let taker = 2;
        let taker_price = FixedI64::from(5);
        let taker_amount = EqFixedU128::from(200);
        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            Sell,
            taker_amount,
        ));

        assert_eq!(all_orders(asset, Buy).len(), 3);

        let taker = 2;
        let taker_price = FixedI64::from(5);
        let taker_amount = EqFixedU128::from(1000);
        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Limit {
                price: taker_price,
                expiration_time
            },
            Sell,
            taker_amount,
        ));

        assert_eq!(all_orders(asset, Buy).len(), 0);
        assert_eq!(all_orders(asset, Sell).len(), 1);
    })
}

#[test]
fn market_order_create_and_market_order_without_best_price_failure() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::inc_providers(&101);
        frame_system::Pallet::<Test>::inc_providers(&102);
        frame_system::Pallet::<Test>::inc_providers(&FAIL_SUBACC);

        let asset = ETH;
        let new_asset_corridor: u32 = 20;
        assert_ok!(ModuleDex::update_asset_corridor(
            RawOrigin::Root.into(),
            asset,
            new_asset_corridor
        ));

        let maker = 1;
        let mut price = FixedI64::from(5);
        OracleMock::set_price(maker, ETH, price).unwrap();

        let maker_amount = EqFixedU128::from(100);
        let expiration_time = 100u64;
        for _ in 0..10 {
            assert_ok!(<ModuleDex as OrderManagement>::create_order(
                maker,
                asset,
                Limit {
                    price,
                    expiration_time
                },
                Buy,
                maker_amount,
            ));
            price = price + FixedI64::from(5);
        }
        for _ in 0..10 {
            assert_ok!(<ModuleDex as OrderManagement>::create_order(
                maker,
                asset,
                Limit {
                    price,
                    expiration_time
                },
                Sell,
                maker_amount,
            ));
            price = price + FixedI64::from(5);
        }

        assert_eq!(all_orders(asset, Buy).len(), 10);
        assert_eq!(all_orders(asset, Sell).len(), 10);

        let taker = 2;
        let taker_amount = EqFixedU128::from(150);
        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Market,
            Sell,
            taker_amount,
        ));

        assert_eq!(all_orders(asset, Buy).len(), 9);
        assert_eq!(all_orders(asset, Sell).len(), 10);

        assert_eq!(
            all_orders(asset, Buy).last().map(|order| order.amount),
            Some(EqFixedU128::from(50))
        );

        let taker = 2;
        let taker_amount = EqFixedU128::from(1000);
        assert_ok!(<ModuleDex as OrderManagement>::create_order(
            taker,
            asset,
            Market,
            Sell,
            taker_amount,
        ));

        assert_eq!(all_orders(asset, Buy).len(), 0);
        assert_eq!(all_orders(asset, Sell).len(), 10);

        assert_err!(
            <ModuleDex as OrderManagement>::create_order(taker, asset, Market, Sell, taker_amount),
            Error::<Test>::NoBestPriceForMarketOrder
        );
    })
}

// TODO fix me!!!!
// better to use treasury pallet instead of mock
// remove fake deposit to treasury after that (need to call genesis on treasury)
// treasury balance cannot go negative
#[allow(dead_code)]
fn charge_penalty_fee() {
    new_test_ext().execute_with(|| {
        let acc = 1;
        let buyout = 1_000_000_000;
        let native_asset = <eq_assets::Pallet<Test> as AssetGetter>::get_main_asset();
        assert_ok!(ModuleDex::charge_penalty_fee(&acc, Some(buyout)));
        assert_eq!(
            ModuleBalances::get_balance(&acc, &native_asset),
            SignedBalance::Negative(PenaltyFee::get() - buyout)
        );
        assert_eq!(
            ModuleBalances::get_balance(
                &TreasuryModuleId::get().into_account_truncating(),
                &native_asset
            ),
            SignedBalance::Negative(buyout)
        );
        assert_eq!(
            ModuleBalances::get_balance(
                &TreasuryModuleId::get().into_account_truncating(),
                &native_asset
            ),
            SignedBalance::Positive(PenaltyFee::get())
        );

        assert_ok!(ModuleDex::charge_penalty_fee(&acc, None));
        assert_eq!(
            ModuleBalances::get_balance(&acc, &native_asset),
            SignedBalance::Negative(PenaltyFee::get() - buyout + PenaltyFee::get())
        );
        assert_eq!(
            ModuleBalances::get_balance(
                &TreasuryModuleId::get().into_account_truncating(),
                &native_asset
            ),
            SignedBalance::Negative(buyout)
        );
        assert_eq!(
            ModuleBalances::get_balance(
                &TreasuryModuleId::get().into_account_truncating(),
                &native_asset
            ),
            SignedBalance::Positive(PenaltyFee::get() + PenaltyFee::get())
        );
    });
}
