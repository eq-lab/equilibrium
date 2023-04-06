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

use crate as eq_lending;
use core::marker::PhantomData;
use eq_aggregates;
use eq_primitives::{
    asset,
    asset::Asset,
    asset::AssetType,
    subaccount::{SubAccType, SubaccountsManager},
    BalanceChange, EqBuyout, MarginCallManager, MarginState, OrderChange, UpdateTimeManager,
    XcmMode,
};
use frame_support::{dispatch::DispatchError, parameter_types, PalletId};
use frame_support::{
    traits::{GenesisBuild, UnixTime},
    weights::Weight,
};
use frame_system::{offchain::SendTransactionTypes, EnsureRoot};
use sp_core::H256;
use sp_runtime::{
    testing::{Header, TestXt, UintAuthorityId},
    traits::{BlakeTwo256, IdentityLookup},
    FixedI64, FixedPointNumber, Perbill, Percent,
};
use std::cell::RefCell;

type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
pub(crate) type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

use core::convert::{TryFrom, TryInto};
use sp_arithmetic::Permill;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        EqBalances: eq_balances::{Pallet, Call, Storage, Event<T>},
        EqLending: eq_lending::{Pallet, Storage, Call, Event<T>},
        EqAggregates: eq_aggregates::{Pallet},
        EqBailsman: eq_bailsman::{Pallet, Event<T>, Call},
        EqAssets: eq_assets::{Pallet, Storage, Call, Event},
    }
);

impl eq_assets::Config for Test {
    type Event = Event;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
    type OnNewAsset = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: Balance = 1;
    pub const EpochDuration: u64 = 3;
    pub const ExpectedBlockTime: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
    pub const LendingModuleId: PalletId = PalletId(*b"eq/lendr");
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

impl eq_aggregates::Config for Test {
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Test>;
}

parameter_types! {
    pub const MinimalCollateral: Balance = 10 * 1_000_000_000;
    pub const TotalIssuance: Balance = 1_000_000_000;
    pub const MinTempBalanceUsd: Balance = 1000000000 * 1_000_000_000; // never reinit
    pub CriticalMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 1000);
}

pub struct TimeMock;

impl TimeMock {
    #[allow(dead_code)]
    pub fn set(millis: u64) {
        CURRENT_TIME.with(|v| *v.borrow_mut() = millis)
    }
}

impl UnixTime for TimeMock {
    fn now() -> core::time::Duration {
        core::time::Duration::from_millis(CURRENT_TIME.with(|v| *v.borrow()))
    }
}

parameter_types! {
    pub const PriceTimeout: u64 = 1;
    pub const MedianPriceTimeout: u64 = 60 * 60 * 2;
    pub const UnsignedPriority: u64 = 100;
}

pub struct MarginCallManagerMock;
impl MarginCallManager<AccountId, Balance> for MarginCallManagerMock {
    fn check_margin_with_change(
        _owner: &AccountId,
        _balance_changes: &[BalanceChange<Balance>],
        _order_changes: &[OrderChange],
    ) -> Result<(MarginState, bool), DispatchError> {
        Ok((MarginState::Good, true))
    }

    fn try_margincall(_owner: &AccountId) -> Result<MarginState, DispatchError> {
        Ok(MarginState::Good)
    }

    fn get_critical_margin() -> EqFixedU128 {
        CriticalMargin::get()
    }
}

parameter_types! {
    pub const MaxBailsmenToDistribute: u32 = 1;
    pub const QueueLengthWeightConstant: u32 = 5;
}

impl<LocalCall> SendTransactionTypes<LocalCall> for Test
where
    Call: From<LocalCall>,
{
    type OverarchingCall = Call;
    type Extrinsic = TestXt<Call, ()>;
}

impl eq_bailsman::Config for Test {
    type AssetGetter = eq_assets::Pallet<Test>;
    type Event = Event;
    type Balance = Balance;
    type BalanceGetter = ModuleBalances;
    type EqCurrency = ModuleBalances;
    type PriceGetter = OracleMock;
    type MinTempBalanceUsd = MinTempBalanceUsd;
    type MinimalCollateral = MinimalCollateral;
    type UnixTime = TimeMock;
    type PalletId = BailsmanModuleId;
    type Aggregates = eq_aggregates::Pallet<Test>;
    type WeightInfo = ();
    type MarginCallManager = MarginCallManagerMock;
    type SubaccountsManager = SubaccountsManagerMock;

    type AuthorityId = UintAuthorityId;
    type MaxBailsmenToDistribute = MaxBailsmenToDistribute;
    type UnsignedPriority = UnsignedPriority;
    type ValidatorOffchainBatcher = ();
    type QueueLengthWeightConstant = QueueLengthWeightConstant;
}

impl Config for Test {
    type Event = Event;
    type Balance = Balance;
    type AssetGetter = eq_assets::Pallet<Test>;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type Aggregates = eq_aggregates::Pallet<Test>;
    type BailsmanManager = EqBailsman;
    type PriceGetter = OracleMock;
    type SubaccountsManager = SubaccountsManagerMock;
    type ModuleId = LendingModuleId;
    type EqCurrency = EqBalances;
    type UnixTime = TimeMock;
    type WeightInfo = ();
}

thread_local! {
    static PRICES: RefCell<Vec<(asset::Asset, FixedI64)>> = RefCell::new(vec![
        (asset::CRV, FixedI64::saturating_from_integer(10000)),
        (asset::BTC, FixedI64::saturating_from_integer(10000)),
        (asset::EOS, FixedI64::saturating_from_integer(3)),
        (asset::ETH, FixedI64::saturating_from_integer(250)),
        (asset::EQD, FixedI64::saturating_from_integer(1)),
        (asset::EQ, FixedI64::saturating_from_integer(1)),
        (asset::DOT, FixedI64::saturating_from_integer(4))
        ]);
    static CURRENT_TIME: RefCell<u64> = RefCell::new(1_598_006_981_634);
}

pub struct EqBuyoutMock;

impl EqBuyout<AccountId, Balance> for EqBuyoutMock {
    fn eq_buyout(_who: &AccountId, _amount: Balance) -> sp_runtime::DispatchResult {
        Ok(())
    }
    fn is_enough(
        _asset: Asset,
        _amount: Balance,
        _amount_buyout: Balance,
    ) -> Result<bool, DispatchError> {
        Ok(true)
    }
}

pub const ACCOUNT_BAILSMAN_1: AccountId = 333;
pub const ACCOUNT_BAILSMAN_2: AccountId = 444;

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
        // hack for not deleting account in transfer
        true
    }

    fn get_owner_id(_subaccount: &AccountId) -> Option<(AccountId, SubAccType)> {
        None
    }

    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        0
    }

    fn is_master(who: &AccountId) -> bool {
        who != &ACCOUNT_BAILSMAN_1 && who != &ACCOUNT_BAILSMAN_2
    }
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
    type BalanceChecker = ModuleLending;
    type PriceGetter = OracleMock;
    type Event = Event;
    type WeightInfo = ();
    type Aggregates = eq_aggregates::Pallet<Test>;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = ModuleBailsman;
    type UpdateTimeManager = RateMock;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type LocationInverter = eq_primitives::mocks::LocationInverterMock;
    type OrderAggregates = ();
    type UnixTime = TimeMock;
}

pub struct RateMock;
impl UpdateTimeManager<u64> for RateMock {
    fn set_last_update(_account_id: &u64) {}
    fn remove_last_update(_accounts_id: &u64) {}
    fn set_last_update_timestamp(_account_id: &u64, _timestamp_ms: u64) {}
}

pub type ModuleLending = Pallet<Test>;
pub type ModuleBalances = eq_balances::Pallet<Test>;
pub type ModuleAggregates = eq_aggregates::Pallet<Test>;
pub type ModuleBailsman = eq_bailsman::Pallet<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::CRV, FixedI64::saturating_from_integer(10000)),
        (asset::BTC, FixedI64::saturating_from_integer(10000)),
        (asset::EOS, FixedI64::saturating_from_integer(3)),
        (asset::ETH, FixedI64::saturating_from_integer(250)),
        (asset::EQD, FixedI64::saturating_from_integer(1)),
        (asset::EQ, FixedI64::saturating_from_integer(1)),
        (asset::DOT, FixedI64::saturating_from_integer(4)),
    ]);

    let mut r = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    eq_assets::GenesisConfig::<Test> {
        _runtime: PhantomData,
        assets: vec![
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
                Permill::from_rational(2u32, 10u32), // ** FOR TESTS **
                2,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::from_percent(90),
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
    .assimilate_storage(&mut r)
    .unwrap();

    eq_balances::GenesisConfig::<Test> {
        balances: vec![
            (
                TreasuryModuleId::get().into_account_truncating(),
                vec![(1_000_000, asset::EQ.get_id())],
            ),
            (
                LendingModuleId::get().into_account_truncating(),
                vec![(300, asset::EQ.get_id())],
            ),
            (1, vec![(1_000, asset::BTC.get_id())]),
            (1, vec![(1_000, asset::ETH.get_id())]),
            (2, vec![(1_000, asset::ETH.get_id())]),
            (3, vec![(1_000, asset::EQD.get_id())]),
        ],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(true)),
    }
    .assimilate_storage(&mut r)
    .unwrap();

    eq_lending::GenesisConfig::<Test> {
        only_bailsmen_till: Some(1_000_000_000_000),
        lender_balances: vec![],
    }
    .assimilate_storage(&mut r)
    .unwrap();

    r.into()
}
