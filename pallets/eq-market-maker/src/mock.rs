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
use crate as eq_market_maker;

type AccountId = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

use core::convert::{TryFrom, TryInto};
use eq_primitives::{Order, Price};
use frame_support::traits::Everything;
use sp_core::H256;
use sp_runtime::{
    parameter_types,
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        MarketMaker: eq_market_maker::{Pallet, Call, Storage, Event<T>}
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = eq_primitives::balance::AccountData<eq_primitives::balance::Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl Config for Test {
    type Event = Event;
    type DexWeightInfo = ();
    type OrderManagement = OrderManagementMock;
}

pub struct OrderManagementMock;
impl OrderManagement for OrderManagementMock {
    type AccountId = AccountId;

    fn create_order(
        _who: Self::AccountId,
        _asset: Asset,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: EqFixedU128,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn delete_order(
        _asset: &Asset,
        _order_id: OrderId,
        _price: FixedI64,
        _reason: eq_primitives::dex::DeleteOrderReason,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn find_order(
        _asset: &Asset,
        _order_id: OrderId,
        _price: Price,
    ) -> Option<Order<Self::AccountId>> {
        None
    }
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into()
}
