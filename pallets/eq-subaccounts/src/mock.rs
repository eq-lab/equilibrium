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

use crate as eq_subaccounts;
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::mocks::TimeZeroDurationMock;
use eq_primitives::{
    asset, asset::AssetType, BalanceChange, MarginCallManager, MarginState, OrderChange,
};
use frame_support::pallet_prelude::PhantomData;
use frame_support::weights::Weight;
use frame_support::PalletId;
use frame_support::{dispatch::DispatchError, parameter_types};
use frame_system::offchain::SendTransactionTypes;
use frame_system::EnsureRoot;
use sp_arithmetic::{FixedI64, FixedPointNumber, Permill};
use sp_core::H256;
use sp_runtime::testing::{TestXt, UintAuthorityId};
use sp_runtime::Percent;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    FixedI128, Perbill,
};

pub(crate) type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;

use core::convert::{TryFrom, TryInto};

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        EqSubaccounts: eq_subaccounts::{Pallet, Call, Storage, Event<T>},
        Timestamp: timestamp::{Pallet, Call, Storage},
        EqBailsman: eq_bailsman::{Pallet, Call, Storage, Event<T>},
        EqBalances: eq_balances::{Pallet, Call, Storage, Event<T>},
        EqWhitelists: eq_whitelists::{Pallet, Call, Storage, Event<T>},
        EqAggregates: eq_aggregates::{Pallet, Call, Storage},
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
    pub const MinimumPeriod: u64 = 1;
    pub const EpochDuration: u64 = 3;
    pub const ExpectedBlockTime: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const MaxPricePoints: usize = 3;

    pub const MinimalCollateral: Balance = 100_000 * 1_000_000_000; // 100_000 USD
    pub const TotalIssuance: Balance = 1_000_000_000;
    pub const MinTempBalanceUsd: Balance = 0; // always reinit
    pub const ExistentialDeposit: Balance = 1;
    pub RiskLowerBound: FixedI128 = FixedI128::saturating_from_rational(1, 2);
    pub RiskUpperBound: FixedI128 = FixedI128::saturating_from_integer(2);
    pub RiskNSigma: FixedI128 = FixedI128::saturating_from_integer(10);
    pub RiskRho: FixedI128 = FixedI128::saturating_from_rational(7, 10);
    pub Alpha: FixedI128 = FixedI128::from(15);
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub CriticalMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 1000);
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

// impl eq_subaccounts::Config for Test {
//     type AssetGetter = eq_assets::Pallet<Test>;
// }

thread_local! {
    static MARGIN_STATE: RefCell<MarginState> = RefCell::new(MarginState::Good);
}

impl timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

pub struct MarginCallManagerMock;
impl MarginCallManagerMock {
    pub(crate) fn set_margin_state(margin_state: MarginState) {
        MARGIN_STATE.with(|v| {
            *v.borrow_mut() = margin_state;
        })
    }

    fn get_margin_state_mock() -> Result<MarginState, DispatchError> {
        let mut margin_state = MarginState::Good;
        MARGIN_STATE.with(|v| {
            margin_state = v.borrow().clone();
        });

        Ok(margin_state)
    }
}

impl MarginCallManager<AccountId, Balance> for MarginCallManagerMock {
    fn check_margin_with_change(
        _owner: &AccountId,
        _balance_changes: &[BalanceChange<Balance>],
        _order_changes: &[OrderChange],
    ) -> Result<(MarginState, bool), DispatchError> {
        let margin = Self::get_margin_state_mock()?;
        Ok((margin, false))
    }

    fn try_margincall(_owner: &AccountId) -> Result<MarginState, DispatchError> {
        Self::get_margin_state_mock()
    }

    fn get_critical_margin() -> EqFixedU128 {
        CriticalMargin::get()
    }
}

pub type ModuleBalances = eq_balances::Pallet<Test>;
pub type ModuleAggregates = eq_aggregates::Pallet<Test>;
pub type ModuleBailsman = eq_bailsman::Pallet<Test>;

parameter_types! {
    pub const UnsignedPriority: u64 = 100;
    pub const MaxBailsmenToDistribute: u32 = 1;
}

impl<LocalCall> SendTransactionTypes<LocalCall> for Test
where
    Call: From<LocalCall>,
{
    type OverarchingCall = Call;
    type Extrinsic = TestXt<Call, ()>;
}

parameter_types! {
    pub const QueueLengthWeightConstant: u32 = 5;
}

impl eq_bailsman::Config for Test {
    type Event = Event;
    type Balance = Balance;
    type AssetGetter = eq_assets::Pallet<Test>;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type EqCurrency = eq_balances::Pallet<Test>;
    type PriceGetter = OracleMock;
    type MinimalCollateral = MinimalCollateral;
    type MinTempBalanceUsd = MinTempBalanceUsd;
    type UnixTime = timestamp::Pallet<Self>;
    type PalletId = BailsmanModuleId;
    type Aggregates = ModuleAggregates;
    type WeightInfo = ();
    type MarginCallManager = MarginCallManagerMock;
    type SubaccountsManager = ModuleSubaccounts;

    type AuthorityId = UintAuthorityId;
    type MaxBailsmenToDistribute = MaxBailsmenToDistribute;
    type UnsignedPriority = UnsignedPriority;
    type ValidatorOffchainBatcher = ();
    type QueueLengthWeightConstant = QueueLengthWeightConstant;
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
    type BalanceChecker = (ModuleBailsman, ModuleSubaccounts);
    type PriceGetter = OracleMock;
    type Event = Event;
    type WeightInfo = ();
    type Aggregates = ModuleAggregates;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = ModuleSubaccounts;
    type BailsmenManager = eq_bailsman::Pallet<Test>;
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

pub struct RateMock;
impl UpdateTimeManager<u64> for RateMock {
    fn set_last_update(_account_id: &AccountId) {}
    fn remove_last_update(_accounts_id: &AccountId) {}
    fn set_last_update_timestamp(_account_id: &AccountId, _timestamp_ms: u64) {}
}

impl eq_aggregates::Config for Test {
    type Balance = Balance;
    type BalanceGetter = ModuleBalances;
}

impl eq_whitelists::Config for Test {
    type Event = Event;
    type WhitelistManagementOrigin = EnsureRoot<AccountId>;
    type OnRemove = ();
    type WeightInfo = ();
}

impl Config for Test {
    type Balance = Balance;
    type Aggregates = ModuleAggregates;
    type EqCurrency = ModuleBalances;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type PriceGetter = OracleMock;
    type BailsmenManager = eq_bailsman::Pallet<Test>;
    type Event = Event;
    type Whitelist = eq_whitelists::Pallet<Test>;
    type UpdateTimeManager = RateMock;
    type WeightInfo = ();
    type IsTransfersEnabled = ModuleBalances;
    type AssetGetter = eq_assets::Pallet<Test>;
}

pub type ModuleSubaccounts = Pallet<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::BTC, FixedI64::saturating_from_integer(10000)),
        (asset::EOS, FixedI64::saturating_from_integer(3)),
        (asset::ETH, FixedI64::saturating_from_integer(250)),
        (asset::EQD, FixedI64::saturating_from_integer(1)),
        (asset::EQ, FixedI64::saturating_from_integer(1)),
        (asset::DOT, FixedI64::saturating_from_integer(4)),
    ]);

    let mut t = frame_system::GenesisConfig::default()
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
                asset::EOS.get_id(),
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
                asset::DOT.get_id(),
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
                asset::CRV.get_id(),
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
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
