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

use crate::Asset;
use financial_pallet::{AssetMetrics, FinancialMetrics};
#[allow(unused_imports)]
use financial_pallet::{Metrics, PerAssetMetrics, PriceLogs, Updates}; // Compiler thinks this import is unused
use frame_support::{StorageMap, StorageValue};
use substrate_fixed::types::I64F64;

/// Trait used to access Financial pallet storage from other pallets
pub trait FinancialStorage {
    type Asset;
    type Price;

    fn get_per_asset_metrics(asset: &Self::Asset)
        -> Option<AssetMetrics<Self::Asset, Self::Price>>;
    fn get_metrics() -> Option<FinancialMetrics<Self::Asset, Self::Price>>;
}

impl<T: financial_pallet::Config> FinancialStorage for financial_pallet::Pallet<T> {
    type Asset = T::Asset;
    type Price = T::Price;

    fn get_per_asset_metrics(
        asset: &Self::Asset,
    ) -> Option<AssetMetrics<Self::Asset, Self::Price>> {
        <PerAssetMetrics<T>>::get(asset)
    }
    fn get_metrics() -> Option<FinancialMetrics<Self::Asset, Self::Price>> {
        <Metrics<T>>::get()
    }
}

/// Empty implementation for using in tests
impl FinancialStorage for () {
    type Asset = Asset;
    type Price = I64F64;

    fn get_per_asset_metrics(
        _asset: &Self::Asset,
    ) -> Option<financial_pallet::AssetMetrics<Self::Asset, Self::Price>> {
        None
    }
    fn get_metrics() -> Option<financial_pallet::FinancialMetrics<Self::Asset, Self::Price>> {
        None
    }
}

pub trait FinancialAssetRemover {
    type Asset;

    fn remove_asset(asset: &Self::Asset);
}

impl<T: financial_pallet::Config> FinancialAssetRemover for financial_pallet::Pallet<T> {
    type Asset = T::Asset;

    fn remove_asset(asset: &Self::Asset) {
        Updates::<T>::remove(*asset);
        PriceLogs::<T>::remove(*asset);
        PerAssetMetrics::<T>::remove(*asset);
    }
}
