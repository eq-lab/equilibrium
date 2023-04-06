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

//! # Equilibrium Assets pallet.
//!
//! Equilibrium Assets pallet is a Substrate module that provides
//! the functionality used for asset abstraction in the network.
//!
//! The asset data is a lookup table containing such properties as
//! a minimal lot of the asset, the asset price step, maker and taker fees, a buyout priority which
//! is taken into account when a margin call happens etc

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

#[allow(unused_imports)]
use core::{convert::TryInto, marker::PhantomData};
use eq_primitives::{
    asset::{
        Asset, AssetData, AssetError, AssetGetter, AssetType, AssetXcmData, AssetXcmGetter,
        OnNewAsset,
    },
    balance_number::EqFixedU128,
};
use eq_utils::eq_ensure;
#[allow(unused_imports)]
use frame_support::debug;
use frame_support::dispatch::DispatchResultWithPostInfo;
use frame_support::traits::Get;
pub use pallet::*;
use sp_runtime::{traits::Zero, DispatchError, FixedI64, FixedPointNumber, Percent, Permill};
use sp_std::vec::Vec;
use xcm::latest::AssetId;

#[cfg(test)]
mod mock;

pub mod migration;
#[cfg(test)]
mod tests;

pub mod benchmarking;
pub mod weights;
pub use weights::WeightInfo;

pub type AssetName = Vec<u8>;

#[frame_support::pallet]
pub mod pallet {
    use crate::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event> + IsType<<Self as frame_system::Config>::Event>;

        /// Network native asset
        /// Commissions are paid in this asset
        #[pallet::constant]
        type MainAsset: Get<eq_primitives::asset::Asset>;

        type OnNewAsset: OnNewAsset;

        type AssetManagementOrigin: EnsureOrigin<Self::Origin>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn assets)]
    pub type Assets<T: Config> = StorageValue<_, Vec<AssetData<Asset>>>;

    #[pallet::storage]
    #[pallet::getter(fn assets_to_remove)]
    pub type AssetsToRemove<T: Config> = StorageValue<_, Vec<Asset>>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub assets: Vec<(
            eq_primitives::asset::AssetIdInnerType, // u64
            EqFixedU128,                            // lot
            FixedI64,                               // price step
            Permill,                                // maker fee
            Permill,                                // taker fee
            Vec<u8>,                                // raw xcm data
            Permill,                                // debt weight
            u64,                                    // buyout priority
            AssetType,                              // asset type (Synthetic / Physical)
            bool,                                   // is the asset enabled in the DEX
            Percent,                                // collateral discount
            Permill,                                // lending debt weight
        )>,
        pub _runtime: PhantomData<T>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                assets: Vec::new(),
                _runtime: PhantomData,
            }
        }
    }

    /// Genesis
    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |config: &GenesisConfig<T>| {
                for (
                    id,
                    lot,
                    price_step,
                    maker_fee,
                    taker_fee,
                    raw_xcm_data,
                    debt_weight,
                    buyout_priority,
                    asset_type,
                    is_dex_enabled,
                    collateral_discount,
                    lending_debt_weight,
                ) in config.assets.iter().cloned()
                {
                    let asset_xcm_data = Decode::decode(&mut &raw_xcm_data[..]).unwrap_or_default();
                    <Pallet<T>>::do_add_asset(
                        Asset::new(id).expect("Asset::new failed on build genesis"),
                        lot,
                        price_step,
                        maker_fee,
                        taker_fee,
                        asset_xcm_data,
                        debt_weight,
                        buyout_priority,
                        asset_type,
                        is_dex_enabled,
                        collateral_discount,
                        lending_debt_weight,
                        vec![],
                    )
                    .expect("do_add_asset failed on build genesis");
                }
            };
            extra_genesis_builder(self);
        }
    }

    #[cfg(feature = "std")]
    impl<T: Config> GenesisConfig<T> {
        /// Direct implementation of `GenesisBuild::build_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn build_storage(&self) -> Result<sp_runtime::Storage, String> {
            <Self as GenesisBuild<T>>::build_storage(self)
        }

        /// Direct implementation of `GenesisBuild::assimilate_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn assimilate_storage(&self, storage: &mut sp_runtime::Storage) -> Result<(), String> {
            <Self as GenesisBuild<T>>::assimilate_storage(self, storage)
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event {
        /// New asset added to store  \[asset, asset_name\]
        NewAsset(eq_primitives::asset::AssetIdInnerType, Vec<u8>),
        /// Asset removed from store \[asset, asset_name\]
        DeleteAsset(eq_primitives::asset::AssetIdInnerType, Vec<u8>),
        /// Asset updated in the store \[asset, asset_name\]
        UpdateAsset(eq_primitives::asset::AssetIdInnerType, Vec<u8>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Asset with the same AssetId already exists
        AssetAlreadyExists,
        /// Cannot delete an asset that does not exist
        AssetNotExists,
        /// Asset was already requested to be removed
        AssetAlreadyToBeRemoved,
        /// Debt weight cannot be negative
        DebtWeightNegative,
        /// Debt weight cannot be over 100%
        DebtWeightMoreThanOne,
        /// Asset name is too long
        AssetNameWrongLength,
        /// Asset name contains a wrong symbol.
        /// Only standard latin letters and digits are allowed.
        AssetNameWrongSymbols,
        /// New asset without prices cannot be collateral.
        CollateralMustBeDisabledWithoutPrices,
        /// Price step cannot be negative
        PriceStepNegative,
        /// Operation not allowed for native asset
        Native,
        /// Collateral discount is negative
        CollateralDiscountNegative,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            let _ = Assets::<T>::translate(migration::migrate_assets_data);
            Weight::from_ref_time(1)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Constructs and adds an asset
        #[pallet::weight(T::WeightInfo::add_asset())]
        pub fn add_asset(
            origin: OriginFor<T>,
            asset_name: AssetName,
            lot: u128,
            price_step: i64,
            maker_fee: Permill,
            taker_fee: Permill,
            asset_xcm_data: AssetXcmData,
            debt_weight: Permill,
            buyout_priority: u64,
            asset_type: AssetType,
            is_dex_enabled: bool,
            collateral_discount: Percent,
            lending_debt_weight: Permill,
            prices: Vec<FixedI64>,
        ) -> DispatchResultWithPostInfo {
            T::AssetManagementOrigin::ensure_origin(origin)?;

            if asset_type == AssetType::Native {
                frame_support::fail!(Error::<T>::Native)
            }

            let asset = Asset::from_bytes(&asset_name).map_err(Self::map_asset_error)?;

            eq_ensure!(
                !(prices.is_empty() && !collateral_discount.is_zero()),
                Error::<T>::CollateralMustBeDisabledWithoutPrices,
                target: "eq_assets",
                "Collateral flag must be disabled.",
            );

            Self::do_add_asset(
                asset,
                EqFixedU128::from_inner(lot),
                FixedI64::from_inner(price_step),
                maker_fee,
                taker_fee,
                asset_xcm_data,
                debt_weight,
                buyout_priority,
                asset_type,
                is_dex_enabled,
                collateral_discount,
                lending_debt_weight,
                prices,
            )?;

            Ok(().into())
        }

        /// Call to remove asset from eq_assets::Assets, eq_oracle, eq_lenders and financial_pallet storages
        /// Doesn't affect mm, xdot and curve pools
        #[pallet::weight(T::WeightInfo::remove_asset())]
        pub fn remove_asset(origin: OriginFor<T>, asset_id: Asset) -> DispatchResultWithPostInfo {
            T::AssetManagementOrigin::ensure_origin(origin)?;

            let mut assets = Self::get_assets_data();
            let mut assets_to_remove = Self::assets_to_remove().unwrap_or(Vec::new());

            match assets.binary_search_by(|x| x.id.cmp(&asset_id)) {
                Ok(idx) if assets[idx].asset_type == AssetType::Native => {
                    frame_support::fail!(Error::<T>::Native)
                }
                Ok(idx) => {
                    if assets_to_remove
                        .iter()
                        .find(|&asset| *asset == asset_id)
                        .is_some()
                    {
                        frame_support::fail!(Error::<T>::AssetAlreadyToBeRemoved)
                    }
                    assets[idx].debt_weight = Permill::zero();
                    assets[idx].lending_debt_weight = Permill::zero();
                    assets[idx].is_dex_enabled = false;
                    <Assets<T>>::put(assets);

                    assets_to_remove.push(asset_id);
                    AssetsToRemove::<T>::put(assets_to_remove);
                }
                Err(_) => frame_support::fail!(Error::<T>::AssetNotExists),
            };

            Self::deposit_event(Event::DeleteAsset(
                asset_id.get_id(),
                asset_id.to_str_bytes(),
            ));
            Ok(().into())
        }

        /// Updates an asset
        #[pallet::weight(T::WeightInfo::update_asset())]
        pub fn update_asset(
            origin: OriginFor<T>,
            asset_id: Asset,
            lot: Option<u128>,
            price_step: Option<i64>,
            maker_fee: Option<Permill>,
            taker_fee: Option<Permill>,
            asset_xcm_data: Option<AssetXcmData>,
            debt_weight: Option<Permill>,
            buyout_priority: Option<u64>,
            asset_type: Option<AssetType>,
            is_dex_enabled: Option<bool>,
            collateral_discount: Option<Percent>,
            lending_debt_weight: Option<Permill>,
        ) -> DispatchResultWithPostInfo {
            T::AssetManagementOrigin::ensure_origin(origin)?;

            Self::do_update_asset(
                asset_id,
                lot,
                price_step,
                maker_fee,
                taker_fee,
                asset_xcm_data,
                debt_weight,
                buyout_priority,
                asset_type,
                is_dex_enabled,
                collateral_discount,
                lending_debt_weight,
            )?;

            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Adds an asset
    pub fn do_add_asset(
        asset: Asset,
        lot: EqFixedU128,
        price_step: FixedI64,
        maker_fee: Permill,
        taker_fee: Permill,
        asset_xcm_data: AssetXcmData,
        debt_weight: Permill,
        buyout_priority: u64,
        asset_type: AssetType,
        is_dex_enabled: bool,
        collateral_discount: Percent,
        lending_debt_weight: Permill,
        prices: Vec<FixedI64>,
    ) -> DispatchResultWithPostInfo {
        let new_asset = AssetData::new(
            asset,
            lot,
            price_step,
            maker_fee,
            taker_fee,
            asset_xcm_data,
            debt_weight,
            buyout_priority,
            asset_type,
            is_dex_enabled,
            collateral_discount,
            lending_debt_weight,
        )
        .map_err(Self::map_asset_error)?;

        let mut assets = Self::get_assets_data();

        match assets.binary_search_by(|x| x.id.cmp(&asset)) {
            Ok(_) => frame_support::fail!(Error::<T>::AssetAlreadyExists),
            Err(idx) => assets.insert(idx, new_asset),
        };

        <Assets<T>>::put(assets);

        T::OnNewAsset::on_new_asset(asset, prices);
        Self::deposit_event(Event::NewAsset(asset.get_id(), asset.to_str_bytes()));
        Ok(().into())
    }

    /// Adds an asset
    pub fn do_update_asset(
        asset: Asset,
        lot: Option<u128>,
        price_step: Option<i64>,
        maker_fee: Option<Permill>,
        taker_fee: Option<Permill>,
        asset_xcm_data: Option<AssetXcmData>,
        debt_weight: Option<Permill>,
        buyout_priority: Option<u64>,
        asset_type: Option<AssetType>,
        is_dex_enabled: Option<bool>,
        collateral_discount: Option<Percent>,
        lending_debt_weight: Option<Permill>,
    ) -> DispatchResultWithPostInfo {
        let mut assets = Self::get_assets_data();

        match assets.binary_search_by(|x| x.id.cmp(&asset)) {
            Ok(idx) => {
                let updated_asset = &mut assets[idx];

                if let Some(lot) = lot {
                    updated_asset.lot = EqFixedU128::from_inner(lot);
                }
                if let Some(price_step) = price_step {
                    let price_step_value = FixedI64::from_inner(price_step);
                    if price_step_value.is_negative() {
                        return Err(Error::<T>::PriceStepNegative.into());
                    }
                    updated_asset.price_step = price_step_value;
                }
                if let Some(maker_fee) = maker_fee {
                    updated_asset.maker_fee = maker_fee;
                }
                if let Some(taker_fee) = taker_fee {
                    updated_asset.taker_fee = taker_fee;
                }
                if let Some(asset_xcm_data) = asset_xcm_data {
                    updated_asset.asset_xcm_data = asset_xcm_data;
                }
                if let Some(debt_weight) = debt_weight {
                    updated_asset.debt_weight = debt_weight;
                }
                if let Some(buyout_priority) = buyout_priority {
                    updated_asset.buyout_priority = buyout_priority;
                }
                if let Some(asset_type) = asset_type {
                    updated_asset.asset_type = asset_type;
                }
                if let Some(is_dex_enabled) = is_dex_enabled {
                    updated_asset.is_dex_enabled = is_dex_enabled;
                }
                if let Some(collateral_discount) = collateral_discount {
                    updated_asset.collateral_discount = collateral_discount;
                }
                if let Some(lending_debt_weight) = lending_debt_weight {
                    updated_asset.lending_debt_weight = lending_debt_weight;
                }
            }
            Err(_) => frame_support::fail!(Error::<T>::AssetNotExists),
        };

        <Assets<T>>::put(assets);

        Self::deposit_event(Event::UpdateAsset(asset.get_id(), asset.to_str_bytes()));

        Ok(().into())
    }

    /// Gets all asset data
    fn get_assets_data() -> Vec<AssetData<Asset>> {
        Self::assets().unwrap_or(Vec::<AssetData<Asset>>::new())
    }

    /// Gets asset data for a specific asset id
    fn get_asset_data(asset_id: &Asset) -> Result<AssetData<Asset>, DispatchError> {
        let assets = Self::get_assets_data();

        match assets.binary_search_by(|x| x.id.cmp(&asset_id)) {
            Ok(idx) => Ok(assets[idx].clone()),
            Err(_) => Err(Error::<T>::AssetNotExists.into()),
        }
    }

    /// Gets main asset
    fn get_main_asset() -> Asset {
        let asset = T::MainAsset::get();
        asset
    }

    /// Technical error mapping
    fn map_asset_error(error: AssetError) -> Error<T> {
        match error {
            AssetError::AssetNameWrongLength => Error::<T>::AssetNameWrongLength,
            AssetError::AssetNameWrongSymbols => Error::<T>::AssetNameWrongSymbols,
            AssetError::DebtWeightMoreThanOne => Error::<T>::DebtWeightMoreThanOne,
            AssetError::DebtWeightNegative => Error::<T>::DebtWeightNegative,
            AssetError::PriceStepNegative => Error::<T>::PriceStepNegative,
            AssetError::CollateralDiscountNegative => Error::<T>::CollateralDiscountNegative,
        }
    }
}

/// Functionality available outside the pallet
impl<T: Config> AssetGetter for Pallet<T> {
    /// Gets asset data for a specific asset id
    fn get_asset_data(asset_id: &Asset) -> Result<AssetData<Asset>, DispatchError> {
        Self::get_asset_data(asset_id).into()
    }

    /// Gets all asset data filtering out the native stable coin USD/EQD
    fn get_assets_data() -> Vec<AssetData<Asset>> {
        Self::get_assets_data()
            .into_iter()
            .filter(|a| a.id != eq_primitives::asset::EQD)
            .collect()
    }

    /// Gets all asset data
    fn get_assets_data_with_usd() -> Vec<AssetData<Asset>> {
        Self::get_assets_data()
    }

    /// Gets ids of assets (except USD/EQD)
    fn get_assets() -> Vec<Asset> {
        Self::get_assets_data()
            .iter()
            .filter_map(|a| {
                if a.id != eq_primitives::asset::EQD {
                    Some(a.id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Gets ids of all assets
    fn get_assets_with_usd() -> Vec<Asset> {
        Self::get_assets_data().iter().map(|a| a.id).collect()
    }

    /// Checks if an asset exists by a provided id
    fn exists(asset_id: Asset) -> bool {
        Self::get_asset_data(&asset_id).is_ok()
    }

    /// Gets the priority (u64) of an asset
    fn priority(asset: Asset) -> Option<u64> {
        match Self::get_asset_data(&asset) {
            Ok(data) => Some(data).map(|ad| ad.buyout_priority),
            Err(_) => None,
        }
    }

    /// Gets the main asset
    fn get_main_asset() -> Asset {
        Self::get_main_asset()
    }

    fn collateral_discount(asset: &Asset) -> EqFixedU128 {
        match Self::get_asset_data(asset) {
            Ok(asset_data) => asset_data.collateral_discount.into(),
            Err(_) => EqFixedU128::zero(),
        }
    }
}

impl<T: Config> AssetXcmGetter for Pallet<T> {
    /// Gets self reserved assets
    fn get_self_reserved_xcm_assets() -> Vec<AssetId> {
        Self::get_assets_data()
            .into_iter()
            .filter_map(|asset_data| match asset_data.asset_xcm_data {
                AssetXcmData::SelfReserved => {
                    let multi_location = asset_data.gen_multi_location().0;
                    Some(AssetId::Concrete(multi_location))
                }
                _ => None,
            })
            .collect()
    }

    /// Gets other reserved asset
    fn get_other_reserved_xcm_assets() -> Vec<AssetId> {
        Self::get_assets_data()
            .into_iter()
            .filter_map(|asset_data| {
                use eq_primitives::asset::OtherReservedData;

                match asset_data.asset_xcm_data {
                    AssetXcmData::OtherReserved(OtherReservedData { multi_location, .. }) => {
                        Some(AssetId::Concrete(multi_location))
                    }
                    _ => None,
                }
            })
            .collect()
    }
}
