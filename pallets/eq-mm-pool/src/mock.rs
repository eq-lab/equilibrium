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
use crate as eq_mm_pool;

use eq_primitives::{
    asset::{self, Asset, AssetType},
    balance::BalanceChecker,
    balance_number::EqFixedU128,
    subaccount::{SubAccType, SubaccountsManager},
    BailsmanManager, BalanceChange, EqBuyout, MarginCallManager, MarginState, Order, OrderChange,
    OrderSide, SignedBalance,
};
use eq_utils::ONE_TOKEN;
use frame_support::{
    parameter_types,
    traits::{Everything, GenesisBuild, UnixTime, WithdrawReasons},
    PalletId,
};
use frame_system::{offchain::SendTransactionTypes, EnsureRoot};
use sp_arithmetic::{FixedI64, Permill};
use sp_core::H256;
use sp_runtime::{
    impl_opaque_keys,
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    DispatchError, DispatchResult, FixedI128, FixedPointNumber, Perbill,
};
use sp_runtime::{ModuleError, Percent};
use std::{cell::RefCell, collections::HashMap, marker::PhantomData};

type AccountId = u64;
type Balance = eq_primitives::balance::Balance;
type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;
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
        Session: session::{Pallet, Call, Storage, Event},
        EqBalances: eq_balances::{Pallet, Call, Storage, Event<T>},
        EqAggregates: eq_aggregates::{Pallet, Call, Storage},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event},
        EqRate: eq_rate::{Pallet, Storage, Call, ValidateUnsigned},
        EqDex: eq_dex::{Pallet, Call, Storage, Event<T>},
        MmPool: eq_mm_pool::{Pallet, Call, Storage, Event<T>},
    }
);

parameter_types! {
    pub const ExistentialDeposit: Balance = 1;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
    pub const MinimumPeriod: u64 = 1;
    pub const UnsignedPriority: u64 = 100;
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

test_utils::implement_financial!();

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
pub struct SubaccountsManagerMock;

impl SubaccountsManager<AccountId> for SubaccountsManagerMock {
    fn create_subaccount_inner(
        who: &AccountId,
        subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        ensure!(
            subacc_type == &SubAccType::Trader,
            DispatchError::Module(ModuleError {
                index: 4,
                error: Default::default(),
                message: None
            })
        );
        Ok(who << 10)
    }

    fn delete_subaccount_inner(
        who: &AccountId,
        subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        ensure!(
            subacc_type == &SubAccType::Trader,
            DispatchError::Module(ModuleError {
                index: 4,
                error: Default::default(), // 19,
                message: None
            })
        );
        Ok(who << 10)
    }

    fn has_subaccount(_who: &AccountId, subacc_type: &SubAccType) -> bool {
        subacc_type == &SubAccType::Trader
    }

    fn get_subaccount_id(who: &AccountId, subacc_type: &SubAccType) -> Option<AccountId> {
        if subacc_type == &SubAccType::Trader {
            Some(who << 10)
        } else {
            None
        }
    }

    fn is_subaccount(who: &AccountId, subacc_id: &AccountId) -> bool {
        // hack for not deleting account in transfer
        (*who << 10 == *subacc_id) & (*who == *subacc_id >> 10)
    }

    fn get_owner_id(subaccount: &AccountId) -> Option<(AccountId, SubAccType)> {
        let owner = subaccount >> 10;
        if owner << 10 == *subaccount {
            Some((owner, SubAccType::Trader))
        } else {
            None
        }
    }

    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        1
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

parameter_types! {
    pub const BasicCurrencyGet: asset::Asset = asset::EQ;
}

pub struct BalanceCheckerMock;

pub const FAIL_ACC: AccountId = 666;

impl BalanceChecker<Balance, AccountId, EqBalances, SubaccountsManagerMock> for BalanceCheckerMock {
    fn need_to_check_impl(
        _who: &AccountId,
        _changes: &Vec<(Asset, SignedBalance<Balance>)>,
    ) -> bool {
        true
    }
    fn can_change_balance_impl(
        who: &AccountId,
        _change: &Vec<(asset::Asset, SignedBalance<Balance>)>,
        _: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        match who {
            &FAIL_ACC => Err(DispatchError::Other("Expected error")),
            _ => Ok(()),
        }
    }
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
        EqFixedU128::saturating_from_rational::<i32, i32>(5, 100)
    }
}

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = BasicCurrencyGet;
    type OnNewAsset = ();
    type WeightInfo = ();
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
    type ExistentialDepositEq = ExistentialDeposit;
    type BalanceChecker = BalanceCheckerMock;
    type PriceGetter = OracleMock;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Aggregates = EqAggregates;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmanManagerMock;
    type UpdateTimeManager = EqRate;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type OrderAggregates = ();
    type UnixTime = TimeMock;
}

thread_local! {
    pub static UNIX_NOW: RefCell<u64> = RefCell::new(0);
}
pub const SECS_PER_BLOCK: u64 = 12;

pub struct TimeMock;

impl TimeMock {
    #[allow(dead_code)]
    pub fn set_secs(secs: u64) {
        UNIX_NOW.with(|now| *now.borrow_mut() = secs)
    }

    #[allow(dead_code)]
    pub fn move_secs(secs: u64) {
        use frame_support::traits::Hooks;

        UNIX_NOW.with(|time| {
            *time.borrow_mut() += secs;

            let end_block = *time.borrow() / SECS_PER_BLOCK;

            while System::block_number() < end_block {
                if System::block_number() > 1 {
                    MmPool::on_finalize(System::block_number());
                    System::on_finalize(System::block_number());
                }
                System::set_block_number(System::block_number() + 1);
                System::on_initialize(System::block_number());
                MmPool::on_initialize(System::block_number());
            }
        });
    }
}

impl UnixTime for TimeMock {
    fn now() -> core::time::Duration {
        UNIX_NOW.with(|now| core::time::Duration::from_secs(*now.borrow()))
    }
}

impl eq_aggregates::Config for Test {
    type Balance = Balance;
    type BalanceGetter = EqBalances;
}

parameter_types! {
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const PriceStepCount: u32 = 10_000;
    pub const PenaltyFee: Balance = 5_000_000_000;
    pub const DexUnsignedPriority: u64 = 100;
    pub const MinSurplus: Balance = 1 * 1_000_000_000; // 1 usd
    pub const MinTempBailsman: Balance = 20 * 1_000_000_000; // 20 usd
}

impl eq_dex::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type DeleteOrderOrigin = EnsureRoot<AccountId>;
    type UpdateAssetCorridorOrigin = EnsureRoot<AccountId>;
    type PriceStepCount = PriceStepCount;
    type PenaltyFee = PenaltyFee;
    type DexUnsignedPriority = DexUnsignedPriority;
    type WeightInfo = ();
    type ValidatorOffchainBatcher = EqRate;
}

impl authorship::Config for Test {
    type FindAuthor = ();
    type EventHandler = ();
}

parameter_types! {
    pub const LendingModuleId: PalletId = PalletId(*b"eq/lendr");
    pub RiskLowerBound: FixedI128 = FixedI128::saturating_from_rational(1, 2);
    pub RiskUpperBound: FixedI128 = FixedI128::saturating_from_integer(2);
    pub RiskNSigma: FixedI128 = FixedI128::saturating_from_integer(10);
    pub RiskRho: FixedI128 = FixedI128::saturating_from_rational(7, 10);
    pub Alpha: FixedI128 = FixedI128::saturating_from_integer(15);
    pub const TreasuryFee: Permill = Permill::from_percent(1);
    pub const WeightFeeTreasury: u32 = 80;
    pub const WeightFeeValidator: u32 = 20;
    pub BaseBailsmanFee: Permill = Permill::from_percent(1);
    pub BaseLenderFee: Permill =  Permill::from_rational(5u32, 1000u32);
    pub LenderPart: Permill = Permill::from_percent(30);
}

impl eq_rate::Config for Test {
    type AutoReinitToggleOrigin = EnsureRoot<AccountId>;
    type BailsmanManager = BailsmanManagerMock;
    type UnixTime = TimeMock;
    type Balance = Balance;
    type BalanceGetter = EqBalances;
    type BalanceRemover = EqBalances;
    type AssetGetter = EqAssets;
    type PriceGetter = OracleMock;
    type Aggregates = EqAggregates;
    type AuthorityId = sp_runtime::testing::UintAuthorityId;
    type MarginCallManager = MarginCallManagerMock;
    type MinSurplus = MinSurplus;
    type MinTempBailsman = MinTempBailsman;
    type EqBuyout = EqBuyoutMock;
    type BailsmanModuleId = BailsmanModuleId;
    type EqCurrency = EqBalances;
    type SubaccountsManager = SubaccountsManagerMock;
    type UnsignedPriority = UnsignedPriority;

    type RiskLowerBound = RiskLowerBound;
    type RiskUpperBound = RiskUpperBound;
    type RiskNSigma = RiskNSigma;
    type Alpha = Alpha;
    type Financial = Pallet<Test>;
    type FinancialStorage = ();

    type TreasuryFee = TreasuryFee;
    type WeightFeeTreasury = WeightFeeTreasury;
    type WeightFeeValidator = WeightFeeValidator;
    type BaseBailsmanFee = BaseBailsmanFee;
    type BaseLenderFee = BaseLenderFee;
    type LenderPart = LenderPart;

    type TreasuryModuleId = TreasuryModuleId;
    type LendingModuleId = LendingModuleId;
    type WeightInfo = ();
    type LendingPoolManager = ();
    type LendingAssetRemoval = ();
    type RedistributeWeightInfo = ();
}

impl<LocalCall> SendTransactionTypes<LocalCall> for Test
where
    RuntimeCall: From<LocalCall>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = sp_runtime::testing::TestXt<RuntimeCall, ()>;
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub eq_rate: EqRate,
    }
}

impl session::Config for Test {
    type ShouldEndSession = session::PeriodicSessions<Period, Offset>;
    type SessionManager = ();
    type SessionHandler = (EqRate,);
    type ValidatorId = u64;
    type ValidatorIdOf = sp_runtime::traits::ConvertInto;
    type Keys = SessionKeys;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}

parameter_types! {
    pub const MmPoolModuleId: PalletId = PalletId(*b"eqmmpool");
}

pub const MM_ID: [u16; 2] = [123, 456];

pub const TRADER_NOT_EXIST: AccountId = 419;
pub const TRADER_EXIST: AccountId = 420;

impl eq_mm_pool::Config for Test {
    type ModuleId = MmPoolModuleId;
    type MarketMakersManagementOrigin = EnsureRoot<AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type EqCurrency = EqBalances;
    type WeightInfo = ();
    type Balance = Balance;
    type Aggregates = EqAggregates;
    type SubaccountsManager = SubaccountsManagerMock;
    type OrderManagement = EqDex;
    type AssetGetter = EqAssets;
    type DexWeightInfo = ();
    type UnixTime = TimeMock;
}

thread_local! {
    pub static REFS: RefCell<HashMap<AccountId, u32>> = RefCell::new(HashMap::new());
}
pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::EQD, FixedI64::saturating_from_integer(10)),
        (asset::BTC, FixedI64::saturating_from_integer(10)),
        (asset::ETH, FixedI64::saturating_from_integer(10)),
        (asset::DOT, FixedI64::saturating_from_integer(10)),
        (asset::CRV, FixedI64::saturating_from_integer(10)),
        (asset::EOS, FixedI64::saturating_from_integer(10)),
        (asset::EQ, FixedI64::saturating_from_integer(10)),
    ]);
    let mut r = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    eq_assets::GenesisConfig::<Test> {
        _runtime: PhantomData,
        assets: // id, lot, price_step, maker_fee, taker_fee, debt_weight, buyout_priority, dex_enabled, collateral_enabled
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
                EqFixedU128::saturating_from_rational(1,1000),
                FixedI64::from(1),
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
                EqFixedU128::saturating_from_rational(1,1000),
                FixedI64::from(1),
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
        is_xcm_enabled: None,
        balances: vec![(
            1,
            vec![
                (1_000_000 * ONE_TOKEN, asset::EQD.get_id()),
                (1000, asset::BTC.get_id()),
                (1000, asset::ETH.get_id()),
                (1000, asset::DOT.get_id()),
            ],
        )],
        is_transfers_enabled: true,
    }
    .assimilate_storage(&mut r)
    .unwrap();

    eq_mm_pool::GenesisConfig::<Test> {
        epoch_duration: 100,
        _runtime: PhantomData,
    }
    .assimilate_storage(&mut r)
    .unwrap();

    r.into()
}

#[allow(dead_code)]
pub fn all_orders(asset: Asset, expected_side: OrderSide) -> Vec<Order<AccountId>> {
    eq_dex::Pallet::<Test>::actual_price_chunks(asset)
        .into_iter()
        .flat_map(|chunk_key| {
            eq_dex::Pallet::<Test>::orders_by_asset_and_chunk_key(asset, chunk_key)
        })
        .filter(|order| order.side == expected_side)
        .collect()
}
