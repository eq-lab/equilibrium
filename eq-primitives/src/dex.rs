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

use crate::vec_map::VecMap;
use crate::{asset::Asset, balance_number::EqFixedU128};
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResultWithPostInfo;
use sp_arithmetic::traits::{CheckedAdd, CheckedMul, CheckedSub, Zero};
use sp_arithmetic::FixedI64;

pub type Price = FixedI64;
pub type OrderId = u64;

#[derive(Decode, Encode, Debug, Clone, Copy, Eq, PartialEq, scale_info::TypeInfo)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Decode, Encode, Debug, Clone, Copy, Eq, PartialEq, scale_info::TypeInfo)]
pub enum OrderType {
    /// Create order and keep it in order book
    Limit {
        price: FixedI64,
        expiration_time: u64,
    },
    /// Executes immediately by best price
    Market,
}

#[derive(Decode, Encode, Debug, Clone, Eq, PartialEq, scale_info::TypeInfo)]
pub struct Order<AccountId> {
    pub order_id: OrderId,
    pub account_id: AccountId,
    pub side: OrderSide,
    pub price: Price,
    pub amount: EqFixedU128,
    pub created_at: u64,
    pub expiration_time: u64,
}

/// Keeps order aggregates for every account by particular asset.
/// Used primarily in margin calculation
#[derive(Eq, PartialEq, Decode, Encode, Debug, Clone, Copy, Default, scale_info::TypeInfo)]
pub struct OrderAggregate {
    /// Represents sum (order.amount(i) * order.price(i)) by every account order of particular asset
    pub amount_by_price: EqFixedU128,
    /// Represents sum (order.amount(i)) by every account order of particular asset
    pub amount: EqFixedU128,
}

impl OrderAggregate {
    pub fn new(amount: EqFixedU128, price: EqFixedU128) -> Option<Self> {
        let amount_by_price = amount.checked_mul(&price)?;

        Some(OrderAggregate {
            amount_by_price,
            amount,
        })
    }

    pub fn add(&mut self, amount: EqFixedU128, price: EqFixedU128) -> Option<()> {
        let amount_by_price = amount.checked_mul(&price)?;
        self.amount_by_price = self.amount_by_price.checked_add(&amount_by_price)?;
        self.amount = self.amount.checked_add(&amount)?;

        Some(())
    }

    pub fn sub(&mut self, amount: EqFixedU128, price: EqFixedU128) -> Option<()> {
        let amount_by_price = amount.checked_mul(&price)?;
        self.amount_by_price = self.amount_by_price.checked_sub(&amount_by_price)?;
        self.amount = self.amount.checked_sub(&amount)?;

        Some(())
    }
}

/// Keeps order aggregates split by order side
#[derive(Eq, PartialEq, Decode, Encode, Debug, Clone, Default, scale_info::TypeInfo)]
pub struct OrderAggregateBySide {
    pub sell: OrderAggregate,
    pub buy: OrderAggregate,
}

impl OrderAggregateBySide {
    pub fn new(amount: EqFixedU128, price: EqFixedU128, side: OrderSide) -> Option<Self> {
        let aggregate = match side {
            OrderSide::Buy => OrderAggregateBySide {
                sell: OrderAggregate::default(),
                buy: OrderAggregate::new(amount, price)?,
            },
            OrderSide::Sell => OrderAggregateBySide {
                sell: OrderAggregate::new(amount, price)?,
                buy: OrderAggregate::default(),
            },
        };

        Some(aggregate)
    }

    pub fn add(&mut self, amount: EqFixedU128, price: EqFixedU128, side: OrderSide) -> Option<()> {
        match side {
            OrderSide::Buy => self.buy.add(amount, price),
            OrderSide::Sell => self.sell.add(amount, price),
        }
    }

    pub fn sub(&mut self, amount: EqFixedU128, price: EqFixedU128, side: OrderSide) -> Option<()> {
        match side {
            OrderSide::Buy => self.buy.sub(amount, price),
            OrderSide::Sell => self.sell.sub(amount, price),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.buy.amount.is_zero() && self.sell.amount.is_zero()
    }

    pub fn get_by_side(&self, side: OrderSide) -> OrderAggregate {
        match side {
            OrderSide::Buy => self.buy,
            OrderSide::Sell => self.sell,
        }
    }
}

/// Provides functionality of the `eq-dex` pallet for other pallets.
pub trait OrderManagement {
    type AccountId;
    /// Create order
    fn create_order(
        who: Self::AccountId,
        asset: Asset,
        order_type: OrderType,
        side: OrderSide,
        amount: EqFixedU128,
    ) -> DispatchResultWithPostInfo;

    /// Delete order, update best price and account aggregate (q(i))
    fn delete_order(
        asset: &Asset,
        order_id: OrderId,
        price: FixedI64,
        reason: DeleteOrderReason,
    ) -> DispatchResultWithPostInfo;

    /// Search order by asset, order_id and price
    fn find_order(asset: &Asset, order_id: OrderId, price: Price)
        -> Option<Order<Self::AccountId>>;
}

/// Provides aggregates for calculations
pub trait OrderAggregates<AccountId> {
    /// Gets triple by every asset Asset, Sum(Amount(i) * Price(i)) and Sum(Amount(i))
    fn get_asset_weights(who: &AccountId) -> VecMap<Asset, OrderAggregateBySide>;
}

impl<AccountId> OrderAggregates<AccountId> for () {
    fn get_asset_weights(_who: &AccountId) -> VecMap<Asset, OrderAggregateBySide> {
        Default::default()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Decode, Encode, scale_info::TypeInfo)]
pub enum DeleteOrderReason {
    /// Deleted by offchain worker due to going out of the corridor
    OutOfCorridor,
    /// Canceled by user
    Cancel,
    /// Deleted by offchain worker due to bad margin
    MarginCall,
    /// Deleted by offchain worker due to disabled pair
    DisableTradingPair,
    /// Deleted by matching
    Match,
    /// Deleted by matching due to exchange error on maker side
    MakerError,
}
