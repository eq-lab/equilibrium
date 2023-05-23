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

//! # Equilibrium Oracle Pallet
//!
//! 1. Various price sources are supported.
//! PriceSource - source of received price data points
//! Custom - custom data source, url template is used.
//! Pancake - price source that provides information for LP token price calculation (not our curve LP tokens!) .
//! JSON path expressions are being parsed to retrieve price data.
//! Once the price source is set up, prices for all currencies supported in the blockchain are fed from it.
//! The price source can be changed on the fly: the validator (node) who feeds the price can do it via an RPC call.

//! 2. Pancake price source gets data from pancake swap contract and calculate price for token.
//! It requires: BCS/ETH node url, contract address, asset settings in offchain storage and prices of pool tokens stored onchain.
//! It calls read methods on smart-contract and receives addresses of both pool tokens,
//! total supply of LP token, calculate LP token price and returns it.

//! 3. Adjustable frequency of price points, it may be changed on the fly. Prices may be fed no faster than once per block.

//! 4. Medianizer is a function/business-logic module which provides a reference median price and works the following way:
//! A single feeder always uses one price source per asset
//! (e.g. a single feeder can’t feed asset price from several different sources).
//! Median works well only when there are >=3 feeders (e.g. we’re able to calculate actual median).
//! In case of a single feeder his price is used as a reference, in case of two feeders,
//! their average price is calculated to obtain the reference price.
//! There is a PriceTimeout parameter which acts as a time-rolling window and shows
//! which data points from which feeders should be taken into account when calculating a reference (median) price.
//! No different data points from the same feeder are used in the reference price calculation.

//! Example:

//! if PriceTimeout is 1 minute:

//! 1. i feed price now - it is used in calculation
//! 2. someone feeds price in 40 seconds - it is used in calculation.
//! 3. someone feeds the price in 65 seconds - my price from step 1 is not used in the calculation.

//! There is a MedianPriceTimeout parameter - if the reference (median) price is not updated more than this timeout,
//! anyone willing to obtain the price will receive an error.

//! 5. Oracle is implemented using offchain workers (implements Substrate’s offchain worker).

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

use alloc::string::{String, ToString};
use core::{
    convert::{TryFrom, TryInto},
    str::FromStr,
};

use equilibrium_curve_amm::traits::CurveAmm;
use equilibrium_curve_amm::PoolId as CurvePoolId;
use financial_pallet::FinancialSystemTrait;
use financial_primitives::OnPriceSet;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
#[cfg(feature = "std")]
use frame_support::traits::GenesisBuild;
use frame_support::{
    codec::{Decode, Encode},
    dispatch::DispatchResult,
    traits::{Get, UnixTime},
};
use frame_system::offchain::{
    AppCrypto, CreateSignedTransaction, ForAll, SendUnsignedTransaction, SignedPayload, Signer,
    SigningTypes,
};
use sp_arithmetic::{FixedI64, FixedPointNumber};
use sp_core::crypto::KeyTypeId;
use sp_core::RuntimeDebug;
use sp_runtime::traits::{AtLeast32BitUnsigned, IdentifyAccount, Saturating};
use sp_runtime::RuntimeAppPublic;
use sp_runtime::{offchain::StorageKind, DispatchError};
use sp_std::{fmt::Debug, iter::Iterator, prelude::*};
use substrate_fixed::types::I64F64;

use crate::price_source::{
    custom::JsonPriceSource, pancake::PancakePriceSource, PriceSourceError, SourceType,
};
use eq_primitives::asset::{self, AmmPool, Asset, AssetData, AssetGetter, AssetType, OnNewAsset};
use eq_primitives::financial_storage::FinancialAssetRemover;
use eq_primitives::price::{PriceGetter, PriceSetter};
use eq_primitives::wrapped_dot::EqDotPrice;
use eq_primitives::xdot_pool::{XBasePrice, XdotPoolInfoTrait};
use eq_primitives::UnsignedPriorityPair;
use eq_primitives::{calculate_unsigned_priority, str_asset};
use eq_primitives::{Aggregates, AggregatesAssetRemover, LendingAssetRemoval, UserGroup};
use eq_utils::{
    eq_ensure,
    fixed::{fixedi64_from_balance, fixedi64_to_i64f64},
    ONE_TOKEN,
};
use eq_whitelists::CheckWhitelisted;
pub use pallet::*;
use price_source::PriceSource;
use sp_arithmetic::traits::UniqueSaturatedFrom;
use sp_runtime::traits::{One, Zero};
use sp_runtime::FixedPointOperand;
pub use weights::WeightInfo;

pub mod benchmarking;
mod mock;
mod price_source;
mod regex_offsets;
mod tests;
pub mod weights;

extern crate alloc;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"orac");
const DB_PREFIX: &[u8] = b"eq-orac/";
const REMOVE_ASSET_PERIOD: u32 = 10;

pub mod crypto {
    //! Module for signing operations

    use core::convert::TryFrom;
    use sp_core::sr25519::Signature as Sr25519Signature;
    use sp_runtime::app_crypto::{app_crypto, sr25519};
    use sp_runtime::traits::Verify;
    use sp_runtime::MultiSignature;

    use super::KEY_TYPE;

    app_crypto!(sr25519, KEY_TYPE);

    /// Struct for implementation of AppCrypto
    pub struct AuthId;
    impl frame_system::offchain::AppCrypto<<MultiSignature as Verify>::Signer, MultiSignature>
        for AuthId
    {
        type RuntimeAppPublic = Public;
        type GenericPublic = sp_core::sr25519::Public;
        type GenericSignature = sp_core::sr25519::Signature;
    }

    /// Struct for implementation of AppCrypto, used in unit tests
    pub struct TestAuthId;
    impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
        for TestAuthId
    {
        type RuntimeAppPublic = Public;
        type GenericPublic = sp_core::sr25519::Public;
        type GenericSignature = sp_core::sr25519::Signature;
    }
}

/// Payload for a price setting with an unsigned transaction
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct PricePayload<Public, BlockNumber> {
    public: Public,
    asset: Asset,
    price: FixedI64,
    block_number: BlockNumber,
}

impl<T: SigningTypes> SignedPayload<T> for PricePayload<T::Public, T::BlockNumber> {
    fn public(&self) -> T::Public {
        self.public.clone()
    }
}

/// Struct for storing added asset price data from one source
#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
pub struct DataPoint<AccountId, BlockNumber> {
    price: FixedI64,
    account_id: AccountId,
    block_number: BlockNumber,
    timestamp: u64,
}

impl<AccountId, BlockNumber> DataPoint<AccountId, BlockNumber> {
    pub fn get(&self) -> (&AccountId, i64) {
        (&self.account_id, self.price.into_inner())
    }
}

/// Struct for storing aggregated asset price data
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
pub struct PricePoint<AccountId, BlockNumber> {
    block_number: BlockNumber,
    timestamp: u64,
    last_fin_recalc_timestamp: u64,
    price: FixedI64,
    data_points: Vec<DataPoint<AccountId, BlockNumber>>,
}

impl<AccountId, BlockNumber: Default> Default for PricePoint<AccountId, BlockNumber> {
    fn default() -> PricePoint<AccountId, BlockNumber> {
        PricePoint {
            block_number: Default::default(),
            timestamp: Default::default(),
            last_fin_recalc_timestamp: Default::default(),
            price: Default::default(),
            data_points: Default::default(),
        }
    }
}

/// PricePoint implementation
impl<AccountId, BlockNumber> PricePoint<AccountId, BlockNumber> {
    pub fn get_block_number(&self) -> &BlockNumber {
        &self.block_number
    }

    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn get_last_fin_recalc_timestamp(&self) -> u64 {
        self.last_fin_recalc_timestamp
    }

    pub fn get_price(&self) -> i64 {
        self.price.into_inner()
    }

    pub fn get_data_points(&self) -> &Vec<DataPoint<AccountId, BlockNumber>> {
        &self.data_points
    }
}

/// Offchain storage accessor
struct OffchainStorage;
impl OffchainStorage {
    /// Gets query for price requests
    fn get_query() -> Option<String> {
        let query_param_raw =
            sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, b"oracle::custom_query");
        if let Some(query_param) = query_param_raw {
            String::from_utf8(query_param)
                .map_err(|e| {
                    log::error!(
                        "Storage value for custom_query cannot be converted to UTF-8. Error {:?}",
                        e
                    );

                    e
                })
                .ok()
        } else {
            log::error!("Storage value for custom_query does not exists");
            None
        }
    }

    /// Gets a value by the key
    fn get_local_storage_val<R: FromStr>(key: &str) -> Option<R> {
        let raw_val = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, key.as_bytes());
        match raw_val {
            Some(val_bytes) => match String::from_utf8(val_bytes.clone()) {
                Ok(val_decoded) => match val_decoded.parse::<R>() {
                    Ok(val) => Some(val),
                    Err(_e) => {
                        log::warn!("Can't parse local storage value {:?}", val_decoded);
                        None
                    }
                },
                Err(_e) => {
                    log::warn!("Can't decode local storage key {:?}: {:?}", key, val_bytes);
                    None
                }
            },
            None => {
                log::warn!("Uninitialized local storage key: {:?}", key);
                None
            }
        }
    }

    /// Get counter
    fn get_counter() -> Option<u32> {
        OffchainStorage::get_local_storage_val::<u32>("oracle::counter")
    }

    /// Get periodicity of price update
    fn get_price_periodicity() -> Option<u32> {
        Self::get_local_storage_val::<u32>("oracle::price_periodicity")
    }

    /// Update counter value
    fn set_counter(value: u32) {
        sp_io::offchain::local_storage_set(
            StorageKind::PERSISTENT,
            b"oracle::counter",
            value.to_string().as_bytes(),
        );
    }

    /// Get source type value
    fn get_source_type() -> Option<String> {
        OffchainStorage::get_local_storage_val::<String>("oracle::resource_type")
    }

    /// Returns collection of pairs (asset, price_strategy) available values for price_strategy is: "price", "reverse".
    /// Price_strategy defines how to serve value from price source for particular asset.
    /// If price_strategy == "price" then value recieved from price source is price.
    /// if price_strategy == "reverse" then price = 1 / value
    fn get_asset_settings() -> Option<Vec<(String, String)>> {
        // List of assets that require price setting
        // example USDC:price, USDT:price, BTC:price, DAI:reverse
        // example USDC, USDT, DAI:reverse, BTC
        OffchainStorage::get_local_storage_val::<String>("oracle::source_assets").map(
            |assets_str| {
                assets_str
                    .split(',')
                    .map(|pair_str| {
                        let mut split_pair = pair_str.split(':');

                        (
                            split_pair.next().unwrap().trim().to_lowercase(),
                            split_pair
                                .next()
                                .map(|v| v.trim().to_lowercase())
                                .unwrap_or(String::from("price")),
                        )
                    })
                    .collect::<Vec<_>>()
            },
        )
    }

    fn clear_asset_settings() {
        sp_io::offchain::local_storage_clear(
            StorageKind::PERSISTENT,
            "oracle::source_assets".as_bytes(),
        );
    }

    fn get_contract_address() -> Option<String> {
        OffchainStorage::get_local_storage_val::<String>("oracle::contract_address")
    }

    fn get_node_url() -> Option<String> {
        OffchainStorage::get_local_storage_val::<String>("oracle::node_url")
    }

    /// Should return exactly 3 token symbols.
    ///
    /// Example for WBNB/BUSD contract: WBNB, BUSD, LP_TOKEN
    fn get_pool_assets() -> Option<Vec<String>> {
        OffchainStorage::get_local_storage_val::<String>("oracle::pool_assets").and_then(|s| {
            let vec = s
                .split(',')
                .map(|c| c.trim().to_lowercase())
                .collect::<Vec<String>>();

            if vec.len() != 3 {
                log::warn!("Can't parse local storage value for pool tokens {:?}", s);
                None
            } else {
                Some(vec)
            }
        })
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use codec::Codec;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + CreateSignedTransaction<Call<Self>>
        + financial_pallet::Config
        + eq_assets::Config
        + financial_pallet::Config
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
        type Call: From<Call<Self>>;
        type FinMetricsRecalcToggleOrigin: EnsureOrigin<Self::Origin>;
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Default
            + Codec
            + From<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>
            + Copy
            + FixedPointOperand;
        /// Timestamp provider
        type UnixTime: UnixTime;
        /// Whitelist checks for price setters
        type Whitelist: CheckWhitelisted<Self::AccountId>;
        /// Pallet setting representing amount of time for which price median is valid
        #[pallet::constant]
        type MedianPriceTimeout: Get<u64>;
        /// Pallet setting representing amount of time for which price point is valid
        #[pallet::constant]
        type PriceTimeout: Get<u64>;
        /// Interface for feeding new prices into Financial pallet
        type OnPriceSet: OnPriceSet<Price = substrate_fixed::types::I64F64, Asset = Asset>;
        /// Interface for removing asset metrics from financial storages
        type FinancialAssetRemover: FinancialAssetRemover<Asset = Asset>;
        /// Interface for invoking asset metrics recalculation in Financial pallet
        type FinancialSystemTrait: FinancialSystemTrait<Asset = Asset, AccountId = Self::AccountId>;
        /// Time between recalculation assets financial data in ms
        #[pallet::constant]
        type FinancialRecalcPeriodBlocks: Get<Self::BlockNumber>;
        /// For priority calculation of an unsigned transaction
        #[pallet::constant]
        type UnsignedPriority: Get<UnsignedPriorityPair>;
        /// Used to deal with Assets
        type AssetGetter: AssetGetter;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        /// Curve Amm implementation
        type CurveAmm: equilibrium_curve_amm::traits::CurveAmm<
            AssetId = Asset,
            Balance = Self::Balance,
            AccountId = Self::AccountId,
        >;
        /// Timeout in blocks to recalculate LP token prices
        /// #[pallet::constant]
        type LpPriceBlockTimeout: Get<u64>;
        /// Trait for Xdot pricing
        type XBasePrice: XBasePrice<Asset, Self::Balance, FixedI64>;
        /// To get total amount of staked DOT
        type EqDotPrice: eq_primitives::wrapped_dot::EqDotPrice;
        /// Used to work with `TotalAggregates` storing aggregated collateral and debt
        type Aggregates: Aggregates<Self::AccountId, Self::Balance>;
        /// Removes entries while asset removal
        type AggregatesAssetRemover: AggregatesAssetRemover;
        /// Lifetime in blocks for unsigned transactions
        #[pallet::constant]
        type UnsignedLifetimeInBlocks: Get<u32>;
        /// Used to clear LendersAggregates, CumulatedRewards storages while asset removal
        type LendingAssetRemoval: LendingAssetRemoval<Self::AccountId>;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight((<T as Config>::WeightInfo::set_price(10), DispatchClass::Operational))]
        /// Adds and saves a new `DataPoint` containing an asset price information. It
        /// would be used for the `PricePoint` calculation. Only whitelisted
        /// accounts can add `DataPoints`
        pub fn set_price(
            origin: OriginFor<T>,
            asset: Asset,
            price: FixedI64,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();
            Self::validate_params(who.clone(), asset, price, current_block)?;

            <Self as PriceSetter<T::AccountId>>::set_price(who, asset, price)?;
            Ok(Pays::No.into())
        }

        #[pallet::weight((<T as Config>::WeightInfo::set_price(10), DispatchClass::Operational))]
        /// Adds new `DataPoint` from an unsigned transaction
        pub fn set_price_unsigned(
            origin: OriginFor<T>,
            payload: PricePayload<T::Public, T::BlockNumber>,
            _signature: T::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;
            let PricePayload {
                public,
                asset,
                price,
                block_number: _,
            } = payload;
            let who = public.into_account();
            Self::validate_params(who.clone(), asset, price, payload.block_number)?;
            <Self as PriceSetter<T::AccountId>>::set_price(who, asset, price)
        }

        #[pallet::weight(10_000)]
        /// Enables or disables auto recalculation of financial metrics
        pub fn set_fin_metrics_recalc_enabled(
            origin: OriginFor<T>,
            enabled: bool,
        ) -> DispatchResultWithPostInfo {
            T::FinMetricsRecalcToggleOrigin::ensure_origin(origin)?;
            <FinMetricsRecalcEnabled<T>>::put(enabled);
            log::trace!(target: "eq_oracle", "Auto recalc of financial metrics set to '{}'", enabled);
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Starts an off-chain task for a given block number
        fn offchain_worker(block_number: T::BlockNumber) {
            // collect the public keys
            let publics =
                <T::AuthorityId as AppCrypto<T::Public, T::Signature>>::RuntimeAppPublic::all()
                    .into_iter()
                    .enumerate()
                    .filter_map(|(_index, key)| {
                        let generic_public = <T::AuthorityId as AppCrypto<
                            T::Public,
                            T::Signature,
                        >>::GenericPublic::from(key);
                        let public: T::Public = generic_public.into();
                        let account_id = public.clone().into_account();
                        if T::Whitelist::in_whitelist(&account_id) {
                            Option::Some(public.clone())
                        } else {
                            Option::None
                        }
                    })
                    .collect();

            let signer = Signer::<T, T::AuthorityId>::all_accounts().with_filter(publics);
            if !signer.can_sign() {
                // not in the whitelist
                return;
            }
            //acquire a lock
            let lock_res = eq_utils::offchain::accure_lock(DB_PREFIX, || {
                // All oracles must set their own price feeding frequency
                // Oracle feeds prices every N blocks, where N = oracle::price_periodicity
                let maybe_price_periodicity = OffchainStorage::get_price_periodicity();
                if maybe_price_periodicity.is_none() {
                    log::warn!("Price periodicity setting doesn't exists");
                    return;
                }

                let price_periodicity = maybe_price_periodicity.unwrap();
                if price_periodicity < 1 {
                    log::warn!(
                        "Unexpected price periodicity {:?}, should be more or equal 1",
                        price_periodicity
                    );
                    return;
                }

                let counter = OffchainStorage::get_counter().unwrap_or(0_u32);
                let counter_next = counter + 1;

                if counter_next == price_periodicity {
                    OffchainStorage::set_counter(0_u32);

                    // Prices source (Custom, Pancake)
                    if let Some(source_type_name) = OffchainStorage::get_source_type() {
                        let source_type = SourceType::from(source_type_name);
                        if let Some(resource) = source_type {
                            Self::update_prices(resource, block_number, &signer);
                        } else {
                            log::warn!("Unexpected price resource type {:?}", source_type);
                            return;
                        }
                    }
                } else if counter_next > price_periodicity {
                    OffchainStorage::set_counter(0_u32);
                } else {
                    OffchainStorage::set_counter(counter_next);
                }
            });
            match lock_res {
                eq_utils::offchain::LockedExecResult::Executed => {
                    log::trace!(target: "eq_oracle", "eq_oracle offchain_worker:executed");
                }
                eq_utils::offchain::LockedExecResult::Locked => {
                    log::trace!(target: "eq_oracle", "eq_oracle offchain_worker:locked");
                }
            }
        }

        fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
            #[allow(unused_must_use)]
            T::OnPriceSet::on_price_set(asset::EQD, I64F64::from_num(1)).unwrap();

            let update_lp_token_prices = || -> DispatchResult {
                let update_price = |asset, amm_type| -> DispatchResult {
                    let lp_price = match amm_type {
                        AmmPool::Curve(pool_id) => Self::calc_curve_lp_token_price(pool_id)?,
                        AmmPool::Yield(pool_id) => {
                            let pool_info = T::XBasePrice::get_pool(pool_id)?;
                            let xbase_virtual_price =
                                T::XBasePrice::get_xbase_virtual_price(&pool_info, None)?;
                            let base_asset = pool_info.base_asset();
                            let base_price = Self::get_price(&base_asset)?;
                            let xbase_price = xbase_virtual_price.saturating_mul(base_price);
                            Self::set_the_only_price(pool_info.xbase_asset(), xbase_price);
                            // T::OnPriceSet::on_price_set(
                            //     pool_info.xbase_asset(),
                            //     fixedi64_to_i64f64(xbase_price),
                            // )?;
                            let lp_virtual_price =
                                T::XBasePrice::get_lp_virtual_price(&pool_info, None)?;
                            lp_virtual_price.saturating_mul(base_price)
                        }
                    };

                    Self::set_the_only_price(asset, lp_price);
                    T::OnPriceSet::on_price_set(asset, fixedi64_to_i64f64(lp_price))?;

                    Ok(())
                };

                for asset_data in T::AssetGetter::get_assets_data() {
                    let asset = asset_data.id;

                    if let AssetType::Lp(amm_type) = asset_data.asset_type {
                        // Ignore a price update error for the individual pool
                        // so all existing pools have a chance to update
                        let _ = update_price(asset, amm_type);
                    }
                }

                Ok(())
            };

            let block_timeout =
                T::BlockNumber::unique_saturated_from(T::LpPriceBlockTimeout::get());
            let current_block = frame_system::Pallet::<T>::block_number();
            if (current_block % block_timeout).is_zero() {
                let _ = update_lp_token_prices();
            }

            if (current_block % REMOVE_ASSET_PERIOD.into()).is_zero() {
                let _ = Self::remove_asset();
            }

            if <FinMetricsRecalcEnabled<T>>::get()
                && (current_block % T::FinancialRecalcPeriodBlocks::get()).is_zero()
            {
                let _ = T::FinancialSystemTrait::recalc_inner();
            }

            Weight::from_ref_time(10_000)
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new price added to the storage. The event contains: `Asset` for the price,
        /// `FixedI64` for the price value that was added, `FixedI64` for a new
        /// aggregated price and `AccountId` of the price submitter
        /// \[asset, new_value, aggregated, submitter\]
        NewPrice(Asset, FixedI64, FixedI64, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The account is not allowed to set prices
        NotAllowedToSubmitPrice,
        /// The same price data point has been already added
        PriceAlreadyAdded,
        /// Incorrect asset
        CurrencyNotFound,
        /// Attempting to submit a new price for constant price currencies
        WrongCurrency,
        /// The price cannot be zero
        PriceIsZero,
        /// The price cannot be negative
        PriceIsNegative,
        /// The price data point is too old and cannot be used
        PriceTimeout,
        /// This method is not allowed in production
        MethodNotAllowed,
        /// LP token pool is not found
        PoolNotFound,
        /// A primitive asset is expected
        PrimitiveAssetExpected,
    }

    /// Pallet storage for added price points
    #[pallet::storage]
    #[pallet::getter(fn price_points)]
    pub(super) type PricePoints<T: Config> = StorageMap<
        _,
        Identity,
        Asset,
        PricePoint<
            <T as frame_system::Config>::AccountId,
            <T as frame_system::Config>::BlockNumber,
        >,
        OptionQuery,
    >;

    #[pallet::type_value]
    pub fn DefaultForFinMetricsRecalcEnabled() -> bool {
        true
    }

    /// Stores flag for the automatic financial metrics recalculation at the start of each block
    #[pallet::storage]
    #[pallet::getter(fn fin_metrics_recalc_enabled)]
    pub type FinMetricsRecalcEnabled<T: Config> =
        StorageValue<_, bool, ValueQuery, DefaultForFinMetricsRecalcEnabled>;

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub prices: Vec<(u64, u64, u64)>,
        pub update_date: u64,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                prices: Default::default(),
                update_date: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |config: &GenesisConfig| {
                let default_price_point = PricePoint {
                    block_number: frame_system::Pallet::<T>::block_number(),
                    timestamp: 0,
                    last_fin_recalc_timestamp: 0,
                    price: FixedI64::saturating_from_integer(-1),
                    data_points: Vec::<DataPoint<T::AccountId, T::BlockNumber>>::new(),
                };
                // with chain spec
                for asset in T::AssetGetter::get_assets() {
                    <PricePoints<T>>::insert(asset, default_price_point.clone());
                }
                for &(asset, nom, denom) in config.prices.iter() {
                    let asset_typed =
                        Asset::new(asset).expect("Asset::new failed on build genesis");
                    if !T::AssetGetter::exists(asset_typed) {
                        panic!("Add price for not existing asset");
                    }
                    let price: FixedI64 = FixedI64::saturating_from_rational(nom, denom);
                    <PricePoints<T>>::mutate(&asset_typed, |maybe_price_point| {
                        let mut price_point: PricePoint<T::AccountId, T::BlockNumber> =
                            Default::default();
                        price_point.timestamp = config.update_date;
                        price_point.price = price;
                        *maybe_price_point = Some(price_point);
                    });
                }
            };
            extra_genesis_builder(self);
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::set_price_unsigned { payload, signature } = call {
                let signature_valid =
                    SignedPayload::<T>::verify::<T::AuthorityId>(payload, signature.clone());
                if !signature_valid {
                    return InvalidTransaction::BadProof.into();
                }

                let current_block = <frame_system::Pallet<T>>::block_number();

                if payload.block_number > current_block {
                    // transaction in future?
                    return InvalidTransaction::Stale.into();
                } else if payload.block_number + T::UnsignedLifetimeInBlocks::get().into()
                    < current_block
                {
                    // transaction was in pool for 5 blocks
                    return InvalidTransaction::Stale.into();
                }

                let account = payload.public.clone().into_account();

                Self::validate_params(account, payload.asset, payload.price, payload.block_number)
                    .map_err(|_| InvalidTransaction::Call)?;

                let priority =
                    calculate_unsigned_priority(&T::UnsignedPriority::get(), payload.block_number);

                ValidTransaction::with_tag_prefix("EqPrice")
                    .priority(priority)
                    .and_provides((payload.public.clone(), payload.asset))
                    .longevity(5) // hotfix, transfer to config
                    .propagate(true)
                    .build()
            } else {
                InvalidTransaction::Call.into()
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    fn find_asset_by_symbol(assets_data: &[AssetData<Asset>], symbol: &str) -> Option<Asset> {
        let asset = assets_data
            .iter()
            .find(|a| {
                let asset_symbol = str_asset!(a.id).unwrap().to_string();
                *asset_symbol == *symbol
            })
            .map(|a| a.id);

        if asset.is_none() {
            log::error!(
                "{}:{} Asset not found by symbol {:?}",
                file!(),
                line!(),
                symbol
            );
        }

        asset
    }

    /// Initializes price source and gets prices
    fn get_prices(resource: SourceType) -> Vec<(Asset, Result<FixedI64, PriceSourceError>)> {
        let assets_data = T::AssetGetter::get_assets_data();

        match resource {
            SourceType::Custom => {
                let json_source = JsonPriceSource::new(assets_data);
                if json_source.is_none() {
                    log::error!(
                        "{}:{} Error while creating Custom source.",
                        file!(),
                        line!()
                    );

                    return Vec::default();
                }

                json_source.unwrap().get_prices()
            }
            SourceType::Pancake => {
                let pool_assets = OffchainStorage::get_pool_assets()
                    .map(|vec| {
                        vec.iter()
                            .map(|symbol| Self::find_asset_by_symbol(&assets_data, symbol.as_str()))
                            .collect::<Option<Vec<Asset>>>()
                    })
                    .flatten();

                if pool_assets.is_none() {
                    log::error!(
                        "{}:{} Pool tokens setting is required for Pancake source.",
                        file!(),
                        line!(),
                    );

                    return Vec::default();
                }
                let pool_assets = pool_assets.unwrap();

                let (token_0, token_1, lp_token) = (pool_assets[0], pool_assets[1], pool_assets[2]);
                let token_0_price = Self::get_price(&token_0);
                let token_1_price = Self::get_price(&token_1);

                if token_0_price.is_err() || token_1_price.is_err() {
                    log::error!(
                        "{}:{} Prices are required for Pancake source. {:?} price {:?}, {:?} price:  {:?}",
                        file!(),
                        line!(),
                        token_0,
                        token_0_price,
                        token_1,
                        token_1_price
                    );

                    return Vec::default();
                }

                let pancake_source = PancakePriceSource::new(
                    token_0_price.unwrap(),
                    token_1_price.unwrap(),
                    lp_token,
                );

                if let Err(err) = pancake_source {
                    log::error!(
                        "{}:{} Error while creating Pancake source. Err {:?}",
                        file!(),
                        line!(),
                        err
                    );

                    return Vec::default();
                }

                pancake_source.unwrap().get_prices()
            }
        }
    }

    fn update_prices(
        source_type: price_source::SourceType,
        block_number: T::BlockNumber,
        signer: &Signer<T, T::AuthorityId, ForAll>,
    ) {
        for (asset, price_result) in Self::get_prices(source_type) {
            match price_result {
                Ok(price) => {
                    Self::submit_tx_update_price(asset, price, block_number, signer);
                }
                Err(err) => {
                    log::error!(
                        "{}:{} Price source return error.  Asset: {:?}, error: {:?}",
                        file!(),
                        line!(),
                        asset,
                        err,
                    );

                    //skip error, try update other asset prices
                    continue;
                }
            }
        }
    }

    /// Prepares unsigned transaction with new price
    fn submit_tx_update_price(
        asset: Asset,
        price: FixedI64,
        block_number: T::BlockNumber,
        signer: &Signer<T, T::AuthorityId, ForAll>,
    ) {
        if asset == asset::DOT {
            if T::AssetGetter::exists(asset::HDOT) {
                signer.send_unsigned_transaction(
                    |account| PricePayload {
                        public: account.public.clone(),
                        asset: asset::HDOT,
                        price: price,
                        block_number,
                    },
                    |payload, signature| Call::set_price_unsigned { payload, signature },
                );
            }

            if T::AssetGetter::exists(asset::STDOT) {
                signer.send_unsigned_transaction(
                    |account| PricePayload {
                        public: account.public.clone(),
                        asset: asset::STDOT,
                        price: price,
                        block_number,
                    },
                    |payload, signature| Call::set_price_unsigned { payload, signature },
                );
            }

            // TODO remove after YIELD !!!
            if T::AssetGetter::exists(asset::XDOT) {
                signer.send_unsigned_transaction(
                    |account| PricePayload {
                        public: account.public.clone(),
                        asset: asset::XDOT,
                        price: price,
                        block_number,
                    },
                    |payload, signature| Call::set_price_unsigned { payload, signature },
                );
            }

            if T::AssetGetter::exists(asset::XDOT2) {
                signer.send_unsigned_transaction(
                    |account| PricePayload {
                        public: account.public.clone(),
                        asset: asset::XDOT2,
                        price: price,
                        block_number,
                    },
                    |payload, signature| Call::set_price_unsigned { payload, signature },
                );
            }

            if T::AssetGetter::exists(asset::XDOT3) {
                signer.send_unsigned_transaction(
                    |account| PricePayload {
                        public: account.public.clone(),
                        asset: asset::XDOT3,
                        price: price,
                        block_number,
                    },
                    |payload, signature| Call::set_price_unsigned { payload, signature },
                );
            }

            if T::AssetGetter::exists(asset::EQDOT) {
                if let Some(eqdot_price_coeff) = T::EqDotPrice::get_price_coeff() {
                    signer.send_unsigned_transaction(
                        |account| PricePayload {
                            public: account.public.clone(),
                            asset: asset::EQDOT,
                            price: price * eqdot_price_coeff,
                            block_number,
                        },
                        |payload, signature| Call::set_price_unsigned { payload, signature },
                    );
                }
            }
        }

        signer.send_unsigned_transaction(
            |account| PricePayload {
                public: account.public.clone(),
                asset,
                price,
                block_number,
            },
            |payload, signature| Call::set_price_unsigned { payload, signature },
        );
    }

    /// Validates all the parameters
    fn validate_params(
        who: T::AccountId,
        asset: Asset,
        price: FixedI64,
        block_number: T::BlockNumber,
    ) -> DispatchResult {
        //log::info!("Who: {:?}", &who);
        eq_ensure!(
            T::Whitelist::in_whitelist(&who),
            Error::<T>::NotAllowedToSubmitPrice,
            target: "eq_oracle",
            "{}:{}. Account not in whitelist. Who: {:?}.",
            file!(),
            line!(),
            who
        );
        eq_ensure!(
            price != FixedI64::zero(),
            Error::<T>::PriceIsZero,
            target: "eq_oracle",
            "{}:{}. Price is equal to zero. Who: {:?}, price: {:?}, asset: {:?}.",
            file!(),
            line!(),
            who,
            price,
        str_asset!(asset)
        );
        eq_ensure!(
            !price.is_negative(),
            Error::<T>::PriceIsNegative,
            target: "eq_oracle",
            "{}:{}. Price is negative. Who: {:?}, price: {:?}, asset: {:?}.",
            file!(),
            line!(),
            who,
            price,
        str_asset!(asset)
        );
        eq_ensure!(
            asset != asset::EQD,
            Error::<T>::WrongCurrency,
            target: "eq_oracle",
            "{}:{}. 'USD' is not allowed to set price. Who: {:?}, price: {:?}, asset: {:?}.",
            file!(),
            line!(),
            who,
            price,
            str_asset!(asset)
        );
        eq_ensure!(
            T::AssetGetter::exists(asset),
            Error::<T>::WrongCurrency,
            target: "eq_oracle",
            "{}:{}. 'Unknown' is not allowed to set price. Who: {:?}, price: {:?}, asset: {:?}.",
            file!(),
            line!(),
            who,
            price,
            str_asset!(asset)
        );

        let maybe_price_point = PricePoints::<T>::get(asset);
        let price_already_added = maybe_price_point.clone().map(|price_point| {
            price_point
                .data_points
                .iter()
                .any(|x| x.account_id == who && x.block_number >= block_number)
        });
        if price_already_added.is_some() {
            eq_ensure!(
                !price_already_added.unwrap(),
                Error::<T>::PriceAlreadyAdded,
                target: "eq_oracle",
                "{}:{}. Account already set price. Who: {:?}, price: {:?}, block: {:?}, timestamp: {:?}.",
                file!(),
                line!(),
                who,
                maybe_price_point.clone().unwrap().price,
                maybe_price_point.clone().unwrap().block_number,
                maybe_price_point.clone().unwrap().timestamp
            );
        }

        return Ok(());
    }

    /// Calculates the Curve LP token price
    fn calc_curve_lp_token_price(
        pool_id: CurvePoolId,
    ) -> Result<FixedI64, sp_runtime::DispatchError> {
        let pool = T::CurveAmm::pool(pool_id).ok_or(Error::<T>::PoolNotFound)?;
        let assets = pool.assets;
        let prices = assets
            .into_iter()
            .map(|a| {
                let data = T::AssetGetter::get_asset_data(&a)?;

                if let AssetType::Lp(_) = data.asset_type {
                    Err(Error::<T>::PrimitiveAssetExpected.into())
                } else {
                    Self::get_price(&a)
                }
            })
            .collect::<Result<Vec<FixedI64>, sp_runtime::DispatchError>>()?;
        let prices_len = prices.len();
        let mean_price = if prices_len == 0 {
            FixedI64::zero()
        } else {
            let prices_sum = prices
                .into_iter()
                .fold(FixedI64::zero(), |acc, x| acc.saturating_add(x));

            prices_sum / FixedI64::saturating_from_integer(prices_len as u64)
        };

        let virtual_price = fixedi64_from_balance(
            // if there is no tokens, virtual price is 1
            T::CurveAmm::get_virtual_price(pool_id).unwrap_or(ONE_TOKEN.into()),
        )
        .ok_or(Error::<T>::PriceIsNegative)?;

        Ok(virtual_price.saturating_mul(mean_price))
    }

    /// A variant when a price is a single value
    pub fn set_the_only_price(asset: Asset, price: FixedI64) {
        let current_block = frame_system::Pallet::<T>::block_number();
        let current_time = <T as pallet::Config>::UnixTime::now().as_secs();
        let zeroes = [0u8; 32];
        let account_id = T::AccountId::decode(&mut &zeroes[..]).expect("Correct default account");

        let mut data_points = Vec::with_capacity(1);
        data_points.push(DataPoint {
            price,
            account_id: account_id.clone(),
            block_number: current_block,
            timestamp: current_time,
        });

        let price_point = PricePoint {
            block_number: current_block,
            timestamp: current_time,
            last_fin_recalc_timestamp: 0,
            price,
            data_points,
        };

        <PricePoints<T>>::insert(asset, price_point);

        Self::deposit_event(Event::NewPrice(asset, price, price, account_id));
    }

    /// Calculate a median over **sorted** price points
    fn calc_median_price(data_points: &Vec<DataPoint<T::AccountId, T::BlockNumber>>) -> FixedI64 {
        let len = data_points.len();
        let new_price = if len % 2 == 0 {
            (data_points[len / 2 - 1].price + data_points[len / 2].price)
                / (FixedI64::one() + FixedI64::one())
        } else {
            data_points[len / 2].price
        };

        new_price
    }

    /// Remove prices from `who` and recalc median price for each asset
    pub fn filter_prices_from(who: &T::AccountId) {
        T::AssetGetter::get_assets().iter().for_each(|asset| {
            <PricePoints<T>>::mutate_exists(asset, |maybe_price_point| {
                match maybe_price_point.as_mut() {
                    None => (),
                    Some(price_point) => {
                        let mut new_data_points = price_point
                            .data_points
                            .iter()
                            .cloned()
                            .filter(|data_point| data_point.account_id != *who)
                            .collect::<Vec<_>>();

                        if new_data_points.len() == 0 {
                            *maybe_price_point = None;
                        } else {
                            new_data_points.sort_by(|a, b| a.price.cmp(&b.price));
                            price_point.price = Self::calc_median_price(&new_data_points);
                            price_point.data_points = new_data_points;
                        }
                    }
                };
            });
        });
    }

    fn remove_asset() -> DispatchResult {
        let mut assets_to_remove = eq_assets::AssetsToRemove::<T>::get().unwrap_or(Vec::new());

        assets_to_remove.retain(|asset_to_remove| {
            let total_aggregates = T::Aggregates::get_total(UserGroup::Balances, *asset_to_remove);
            let balances_removed =
                total_aggregates.collateral.is_zero() && total_aggregates.debt.is_zero();

            if balances_removed {
                PricePoints::<T>::remove(asset_to_remove);
                T::FinancialAssetRemover::remove_asset(asset_to_remove);
                T::LendingAssetRemoval::remove_from_aggregates_and_rewards(asset_to_remove);
                T::AggregatesAssetRemover::remove_asset(asset_to_remove);
                let _ = T::FinancialSystemTrait::recalc_inner();

                let mut assets = T::AssetGetter::get_assets_data_with_usd();
                assets.retain(|asset| asset.id != *asset_to_remove);
                eq_assets::Assets::<T>::put(assets);
            }

            !balances_removed
        });

        eq_assets::AssetsToRemove::<T>::put(assets_to_remove);

        Ok(())
    }
}

impl<T: Config> PriceGetter for Pallet<T> {
    fn get_price<FixedNumber>(asset: &Asset) -> Result<FixedNumber, sp_runtime::DispatchError>
    where
        FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>,
    {
        if asset == &asset::EQD {
            return Ok(FixedNumber::one());
        }

        let item = <PricePoints<T>>::get(&asset).ok_or_else(|| {
            log::error!(
                "{}:{}. Currency not found in PricePoints. asset: {:?}.",
                file!(),
                line!(),
                str_asset!(asset)
            );
            Error::<T>::CurrencyNotFound
        })?;

        let price: FixedNumber = item
            .price
            .try_into()
            .map_err(|_| DispatchError::Other("FixedI64 convert"))?;
        eq_ensure!(
            price != FixedNumber::zero(),
            Error::<T>::PriceIsZero,
            target: "eq_oracle",
            "{}:{}. Price is equal to zero. Price: {:?}, asset: {:?}.",
            file!(),
            line!(),
            price,
        str_asset!(asset)
        );
        eq_ensure!(
            !price.is_negative(),
            Error::<T>::PriceIsNegative,
            target: "eq_oracle",
            "{}:{}. Price is negative. Price: {:?}, asset: {:?}.",
            file!(),
            line!(),
            price,
        str_asset!(asset)
        );
        let current_time = <T as pallet::Config>::UnixTime::now().as_secs();
        eq_ensure!(
            (current_time < item.timestamp + T::MedianPriceTimeout::get()),
            Error::<T>::PriceTimeout,
            target: "eq_oracle",
            "{}:{}. {:?} Price received after time is out. Current time: {:?}, price_point timestamp + {:?} seconds: {:?}.",
            file!(), line!(),str_asset!(asset), current_time, T::MedianPriceTimeout::get(), item.timestamp + T::MedianPriceTimeout::get()
        );
        Ok(price)
    }
}

impl<T: Config> PriceSetter<T::AccountId> for Pallet<T> {
    /// The actual implementation of updating an asset price value for the current timestamp
    fn set_price(who: T::AccountId, asset: Asset, price: FixedI64) -> DispatchResultWithPostInfo {
        let mut new_price = price;
        // mutate a price point in the storage by the asset
        <PricePoints<T>>::mutate(&asset, |maybe_price_point| {
            let mut price_point = maybe_price_point.clone().unwrap_or_default();
            let current_block = frame_system::Pallet::<T>::block_number();
            let current_time = <T as pallet::Config>::UnixTime::now().as_secs(); // always same within block
            if price_point.block_number == current_block {
                if price_point
                    .data_points
                    .iter()
                    .any(|x| x.account_id == who && x.block_number == current_block)
                {
                    log::error!("{}:{}. Account already set price. Who: {:?}, price: {:?}, block: {:?}, timestamp: {:?}.",
                    file!(), line!(), who, price_point.price, price_point.block_number, price_point.timestamp);
                    return Err(Error::<T>::PriceAlreadyAdded);
                }
            }

            price_point.block_number = current_block;
            price_point.timestamp = current_time;
            let dp = DataPoint {
                account_id: who.clone(),
                price: price,
                block_number: current_block,
                timestamp: current_time,
            };

            let mut actual_data_points: Vec<_> = price_point
                .data_points
                .iter()
                .filter_map(|x| {
                    if current_time - x.timestamp < T::PriceTimeout::get() && x.account_id != who {
                        Some(x.clone())
                    } else {
                        None
                    }
                })
                .chain(sp_std::iter::once(dp))
                .collect();

            // calculate a median over price points for the moment
            actual_data_points.sort_by(|a, b| a.price.cmp(&b.price));

            new_price = Self::calc_median_price(&actual_data_points);
            price_point.price = new_price;
            price_point.data_points = actual_data_points;
            log::trace!(
                target: "eq_oracle",
                "Med(Avg) calc price:{:?} new_price:{:?} {:?}",
                price,
                new_price,
                str_asset!(asset)
            );
            *maybe_price_point = Some(price_point);
            Ok(().into())
        })?;
        T::OnPriceSet::on_price_set(asset.clone(), fixedi64_to_i64f64(price))?;
        Self::deposit_event(Event::NewPrice(asset, price, new_price, who));

        Ok(().into())
    }
}

impl<T: Config> OnNewAsset for Pallet<T> {
    fn on_new_asset(asset: Asset, prices: Vec<FixedI64>) {
        match prices.first() {
            Some(price) => {
                Self::set_the_only_price(asset, *price);
            }
            None => {
                // do nothing
            }
        }
    }
}

/// Genesis boilerplate
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
