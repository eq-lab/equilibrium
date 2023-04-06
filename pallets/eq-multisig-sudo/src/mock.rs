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
#![deny(warnings)]
#![allow(dead_code)]

use super::*;
use crate as eq_multisig_sudo;
use core::convert::TryFrom;
use sp_core::H256;
use sp_io;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

use frame_support::{traits::Everything, weights::Weight};

use frame_support::parameter_types;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// --------- LOGGER FOR CALL BOXING -----------------------
// Logger module to track execution.
pub mod logger {
    use crate::ensure_signed;
    use frame_support::{decl_event, decl_module, decl_storage, pallet_prelude::Weight};
    use frame_system::ensure_root;

    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    }

    decl_storage! {
        trait Store for Module<T: Config> as Logger {
            AccountLog get(fn account_log): Vec<T::AccountId>;
            I32Log get(fn i32_log): Vec<i32>;
        }
    }

    decl_event! {
        pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId {
            AppendI32(i32, Weight),
            AppendI32AndAccount(AccountId, i32, Weight),
        }
    }

    decl_module! {
        pub struct Module<T: Config> for enum Call where origin: <T as frame_system::Config>::Origin {
            fn deposit_event() = default;

            #[weight = Weight::zero()]
            fn privileged_i32_log(origin, i: i32, weight: Weight){
                // Ensure that the `origin` is `Root`.
                ensure_root(origin)?;
                <I32Log>::append(i);
                Self::deposit_event(RawEvent::AppendI32(i, weight));
            }

            #[weight = Weight::zero()]
            fn non_privileged_log(origin, i: i32, weight: Weight){
                // Ensure that the `origin` is some signed account.
                let sender = ensure_signed(origin)?;
                <I32Log>::append(i);
                <AccountLog<T>>::append(sender.clone());
                Self::deposit_event(RawEvent::AppendI32AndAccount(sender, i, weight));
            }
        }
    }
}

use core::convert::TryInto;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        EqMultisigSudo: eq_multisig_sudo::{Pallet, Call, Config<T>, Storage, Event<T>},
        Logger: logger::{Pallet, Call, Storage, Event<T>},
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
}

impl frame_system::Config for Test {
    type AccountId = u64;
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

// Implement the logger module's `Config` on the Test runtime.
impl logger::Config for Test {
    type Event = Event;
}

parameter_types! {
    pub const MaxSignatories: u32 = 4;
}

impl Config for Test {
    type Event = Event;
    type Call = Call;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

pub type ModuleMultisigSudo = Pallet<Test>;
pub type LoggerCall = logger::Call<Test>;
pub type ModuleCall = eq_multisig_sudo::Call<Test>;

pub fn new_test_ext(multisig_keys: Vec<u64>, threshold: u32) -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();
    eq_multisig_sudo::GenesisConfig::<Test> {
        keys: multisig_keys,
        threshold: threshold,
    }
    .assimilate_storage(&mut t)
    .unwrap();
    t.into()
}
