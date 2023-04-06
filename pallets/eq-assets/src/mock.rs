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

use std::cell::RefCell;

use super::*;

use crate as eq_assets;
use core::convert::TryFrom;
use frame_support::{
    parameter_types,
    traits::{ConstU32, Everything},
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};
use system::EnsureRoot;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
use core::convert::TryInto;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event},
    }
);

pub type ModuleAssets = Pallet<Test>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
}

impl system::Config for Test {
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
    type AccountId = u64;
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
    type SS58Prefix = SS58Prefix;
    type MaxConsumers = ConstU32<16>;
    type OnSetCode = ();
}

thread_local! {
    pub static ON_NEW_ASSET_CALLS: RefCell<u32>  = RefCell::new(0);
}

pub struct OnNewAssetMock;

impl OnNewAsset for OnNewAssetMock {
    fn on_new_asset(_asset: Asset, _prices: Vec<FixedI64>) {
        ON_NEW_ASSET_CALLS.with(|args| {
            *args.borrow_mut() += 1;
        });
    }
}

pub fn new_assets_called() -> u32 {
    ON_NEW_ASSET_CALLS.with(|args| *args.borrow())
}

impl eq_assets::Config for Test {
    type Event = Event;
    type AssetManagementOrigin = EnsureRoot<u64>;
    type MainAsset = MainAsset;
    type OnNewAsset = OnNewAssetMock;
    type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into()
}
