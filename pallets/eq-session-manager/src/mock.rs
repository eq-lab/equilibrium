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
use crate as eq_session_manager;
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::weights::Weight;
use frame_support::{parameter_types, traits::OnInitialize};
use frame_system::EnsureRoot;
use pallet_session::{SessionHandler, ShouldEndSession};
use sp_core::{crypto::key_types::DUMMY, H256};
use sp_runtime::{
    impl_opaque_keys,
    testing::{Header, UintAuthorityId},
    traits::{BlakeTwo256, ConvertInto, IdentityLookup, OpaqueKeys},
    Perbill,
};
use std::cell::RefCell;

impl_opaque_keys! {
    pub struct MockSessionKeys {
        pub dummy: UintAuthorityId,
    }
}

impl From<UintAuthorityId> for MockSessionKeys {
    fn from(dummy: UintAuthorityId) -> Self {
        Self { dummy }
    }
}

type AccountId = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

use core::convert::{TryFrom, TryInto};

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        EqSessionManager: eq_session_manager::{Pallet, Call, Storage, Event<T>},
        Session: pallet_session::{Pallet, Call, Storage, Event},
    }
);

thread_local! {
    pub static VALIDATORS: RefCell<Vec<u64>> = RefCell::new(vec![1, 2, 3]);
    pub static NEXT_VALIDATORS: RefCell<Vec<u64>> = RefCell::new(vec![1, 2, 3]);
    pub static FORCE_SESSION_END: RefCell<bool> = RefCell::new(false);
    pub static SESSION_LENGTH: RefCell<u64> = RefCell::new(2);
    pub static SESSION_CHANGED: RefCell<bool> = RefCell::new(false);
    pub static TEST_SESSION_CHANGED: RefCell<bool> = RefCell::new(false);
    pub static DISABLED: RefCell<bool> = RefCell::new(false);
    // Stores if `on_before_session_end` was called
    pub static BEFORE_SESSION_END_CALLED: RefCell<bool> = RefCell::new(false);
}

pub struct TestSessionHandler;
impl SessionHandler<AccountId> for TestSessionHandler {
    const KEY_TYPE_IDS: &'static [sp_runtime::KeyTypeId] = &[DUMMY];
    fn on_genesis_session<T: OpaqueKeys>(_validators: &[(u64, T)]) {}
    fn on_new_session<T: OpaqueKeys>(
        changed: bool,
        validators: &[(u64, T)],
        _queued_validators: &[(u64, T)],
    ) {
        println!(
            "on_new_session validators {:?}",
            validators.iter().map(|(v, _)| v).collect::<Vec<_>>()
        );
        SESSION_CHANGED.with(|l| *l.borrow_mut() = changed);
        VALIDATORS.with(|l| *l.borrow_mut() = validators.iter().map(|(x, _)| *x).collect());
    }
    fn on_disabled(_validator_index: u32) {
        DISABLED.with(|l| *l.borrow_mut() = true)
    }
    fn on_before_session_ending() {
        BEFORE_SESSION_END_CALLED.with(|b| *b.borrow_mut() = true);
    }
}

pub struct TestShouldEndSession;
impl ShouldEndSession<u64> for TestShouldEndSession {
    fn should_end_session(now: u64) -> bool {
        let l = SESSION_LENGTH.with(|l| *l.borrow());
        now % l == 0
            || FORCE_SESSION_END.with(|l| {
                let r = *l.borrow();
                *l.borrow_mut() = false;
                r
            })
    }
}

pub fn validators() -> Vec<u64> {
    VALIDATORS.with(|l| l.borrow().to_vec())
}

pub fn force_new_session() {
    FORCE_SESSION_END.with(|l| *l.borrow_mut() = true)
}

pub fn initialize_block(block: u64) {
    SESSION_CHANGED.with(|l| *l.borrow_mut() = false);
    System::set_block_number(block);
    <Session as OnInitialize<u64>>::on_initialize(block);
}

pub fn session_changed() -> bool {
    SESSION_CHANGED.with(|l| *l.borrow())
}

parameter_types! {
    pub const MinimumPeriod: u64 = 1;
    pub const EpochDuration: u64 = 3;
    pub const ExpectedBlockTime: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const MaxPricePoints: usize = 3;
}

type DummyValidatorId = u64;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
}
impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type RuntimeEvent = RuntimeEvent;
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

impl pallet_session::Config for Test {
    type ShouldEndSession = TestShouldEndSession;
    type SessionManager = Pallet<Test>;
    type SessionHandler = TestSessionHandler;
    type ValidatorId = u64;
    type ValidatorIdOf = ConvertInto;
    type Keys = MockSessionKeys;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = ();
    type WeightInfo = ();
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorsManagementOrigin = EnsureRoot<AccountId>;
    type ValidatorId = DummyValidatorId;
    type ValidatorIdOf = ();
    type RegistrationChecker = Session;
    type WeightInfo = ();
}

pub type ModuleSessionManager = Pallet<Test>;

pub type ErrorSessionManager = Error<Test>;

pub fn initial_validators() -> Vec<u64> {
    vec![111, 222]
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    pallet_session::GenesisConfig::<Test> {
        keys: initial_validators()
            .iter()
            .map(|&v| (v, v, UintAuthorityId(v).into()))
            .collect(),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    eq_session_manager::GenesisConfig::<Test> {
        validators: initial_validators(),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
