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

//! # Equilibrium Dex Pallet
//!

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(warnings)]

pub mod benchmarking;
mod mock;
mod tests;
pub mod weights;
pub use weights::WeightInfo;

use codec::{Decode, Encode};
use either::Either;
use eq_primitives::{
    asset::{Asset, AssetData, AssetGetter, EQD},
    balance::{BalanceGetter, EqCurrency},
    balance_number::EqFixedU128,
    offchain_batcher::{OffchainErr, OffchainResult, ValidatorOffchainBatcher},
    signed_balance::SignedBalance,
    subaccount::{SubAccType, SubaccountsManager},
    DeleteOrderReason, EqBuyout, MarginCallManager, MarginState, Order, OrderAggregateBySide,
    OrderAggregates, OrderChange, OrderId, OrderManagement, OrderSide, OrderType, Price,
    PriceGetter,
};
use eq_utils::{eq_ensure, fixed::balance_from_eq_fixedu128, ok_or_error, vec_map::VecMap};
use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    traits::{ExistenceRequirement, Get, WithdrawReasons},
};
use frame_system::{
    ensure_signed,
    offchain::{SendTransactionTypes, SubmitTransaction},
};
use sp_application_crypto::RuntimeAppPublic;
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::{
    traits::AccountIdConversion, ArithmeticError, DispatchError, DispatchResult, FixedI64,
    FixedPointNumber, RuntimeDebug,
};
use sp_std::prelude::*;
use sp_std::vec::Vec;

use crate::Operation::{Decrease, Increase};
use core::convert::TryInto;
use eq_primitives::OrderSide::{Buy, Sell};
use eq_primitives::OrderType::{Limit, Market};
use frame_support::traits::UnixTime;
pub use pallet::*;
use sp_arithmetic::traits::{CheckedSub, Zero};
use sp_runtime::traits::CheckedDiv;
use sp_std::borrow::Cow;
use sp_std::vec;

type ChunkKey = u64;
const DB_PREFIX: &[u8] = b"eq-dex/";

#[derive(Decode, Encode, Debug, Clone, Copy, Eq, PartialEq)]
enum Operation {
    Increase,
    Decrease,
}

#[derive(Decode, Encode, Debug, Clone, Eq, PartialEq, Default, scale_info::TypeInfo)]
pub struct BestPrice {
    pub ask: Option<Price>,
    pub bid: Option<Price>,
}

pub type AuthIndex = u32;

/// Request data for offchain signing.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct OperationRequestDexDeleteOrder<
    BlockNumber: Decode + Encode + Copy + BaseArithmetic,
    AccountId,
    Balance,
> {
    /// An asset to which the order belongs
    pub asset: Asset,
    /// An id of the order
    pub order_id: OrderId,
    /// Price of the order
    pub price: FixedI64,
    /// Order's owner
    pub who: AccountId,
    /// Amount of native asset to buyout if needed
    pub buyout: Option<Balance>,
    /// An index of the authority on the list of validators.
    pub authority_index: AuthIndex,
    /// The length of session validator set.
    pub validators_len: u32,
    /// Number of a block.
    pub block_num: BlockNumber,
    /// Order delete reason
    pub reason: DeleteOrderReason,
}

#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use eq_primitives::DeleteOrderReason;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    #[pallet::getter(fn order_id_counter)]
    pub(super) type OrderIdCounter<T: Config> = StorageValue<_, OrderId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn orders_by_asset_and_chunk_key)]
    pub(super) type OrdersByAssetAndChunkKey<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        Asset,
        Blake2_128Concat,
        ChunkKey,
        Vec<Order<T::AccountId>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn actual_price_chunks)]
    pub(super) type ActualChunksByAsset<T: Config> =
        StorageMap<_, Blake2_128Concat, Asset, Vec<ChunkKey>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn asset_ask_bid_prices)]
    pub(super) type BestPriceByAsset<T: Config> =
        StorageMap<_, Blake2_128Concat, Asset, BestPrice, ValueQuery>;

    /// Keep by every asset VecMap<Asset, OrderAggregateBySide>
    #[pallet::storage]
    #[pallet::getter(fn asset_weight)]
    pub(super) type AssetWeightByAccountId<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        VecMap<Asset, OrderAggregateBySide>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn asset_chunk_corridor)]
    pub(super) type ChunkCorridorByAsset<T: Config> =
        StorageMap<_, Blake2_128Concat, Asset, u32, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub chunk_corridors: Vec<(Asset, u32)>,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                chunk_corridors: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |config: &GenesisConfig| {
                for &(asset, chunk_corridor) in config.chunk_corridors.iter() {
                    <ChunkCorridorByAsset<T>>::insert(asset, chunk_corridor);
                }
            };
            extra_genesis_builder(self);
        }
    }

    #[cfg(feature = "std")]
    impl GenesisConfig {
        /// Direct implementation of `GenesisBuild::build_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn build_storage<T: Config>(&self) -> Result<sp_runtime::Storage, String> {
            <Self as GenesisBuild<T>>::build_storage(self)
        }

        /// Direct implementation of `GenesisBuild::assimilate_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn assimilate_storage<T: Config>(
            &self,
            storage: &mut sp_runtime::Storage,
        ) -> Result<(), String> {
            <Self as GenesisBuild<T>>::assimilate_storage(self, storage)
        }
    }

    #[pallet::config]
    pub trait Config:
        frame_system::Config + SendTransactionTypes<Call<Self>> + eq_rate::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type DeleteOrderOrigin: EnsureOrigin<Self::Origin>;

        type UpdateAssetCorridorOrigin: EnsureOrigin<Self::Origin>;
        /// Used for group orders in chunks. Should be positive value
        #[pallet::constant]
        type PriceStepCount: Get<u32>;
        /// Fee for deleting orders by offchain worker
        #[pallet::constant]
        type PenaltyFee: Get<Self::Balance>;
        /// Used for calculation unsigned transaction priority
        #[pallet::constant]
        type DexUnsignedPriority: Get<TransactionPriority>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        /// Used to execute batch operations for every `AuthorityId` key in keys storage
        type ValidatorOffchainBatcher: ValidatorOffchainBatcher<
            Self::AuthorityId,
            Self::BlockNumber,
            Self::AccountId,
        >;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::create_limit_order().max(<T as pallet::Config>::WeightInfo::create_market_order()))]
        pub fn create_order(
            origin: OriginFor<T>,
            asset: Asset,
            order_type: OrderType,
            side: OrderSide,
            amount: EqFixedU128,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            <Self as OrderManagement>::create_order(who, asset, order_type, side, amount)
        }

        /// Delete order.
        /// The dispatch origin for this call must be _None_ (unsigned transaction).
        #[pallet::call_index(1)]
        #[pallet::weight((<T as pallet::Config>::WeightInfo::delete_order() + <T as pallet::Config>::WeightInfo::validate_unsigned(),
                          DispatchClass::Operational))]
        pub fn delete_order(
            origin: OriginFor<T>,
            request: OperationRequestDexDeleteOrder<T::BlockNumber, T::AccountId, T::Balance>,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            Self::charge_penalty_fee(&request.who, request.buyout)?;

            <Self as OrderManagement>::delete_order(
                &request.asset,
                request.order_id,
                request.price,
                request.reason,
            )
        }

        /// Delete order. This must be called by order owner or root.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::delete_order_external())]
        pub fn delete_order_external(
            origin: OriginFor<T>,
            asset: Asset,
            order_id: OrderId,
            price: FixedI64,
        ) -> DispatchResultWithPostInfo {
            let maybe_who = match T::DeleteOrderOrigin::try_origin(origin) {
                Ok(_) => None,
                Err(o) => Some(ensure_signed(o)?),
            };
            if let Some(who) = &maybe_who {
                let order =
                    Self::find_order(&asset, order_id, price).ok_or(Error::<T>::OrderNotFound)?;

                T::SubaccountsManager::get_owner_id(&order.account_id)
                    .and_then(|(master_account_id, _)| (master_account_id == *who).then(|| ()))
                    .ok_or(Error::<T>::OnlyOwnerCanRemoveOrder)?;
            };

            <Self as OrderManagement>::delete_order(
                &asset,
                order_id,
                price,
                DeleteOrderReason::Cancel,
            )
        }

        /// Update stored asset corridor value
        #[pallet::call_index(3)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::update_asset_corridor())]
        pub fn update_asset_corridor(
            origin: OriginFor<T>,
            asset: Asset,
            new_corridor_value: u32,
        ) -> DispatchResultWithPostInfo {
            T::UpdateAssetCorridorOrigin::ensure_origin(origin)?;

            Self::do_update_asset_corridor(asset, new_corridor_value);
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Starts the off-chain task for given block number
        fn offchain_worker(block_number: T::BlockNumber) {
            // Only send messages if we are a potential validator
            if !sp_io::offchain::is_validator() {
                log::trace!(
                    target: "eq_dex",
                    "EqDex.offchain_worker({:?}): not a validator, skipping",
                    block_number
                );
                return;
            }
            // We don't want several checks in parallel, so we adding lock
            let lock_res = eq_utils::offchain::accure_lock(DB_PREFIX, || {
                // doesn't return error anyway, all errors are logged inside `execute_batch`
                #[allow(unused_must_use)]
                {
                    T::ValidatorOffchainBatcher::execute_batch(
                        block_number,
                        offchain::delete_unfit_orders::<T>,
                        "eq-dex",
                    );
                }
            });

            match lock_res {
                eq_utils::offchain::LockedExecResult::Executed => {
                    log::trace!(target: "eq_dex", "dex offchain_worker:executed");
                }
                eq_utils::offchain::LockedExecResult::Locked => {
                    log::trace!(target: "eq_dex", "dex offchain_worker:locked");
                }
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Order was created
        /// `[subaccount_id, order_id, asset, amount, price, side, created_at, expiration_time]`
        OrderCreated(
            T::AccountId,
            u64,
            Asset,
            EqFixedU128,
            FixedI64,
            OrderSide,
            u64,
            u64,
        ),
        /// Order was deleted
        /// `[account_id, order_id, asset, reason]`
        OrderDeleted(T::AccountId, u64, Asset, DeleteOrderReason),
        /// Orders matched
        /// `[asset, taker_rest, maker_price, maker_order_id, maker, taker, maker_fee, taker_fee, exchange_amount, maker_side]`
        Match(
            Asset,
            EqFixedU128,
            FixedI64,
            OrderId,
            T::AccountId,
            T::AccountId,
            T::Balance,
            T::Balance,
            EqFixedU128,
            OrderSide,
        ),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Only trader subaccounts may create DEX orders.
        AccountIsNotTrader,
        /// Only order originator may cancel an order
        OnlyOwnerCanRemoveOrder,
        /// No order found by order id and/or price
        OrderNotFound,
        /// Order price should be a positive value
        OrderPriceShouldBePositive,
        /// Order quantity should be a positive value
        OrderAmountShouldBePositive,
        /// Order amount should satisfy instrument lot size
        OrderAmountShouldSatisfyLot,
        /// Order price should satisfy instrument price step
        OrderPriceShouldSatisfyPriceStep,
        /// Order price should be in a corridor
        OrderPriceShouldBeInCorridor,
        /// Inconsistent storage state
        InconsistentStorage,
        /// Insufficient margin to place an order
        BadMargin,
        /// There is no best price for market order
        NoBestPriceForMarketOrder,
        /// Asset trading is disabled
        DexIsDisabledForAsset,
        /// Price step should be a positive value
        PriceStepShouldBePositive,
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            const INVALID_VALIDATORS_LEN: u8 = 10;
            const ORDER_NOT_FOUND: u8 = 15;
            match call {
                Call::delete_order { request, signature } => {
                    // verify that the incoming (unverified) pubkey is actually an authority id
                    log::error!("keys");
                    let keys = eq_rate::Keys::<T>::get();
                    if keys.len() as u32 != request.validators_len {
                        return InvalidTransaction::Custom(INVALID_VALIDATORS_LEN).into();
                    }
                    log::error!("authority_id");

                    let authority_id = match keys.get(request.authority_index as usize) {
                        Some(id) => id,
                        None => return InvalidTransaction::BadProof.into(),
                    };
                    log::error!("order");

                    let _order =
                        match Self::find_order(&request.asset, request.order_id, request.price) {
                            Some(ord) => ord,
                            None => return InvalidTransaction::Custom(ORDER_NOT_FOUND).into(),
                        };

                    log::error!("signature_valid");
                    // check signature (this is expensive so we do it last).
                    let signature_valid = request
                        .using_encoded(|encoded_req| authority_id.verify(&encoded_req, &signature));
                    if !signature_valid {
                        return InvalidTransaction::BadProof.into();
                    }
                    log::error!("signature_valid");

                    let priority = T::DexUnsignedPriority::get();

                    ValidTransaction::with_tag_prefix("DexDeleteOrder")
                        .priority(priority)
                        .and_provides(request.order_id)
                        .longevity(64)
                        .propagate(true)
                        .build()
                }
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}

mod offchain {
    use super::*;
    use eq_primitives::{Aggregates, UserGroup};

    pub(super) fn delete_unfit_orders<T: Config>(
        authority_index: u32,
        authority_key: T::AuthorityId,
        block: T::BlockNumber,
        validators_len: u32,
    ) -> OffchainResult<()> {
        let mut orders_data = get_orders_out_of_price_corridor_or_dex_disabled::<T>();
        orders_data.extend(get_orders_of_bad_margin_accounts::<T>());
        orders_data.sort_unstable_by(|a, b| {
            (a.0, a.1, a.2, a.3.clone()).cmp(&(b.0, b.1, b.2, b.3.clone()))
        });
        orders_data.dedup_by_key(|(asset, order_id, price, account_id, _)| {
            (
                asset.clone(),
                order_id.clone(),
                price.clone(),
                account_id.clone(),
            )
        });

        let native_asset = T::AssetGetter::get_main_asset();
        let penalty_fee = T::PenaltyFee::get();
        orders_data
            .into_iter()
            .filter(|(_, order_id, _, _, _)| {
                *order_id % Into::<u64>::into(validators_len) == Into::<u64>::into(authority_index)
            })
            .for_each(|(asset, order_id, price, account_id, reason)| {
                let buyout = match T::BalanceGetter::get_balance(&account_id, &native_asset) {
                    SignedBalance::Negative(amount) => Some(amount + penalty_fee),
                    SignedBalance::Positive(amount) => {
                        (penalty_fee > amount).then(|| penalty_fee - amount)
                    }
                };

                let _ = <Pallet<T>>::submit_tx_delete_order_for_single_authority(
                    asset,
                    order_id,
                    price,
                    account_id,
                    buyout,
                    authority_index,
                    authority_key.clone(),
                    block,
                    validators_len,
                    reason,
                );
            });

        Ok(())
    }

    /// Delete orders out of price corridor and for dex-disabled assets
    fn get_orders_out_of_price_corridor_or_dex_disabled<T: Config>(
    ) -> Vec<(Asset, OrderId, Price, T::AccountId, DeleteOrderReason)> {
        let mut orders_data = Vec::default();

        for asset_data in <T>::AssetGetter::get_assets_data() {
            let asset = asset_data.id;
            for (_chunk_key, orders) in <Pallet<T>>::iter_orders_by_asset(&asset) {
                for order in orders.into_iter() {
                    if !asset_data.is_dex_enabled {
                        orders_data.push((
                            asset,
                            order.order_id,
                            order.price,
                            order.account_id,
                            DeleteOrderReason::DisableTradingPair,
                        ));
                    } else if let Err(_) = <Pallet<T>>::ensure_order_in_corridor(asset, order.price)
                    {
                        orders_data.push((
                            asset,
                            order.order_id,
                            order.price,
                            order.account_id,
                            DeleteOrderReason::OutOfCorridor,
                        ));
                    };
                }
            }
        }

        orders_data
    }

    fn get_orders_of_bad_margin_accounts<T: Config>(
    ) -> Vec<(Asset, OrderId, Price, T::AccountId, DeleteOrderReason)> {
        let mut orders_data = Vec::new();

        let order_data_map = prepare_order_data_map::<T>();

        for account_id in T::Aggregates::iter_account(UserGroup::Borrowers) {
            if let Some(order_data_vec) = order_data_map.get(&account_id) {
                match T::MarginCallManager::check_margin(&account_id) {
                    Err(margin_error) => {
                        log::error!(
                            "{}:{}. Cant check margin for account. account_id: {:?}, error: {:?}",
                            file!(),
                            line!(),
                            account_id,
                            margin_error
                        );
                    }
                    Ok(MarginState::Good | MarginState::SubGood) => { /* Good Margin */ }
                    Ok(_) => orders_data.extend(order_data_vec.clone()),
                }
            }
        }

        orders_data
    }

    fn prepare_order_data_map<T: Config>(
    ) -> VecMap<T::AccountId, Vec<(Asset, OrderId, Price, T::AccountId, DeleteOrderReason)>> {
        <Pallet<T>>::iter_orders()
            .flat_map(|(asset, _, orders)| {
                orders
                    .into_iter()
                    .map(move |o| (o.account_id, asset, o.order_id, o.price))
            })
            .fold(
                VecMap::<T::AccountId, Vec<(Asset, OrderId, Price, T::AccountId, DeleteOrderReason)>>::new(),
                |mut acc, (account_id, asset, order_id, price)| {
                    match acc.get_mut(&account_id) {
                        Some(orders) => {
                            orders.push((asset, order_id, price, account_id, DeleteOrderReason::MarginCall));
                        }
                        None => {
                            acc.insert(
                                account_id.clone(),
                                vec![(asset, order_id, price, account_id, DeleteOrderReason::MarginCall)],
                            );
                        }
                    };
                    acc
                },
            )
    }
}

impl<T: Config> Pallet<T> {
    fn submit_tx_delete_order_for_single_authority(
        asset: Asset,
        order_id: OrderId,
        price: FixedI64,
        who: T::AccountId,
        buyout: Option<T::Balance>,
        authority_index: u32,
        authority_key: T::AuthorityId,
        block: T::BlockNumber,
        validators_len: u32,
        reason: DeleteOrderReason,
    ) -> OffchainResult<()> {
        let request = OperationRequestDexDeleteOrder::<T::BlockNumber, T::AccountId, T::Balance> {
            asset,
            order_id,
            price,
            who,
            buyout,
            authority_index,
            validators_len,
            block_num: block,
            reason,
        };

        let option_signature = authority_key.sign(&request.encode());
        let signature = ok_or_error!(
            option_signature,
            OffchainErr::FailedSigning,
            "{}:{}. Couldn't sign. Key: {:?}, authority_index: {:?}, \
                    validators_len: {:?}, block_num:{:?}.",
            file!(),
            line!(),
            authority_key,
            &request.authority_index,
            &request.validators_len,
            &request.block_num
        )?;
        let sign = signature.clone();
        let call = Call::delete_order { request, signature };

        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(
            |_| {
                log::trace!(
                    target: "eq_dex",
                    "{}:{}. Submit delete_order error. Asset: {:?}, order_id: {:?}, price: {:?}, signature: {:?}, \
                    authority_index: {:?}, validators_len: {:?}, block_num:{:?}.",
                    file!(),
                    line!(),
                    asset,
                    order_id,
                    price,
                    sign,
                    authority_index,
                    validators_len,
                    block
                );
                OffchainErr::SubmitTransaction
            },
        )?;

        Ok(())
    }

    fn charge_penalty_fee(
        who: &T::AccountId,
        buyout: Option<T::Balance>,
    ) -> DispatchResultWithPostInfo {
        let basic_asset = T::AssetGetter::get_main_asset();
        let amount = T::PenaltyFee::get();
        let treasury_acc = T::TreasuryModuleId::get().into_account_truncating();

        if let Some(amount) = buyout {
            T::EqBuyout::eq_buyout(who, amount)?;
        }

        T::EqCurrency::currency_transfer(
            who,
            &treasury_acc,
            basic_asset,
            amount,
            ExistenceRequirement::KeepAlive,
            eq_primitives::TransferReason::InterestFee,
            false,
        )?;

        Ok(().into())
    }

    /// Create limit order
    fn create_limit_order(
        borrower_id: T::AccountId,
        asset: Asset,
        price: Price,
        side: OrderSide,
        amount: EqFixedU128,
        expiration_time: u64,
        asset_data: &AssetData<Asset>,
    ) -> DispatchResultWithPostInfo {
        Self::ensure_price_satisfies_price_step(&borrower_id, &asset_data, price)?;
        Self::ensure_order_in_corridor(asset, price)?;

        let order_changes = &[OrderChange {
            asset,
            price,
            amount,
            side,
        }];

        // order creating isalways decreasing margin, so no need to check that margin was increased or not
        let (margin_state, _) =
            T::MarginCallManager::check_margin_with_change(&borrower_id, &[], order_changes)?;

        eq_ensure!(
            margin_state == MarginState::Good,
            Error::<T>::BadMargin,
            "{}:{}. Account should be with good margin. Account : {:?} margin_state {:?}.",
            file!(),
            line!(),
            borrower_id,
            margin_state,
        );

        let order_id = Self::get_order_id();
        let created_at = T::UnixTime::now().as_secs();

        let order = Order {
            order_id,
            account_id: borrower_id.clone(),
            amount,
            created_at,
            side,
            price,
            expiration_time,
        };

        let chunk_key = Self::get_chunk_key(price, asset_data.price_step)?;

        OrdersByAssetAndChunkKey::<T>::try_mutate_exists(
            asset,
            chunk_key,
            |maybe_orders| -> DispatchResult {
                match maybe_orders {
                    Some(orders) => {
                        let index = match orders.binary_search_by(|o| {
                            o.price
                                .cmp(&order.price)
                                .then(o.created_at.cmp(&order.created_at))
                                .then(o.order_id.cmp(&order.order_id))
                        }) {
                            Ok(matched_index) => matched_index + 1,
                            Err(index) => index,
                        };
                        orders.insert(index, order);
                    }
                    None => *maybe_orders = Some(vec![order]),
                };

                let _ = ActualChunksByAsset::<T>::try_mutate_exists(asset, |maybe_chunks| {
                    match maybe_chunks {
                        Some(chunks) => {
                            match chunks.binary_search(&chunk_key) {
                                Err(index) => {
                                    chunks.insert(index, chunk_key);
                                    Ok(())
                                }
                                _ => {
                                    Err(()) //chunk already exists
                                }
                            }
                        }
                        None => {
                            *maybe_chunks = Some(vec![chunk_key]);
                            Ok(())
                        }
                    }
                });

                let _ = BestPriceByAsset::<T>::try_mutate(asset, |best_price| match side {
                    Buy if best_price
                        .bid
                        .map(|best_price| price > best_price)
                        .unwrap_or(true) =>
                    {
                        best_price.bid = Some(price);
                        Ok(())
                    }
                    Sell if best_price
                        .ask
                        .map(|best_price| price < best_price)
                        .unwrap_or(true) =>
                    {
                        best_price.ask = Some(price);
                        Ok(())
                    }
                    _ => Err(()), //no need to update
                });

                Self::update_asset_weight(
                    borrower_id.clone(),
                    asset,
                    amount,
                    price,
                    side,
                    Increase,
                )?;

                Ok(())
            },
        )?;

        Self::deposit_event(Event::OrderCreated(
            borrower_id,
            order_id,
            asset,
            amount,
            price,
            side,
            created_at,
            expiration_time,
        ));

        Ok(().into())
    }

    fn update_asset_weight(
        account_id: T::AccountId,
        asset: Asset,
        amount: EqFixedU128,
        price: FixedI64,
        side: OrderSide,
        operation: Operation,
    ) -> DispatchResult {
        eq_ensure!(
            price.is_positive(),
            Error::<T>::OrderPriceShouldBePositive,
            target: "eq_dex",
            "{}:{}. Order price should be positive. Order owner: {:?}, price: {:?}",
            file!(),
            line!(),
            account_id,
            price,
        );
        let price = price.try_into().map_err(|_| ArithmeticError::Overflow)?;

        AssetWeightByAccountId::<T>::mutate(account_id, |asset_weights| -> DispatchResult {
            let order_aggregate = asset_weights.entry(asset).or_default();

            match operation {
                Increase => {
                    order_aggregate
                        .add(amount, price, side)
                        .ok_or(ArithmeticError::Overflow)?;
                }
                Decrease => {
                    order_aggregate
                        .sub(amount, price, side)
                        .ok_or(ArithmeticError::Underflow)?;
                    if order_aggregate.is_zero() {
                        asset_weights.remove(&asset);
                    }
                }
            }

            Ok(())
        })
    }

    fn get_order_id() -> OrderId {
        let order_id = Self::order_id_counter() + 1;
        OrderIdCounter::<T>::put(order_id);
        order_id
    }

    pub(crate) fn get_chunk_key(
        price: FixedI64,
        price_step: FixedI64,
    ) -> Result<u64, DispatchError> {
        eq_ensure!(
            price.is_positive(),
            Error::<T>::OrderPriceShouldBePositive,
            target: "eq_dex",
            "{}:{}. Order price should be positive. Price: {:?}",
            file!(),
            line!(),
            price,
        );

        let price_step_count = FixedI64::saturating_from_integer(T::PriceStepCount::get());
        eq_ensure!(
            !price_step.is_zero() && !price_step_count.is_zero(),
            Error::<T>::PriceStepShouldBePositive,
            target: "eq_dex",
            "{}:{}. Price step and price step count should be positive values. Price_step: {:?}, PriceStepCount: {:?}",
            file!(),
            line!(),
            price_step,
            price_step_count
        );

        let denominator = price_step_count * price_step;
        if denominator.is_zero() {
            frame_support::fail!(ArithmeticError::DivisionByZero);
        }
        let inner = price
            .checked_div(&denominator)
            .ok_or(ArithmeticError::DivisionByZero)?
            .into_inner();

        Ok((inner / FixedI64::accuracy()) as u64)
    }

    fn try_match(
        taker_account: &T::AccountId,
        taker_side: OrderSide,
        taker_type: OrderType,
        taker_amount: EqFixedU128,
        asset: &Asset,
    ) -> Result<Option<EqFixedU128>, DispatchError> {
        let asset_data = T::AssetGetter::get_asset_data(&asset)?;

        let ask_bid_prices = Self::asset_ask_bid_prices(asset);

        let (best_price, no_match_ord) = match taker_side {
            Buy => (ask_bid_prices.ask, core::cmp::Ordering::Less),
            Sell => (ask_bid_prices.bid, core::cmp::Ordering::Greater),
        };

        let best_price = match taker_type {
            Limit { ref price, .. } => {
                match best_price.filter(|best_price| price.cmp(best_price) != no_match_ord) {
                    Some(best_price) => best_price,
                    None => return Ok(Some(taker_amount)),
                }
            }
            Market => match best_price {
                Some(best_price) => best_price,
                None => return Err(Error::<T>::NoBestPriceForMarketOrder.into()),
            },
        };

        let chunks = Self::actual_price_chunks(asset);
        let chunk_key = Self::get_chunk_key(best_price, asset_data.price_step)?;
        let start_chunk_index = chunks
            .binary_search(&chunk_key)
            .map_err(|_| Error::<T>::InconsistentStorage)?;
        let chunks_directed = match taker_side {
            Buy => Either::Left(start_chunk_index..chunks.len()),
            Sell => Either::Right((0..start_chunk_index + 1).rev()),
        };

        let mut rest = taker_amount;
        'outer: for chunk_index in chunks_directed {
            let chunk_id = chunks[chunk_index];
            let chunk = Self::orders_by_asset_and_chunk_key(asset, chunk_id);

            let chunk_iterator = match taker_side {
                Buy => Either::Left(chunk.iter()),
                Sell => Either::Right(chunk.iter().rev()),
            };

            for maker_order in chunk_iterator {
                if taker_side == maker_order.side {
                    continue;
                }

                let price_match = match (taker_type, taker_side) {
                    (Limit { price, .. }, Buy) => price >= maker_order.price,
                    (Limit { price, .. }, Sell) => price <= maker_order.price,
                    (Market, _) => true,
                };

                if rest == EqFixedU128::zero() || !price_match {
                    break 'outer;
                }

                let delta_rest = Self::match_two_orders(
                    taker_account,
                    rest,
                    taker_type,
                    taker_side,
                    maker_order,
                    asset,
                )?;

                rest = rest - delta_rest;
            }
        }

        if rest.is_zero() {
            return Ok(None);
        }

        Ok(Some(rest))
    }

    /// Checks if `taker_price` and `taker_side` matches with `maker_order` and makes exchange.
    /// Arguments:
    /// - `taker_account` - taker's AccountId
    /// - `taker_rest` - the rest of taker's order
    /// - `taker_price` - taker's price
    /// - `taker_side` - taker's order side
    /// - `maker_order` - maker's order (order from storage)
    /// - `asset` - orders asset
    /// Returns the delta amount of taker's order.
    /// Returns None if exchange result with an error and deletes maker order.
    /// Rest is unchanged if there is no match by price.
    /// Maker's order will be deleted or modified. Maker's aggregate q(i) also will be modified.
    fn match_two_orders(
        taker_account: &T::AccountId,
        taker_rest: EqFixedU128,
        _taker_type: OrderType,
        taker_side: OrderSide,
        maker_order: &Order<T::AccountId>,
        asset: &Asset,
    ) -> Result<EqFixedU128, DispatchError> {
        let maker_account = &maker_order.account_id;
        let exchange_amount = taker_rest.min(maker_order.amount);
        let usd_amount = exchange_amount
            * maker_order
                .price
                .try_into()
                .map_err(|_| Error::<T>::OrderPriceShouldBePositive)?;
        let usd_amount_b =
            balance_from_eq_fixedu128::<T::Balance>(usd_amount).ok_or(ArithmeticError::Overflow)?;
        let exchange_amount_b = balance_from_eq_fixedu128::<T::Balance>(exchange_amount)
            .ok_or(ArithmeticError::Overflow)?;

        let pair = match taker_side {
            Buy => (&EQD, asset),
            Sell => (asset, &EQD),
        };

        let pair_amounts = match taker_side {
            Buy => (usd_amount_b, exchange_amount_b),
            Sell => (exchange_amount_b, usd_amount_b),
        };

        let asset_data = T::AssetGetter::get_asset_data(asset)?;
        let taker_fee_value = asset_data.taker_fee.mul_floor(usd_amount_b);
        let maker_fee_value = asset_data.maker_fee.mul_floor(usd_amount_b);

        T::EqCurrency::withdraw(
            taker_account,
            EQD,
            taker_fee_value,
            false,
            None,
            WithdrawReasons::empty(),
            ExistenceRequirement::AllowDeath,
        )?;

        T::EqCurrency::withdraw(
            maker_account,
            EQD,
            maker_fee_value,
            false,
            None,
            WithdrawReasons::empty(),
            ExistenceRequirement::AllowDeath,
        )?;

        let exchange_result =
            T::EqCurrency::exchange((taker_account, maker_account), pair, pair_amounts);

        let maker_exchange_failed = match exchange_result {
            Ok(()) => {
                // deposit maker&taker fee to Treasury
                T::EqCurrency::deposit_creating(
                    &T::TreasuryModuleId::get().into_account_truncating(),
                    EQD,
                    taker_fee_value + maker_fee_value,
                    false,
                    None,
                )?;

                false
            }
            Err((error, may_be_account)) => {
                T::EqCurrency::deposit_creating(taker_account, EQD, taker_fee_value, false, None)?;
                T::EqCurrency::deposit_creating(maker_account, EQD, maker_fee_value, false, None)?;

                // unwind if maker is not a source of exchange errror
                let account_id = may_be_account
                    .filter(|acc| acc != taker_account)
                    .ok_or(error)?;

                eq_ensure!(
                    &account_id == maker_account,
                    Error::<T>::InconsistentStorage,
                    target: "eq_dex",
                    "Exchange error could be only caused by {:?} or {:?}, but caused by {:?}",
                    taker_account,
                    maker_account,
                    account_id,
                );

                true
            }
        };

        if maker_exchange_failed || maker_order.amount == exchange_amount {
            <Self as OrderManagement>::delete_order(
                &asset,
                maker_order.order_id,
                maker_order.price,
                if maker_exchange_failed {
                    DeleteOrderReason::MakerError
                } else {
                    DeleteOrderReason::Match
                },
            )
            .map_err(|e| e.error)?;
        } else {
            let asset_data = T::AssetGetter::get_asset_data(asset)?;
            let chunk_key = Self::get_chunk_key(maker_order.price, asset_data.price_step)?;
            let new_amount = maker_order
                .amount
                .checked_sub(&exchange_amount)
                .ok_or(ArithmeticError::Overflow)?;
            let modified_order = Order {
                amount: new_amount,
                ..maker_order.clone()
            };
            OrdersByAssetAndChunkKey::<T>::try_mutate_exists(
                asset,
                chunk_key,
                |maybe_orders| -> DispatchResult {
                    match maybe_orders {
                        Some(orders) => {
                            match orders.binary_search_by(|o| {
                                o.price
                                    .cmp(&modified_order.price)
                                    .then(o.order_id.cmp(&modified_order.order_id))
                            }) {
                                Ok(i) => orders[i] = modified_order.clone(),
                                Err(_) => return Err(Error::<T>::InconsistentStorage.into()),
                            };
                        }
                        None => return Err(Error::<T>::InconsistentStorage.into()),
                    };
                    Ok(())
                },
            )?;
            Self::update_asset_weight(
                modified_order.account_id,
                *asset,
                exchange_amount, //modified_order.amount,
                modified_order.price,
                modified_order.side,
                Decrease,
            )?;
        };

        if maker_exchange_failed {
            Ok(EqFixedU128::zero())
        } else {
            // exchange_amount > 0
            Self::deposit_event(Event::Match(
                *asset,
                taker_rest - exchange_amount,
                maker_order.price,
                maker_order.order_id,
                maker_order.account_id.clone(),
                taker_account.clone(),
                maker_fee_value,
                taker_fee_value,
                exchange_amount,
                maker_order.side,
            ));

            Ok(exchange_amount)
        }
    }

    fn ensure_amount_satisfies_lot(
        who: &T::AccountId,
        asset_data: &AssetData<Asset>,
        amount: &EqFixedU128,
    ) -> DispatchResult {
        eq_ensure!(
            amount.is_positive(),
            Error::<T>::OrderAmountShouldBePositive,
            target: "eq_dex",
            "{}:{}. Order amount should be positive value. Who: {:?} amount {:?}.",
            file!(),
            line!(),
            who,
            amount
        );

        let satisfy_amount = amount.checked_div(&asset_data.lot).map_or_else(
            || false,
            |r| r > EqFixedU128::zero() && r.frac() == EqFixedU128::zero(),
        );

        eq_ensure!(
            satisfy_amount,
            Error::<T>::OrderAmountShouldSatisfyLot,
            target: "eq_dex",
            "{}:{}. Order amount {:?} should satisfy asset lot {:?}. Who {:?}",
            file!(),
            line!(),
            amount,
            asset_data.lot,
            who
        );

        Ok(())
    }

    fn ensure_price_satisfies_price_step(
        who: &T::AccountId,
        asset_data: &AssetData<Asset>,
        price: Price,
    ) -> DispatchResult {
        eq_ensure!(
            price.is_positive(),
            Error::<T>::OrderPriceShouldBePositive,
            target: "eq_dex",
            "{}:{}. Order price should be positive. Subaccount borrower id: {:?}, price: {:?}",
            file!(),
            line!(),
            who,
            price,
        );

        let satisfy_price = price.checked_div(&asset_data.price_step).map_or_else(
            || false,
            |r| r > FixedI64::zero() && r.frac() == FixedI64::zero(),
        );

        eq_ensure!(
            satisfy_price,
            Error::<T>::OrderPriceShouldSatisfyPriceStep,
            target: "eq_dex",
            "{}:{}. Order price {:?} should satisfy asset price_step {:?}. Who {:?}",
            file!(),
            line!(),
            price,
            asset_data.price_step,
            who
        );

        Ok(())
    }

    fn ensure_dex_is_enabled(asset_data: &AssetData<Asset>) -> DispatchResult {
        asset_data
            .is_dex_enabled
            .then(|| ())
            .ok_or(Error::<T>::DexIsDisabledForAsset.into())
    }

    fn ensure_order_in_corridor(asset: Asset, price: FixedI64) -> DispatchResult {
        eq_ensure!(
            price.is_positive(),
            Error::<T>::OrderPriceShouldBePositive,
            target: "eq_dex",
            "{}:{}. Order price should be positive. Price: {:?}",
            file!(),
            line!(),
            price,
        );

        let asset_data = T::AssetGetter::get_asset_data(&asset)?;
        let chunk_key = Self::get_chunk_key(price, asset_data.price_step)? as i64;
        let corridor = ChunkCorridorByAsset::<T>::get(&asset) as i64;
        let best_price = BestPriceByAsset::<T>::get(&asset);
        let oracle_price: FixedI64 = T::PriceGetter::get_price(&asset)?;

        let mid_price = match (best_price.ask, best_price.bid) {
            (None, None) => oracle_price,
            (None, Some(best_bid)) => oracle_price.max(best_bid),
            (Some(best_ask), None) => oracle_price.min(best_ask),
            (Some(best_ask), Some(best_bid)) => {
                let ask_price = oracle_price.min(best_ask);
                let bid_price = oracle_price.max(best_bid);

                (ask_price + bid_price) / FixedI64::from(2)
            }
        };

        let asset_mid_chunk: i64 = ((mid_price
            / (FixedI64::saturating_from_integer(T::PriceStepCount::get())
                * asset_data.price_step))
            .into_inner()
            / FixedI64::accuracy()) as i64;

        let compare_result =
            chunk_key >= asset_mid_chunk - corridor && chunk_key <= asset_mid_chunk + corridor;

        match compare_result {
            true => Ok(()),
            false => Err(Error::<T>::OrderPriceShouldBeInCorridor.into()),
        }
    }

    /// Only for offchain worker!
    pub(self) fn iter_orders_by_asset(
        asset: &Asset,
    ) -> Box<dyn Iterator<Item = (ChunkKey, Vec<Order<T::AccountId>>)>> {
        Box::new(<OrdersByAssetAndChunkKey<T>>::iter_prefix(asset))
    }

    /// Only for offchain worker!
    pub(self) fn iter_orders(
    ) -> Box<dyn Iterator<Item = (Asset, ChunkKey, Vec<Order<T::AccountId>>)>> {
        Box::new(OrdersByAssetAndChunkKey::<T>::iter())
    }

    fn do_update_asset_corridor(asset: Asset, new_corridor_value: u32) {
        // TODO: delete all orders / push orders again
        let old_corridor_value = <ChunkCorridorByAsset<T>>::get(asset);
        if old_corridor_value != new_corridor_value {
            <ChunkCorridorByAsset<T>>::insert(asset, new_corridor_value);
        }
    }
}

impl<T: Config> OrderManagement for Pallet<T> {
    type AccountId = T::AccountId;

    fn create_order(
        who: Self::AccountId,
        asset: Asset,
        order_type: OrderType,
        side: OrderSide,
        amount: EqFixedU128,
    ) -> DispatchResultWithPostInfo {
        let asset_data = T::AssetGetter::get_asset_data(&asset)?;
        let trading_acc_id = T::SubaccountsManager::get_subaccount_id(&who, &SubAccType::Trader)
            .ok_or(Error::<T>::AccountIsNotTrader)?;

        Self::ensure_dex_is_enabled(&asset_data)?;
        Self::ensure_amount_satisfies_lot(&who, &asset_data, &amount)?;

        match (
            order_type,
            Self::try_match(&trading_acc_id, side, order_type, amount, &asset)?,
        ) {
            (
                Limit {
                    price,
                    expiration_time,
                },
                Some(amount),
            ) => {
                Self::create_limit_order(
                    trading_acc_id,
                    asset,
                    price,
                    side,
                    amount,
                    expiration_time,
                    &asset_data,
                )?;
            }
            // order is fully matched, we don't need to do anything
            (Limit { .. }, None) => {}
            // market order is only for matching
            (Market, _) => {}
        }

        Ok(().into())
    }

    fn delete_order(
        asset: &Asset,
        order_id: OrderId,
        price: FixedI64,
        reason: DeleteOrderReason,
    ) -> DispatchResultWithPostInfo {
        fn get_ask_price<T: Config>(
            current_chunk: &[Order<T::AccountId>],
            start_chunk: ChunkKey,
            asset: &Asset,
        ) -> Option<Price> {
            log::error!("get_ask_price");
            current_chunk
                .iter()
                .map(|o| Cow::Borrowed(o))
                .chain(
                    ActualChunksByAsset::<T>::get(asset)
                        .iter()
                        .filter(|&&c| c > start_chunk)
                        .flat_map(|&c| OrdersByAssetAndChunkKey::<T>::get(asset, c))
                        .map(|o| Cow::Owned(o)),
                )
                .filter(|o| o.side == Sell)
                .map(|o| o.price)
                .next()
        }

        fn get_bid_price<T: Config>(
            current_chunk: &[Order<T::AccountId>],
            start_chunk: ChunkKey,
            asset: &Asset,
        ) -> Option<Price> {
            log::error!("get_bid_price");
            current_chunk
                .iter()
                .map(|o| Cow::Borrowed(o))
                .rev()
                .chain(
                    ActualChunksByAsset::<T>::get(asset)
                        .iter()
                        .rev()
                        .filter(|&&c| c < start_chunk)
                        .flat_map(|&c| {
                            OrdersByAssetAndChunkKey::<T>::get(asset, c)
                                .into_iter()
                                .rev()
                        })
                        .map(|o| Cow::Owned(o)),
                )
                .filter(|o| o.side == Buy)
                .map(|o| o.price)
                .next()
        }
        log::error!("find_order");
        let order = Self::find_order(&asset, order_id, price).ok_or(Error::<T>::OrderNotFound)?;
        log::error!("asset_data");
        let asset_data = T::AssetGetter::get_asset_data(asset)?;
        log::error!("chunk_key");
        let chunk_key = Self::get_chunk_key(order.price, asset_data.price_step)?;
        log::error!("mutate_exists");
        OrdersByAssetAndChunkKey::<T>::mutate_exists(
            asset,
            chunk_key,
            |maybe_orders| -> DispatchResult {
                match maybe_orders.as_mut() {
                    None => Err(Error::<T>::InconsistentStorage),
                    Some(orders) => {
                        let index = orders
                            .binary_search_by(|i| {
                                i.price
                                    .cmp(&order.price)
                                    .then(i.order_id.cmp(&order.order_id))
                            })
                            .map_err(|_| Error::<T>::InconsistentStorage)?;

                        let removed = orders.remove(index);

                        let _ = BestPriceByAsset::<T>::try_mutate(asset, |b| match removed.side {
                            Sell if Some(removed.price) == b.ask => {
                                //search lowest price in current and next chunk
                                b.ask = get_ask_price::<T>(orders, chunk_key, asset);
                                Ok(())
                            }
                            Buy if Some(removed.price) == b.bid => {
                                //search highest price in current and prev chunk
                                b.bid = get_bid_price::<T>(orders, chunk_key, asset);
                                Ok(())
                            }
                            _ => Err(()),
                        });

                        if orders.len() == 0 {
                            *maybe_orders = None;

                            ActualChunksByAsset::<T>::try_mutate(
                                asset,
                                |chunks| -> DispatchResult {
                                    let index = chunks
                                        .binary_search(&chunk_key)
                                        .map_err(|_| Error::<T>::InconsistentStorage)?;
                                    chunks.remove(index);

                                    Ok(())
                                },
                            )?;
                        }

                        Self::update_asset_weight(
                            removed.account_id.clone(),
                            *asset,
                            removed.amount,
                            removed.price,
                            removed.side,
                            Decrease,
                        )?;

                        Ok(())
                    }
                }?;

                Ok(())
            },
        )?;

        Self::deposit_event(Event::OrderDeleted(
            order.account_id,
            order_id,
            *asset,
            reason,
        ));

        Ok(().into())
    }

    fn find_order(
        asset: &Asset,
        order_id: OrderId,
        price: Price,
    ) -> Option<Order<Self::AccountId>> {
        let asset_data = T::AssetGetter::get_asset_data(asset).ok()?;
        let chunk_key = Self::get_chunk_key(price, asset_data.price_step).ok()?;

        match <OrdersByAssetAndChunkKey<T>>::try_get(asset, chunk_key) {
            Ok(orders) => {
                match orders
                    .binary_search_by(|o| o.price.cmp(&price).then(o.order_id.cmp(&order_id)))
                {
                    Ok(index) => Some(orders[index].clone()),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl<T: Config> OrderAggregates<T::AccountId> for Pallet<T> {
    fn get_asset_weights(account_id: &T::AccountId) -> VecMap<Asset, OrderAggregateBySide> {
        AssetWeightByAccountId::<T>::try_get(account_id)
            .map_or(None, |val| Some(val))
            .unwrap_or_default()
    }
}