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

//! # Equilibrium Market Maker Pallet
//!
//! Pallet provides methods to create/delete orders without fee to special whitelisted accounts.
//! Also pallet has methods to add/remove market maker accounts to whitelist

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

mod mock;
mod tests;

use core::convert::TryInto;
use eq_dex::WeightInfo as _;
use eq_primitives::{
    asset::Asset,
    balance_number::EqFixedU128,
    dex::{DeleteOrderReason, OrderManagement},
    OrderId, OrderSide, OrderType,
};
use frame_support::{dispatch::DispatchResultWithPostInfo, traits::Get as _};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use sp_runtime::{DispatchResult, FixedI64};
use sp_std::prelude::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Weight information for extrinsics of eqDex.
        type DexWeightInfo: eq_dex::WeightInfo;
        /// Used to operate on Dex
        type OrderManagement: OrderManagement<AccountId = Self::AccountId>;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Add account to whitelist
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn add_to_whitelist(
            origin: OriginFor<T>,
            account_id: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            WhiteList::<T>::insert(&account_id, ());
            Self::deposit_event(Event::AddedToWhitelist(account_id));
            Ok(().into())
        }

        /// Remove account from whitelist
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn remove_from_whitelist(
            origin: OriginFor<T>,
            account_id: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            WhiteList::<T>::remove(&account_id);
            Self::deposit_event(Event::RemovedFromWhitelist(account_id));
            Ok(().into())
        }

        /// Create order. This must be called by whitelisted account
        #[pallet::call_index(2)]
        #[pallet::weight(
        <T as pallet::Config>::DexWeightInfo::create_limit_order()
        .max(<T as pallet::Config>::DexWeightInfo::create_market_order())
        .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1))
        )]
        pub fn create_order(
            origin: OriginFor<T>,
            currency: Asset,
            order_type: OrderType,
            side: OrderSide,
            amount: EqFixedU128,
        ) -> DispatchResultWithPostInfo {
            let account = ensure_signed(origin)?;
            Self::ensure_whitelisted(&account)?;
            T::OrderManagement::create_order(account.clone(), currency, order_type, side, amount)?;
            Ok(Pays::No.into())
        }

        /// Delete order. This must be called by whitelisted account
        #[pallet::call_index(3)]
        #[pallet::weight(
        <T as Config>::DexWeightInfo::delete_order_external()
        .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1))
        )]
        pub fn delete_order(
            origin: OriginFor<T>,
            currency: Asset,
            order_id: OrderId,
            price: FixedI64,
        ) -> DispatchResultWithPostInfo {
            let account = ensure_signed(origin)?;
            Self::ensure_whitelisted(&account)?;
            T::OrderManagement::delete_order(
                &currency,
                order_id,
                price,
                DeleteOrderReason::Cancel,
            )?;
            Ok(Pays::No.into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// `AccountId` was added to the whitelist. \[who\]
        AddedToWhitelist(T::AccountId),
        /// `AccountId` was removed from the whitelist. \[who\]
        RemovedFromWhitelist(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Attempt to execute from not whitelisted account
        NotWhitelistedAccount,
    }

    #[pallet::storage]
    #[pallet::getter(fn whitelist)]
    pub type WhiteList<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;
}

impl<T: Config> Pallet<T> {
    fn ensure_whitelisted(account_id: &T::AccountId) -> DispatchResult {
        if WhiteList::<T>::contains_key(account_id) {
            Ok(())
        } else {
            Err(Error::<T>::NotWhitelistedAccount.into())
        }
    }
}