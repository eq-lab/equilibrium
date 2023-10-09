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

use crate as eq_bailsman;
use core::fmt::Debug;
use eq_balances;
use eq_primitives::{
    asset,
    asset::AssetType,
    mocks::TimeZeroDurationMock,
    subaccount::{SubAccType, SubaccountsManager},
    EqBuyout, OrderChange, UpdateTimeManager, XcmMode,
};
#[cfg(feature = "std")]
use frame_support::traits::GenesisBuild;
use frame_support::{parameter_types, traits::Everything, weights::Weight};
use sp_arithmetic::{FixedI64, FixedPointNumber, Permill};
use sp_core::H256;
use sp_runtime::{
    testing::{Header, TestXt, UintAuthorityId},
    traits::{BlakeTwo256, IdentityLookup},
    DispatchError, Percent,
};
use std::cell::RefCell;
use std::marker::PhantomData;
use system::EnsureRoot;

type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
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
        ModuleBailsman: eq_bailsman::{Pallet, Call, Storage, Event<T>},
        ModuleBalances: eq_balances::{Pallet, Call, Storage, Event<T>},
        ModuleAggregates: eq_aggregates::{Pallet, Call, Storage},
        ModuleAssets: eq_assets::{Pallet, Call, Storage, Event}
    }
);

parameter_types! {
    pub const MinimumPeriod: u64 = 1;
    pub const MaxPricePoints: u32 = 3;
    pub const MinimalCollateral: Balance = 5 * 1_000_000_000;
    pub const MinTempBalanceUsd: Balance = 0; // always reinit
    pub const ExistentialDeposit: Balance = 1;
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub CriticalMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 1000);
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/resrv");
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
    type AccountData = eq_primitives::balance::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub const BALANCE_ACCURACY: f64 = 1_000_000_000.0;

thread_local! {
    static PRICES: RefCell<Vec<(asset::Asset, FixedI64)>> = RefCell::new(vec![
        (asset::BTC, FixedI64::saturating_from_integer(10000)),
        (asset::EOS, FixedI64::saturating_from_integer(3)),
        (asset::ETH, FixedI64::saturating_from_integer(250)),
        (asset::EQD, FixedI64::saturating_from_integer(1)),
        (asset::EQ, FixedI64::saturating_from_integer(1)),
        (asset::DOT, FixedI64::saturating_from_integer(4)),
        (asset::CRV, FixedI64::saturating_from_integer(5)),
        (asset::USDC, FixedI64::saturating_from_integer(1)),
        (asset::USDT, FixedI64::saturating_from_integer(1)),
        ]);
}

pub struct OracleMock;

pub trait PriceSetter {
    fn set_price_mock(currency: &asset::Asset, value: &FixedI64);
}

impl PriceSetter for OracleMock {
    fn set_price_mock(currency: &asset::Asset, value: &FixedI64) {
        PRICES.with(|v| {
            let mut vec = v.borrow().clone();
            for pair in vec.iter_mut() {
                if pair.0 == *currency {
                    pair.1 = value.clone();
                }
            }

            *v.borrow_mut() = vec;
        });
    }
}
impl PriceGetter for OracleMock {
    fn get_price<FixedNumber>(asset: &Asset) -> Result<FixedNumber, sp_runtime::DispatchError>
    where
        FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>,
    {
        let mut return_value = Ok(FixedNumber::zero());
        PRICES.with(|v| {
            let value = v.borrow().clone();
            for pair in value.iter() {
                if pair.0 == *asset {
                    return_value = pair
                        .1
                        .clone()
                        .try_into()
                        .map_err(|_| sp_runtime::DispatchError::Other("Positive price"));
                }
            }
        });

        return_value
    }
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

pub struct EqBuyoutMock;

impl EqBuyout<u64, Balance> for EqBuyoutMock {
    fn eq_buyout(_who: &AccountId, _amount: Balance) -> sp_runtime::DispatchResult {
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

thread_local! {
    static ACC_OWNER: RefCell<Option<(AccountId, SubAccType)>> = RefCell::new(None);
}

pub struct SubaccountsManagerMock;
impl SubaccountsManagerMock {
    pub fn set_account_owner(account_id: AccountId, subacc: SubAccType) {
        ACC_OWNER.with(|v| {
            *v.borrow_mut() = Some((account_id, subacc));
        });
    }
}

impl SubaccountsManager<AccountId> for SubaccountsManagerMock {
    fn create_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        Ok(AccountId::default())
    }

    fn delete_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        Ok(AccountId::default())
    }

    fn has_subaccount(_who: &AccountId, _subacc_type: &SubAccType) -> bool {
        true
    }

    fn get_subaccount_id(_who: &AccountId, _subacc_type: &SubAccType) -> Option<AccountId> {
        Some(AccountId::default())
    }

    fn is_subaccount(_who: &AccountId, _subacc_id: &AccountId) -> bool {
        false
    }

    fn get_owner_id(_subaccount: &AccountId) -> Option<(AccountId, SubAccType)> {
        let mut result = None;
        ACC_OWNER.with(|v| {
            result = v.borrow().clone();
        });

        result
    }

    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        0
    }
}

impl eq_balances::Config for Test {
    type ParachainId = eq_primitives::mocks::ParachainId;
    type RuntimeEvent = RuntimeEvent;
    type ToggleTransferOrigin = EnsureRoot<AccountId>;
    type ForceXcmTransferOrigin = EnsureRoot<AccountId>;
    type AccountStore = System;
    type Balance = Balance;
    type ExistentialDeposit = ExistentialDeposit;
    type ExistentialDepositBasic = ExistentialDeposit;
    type BalanceChecker = ModuleBailsman;
    type PriceGetter = OracleMock;
    type Aggregates = eq_aggregates::Pallet<Test>;
    type TreasuryModuleId = TreasuryModuleId;
    type BailsmanModuleId = BailsmanModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = Pallet<Test>;
    type UpdateTimeManager = RateMock;
    type AssetGetter = eq_assets::Pallet<Test>;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type ModuleId = BalancesModuleId;
    type WeightInfo = ();
    type OrderAggregates = ();
    type UnixTime = TimeZeroDurationMock;
}

pub struct RateMock;
impl UpdateTimeManager<AccountId> for RateMock {
    fn set_last_update(_account_id: &AccountId) {}
    fn remove_last_update(_account_id: &AccountId) {}
    fn set_last_update_timestamp(_account_id: &u64, _timestamp_ms: u64) {}
}

impl eq_aggregates::Config for Test {
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Test>;
}

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
    type OnNewAsset = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const PriceTimeout: u64 = 1;
    pub const MedianPriceTimeout: u64 = 60 * 60 * 2;
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
}

pub const ACCOUNT_ID_BAD_SUB_GOOD: u64 = 23;
pub const ACCOUNT_ID_SUB_GOOD: u64 = 24;
pub struct MarginCallManagerMock;
impl MarginCallManagerMock {
    fn get_margin_state_mock(owner: &AccountId) -> Result<(MarginState, bool), DispatchError> {
        if *owner == ACCOUNT_ID_BAD_SUB_GOOD {
            Ok((MarginState::SubGood, false))
        } else if *owner == ACCOUNT_ID_SUB_GOOD {
            Ok((MarginState::SubGood, true))
        } else {
            Ok((MarginState::Good, false))
        }
    }
}

impl MarginCallManager<AccountId, Balance> for MarginCallManagerMock {
    fn check_margin_with_change(
        owner: &AccountId,
        _balance_changes: &[BalanceChange<Balance>],
        _order_changes: &[OrderChange],
    ) -> Result<(MarginState, bool), DispatchError> {
        Self::get_margin_state_mock(owner)
    }

    fn try_margincall(owner: &AccountId) -> Result<MarginState, DispatchError> {
        Self::get_margin_state_mock(owner).map(|r| r.0)
    }

    fn get_critical_margin() -> EqFixedU128 {
        CriticalMargin::get()
    }
}

parameter_types! {
    pub const MaxBailsmenToDistribute: u32 = 1;
    pub const UnsignedPriority: u64 = 0;
    pub const QueueLengthWeightConstant: u32 = 5;
}

impl<LocalCall> SendTransactionTypes<LocalCall> for Test
where
    RuntimeCall: From<LocalCall>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = TestXt<RuntimeCall, ()>;
}

impl Config for Test {
    type AssetGetter = eq_assets::Pallet<Test>;
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type BalanceGetter = ModuleBalances;
    type EqCurrency = ModuleBalances;
    type PriceGetter = OracleMock;
    type MinimalCollateral = MinimalCollateral;
    type MinTempBalanceUsd = MinTempBalanceUsd;
    type UnixTime = pallet_timestamp::Pallet<Self>;
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

pub fn new_test_ext() -> sp_io::TestExternalities {
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
            (
                asset::USDC.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                7,
                AssetType::Physical,
                true,
                Percent::zero(),
                Permill::one(),
            ),
            (
                asset::USDT.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::from_rational(2u32, 5u32),
                8,
                AssetType::Physical,
                true,
                Percent::from_rational(5u32, 10u32),
                Permill::one(),
            ),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    eq_balances::GenesisConfig::<Test> {
        balances: vec![
            (
                1,
                vec![(1000_000_000_000_000 as Balance, asset::BTC.get_id())],
            ),
            (
                2,
                vec![(1000_000_000_000_000 as Balance, asset::BTC.get_id())],
            ),
        ],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(false)),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    eq_bailsman::GenesisConfig::<Test> { bailsmen: vec![] }
        .assimilate_storage(&mut t)
        .unwrap();

    t.into()
}
