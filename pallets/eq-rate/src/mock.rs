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
use crate as eq_rate;
use asset::{Asset, AssetType};
use core::marker::PhantomData;
use eq_primitives::{
    asset,
    asset::{AssetData, DAI, EQ},
    balance::{DepositReason, EqCurrency, WithdrawReason, XcmDestination},
    balance_number::EqFixedU128,
    financial_storage::FinancialStorage,
    subaccount::{SubAccType, SubaccountsManager},
    OrderChange, SignedBalance, TransferReason,
};
use financial_pallet::{AssetMetrics, Duration, Financial, FinancialMetrics};
use financial_primitives::{CalcReturnType, CalcVolatilityType, OnPriceSet};
use frame_support::traits::{ExistenceRequirement, FindAuthor, LockIdentifier, WithdrawReasons};
use frame_support::{parameter_types, PalletId};
use frame_support::{traits::GenesisBuild, weights::Weight};
use sp_arithmetic::{FixedI128, FixedI64, FixedPointNumber, Permill};
use sp_core::H256;
use sp_runtime::{
    impl_opaque_keys,
    testing::{Header, TestXt, UintAuthorityId},
    traits::{BlakeTwo256, ConvertInto, IdentityLookup},
    DispatchResult, Perbill, Percent,
};
use sp_std::ops::Range;
use std::{cell::RefCell, collections::HashMap};
use substrate_fixed::types::I64F64;
use system::EnsureRoot;

pub(crate) type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
pub(crate) type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type BlockNumber = u64;

use core::convert::{TryFrom, TryInto};
use sp_runtime::traits::One;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        Authorship: authorship::{Pallet, Storage},
        System: system::{Pallet, Call, Event<T>},
        Balances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        EqBailsman: eq_bailsman::{Pallet, Call, Storage, Event<T>},
        EqRate: eq_rate::{Pallet, Storage, Call, ValidateUnsigned},
        Timestamp: timestamp::{Pallet, Call, Storage},
        EqSessionManager: eq_session_manager::{Pallet, Call, Storage, Event<T>},
        Session: pallet_session::{Pallet, Call, Storage, Event},
        EqMarginCall: eq_margin_call::{Pallet, Call, Storage, Event<T>},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event},
        EqAggregates: eq_aggregates::{Pallet, Call, Storage}
    }
);

impl_opaque_keys! {
    pub struct SessionKeys {
        pub eq_rate: ModuleRate,
    }
}

pub type Extrinsic = TestXt<RuntimeCall, ()>;

parameter_types! {
    pub const MinimumPeriod: u64 = 1;
    pub const EpochDuration: u64 = 3;
    pub const ExpectedBlockTime: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const MaxPricePoints: usize = 3;
    pub CriticalLtv: FixedI128 = FixedI128::saturating_from_rational(105, 100);
    pub const MinimalCollateral: Balance = 10 * 1_000_000_000; // todo change
    pub const TotalIssuance: Balance = 1_000_000_000;
    pub const ExistentialDeposit: Balance = 1;
    pub const ExistentialDepositBasic: Balance = 1;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
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

impl eq_balances::Config for Test {
    type ParachainId = eq_primitives::mocks::ParachainId;
    type ToggleTransferOrigin = EnsureRoot<AccountId>;
    type ForceXcmTransferOrigin = EnsureRoot<AccountId>;
    type AssetGetter = eq_assets::Pallet<Test>;
    type AccountStore = System;
    type Balance = Balance;
    type ExistentialDeposit = ExistentialDeposit;
    type ExistentialDepositBasic = ExistentialDepositBasic;
    type BalanceChecker = eq_bailsman::Pallet<Test>;
    type PriceGetter = OracleMock;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Aggregates = ModuleAggregates;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = eq_bailsman::Pallet<Test>;
    type UpdateTimeManager = Pallet<Test>;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type OrderAggregates = ();
    type UnixTime = EqRate;
}

impl eq_aggregates::Config for Test {
    type Balance = Balance;
    type BalanceGetter = ModuleBalances;
}

type DummyValidatorId = u64;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
}

pub const AUTHOR_11_ACCOUNT: DummyValidatorId = 11;
pub struct Author11;
impl FindAuthor<DummyValidatorId> for Author11 {
    fn find_author<'a, I>(_digests: I) -> Option<DummyValidatorId>
    where
        I: 'a + IntoIterator<Item = (frame_support::ConsensusEngineId, &'a [u8])>,
    {
        Some(AUTHOR_11_ACCOUNT)
    }
}

impl authorship::Config for Test {
    type FindAuthor = Author11;
    type EventHandler = ();
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = BlockNumber;
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

parameter_types! {
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
    pub const UnsignedPriority: u64 = 100;
    pub const MinSurplus: Balance = 1 * 1000_000_000; // 1 usd
    pub const MinTempBalanceUsd: Balance = 20 * 1000_000_000; // 20 usd
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
}

impl pallet_session::Config for Test {
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionManager = ();
    type SessionHandler = (ModuleRate,);
    type ValidatorId = u64;
    type ValidatorIdOf = ConvertInto;
    type Keys = SessionKeys;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}

thread_local! {
    pub static ACCOUNTS: RefCell<HashMap<AccountId, Vec<(Asset, SignedBalance<Balance>)>>> = RefCell::new(HashMap::new());
    pub static EQ_BUYOUT_ARGS: RefCell<Option<(AccountId, Balance)>> = RefCell::new(None);
}

impl timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

thread_local! {
    static FEE: RefCell<Balance> = RefCell::new(2 * 1000_000_000 ); // 2 usd
}

pub fn set_fee(fee: Balance) {
    FEE.with(|a| {
        *a.borrow_mut() = fee;
    });
}

impl Financial for Pallet<Test> {
    type Asset = Asset;
    type Price = I64F64;
    type AccountId = AccountId;
    fn calc_return(
        _return_type: CalcReturnType,
        _asset: Self::Asset,
    ) -> Result<Vec<Self::Price>, DispatchError> {
        Ok(vec![])
    }
    fn calc_vol(
        _return_type: CalcReturnType,
        _volatility_type: CalcVolatilityType,
        _asset: Self::Asset,
    ) -> Result<Self::Price, DispatchError> {
        Ok(I64F64::from_num(0))
    }
    fn calc_corr(
        _return_type: CalcReturnType,
        _correlation_type: CalcVolatilityType,
        _asset1: Self::Asset,
        _asset2: Self::Asset,
    ) -> Result<(Self::Price, Range<Duration>), DispatchError> {
        Ok((
            I64F64::from_num(0),
            core::time::Duration::new(5, 0).into()..core::time::Duration::new(5, 0).into(),
        ))
    }
    fn calc_portf_vol(
        _return_type: CalcReturnType,
        _vol_cor_type: CalcVolatilityType,
        _account_id: Self::AccountId,
    ) -> Result<Self::Price, DispatchError> {
        Ok(I64F64::from_num(0))
    }
    fn calc_portf_var(
        _return_type: CalcReturnType,
        _vol_cor_type: CalcVolatilityType,
        _account_id: Self::AccountId,
        _z_score: u32,
    ) -> Result<Self::Price, DispatchError> {
        Ok(I64F64::from_num(0))
    }
    fn calc_rv(
        _return_type: CalcReturnType,
        _ewma_length: u32,
        _asset: Self::Asset,
    ) -> Result<Self::Price, DispatchError> {
        Ok(I64F64::from_num(0))
    }
}

impl OnPriceSet for Pallet<Test> {
    type Asset = Asset;
    type Price = I64F64;
    fn on_price_set(_asset: Self::Asset, _price: Self::Price) -> Result<(), DispatchError> {
        Ok(())
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
        CriticalMargin::get()
    }
}

parameter_types! {
    pub const MaxBailsmenToDistribute: u32 = 1;
    pub const QueueLengthWeightConstant: u32 = 5;
}

impl eq_bailsman::Config for Test {
    type AssetGetter = eq_assets::Pallet<Test>;
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type EqCurrency = eq_balances::Pallet<Test>;
    type PriceGetter = OracleMock;
    type MinimalCollateral = MinimalCollateral;
    type MinTempBalanceUsd = MinTempBalanceUsd;
    type UnixTime = ModuleTimestamp;

    type PalletId = BailsmanModuleId;
    type Aggregates = ModuleAggregates;

    type WeightInfo = ();
    type MarginCallManager = MarginCallManagerMock;
    type SubaccountsManager = SubaccountsManagerMock;

    type AuthorityId = UintAuthorityId;
    type MaxBailsmenToDistribute = MaxBailsmenToDistribute;
    type UnsignedPriority = UnsignedPriority;
    type ValidatorOffchainBatcher = ();
    type QueueLengthWeightConstant = QueueLengthWeightConstant;
}

parameter_types! {
    pub const LendingModuleId: PalletId = PalletId(*b"eq/lendr");
    pub RiskLowerBound: FixedI128 = FixedI128::saturating_from_rational(1, 2);
    pub RiskUpperBound: FixedI128 = FixedI128::saturating_from_integer(2);
    pub RiskNSigma: FixedI128 = FixedI128::saturating_from_integer(10);
    pub RiskRho: FixedI128 = FixedI128::saturating_from_rational(7, 10);
    pub Alpha: FixedI128 = FixedI128::from(15);
    pub const TreasuryFee: Permill = Permill::from_percent(1);
    pub const WeightFeeTreasury: u32 = 80;
    pub const WeightFeeValidator: u32 = 20;
    pub BaseBailsmanFee: Permill = Permill::from_percent(1);
    pub BaseLenderFee: Permill =  Permill::from_rational(5u32, 1000u32);
    pub const LenderPart: Permill = Permill::from_percent(30);
}

impl Config for Test {
    type AutoReinitToggleOrigin = EnsureRoot<AccountId>;
    type AuthorityId = UintAuthorityId;
    type BailsmanManager = ModuleBailsman;
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type BalanceRemover = eq_balances::Pallet<Test>;
    type UnsignedPriority = UnsignedPriority;
    type MinSurplus = MinSurplus;
    type MinTempBailsman = MinTempBalanceUsd;
    type UnixTime = ModuleTimestamp;
    type EqBuyout = EqBuyoutMock;
    type EqCurrency = EqCurrencyMock;
    type BailsmanModuleId = BailsmanModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type MarginCallManager = eq_margin_call::Pallet<Test>;
    type AssetGetter = eq_assets::Pallet<Test>;
    type WeightInfo = ();
    type PriceGetter = OracleMock;
    type Aggregates = ModuleAggregates;
    type RiskLowerBound = RiskLowerBound;
    type RiskUpperBound = RiskUpperBound;
    type RiskNSigma = RiskNSigma;
    type Alpha = Alpha;
    type Financial = Pallet<Test>;
    type FinancialStorage = Pallet<Test>;
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

impl eq_session_manager::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorsManagementOrigin = EnsureRoot<AccountId>;
    type ValidatorId = DummyValidatorId;
    type RegistrationChecker = Session;
    type ValidatorIdOf = ();
    type WeightInfo = ();
}

impl FinancialStorage for Pallet<Test> {
    type Asset = Asset;
    type Price = I64F64;
    fn get_metrics() -> Option<FinancialMetrics<Self::Asset, Self::Price>> {
        None
    }
    fn get_per_asset_metrics(
        _asset: &Self::Asset,
    ) -> Option<AssetMetrics<Self::Asset, Self::Price>> {
        None
    }
}

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
    type OnNewAsset = ();
    type WeightInfo = ();
}

impl<LocalCall> system::offchain::SendTransactionTypes<LocalCall> for Test
where
    RuntimeCall: From<LocalCall>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = Extrinsic;
}

pub struct EqBuyoutMock;

pub fn clear_eq_buyout_args() {
    let _args = EQ_BUYOUT_ARGS.with(|args| {
        *args.borrow_mut() = None;
    });
}

pub fn get_eq_buyout_args() -> Option<(AccountId, Balance)> {
    EQ_BUYOUT_ARGS.with(|args| *args.borrow())
}

impl EqBuyout<AccountId, Balance> for EqBuyoutMock {
    fn eq_buyout(who: &AccountId, amount: Balance) -> sp_runtime::DispatchResult {
        let _args = EQ_BUYOUT_ARGS.with(|args| {
            *args.borrow_mut() = Some((who.clone(), amount.clone()));
        });
        Ok(())
    }

    fn is_enough(
        _currency: Asset,
        _amount: Balance,
        _amount_buyout: Balance,
    ) -> Result<bool, DispatchError> {
        Ok(true)
    }
}

frame_support::parameter_types! {
    pub const MaxLocks: u32 = 10;
}

pub struct EqCurrencyMock;

#[allow(unused_variables)]
impl EqCurrency<AccountId, Balance> for EqCurrencyMock {
    type Moment = BlockNumber;
    type MaxLocks = MaxLocks;

    fn can_be_deleted(who: &AccountId) -> Result<bool, sp_runtime::DispatchError> {
        // Only for specific ids true will return
        if *who == 987 || *who == 654 {
            return Ok(true);
        }

        Err(sp_runtime::DispatchError::Other("Cannot delete"))
    }

    fn total_balance(who: &AccountId, asset: Asset) -> Balance {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn debt(who: &AccountId, asset: Asset) -> Balance {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn currency_total_issuance(currency: Asset) -> Balance {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn minimum_balance_value() -> Balance {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn free_balance(who: &AccountId, asset: Asset) -> Balance {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn ensure_can_withdraw(
        who: &AccountId,
        asset: Asset,
        amount: Balance,
        _: WithdrawReasons,
        _new_balance: Balance,
    ) -> DispatchResult {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn currency_transfer(
        transactor: &AccountId,
        dest: &AccountId,
        asset: Asset,
        value: Balance,
        existence_requirement: ExistenceRequirement,
        transfer_reason: TransferReason,
        ensure_can_change: bool,
    ) -> DispatchResult {
        ModuleBalances::currency_transfer(
            transactor,
            dest,
            asset,
            value,
            existence_requirement,
            transfer_reason,
            ensure_can_change,
        )
    }

    fn deposit_into_existing(
        who: &AccountId,
        asset: Asset,
        value: Balance,
        _: Option<DepositReason>,
    ) -> Result<(), DispatchError> {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn deposit_creating(
        who: &AccountId,
        asset: Asset,
        value: Balance,
        ensure_can_change: bool,
        _: Option<DepositReason>,
    ) -> Result<(), DispatchError> {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn withdraw(
        who: &AccountId,
        asset: Asset,
        value: Balance,
        ensure_can_change: bool,
        reasons: Option<WithdrawReason>,
        _: WithdrawReasons,
        _liveness: ExistenceRequirement,
    ) -> Result<(), sp_runtime::DispatchError> {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn make_free_balance_be(who: &AccountId, asset: Asset, value: SignedBalance<Balance>) {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn delete_account(
        _account_id: &AccountId,
    ) -> std::result::Result<(), sp_runtime::DispatchError> {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn exchange(
        _accounts: (&AccountId, &AccountId),
        _assets: (&Asset, &Asset),
        _values: (Balance, Balance),
    ) -> Result<(), (DispatchError, Option<AccountId>)> {
        Ok(())
    }

    fn reserve(who: &AccountId, asset: Asset, amount: Balance) -> DispatchResult {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn unreserve(who: &AccountId, asset: Asset, amount: Balance) -> Balance {
        Balance::zero()
    }

    fn xcm_transfer(
        _from: &AccountId,
        _asset: Asset,
        _amount: Balance,
        _to: XcmDestination,
    ) -> DispatchResult {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn set_lock(_: LockIdentifier, _: &AccountId, _: Balance) {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn extend_lock(_: LockIdentifier, _: &AccountId, _: Balance) {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn remove_lock(_: LockIdentifier, _: &AccountId) {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn reserved_balance(who: &AccountId, asset: Asset) -> Balance {
        unimplemented!()
    }

    fn slash_reserved(
        who: &AccountId,
        asset: Asset,
        value: Balance,
    ) -> (eq_balances::NegativeImbalance<Balance>, Balance) {
        unimplemented!()
    }

    fn repatriate_reserved(
        slashed: &AccountId,
        beneficiary: &AccountId,
        asset: Asset,
        value: Balance,
        status: frame_support::traits::BalanceStatus,
    ) -> Result<Balance, DispatchError> {
        unimplemented!()
    }
}

parameter_types! {
    pub InitialMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 100);
    pub MaintenanceMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(25, 1000);
    pub CriticalMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 1000);
    pub MaintenancePeriod: u64 = 86_400u64;
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
        debt_weight: Permill::zero(),
        asset_xcm_data: eq_primitives::asset::AssetXcmData::None,
        taker_fee: Permill::from_rational(1u32, 1000u32),
        collateral_discount: Percent::one(),
        lending_debt_weight: Permill::one(),
    });
}

pub struct AssetGetterMock;
impl AssetGetterMock {
    pub fn set_asset_data(asset_data: AssetData<Asset>) {
        ASSET_DATA.with(|v| *v.borrow_mut() = asset_data);
    }
}
impl AssetGetter for AssetGetterMock {
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

    fn collateral_discount(_asset: &Asset) -> EqFixedU128 {
        EqFixedU128::one()
    }
}

impl eq_margin_call::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type UnixTime = ModuleTimestamp;
    type BailsmenManager = ModuleBailsman;
    type BalanceGetter = eq_balances::Pallet<Test>;
    type PriceGetter = OracleMock;
    type InitialMargin = InitialMargin;
    type MaintenanceMargin = MaintenanceMargin;
    type CriticalMargin = CriticalMargin;
    type MaintenancePeriod = MaintenancePeriod;
    type OrderAggregates = ();
    type AssetGetter = AssetGetterMock;
    type SubaccountsManager = SubaccountsManagerMock;
    type WeightInfo = ();
}

pub type ModuleRate = Pallet<Test>;
pub type ModuleSystem = system::Pallet<Test>;
pub type ModuleTimestamp = timestamp::Pallet<Test>;
pub type ModuleBailsman = eq_bailsman::Pallet<Test>;
pub type ModuleBalances = eq_balances::Pallet<Test>;
pub type ModuleAggregates = eq_aggregates::Pallet<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::BTC, FixedI64::saturating_from_integer(10000i32)),
        (asset::EOS, FixedI64::saturating_from_integer(3i32)),
        (asset::ETH, FixedI64::saturating_from_integer(250i32)),
        (asset::EQD, FixedI64::from(1)),
        (asset::CRV, FixedI64::from(1)),
        (asset::DOT, FixedI64::from(1)),
        (asset::EQ, FixedI64::from(1)),
    ]);
    let num_validators = 5;
    let validators = (0..num_validators)
        .map(|x| ((x + 1) * 10 + 1) as AccountId)
        .collect::<Vec<_>>();

    let mut t = system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    eq_session_manager::GenesisConfig::<Test> {
        validators: validators.clone(),
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

    eq_bailsman::GenesisConfig::<Test> { bailsmen: vec![] }
        .assimilate_storage(&mut t)
        .unwrap();

    t.into()
}
