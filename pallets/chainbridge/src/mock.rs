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
use crate as chainbridge;
use eq_assets;
use eq_primitives::{
    asset,
    asset::AssetType,
    balance_number::EqFixedU128,
    mocks::TimeZeroDurationMock,
    subaccount::{SubAccType, SubaccountsManager},
    BailsmanManager, EqBuyout, UpdateTimeManager, XcmMode,
};
use eq_utils::ONE_TOKEN;
use frame_support::{assert_ok, parameter_types, traits::GenesisBuild, weights::Weight, PalletId};
use sp_arithmetic::{FixedI64, Permill};
use sp_core::H256;
use sp_runtime::DispatchResult;
use sp_runtime::{
    testing::Header,
    traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
    DispatchError, Percent,
};
use std::marker::PhantomData;
use system::EnsureRoot;

pub(crate) type AccountId = u64;
type Balance = eq_primitives::balance::Balance;
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
        frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
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
    fn remove_last_update(_account_id: &AccountId) {}
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
    pub const ExistentialDepositBasic: Balance = 1;
}

impl eq_balances::Config for Test {
    type ParachainId = eq_primitives::mocks::ParachainId;
    type ToggleTransferOrigin = EnsureRoot<AccountId>;
    type ForceXcmTransferOrigin = EnsureRoot<AccountId>;
    type AssetGetter = eq_assets::Pallet<Test>;
    type AccountStore = System;
    type Balance = Balance;
    type ExistentialDeposit = ExistentialDeposit;
    type ExistentialDepositBasic = ExistentialDepositBasic;
    type ExistentialDepositEq = ExistentialDeposit;
    type BalanceChecker = ();
    type PriceGetter = OracleMock;
    type RuntimeEvent = RuntimeEvent;
    type Aggregates = eq_aggregates::Pallet<Test>;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmanManagerMock;
    type UpdateTimeManager = RateMock;
    type BailsmanModuleId = BailsmanModuleId;
    type WeightInfo = ();
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type OrderAggregates = ();
    type UnixTime = TimeZeroDurationMock;
}

parameter_types! {
    pub const TestChainId: u8 = 5;
    pub const BasicCurrencyGet: asset::Asset = asset::Q;
}

pub type BasicCurrency = eq_primitives::balance_adapter::BalanceAdapter<
    Balance,
    eq_balances::Pallet<Test>,
    BasicCurrencyGet,
>;

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = BasicCurrencyGet;
    type OnNewAsset = ();
    type WeightInfo = ();
}

impl chainbridge::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Currency = BasicCurrency;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type AdminOrigin = system::EnsureRoot<Self::AccountId>;
    type Proposal = RuntimeCall;
    type ChainIdentity = TestChainId;
    type WeightInfo = ();
}

pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<u32, u64, RuntimeCall, ()>;

use core::convert::{TryFrom, TryInto};

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: system::{Pallet, Call, Event<T>},
        Balances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>},
        EqAggregates: eq_aggregates::{Pallet, Call, Storage},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event}
    }
);

pub const RELAYER_A: AccountId = 0x2;
pub const RELAYER_B: AccountId = 0x3;
pub const RELAYER_C: AccountId = 0x4;
pub const ENDOWED_BALANCE: Balance = 100_000_000;
pub const TEST_THRESHOLD: u32 = 2;
pub const TEST_PROPOSAL_LIFETIME: u64 = 200_000;

pub fn new_test_ext_params(relayers: Vec<AccountId>) -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::BTC, FixedI64::from_inner(1)),
        (asset::EOS, FixedI64::from_inner(1)),
        (asset::ETH, FixedI64::from_inner(1)),
        (asset::EQD, FixedI64::from_inner(1)),
        (asset::EQ, FixedI64::from_inner(1)),
        (asset::Q, FixedI64::from_inner(1)),
    ]);

    let fee_id = FEE_MODULE_ID.into_account_truncating();
    let bridge_id = MODULE_ID.into_account_truncating();
    let mut t = system::GenesisConfig::default()
        .build_storage::<Test>()
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
                asset::Q.get_id(),
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
            (fee_id, vec![(0, asset::Q.get_id())]),
            (bridge_id, vec![(ENDOWED_BALANCE, asset::Q.get_id())]),
        ],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(false)),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    chainbridge::GenesisConfig::<Test> {
        _runtime: PhantomData,
        chains: vec![],
        fees: vec![],
        relayers,
        threshold: DEFAULT_RELAYER_THRESHOLD,
        resources: vec![],
        proposal_lifetime: DEFAULT_PROPOSAL_LIFETIME as u64,
        min_nonces: vec![],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    new_test_ext_params(vec![])
}

pub fn new_test_ext_initialized(
    src_id: ChainId,
    r_id: ResourceId,
    resource: Vec<u8>,
    lifetime: u64,
) -> sp_io::TestExternalities {
    let mut t = new_test_ext_params(vec![RELAYER_A, RELAYER_B, RELAYER_C]);
    t.execute_with(|| {
        // Set and check threshold
        assert_ok!(ChainBridge::set_threshold(
            RuntimeOrigin::root(),
            TEST_THRESHOLD
        ));
        assert_eq!(ChainBridge::relayer_threshold(), TEST_THRESHOLD);
        // Whitelist chain
        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            src_id,
            DEFAULT_FEE
        ));
        // Check allowability
        assert!(!<DisabledChains<Test>>::contains_key(src_id));
        // Set and check resource ID mapped to some junk data
        assert_ok!(ChainBridge::set_resource(
            RuntimeOrigin::root(),
            r_id,
            resource
        ));
        assert_eq!(ChainBridge::resource_exists(r_id), true);
        // Set proposal lifetime
        assert_ok!(ChainBridge::set_proposal_lifetime(
            RuntimeOrigin::root(),
            lifetime
        ));
        assert_eq!(ChainBridge::proposal_lifetime(), lifetime);
        // Set minimal nonce
        assert_ok!(ChainBridge::set_min_nonce(RuntimeOrigin::root(), src_id, 0));
    });
    t
}

// Checks events against the latest. A contiguous set of events must be provided. They must
// include the most recent event, but do not have to include every past event.
pub fn assert_events(mut expected: Vec<RuntimeEvent>) {
    let mut actual: Vec<RuntimeEvent> = system::Pallet::<Test>::events()
        .iter()
        .map(|e| e.event.clone())
        .collect();

    expected.reverse();

    for evt in expected {
        let next = actual.pop().expect("event expected");
        assert_eq!(next, evt.into(), "Events don't match (actual,expected)");
    }
}
