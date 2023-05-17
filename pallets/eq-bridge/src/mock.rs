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
use crate::{self as eq_bridge};
use eq_primitives::balance::BalanceGetter;
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::mocks::TimeZeroDurationMock;
use eq_primitives::{
    asset,
    asset::AssetType,
    subaccount::{SubAccType, SubaccountsManager},
    BailsmanManager, EqBuyout, UpdateTimeManager,
};
use eq_primitives::{SignedBalance, XcmMode};
use eq_utils::ONE_TOKEN;
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{parameter_types, PalletId};
use frame_system::{self as system, EnsureRoot};
use sp_core::hashing::blake2_128;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
    DispatchError,
};
use sp_runtime::{DispatchResult, Percent};
use std::marker::PhantomData;

pub(crate) type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;

pub(crate) const DEFAULT_FEE: Balance = 10 * ONE_TOKEN;

parameter_types! {
    pub const MaxLocks: u32 = 100;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
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
    type AccountData = eq_primitives::balance::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub struct RateMock;
impl UpdateTimeManager<AccountId> for RateMock {
    fn set_last_update(_account_id: &AccountId) {}
    fn remove_last_update(_accounts_id: &AccountId) {}
    fn set_last_update_timestamp(_account_id: &AccountId, _timestamp_ms: u64) {}
}

pub struct SubaccountsManagerMock;
impl SubaccountsManager<AccountId> for SubaccountsManagerMock {
    fn create_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        Ok(9999_u64)
    }
    fn delete_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        Ok(9999_u64)
    }
    fn has_subaccount(_who: &AccountId, _subacc_type: &SubAccType) -> bool {
        true
    }
    fn get_subaccount_id(_who: &AccountId, _subacc_type: &SubAccType) -> Option<AccountId> {
        Some(9999_u64)
    }
    fn is_subaccount(_who: &AccountId, _subacc_id: &AccountId) -> bool {
        false
    }
    fn get_owner_id(_subaccount: &AccountId) -> Option<(AccountId, SubAccType)> {
        None
    }
    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        0
    }
}

pub struct BailsmanManagerMock;
impl BailsmanManager<AccountId, Balance> for BailsmanManagerMock {
    fn register_bailsman(_who: &AccountId) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
    fn unregister_bailsman(_who: &AccountId) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
    fn receive_position(
        _who: &AccountId,
        _is_deleting_position: bool,
    ) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn redistribute(_who: &AccountId) -> Result<u32, sp_runtime::DispatchError> {
        Ok(1)
    }

    fn get_account_distribution(
        _who: &AccountId,
    ) -> Result<eq_primitives::AccountDistribution<Balance>, sp_runtime::DispatchError> {
        unimplemented!()
    }

    fn should_unreg_bailsman(
        _: &AccountId,
        _: &[(asset::Asset, SignedBalance<Balance>)],
        _: Option<(Balance, Balance)>,
    ) -> Result<bool, sp_runtime::DispatchError> {
        Ok(false)
    }

    fn bailsmen_count() -> u32 {
        0
    }

    fn distribution_queue_len() -> u32 {
        0
    }
}

pub struct EqBuyoutMock;
impl EqBuyout<AccountId, Balance> for EqBuyoutMock {
    fn eq_buyout(_who: &AccountId, _amount: Balance) -> DispatchResult {
        Ok(())
    }
    fn is_enough(
        _currency: asset::Asset,
        _amount: Balance,
        _amount_buyout: Balance,
    ) -> Result<bool, DispatchError> {
        Ok(true)
    }
}

impl eq_aggregates::Config for Test {
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Test>;
}

parameter_types! {
    pub const ExistentialDeposit: Balance = 1;
}

impl eq_balances::Config for Test {
    type ParachainId = eq_primitives::mocks::ParachainId;
    type ToggleTransferOrigin = EnsureRoot<AccountId>;
    type ForceXcmTransferOrigin = EnsureRoot<AccountId>;
    type AssetGetter = eq_assets::Pallet<Test>;
    type AccountStore = System;
    type Balance = Balance;
    type ExistentialDeposit = ExistentialDeposit;
    type ExistentialDepositBasic = ExistentialDeposit;
    type BalanceChecker = ();
    type PriceGetter = OracleMock;
    type Event = Event;
    type WeightInfo = ();
    type Aggregates = eq_aggregates::Pallet<Test>;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmanManagerMock;
    type UpdateTimeManager = RateMock;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type LocationInverter = eq_primitives::mocks::LocationInverterMock;
    type OrderAggregates = ();
    type UnixTime = TimeZeroDurationMock;
}

parameter_types! {
    pub const TestChainId: u8 = 5;
    pub const ProposalLifetime: u64 = 144000;
    pub const BasicCurrencyGet: asset::Asset = asset::EQ;
    pub const EthCurrencyGet: asset::Asset = asset::ETH;
    pub const EqdCurrencyGet: asset::Asset = asset::EQD;
    pub const SyntCurrencyGet: asset::Asset = SYNT;
}

pub type BasicCurrency = eq_primitives::balance_adapter::BalanceAdapter<
    Balance,
    eq_balances::Pallet<Test>,
    BasicCurrencyGet,
>;

impl eq_assets::Config for Test {
    type Event = Event;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = BasicCurrencyGet;
    type OnNewAsset = ();
    type WeightInfo = ();
}

impl chainbridge::Config for Test {
    type Event = Event;
    type Balance = Balance;
    type Currency = BasicCurrency;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type AdminOrigin = EnsureRoot<AccountId>;
    type Proposal = Call;
    type ChainIdentity = TestChainId;
    type WeightInfo = ();
}

parameter_types! {
    pub HashId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
    pub NativeTokenId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"DAV"));
    pub EthTokenId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"ETH"));
    pub SyntheticTokenId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"SYNT"));
    pub EqdTokenId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"EQD"));
    pub Lpt0TokenId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"LPT0"));
}

impl Config for Test {
    type Event = Event;
    type BridgeManagementOrigin = EnsureRoot<AccountId>;
    type BridgeOrigin = chainbridge::EnsureBridge<Test>;
    type EqCurrency = eq_balances::Pallet<Test>;
    type AssetGetter = eq_assets::Pallet<Test>;
    type WeightInfo = ();
}

pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<u32, u64, Call, ()>;

use core::convert::{TryFrom, TryInto};
use sp_arithmetic::{FixedI64, Permill};

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: system::{Pallet, Call, Event<T>},
        Balances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>},
        EqBridge: eq_bridge::{Pallet, Call, Event<T>},
        EqAggregates: eq_aggregates::{Pallet, Call, Storage},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event}
    }
);

pub const RELAYER_A: AccountId = 0x2;
pub const RELAYER_B: AccountId = 0x3;
pub const RELAYER_C: AccountId = 0x4;
pub const USER: AccountId = 0x5;
pub const ENDOWED_BALANCE: Balance = 1000 * ONE_TOKEN;
pub const SYNT: Asset = Asset(1937337972); //::from_bytes(b"synt"); 0x73796E74
pub const LPT0: Asset = Asset(1819309104); //::from_bytes(b"lpt0"); 0x6C707430
pub type ModuleBalances = eq_balances::Pallet<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![]);
    let fee_id = chainbridge::FEE_MODULE_ID.into_account_truncating();
    let bridge_id = chainbridge::MODULE_ID.into_account_truncating();
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    chainbridge::GenesisConfig::<Test> {
        _runtime: PhantomData,
        relayers: vec![],
        threshold: 1,
        chains: vec![],
        resources: vec![],
        proposal_lifetime: 144_000,
        fees: vec![(0, DEFAULT_FEE / ONE_TOKEN)], // in genesis config integer part of balance
        min_nonces: vec![],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    eq_assets::GenesisConfig::<Test> {
        _runtime: PhantomData,
        assets: // id, lot, price_step, maker_fee, taker_fee, debt_weight, buyout_priority
        vec![
            (
                asset::EQD.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                1,
                AssetType::Synthetic,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::BTC.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                2,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::ETH.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                3,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::DOT.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                4,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::CRV.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                5,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::EOS.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                6,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                SYNT.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                7,
                AssetType::Synthetic,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                LPT0.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                8,
                AssetType::Lp(eq_primitives::asset::AmmPool::Curve(0)),
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::EQ.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                u64::MAX,
                AssetType::Native,
                true,
                Percent::one(),
                Permill::one(),
            ),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    eq_balances::GenesisConfig::<Test> {
        balances: vec![
            (fee_id, vec![(0, asset::EQ.get_id())]),
            (bridge_id, vec![(ENDOWED_BALANCE, asset::EQ.get_id())]),
            (RELAYER_A, vec![(ENDOWED_BALANCE, asset::EQ.get_id())]),
            (USER, vec![(ENDOWED_BALANCE, asset::EQ.get_id())]),
        ],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(true)),
    }
    .assimilate_storage(&mut t)
    .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

fn last_event() -> Event {
    system::Pallet::<Test>::events()
        .pop()
        .map(|e| e.event)
        .expect("Event expected")
}

pub fn expect_event<E: Into<Event>>(e: E) {
    assert_eq!(last_event(), e.into());
}

// Asserts that the event was emitted at some point.
pub fn event_exists<E: Into<Event>>(e: E) {
    let actual: Vec<Event> = system::Pallet::<Test>::events()
        .iter()
        .map(|e| e.event.clone())
        .collect();
    let e: Event = e.into();
    let mut exists = false;
    for evt in actual {
        if evt == e {
            exists = true;
            break;
        }
    }
    assert!(exists);
}

// Checks events against the latest. A contiguous set of events must be provided. They must
// include the most recent event, but do not have to include every past event.
pub fn assert_events(mut expected: Vec<Event>) {
    let mut actual: Vec<Event> = system::Pallet::<Test>::events()
        .iter()
        .map(|e| e.event.clone())
        .collect();

    expected.reverse();

    for evt in expected {
        let next = actual.pop().expect("event expected");
        assert_eq!(next, evt.into(), "Events don't match");
    }
}

pub fn get_basic_balance(acc: AccountId) -> SignedBalance<u128> {
    ModuleBalances::get_balance(&acc, &BasicCurrencyGet::get())
}

pub fn get_eth_balance(acc: AccountId) -> SignedBalance<u128> {
    ModuleBalances::get_balance(&acc, &eq_primitives::asset::ETH)
}

pub fn get_synth_balance(acc: AccountId) -> SignedBalance<u128> {
    ModuleBalances::get_balance(&acc, &SYNT)
}

pub fn get_eqd_balance(acc: AccountId) -> SignedBalance<u128> {
    ModuleBalances::get_balance(&acc, &eq_primitives::asset::EQD)
}

pub fn get_lpt0_balance(acc: AccountId) -> SignedBalance<u128> {
    ModuleBalances::get_balance(&acc, &LPT0)
}
