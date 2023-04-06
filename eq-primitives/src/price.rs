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

use core::convert::TryFrom;

use crate::asset::Asset;
use frame_support::dispatch::DispatchResultWithPostInfo;
use sp_runtime::{
    traits::{One, Zero},
    FixedI64, FixedPointNumber,
};
use sp_std::fmt::Debug;

/// Interface for getting a current price of an `Asset`
pub trait PriceGetter {
    /// Gets a current price for a given `Asset`
    fn get_price<FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>>(
        asset: &Asset,
    ) -> Result<FixedNumber, sp_runtime::DispatchError>;
}

/// Interface for adding a new `DataPoint` containing `asset` price information
pub trait PriceSetter<AccountId> {
    /// Adds a new `DataPoint` with a `price` for an `asset`
    fn set_price(who: AccountId, asset: Asset, price: FixedI64) -> DispatchResultWithPostInfo;
}

#[cfg(feature = "std")]
pub mod mock {
    use crate::{asset::Asset, PriceGetter, PriceSetter};
    use core::{
        cell::RefCell,
        convert::{TryFrom, TryInto},
        fmt::Debug,
        iter::FromIterator,
        marker::PhantomData,
    };
    use frame_support::dispatch::DispatchResultWithPostInfo;
    use sp_runtime::{
        traits::{One, Zero},
        FixedI64, FixedPointNumber, TokenError,
    };
    use std::collections::HashMap;

    pub struct OracleMock<AccountId>(PhantomData<AccountId>);

    impl<AccountId> OracleMock<AccountId> {
        thread_local! {
            static PRICES: RefCell<HashMap<Asset, FixedI64>> = RefCell::new(HashMap::from([]));
        }

        pub fn init(prices: Vec<(Asset, FixedI64)>) {
            Self::PRICES.with(|h| {
                *h.borrow_mut() = HashMap::from_iter(prices);
            });
        }

        fn add(asset: Asset, price: FixedI64) {
            Self::PRICES.with(|h| {
                let mut hashmap = h.borrow().clone();
                hashmap.insert(asset, price);
                *h.borrow_mut() = hashmap;
            });
        }

        fn get(asset: &Asset) -> Option<FixedI64> {
            let mut result = None;
            Self::PRICES.with(|h| {
                let hashmap = h.borrow().clone();
                result = hashmap.get(asset).map(|v| v.to_owned());
            });
            result
        }

        pub fn remove(asset: &Asset) {
            Self::PRICES.with(|h| {
                let mut hashmap = h.borrow().clone();
                hashmap.remove(asset);
                *h.borrow_mut() = hashmap;
            });
        }
    }

    impl<AccountId> PriceSetter<AccountId> for OracleMock<AccountId> {
        fn set_price(_who: AccountId, asset: Asset, price: FixedI64) -> DispatchResultWithPostInfo {
            Self::add(asset, price);

            Ok(().into())
        }
    }

    impl<AccountId> PriceGetter for OracleMock<AccountId> {
        fn get_price<FixedNumber>(asset: &Asset) -> Result<FixedNumber, sp_runtime::DispatchError>
        where
            FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>,
        {
            Self::get(asset)
                .ok_or(sp_runtime::DispatchError::Token(TokenError::UnknownAsset))
                .map(TryInto::try_into)?
                .map_err(|_| sp_runtime::DispatchError::Other("Positice price"))
        }
    }
}
