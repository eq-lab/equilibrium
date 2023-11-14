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

use super::*;
use crate::mock::sp_api_hidden_includes_construct_runtime::hidden_include::traits::GenesisBuild;
use crate::{self as eq_lockdrop};
use core::fmt::Debug;
use eq_primitives::mocks::TimeZeroDurationMock;
use eq_primitives::XcmMode;
use eq_primitives::{
    asset,
    asset::{Asset, AssetData, AssetType},
    balance_number::EqFixedU128,
    subaccount::{SubAccType, SubaccountsManager},
    Aggregates, BailsmanManager, BlockNumberToBalance, EqBuyout, MarginCallManager, SignedBalance,
    TotalAggregates, UpdateTimeManager, UserGroup,
};
use frame_support::parameter_types;
use frame_support::PalletId;
use frame_system as system;
use frame_system::offchain::SendTransactionTypes;
use sp_core::H256;
use sp_runtime::{
    generic::Header,
    testing::UintAuthorityId,
    traits::{BlakeTwo256, IdentityLookup, Member, One},
    DispatchError, FixedI128, FixedI64, FixedPointNumber, Perbill, Permill,
};
use sp_runtime::{DispatchResult, Percent};
use std::{cell::RefCell, marker::PhantomData};
use system::EnsureRoot;

type AccountId = u64;
type Balance = eq_primitives::balance::Balance;
type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;

use core::convert::{TryFrom, TryInto};

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        EqLockdrop: eq_lockdrop::{Pallet, Call, Storage, Event<T>},
        EqBalances: eq_balances::{Pallet, Call, Storage, Event<T>},
        Timestamp: timestamp::{Pallet, Call, Storage},
        EqRate: eq_rate::{Pallet, Storage, Call, ValidateUnsigned},
        Session: pallet_session::{Pallet, Call, Storage, Event},
        // EqBailsman: eq_bailsman::{Pallet, Call, Storage, Event<T>},
        EqSessionManager: eq_session_manager::{Pallet, Call, Storage, Event<T>},
        // EqMarginCall: eq_margin_call::{Pallet, Call, Storage, Event<T>},
        EqVesting: eq_vesting::{Pallet, Call, Storage, Event<T>},
        EqAssets: eq_assets::{Pallet, Storage, Call, Event}
    }
);

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
    type OnNewAsset = ();
    type WeightInfo = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type DummyValidatorId = u64;

pub type ModuleBalances = eq_balances::Pallet<Test>;
pub type ModuleTimestamp = timestamp::Pallet<Test>;
pub type ModuleLockdrop = eq_lockdrop::Pallet<Test>;
pub type ModuleVesting = eq_vesting::Pallet<Test>;

parameter_types! {
    pub const BlockHashCount: u32 = 250;
    pub const SS58Prefix: u8 = 42;
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
}

impl system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = u32;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header<u32, BlakeTwo256>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = eq_primitives::balance::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub type ModuleSystem = frame_system::Pallet<Test>;

impl eq_session_manager::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorsManagementOrigin = EnsureRoot<AccountId>;
    type ValidatorId = DummyValidatorId;
    type RegistrationChecker = Session;
    type ValidatorIdOf = ();
    type WeightInfo = ();
}

/*impl eq_bailsman::Config for Test {
    type AssetGetter = eq_assets::Pallet<Test>;
    type RuntimeEvent = RuntimeEvent;
    type UnixTime = ModuleTimestamp;
    type Balance = u128;
    type BalanceGetter = ModuleBalances;
    type EqCurrency = ModuleBalances;
    type PriceGetter = OracleMock; // OracleMock;
    type MinimalCollateral = MinimalCollateral;
    type MinTempBalanceUsd = MinTempBalanceUsd;
    type RiskLowerBound = RiskLowerBound;
    type RiskUpperBound = RiskUpperBound;
    type RiskNSigma = RiskNSigma;
    type RiskRho = RiskRho;
    type Alpha = Alpha;
    type PalletId = BailsmanModuleId;
    type Aggregates = AggregatesMock;
    type Financial = Pallet<Test>;
    type FinancialStorage = Pallet<Test>;
    type WeightInfo = ();
    type MarginCallManager = MarginCallManagerMock;
    type SubaccountsManager = SubaccountsManagerMock;
}*/

parameter_types! {
    pub const ExistentialDeposit: Balance = 1;
    pub const EpochDuration: u64 = 3;
    pub const ExpectedBlockTime: u64 = 1;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const LockdropModuleId: PalletId = PalletId(*b"eq/lkdrp");
    pub const LockPeriod: u64 = 90 * 24 * 60 * 60; // 90 days
    pub const MinLockAmount: Balance = 0;
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
}

thread_local! {
    static PRICES: RefCell<Vec<(asset::Asset, FixedI64)>> = RefCell::new(vec![
        (asset::CRV, FixedI64::saturating_from_integer(10000)),
        (asset::BTC, FixedI64::saturating_from_integer(10000)),
        (asset::EOS, FixedI64::saturating_from_integer(3)),
        (asset::ETH, FixedI64::saturating_from_integer(250)),
        (asset::EQD, FixedI64::saturating_from_integer(1)),
        ]);
    static CURRENT_TIME: RefCell<u64> = RefCell::new(1598006981634);
}

/*pub struct OracleMock;

pub trait PriceSetter {
    fn set_price_mock(asset: &Asset, value: &FixedI64);
}

impl PriceSetter for OracleMock {
    fn set_price_mock(asset: &Asset, value: &FixedI64) {
        PRICES.with(|v| {
            let mut vec = v.borrow().clone();
            for pair in vec.iter_mut() {
                if pair.0 == *asset {
                    pair.1 = value.clone();
                }
            }

            *v.borrow_mut() = vec;
        });
    }
}

impl PriceGetter for OracleMock {
    fn get_price(asset: &Asset) -> Result<FixedI64, sp_runtime::DispatchError> {
        let mut return_value = FixedI64::zero();
        PRICES.with(|v| {
            let value = v.borrow().clone();
            for pair in value.iter() {
                if pair.0 == *asset {
                    return_value = pair.1.clone();
                }
            }
        });

        Ok(return_value)
    }
}*/

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

thread_local! {
        pub static ASSET_DATA: RefCell<AssetData<Asset>> = RefCell::new(AssetData {
        asset_type: AssetType::Physical,
        id: eq_primitives::asset::EQD,
        is_dex_enabled: true,
        price_step: FixedI64::from(1),
        buyout_priority: 1,
        lot: EqFixedU128::from(1),
        maker_fee: Permill::from_rational(5u32, 10_000u32),
        asset_xcm_data: eq_primitives::asset::AssetXcmData::None,
        debt_weight: Permill::zero(),
        taker_fee: Permill::from_rational(1u32, 1000u32),
        collateral_discount: Percent::one(),
        lending_debt_weight: Permill::one(),
    });
}

/*pub struct AssetGetterMock;
impl AssetGetterMock {
    pub fn set_asset_data(asset_data: AssetData<Asset>) {
        ASSET_DATA.with(|v| *v.borrow_mut() = asset_data);
    }
}
impl AssetGetter for AssetGetterMock {
    fn is_allowed_as_collateral(_: &Asset) -> bool {
        true
    }

    fn get_collateral_enabled_assets() -> Vec<Asset> {
        Vec::default()
    }

    fn get_asset_data(asset: &Asset) -> Result<AssetData<Asset>, DispatchError> {
        frame_support::ensure!(*asset != DAI, eq_assets::Error::<Test>::AssetNotExists);
        ASSET_DATA.with(|v| Ok(v.borrow().clone()))
    }

    fn exists(asset: Asset) -> bool {
        asset != DAI
    }

    fn get_assets_data() -> Vec<AssetData<Asset>> {
        Vec::default()
    }

    fn get_assets_data_with_usd() -> Vec<AssetData<Asset>> {
        Vec::default()
    }

    fn get_assets() -> Vec<Asset> {
        vec![asset::ETH]
    }

    fn get_assets_with_usd() -> Vec<Asset> {
        Vec::default()
    }

    fn priority(_asset: Asset) -> Option<u64> {
        None
    }

    fn get_main_asset() -> Asset {
        EQ
    }
}

parameter_types! {
    pub InitialMargin: FixedI128 = FixedI128::saturating_from_rational(5, 100);
    pub MaintenanceMargin: FixedI128 = FixedI128::saturating_from_rational(25, 1000);
    pub CriticalMargin: FixedI128 = FixedI128::saturating_from_rational(5, 1000);
    pub MaintenancePeriod: u64 = 86_400u64;
}

impl eq_margin_call::Config for Test {
    type SubaccountsManager = SubaccountsManagerMock;
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type UnixTime = ModuleTimestamp;
    type BailsmenManager = eq_bailsman::Pallet<Test>;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type PriceGetter = PriceGetter; // OracleMock;
    type InitialMargin = InitialMargin;
    type MaintenanceMargin = MaintenanceMargin;
    type CriticalMargin = CriticalMargin;
    type MaintenancePeriod = MaintenancePeriod;
    type OrderAggregates = ();
    type AssetGetter = AssetGetterMock;
    type WeightInfo = ();
}*/

pub struct SubaccountsManagerMock;

impl SubaccountsManager<u64> for SubaccountsManagerMock {
    fn create_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<u64, DispatchError> {
        Ok(9999_u64)
    }

    fn delete_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<u64, DispatchError> {
        Ok(9999_u64)
    }

    fn has_subaccount(_who: &AccountId, _subacc_type: &SubAccType) -> bool {
        true
    }

    fn get_subaccount_id(_who: &AccountId, _subacc_type: &SubAccType) -> Option<u64> {
        Some(9999_u64)
    }

    fn is_subaccount(_who: &AccountId, _subacc_id: &u64) -> bool {
        false
    }

    fn get_owner_id(_subaccount: &AccountId) -> Option<(u64, SubAccType)> {
        None
    }

    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        0
    }
}

pub struct AggregatesMock;

impl Aggregates<AccountId, Balance> for AggregatesMock {
    fn in_usergroup(_account_id: &DummyValidatorId, _user_group: UserGroup) -> bool {
        true
    }
    fn set_usergroup(
        _account_id: &DummyValidatorId,
        _user_group: UserGroup,
        _is_in: bool,
    ) -> DispatchResult {
        Ok(())
    }

    fn update_total(
        _account_id: &DummyValidatorId,
        _asset: Asset,
        _prev_balance: &SignedBalance<Balance>,
        _delta_balance: &SignedBalance<Balance>,
    ) -> DispatchResult {
        Ok(())
    }

    fn iter_account(_user_group: UserGroup) -> Box<dyn Iterator<Item = DummyValidatorId>> {
        panic!("AggregatesMock not implemented");
    }
    fn iter_total(
        _user_group: UserGroup,
    ) -> Box<dyn Iterator<Item = (Asset, TotalAggregates<u128>)>> {
        panic!("AggregatesMock not implemented");
    }
    fn get_total(_user_group: UserGroup, _asset: Asset) -> TotalAggregates<u128> {
        TotalAggregates {
            collateral: 1000,
            debt: 10,
        }
    }
}

pub struct MarginCallManagerMock<AccountId, Balance>(PhantomData<(AccountId, Balance)>);

impl<AccountId, Balance> MarginCallManager<AccountId, Balance>
    for MarginCallManagerMock<AccountId, Balance>
where
    Balance: Member + Debug,
{
    fn check_margin_with_change(
        _owner: &AccountId,
        _balance_changes: &[eq_primitives::BalanceChange<Balance>],
        _order_changes: &[eq_primitives::OrderChange],
    ) -> Result<(eq_primitives::MarginState, bool), DispatchError> {
        Ok((eq_primitives::MarginState::Good, false))
    }

    fn try_margincall(_owner: &AccountId) -> Result<eq_primitives::MarginState, DispatchError> {
        Ok(eq_primitives::MarginState::Good)
    }

    fn get_critical_margin() -> EqFixedU128 {
        EqFixedU128::saturating_from_rational(5, 1000)
    }
}

pub struct BailsmanManagerMock;

impl BailsmanManager<u64, u128> for BailsmanManagerMock {
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
        _: &[(asset::Asset, SignedBalance<u128>)],
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

pub struct RateMock;
impl UpdateTimeManager<u64> for RateMock {
    fn set_last_update(_account_id: &u64) {}
    fn remove_last_update(_accounts_id: &u64) {}
    fn set_last_update_timestamp(_account_id: &u64, _timestamp_ms: u64) {}
}

impl eq_balances::Config for Test {
    type ParachainId = eq_primitives::mocks::ParachainId;
    type ToggleTransferOrigin = EnsureRoot<AccountId>;
    type ForceXcmTransferOrigin = EnsureRoot<AccountId>;
    type AssetGetter = eq_assets::Pallet<Test>;
    type AccountStore = System;
    type Balance = u128;
    type ExistentialDeposit = ExistentialDeposit;
    type ExistentialDepositBasic = ExistentialDeposit;
    type ExistentialDepositEq = ExistentialDeposit;
    type BalanceChecker = ();
    type PriceGetter = OracleMock;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Aggregates = AggregatesMock;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmanManagerMock;
    type UpdateTimeManager = RateMock;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = ();
    type XcmToFee = ();
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type OrderAggregates = ();
    type UnixTime = TimeZeroDurationMock;
}
parameter_types! {
    pub const MinVestedTransfer: u128 = 10;
    pub const BasicCurrencyGet: asset::Asset = asset::EQ;
    pub const VestingModuleId: PalletId = PalletId(*b"eq/vestn");
}

pub type BasicCurrency = eq_primitives::balance_adapter::BalanceAdapter<
    u128,
    eq_balances::Pallet<Test>,
    BasicCurrencyGet,
>;

impl eq_vesting::Config for Test {
    type PalletId = VestingModuleId;
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type Currency = BasicCurrency;
    type BlockNumberToBalance = BlockNumberToBalance;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = ();
    type IsTransfersEnabled = ModuleBalances;
}

parameter_types! {
    pub const LockDropUnsignedPriority: u64 = 100;
}

impl eq_lockdrop::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = LockdropModuleId;
    type LockPeriod = LockPeriod;
    type Vesting = eq_vesting::Pallet<Test>;
    type ValidatorOffchainBatcher = eq_rate::Pallet<Test>;
    type MinLockAmount = MinLockAmount;
    type LockDropUnsignedPriority = LockDropUnsignedPriority;
    type WeightInfo = ();
}

pub type Extrinsic = sp_runtime::testing::TestXt<RuntimeCall, ()>;

impl<LocalCall> SendTransactionTypes<LocalCall> for Test
where
    RuntimeCall: From<LocalCall>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = Extrinsic;
}

parameter_types! {
    pub const Period: u32 = 1;
    pub const Offset: u32 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const MinimalCollateral: u128 = 10 * 1_000_000_000;
    pub const MinTempBalanceUsd: u128 = 0;
    pub const UnsignedPriority: u64 = 100;
}

sp_runtime::impl_opaque_keys! {
    pub struct SessionKeys {
        pub eq_rate: eq_rate::Pallet<Test>,
    }
}

impl pallet_session::Config for Test {
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionManager = ();
    type SessionHandler = (eq_rate::Pallet<Test>,);
    type ValidatorId = u64;
    type ValidatorIdOf = sp_runtime::traits::ConvertInto;
    type Keys = SessionKeys;
    type RuntimeEvent = RuntimeEvent;
    // type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}

parameter_types! {
    pub const MinSurplus: u128 = 1 * 1000_000_000; // 1 usd
    pub const MinTempBailsman: u128 = 20 * 1000_000_000; // 20 usd
}

test_utils::implement_financial!();

parameter_types! {
    pub const LendingModuleId: PalletId = PalletId(*b"eq/lendr");
    pub TreasuryFee: Permill = Permill::from_percent(1);
    pub const WeightFeeTreasury: u32 = 80;
    pub const WeightFeeValidator: u32 = 20;
    pub BaseBailsmanFee: Permill = Permill::from_percent(1);
    pub BaseLenderFee: Permill =  Permill::from_rational(5u32, 1000u32);
    pub LenderPart: Permill = Permill::from_percent(30);
    pub RiskLowerBound: FixedI128 = FixedI128::saturating_from_rational(1, 2);
    pub RiskUpperBound: FixedI128 = FixedI128::saturating_from_integer(2);
    pub RiskNSigma: FixedI128 = FixedI128::saturating_from_integer(10);
    pub RiskRho: FixedI128 = FixedI128::saturating_from_rational(7, 10);
    pub Alpha: FixedI128 = FixedI128::from(15);
}

impl eq_rate::Config for Test {
    type AutoReinitToggleOrigin = EnsureRoot<AccountId>;
    type BailsmanManager = BailsmanManagerMock;
    type AuthorityId = sp_runtime::testing::UintAuthorityId;
    type Balance = u128;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type BalanceRemover = eq_balances::Pallet<Test>;
    type UnsignedPriority = UnsignedPriority;
    type MinSurplus = MinSurplus;
    type MinTempBailsman = MinTempBailsman;
    type UnixTime = ModuleTimestamp;
    type EqBuyout = EqBuyoutMock;
    type EqCurrency = eq_balances::Pallet<Test>;
    type BailsmanModuleId = BailsmanModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type MarginCallManager = MarginCallManagerMock<u64, u128>; // eq_margin_call::Pallet<Test>;
    type AssetGetter = eq_assets::Pallet<Test>;
    type WeightInfo = ();
    type PriceGetter = OracleMock;
    type Aggregates = AggregatesMock;
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
    type LendingPoolManager = ();
    type LendingAssetRemoval = ();
    type RedistributeWeightInfo = ();
}

impl authorship::Config for Test {
    type FindAuthor = ();
    type EventHandler = ();
}

parameter_types! {
    pub const MinimumPeriod: u64 = 1;
}

impl timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![(asset::EQ, FixedI64::one())]);
    let num_validators = 5;
    let validators = (0..num_validators)
        .map(|x| ((x + 1) * 10 + 1) as AccountId)
        .collect::<Vec<_>>();

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
    .assimilate_storage(&mut t)
    .unwrap();

    eq_session_manager::GenesisConfig::<Test> {
        validators: validators.clone(),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    eq_balances::GenesisConfig::<Test> {
        balances: vec![],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(true)),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_session::GenesisConfig::<Test> {
        keys: validators
            .iter()
            .map(|x| {
                (
                    *x,
                    *x,
                    SessionKeys {
                        eq_rate: UintAuthorityId(*x as u64),
                    },
                )
            })
            .collect(),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
