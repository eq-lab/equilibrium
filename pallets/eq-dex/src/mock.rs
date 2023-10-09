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
use crate as eq_dex;
use crate::mock::sp_api_hidden_includes_construct_runtime::hidden_include::traits::GenesisBuild;
use eq_primitives::asset::AssetType;
use eq_primitives::mocks::TimeZeroDurationMock;
use eq_primitives::XcmMode;
use eq_primitives::{
    asset,
    asset::{Asset, AssetData, DAI, EQ},
    financial_storage::FinancialStorage,
    subaccount::{SubAccType, SubaccountsManager},
    Aggregates, BalanceChange, EqBuyout, MarginCallManager, MarginState, OrderChange,
    SignedBalance, TotalAggregates, UpdateTimeManager, UserGroup,
};
use eq_utils::ONE_TOKEN;
use financial_pallet::{AssetMetrics, Duration, Financial, FinancialMetrics};
use financial_primitives::{CalcReturnType, CalcVolatilityType};
use frame_support::{parameter_types, PalletId};
use frame_system::offchain::SendTransactionTypes;
use frame_system::EnsureRoot;
use sp_arithmetic::{FixedI128, FixedPointNumber, Permill};
use sp_runtime::Percent;
use sp_runtime::{
    impl_opaque_keys,
    testing::{Header, UintAuthorityId, H256},
    traits::{BlakeTwo256, IdentityLookup},
    DispatchError, Perbill,
};
use sp_std::ops::Range;
use std::cell::RefCell;
use std::marker::PhantomData;
use substrate_fixed::types::I64F64;

type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;
pub(crate) type ModuleTimestamp = pallet_timestamp::Pallet<Test>;
pub(crate) type ModuleSystem = frame_system::Pallet<Test>;
pub type ModuleRate = eq_rate::Pallet<Test>;
pub type ModuleDex = eq_dex::Pallet<Test>;
pub type ModuleBailsman = eq_bailsman::Pallet<Test>;

use core::convert::{TryFrom, TryInto};
use sp_runtime::traits::One;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage},
        EqDex: eq_dex::{Pallet, Call, Storage, Event<T>},
        EqRate: eq_rate::{Pallet, Storage, Call, ValidateUnsigned},
        Session: pallet_session::{Pallet, Call, Storage, Event},
        EqSessionManager: eq_session_manager::{Pallet, Call, Storage, Event<T>},
        Balances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        EqBailsman: eq_bailsman::{Pallet, Call, Storage, Event<T>},
        EqMarginCall: eq_margin_call::{Pallet, Call, Storage, Event<T>},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event, Config<T>}
    }
);

pub struct SubaccountsManagerMock;
impl SubaccountsManager<AccountId> for SubaccountsManagerMock {
    fn create_subaccount_inner(
        who: &AccountId,
        subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        let subaccount_id = who + 100;
        SUBACCOUNTS.with(|v| {
            let mut vec = v.borrow_mut();
            vec.push((*who, *subacc_type, subaccount_id));
        });

        Ok(subaccount_id)
    }
    fn delete_subaccount_inner(
        who: &AccountId,
        subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        SUBACCOUNTS.with(|v| {
            let mut vec = v.borrow_mut();
            let maybe_position = vec
                .iter()
                .position(|(acc_id, sub_type, _)| *acc_id == *who && *sub_type == *subacc_type);
            if let Some(pos) = maybe_position {
                vec.remove(pos);
            }
        });

        Ok(0)
    }
    fn has_subaccount(who: &AccountId, _subacc_type: &SubAccType) -> bool {
        match &who {
            0u64 => false,
            _ => true,
        }
    }
    fn get_subaccount_id(who: &AccountId, subacc_type: &SubAccType) -> Option<AccountId> {
        let mut subaccount_id: Option<AccountId> = None;
        SUBACCOUNTS.with(|v| {
            let maybe = v
                .borrow()
                .iter()
                .filter(|(ai, st, _)| *ai == *who && *st == *subacc_type)
                .map(|(_, _, id)| id.clone())
                .next();
            if let Some(id) = maybe {
                subaccount_id = Some(id);
            };
        });

        subaccount_id
    }
    fn is_subaccount(_who: &AccountId, _subacc_id: &AccountId) -> bool {
        false
    }
    fn get_owner_id(subaccount: &AccountId) -> Option<(AccountId, SubAccType)> {
        let mut result: Option<(AccountId, SubAccType)> = None;
        SUBACCOUNTS.with(|v| {
            let maybe = v
                .borrow()
                .iter()
                .filter(|(_, _, sai)| *sai == *subaccount)
                .map(|(ai, st, _)| (ai.clone(), *st))
                .next();
            result = maybe;
        });

        result
    }
    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        0
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
        debt_weight: Permill::zero(),
        asset_xcm_data: eq_primitives::asset::AssetXcmData::None,
        taker_fee: Permill::from_rational(1u32, 1000u32),
        collateral_discount: Percent::one(),
        lending_debt_weight: Permill::one(),
    });

    pub static ORDER_AGGREGATES: RefCell<VecMap<Asset, OrderAggregateBySide>> = Default::default();

    pub static SUBACCOUNTS: RefCell<Vec<(AccountId, SubAccType, AccountId)>> = RefCell::new(vec![
        (1, SubAccType::Trader, 101),
        (2, SubAccType::Trader, 102),
        (FAIL_ACC, SubAccType::Trader, FAIL_SUBACC),
    ]);

    pub static MARGIN_STATE: RefCell<Vec<(AccountId, (MarginState, bool))>> = Default::default();
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
        EqAssets::get_assets_data()
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

pub struct MarginCallManagerMock;

impl MarginCallManagerMock {
    pub(crate) fn set_margin_state(
        account_id: AccountId,
        margin_state: MarginState,
        is_margin_increased: bool,
    ) {
        MARGIN_STATE.with(|v| {
            v.borrow_mut()
                .push((account_id, (margin_state, is_margin_increased)));
        })
    }
}

impl MarginCallManager<AccountId, Balance> for MarginCallManagerMock {
    fn check_margin_with_change(
        owner: &AccountId,
        _balance_changes: &[BalanceChange<Balance>],
        _order_changes: &[OrderChange],
    ) -> Result<(MarginState, bool), DispatchError> {
        MARGIN_STATE.with(|v| {
            v.borrow()
                .iter()
                .find(|(acc_id, _)| *acc_id == *owner)
                .map(|(_, state)| Ok(*state))
                .unwrap_or(Ok((MarginState::Good, true)))
        })
    }

    fn try_margincall(owner: &AccountId) -> Result<MarginState, DispatchError> {
        Self::check_margin(owner)
    }

    fn get_critical_margin() -> EqFixedU128 {
        CriticalMargin::get()
    }
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
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

type DummyValidatorId = u64;

pub struct EqBuyoutMock;

impl EqBuyout<AccountId, Balance> for EqBuyoutMock {
    fn eq_buyout(who: &AccountId, amount: Balance) -> sp_runtime::DispatchResult {
        let native_asset = <eq_assets::Pallet<Test> as AssetGetter>::get_main_asset();
        <eq_balances::Pallet<Test> as EqCurrency<AccountId, Balance>>::currency_transfer(
            &TreasuryModuleId::get().into_account_truncating(),
            who,
            native_asset,
            amount,
            ExistenceRequirement::AllowDeath,
            eq_primitives::TransferReason::Common,
            false,
        )?;

        <eq_balances::Pallet<Test> as EqCurrency<AccountId, Balance>>::currency_transfer(
            who,
            &TreasuryModuleId::get().into_account_truncating(),
            asset::EOS,
            1_000_000_000,
            ExistenceRequirement::AllowDeath,
            eq_primitives::TransferReason::Common,
            false,
        )?;

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
    pub static USER_GROUPS: RefCell<Vec<(UserGroup, AccountId)>> = Default::default();
}

pub struct AggregatesMock;
impl Aggregates<AccountId, Balance> for AggregatesMock {
    fn in_usergroup(_account_id: &AccountId, _user_group: UserGroup) -> bool {
        true
    }
    fn set_usergroup(
        account_id: &AccountId,
        user_group: UserGroup,
        _is_in: bool,
    ) -> DispatchResult {
        USER_GROUPS.with(|v| {
            v.borrow_mut()
                .push((user_group.clone(), account_id.clone()));
        });

        Ok(())
    }

    fn update_total(
        _account_id: &AccountId,
        _asset: Asset,
        _prev_balance: &SignedBalance<Balance>,
        _delta_balance: &SignedBalance<Balance>,
    ) -> DispatchResult {
        Ok(())
    }

    fn iter_account(user_group: UserGroup) -> Box<dyn Iterator<Item = AccountId>> {
        Box::new(USER_GROUPS.with(|v| {
            let user_group = user_group.clone();
            v.borrow()
                .clone()
                .into_iter()
                .filter(move |(ug, _)| *ug == user_group)
                .map(|(_, id)| id)
                .into_iter()
        }))
    }
    fn iter_total(
        _user_group: UserGroup,
    ) -> Box<dyn Iterator<Item = (Asset, TotalAggregates<Balance>)>> {
        panic!("AggregatesMock not implemented");
    }
    fn get_total(_user_group: UserGroup, _asset: Asset) -> TotalAggregates<Balance> {
        TotalAggregates {
            collateral: 0,
            debt: 0,
        }
    }
}

pub struct OrderAggregatesMock;

impl OrderAggregatesMock {
    pub(crate) fn _set_order_aggregates(weights: Vec<(Asset, OrderAggregateBySide)>) {
        ORDER_AGGREGATES.with(|vec| {
            let weights_map: VecMap<_, _> = weights.iter().cloned().collect();
            *vec.borrow_mut() = weights_map;
        })
    }
}

impl eq_primitives::OrderAggregates<AccountId> for OrderAggregatesMock {
    fn get_asset_weights(_who: &AccountId) -> VecMap<Asset, OrderAggregateBySide> {
        ORDER_AGGREGATES.with(|v| v.borrow().clone())
    }
}

pub struct RateMock;

impl UpdateTimeManager<AccountId> for RateMock {
    fn set_last_update(_account_id: &AccountId) {}
    fn remove_last_update(_accounts_id: &AccountId) {}
    fn set_last_update_timestamp(_account_id: &AccountId, _timestamp_ms: u64) {}
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
            std::time::Duration::new(5, 0).into()..std::time::Duration::new(5, 0).into(),
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

parameter_types! {
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
    pub const BlockHashCount: u64 = 250;
    pub const MinimumPeriod: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const MinSurplus: Balance = 1 * 1000_000_000; // 1 usd
    pub const MinTempBailsman: Balance = 20 * 1000_000_000; // 20 usd
    pub const UnsignedPriority: u64 = 100;
    pub const DepositEq: Balance = 0;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const ExistentialDeposit: Balance = 1;
    pub CriticalLtv: FixedI128 = FixedI128::saturating_from_rational(105, 100);
    pub const MinimalCollateral: Balance = 10 * 1_000_000_000;
    pub const MinTempBalanceUsd: Balance = 0; // always reinit
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
}

thread_local! {
    static CURRENT_TIME: RefCell<u64> = RefCell::new(1598006981634);
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub eq_rate: ModuleRate,
    }
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

impl eq_session_manager::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorsManagementOrigin = EnsureRoot<AccountId>;
    type ValidatorId = DummyValidatorId;
    type RegistrationChecker = Session;
    type ValidatorIdOf = ();
    type WeightInfo = ();
}

impl pallet_session::Config for Test {
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionManager = ();
    type SessionHandler = (ModuleRate,);
    type ValidatorId = u64;
    type ValidatorIdOf = sp_runtime::traits::ConvertInto;
    type Keys = SessionKeys;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}

pub struct BalanceCheckerMock {}

pub const FAIL_ACC: AccountId = 666;
pub const FAIL_SUBACC: AccountId = 766;

impl eq_primitives::balance::BalanceChecker<Balance, AccountId, Balances, SubaccountsManagerMock>
    for BalanceCheckerMock
{
    fn can_change_balance_impl(
        who: &AccountId,
        change: &Vec<(asset::Asset, SignedBalance<Balance>)>,
        _: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        let all_positive = change.iter().all(|(_, sb)| match sb {
            SignedBalance::Positive(_) => true,
            SignedBalance::Negative(_) => false,
        });
        if all_positive {
            return Ok(());
        }
        match who {
            &FAIL_ACC | &FAIL_SUBACC => Err(DispatchError::Other("Expected error")),
            _ => Ok(()),
        }
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
    type BalanceChecker = BalanceCheckerMock;
    type PriceGetter = OracleMock;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Aggregates = AggregatesMock;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = ModuleBailsman;
    type UpdateTimeManager = RateMock;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type OrderAggregates = ();
    type UnixTime = TimeZeroDurationMock;
}

impl authorship::Config for Test {
    type FindAuthor = ();
    type EventHandler = ();
}

pub type ModuleBalances = eq_balances::Pallet<Test>;

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
    pub LenderPart: Permill = Permill::from_percent(30);
}

impl eq_rate::Config for Test {
    type AutoReinitToggleOrigin = EnsureRoot<AccountId>;
    type BailsmanManager = ModuleBailsman;
    type AuthorityId = sp_runtime::testing::UintAuthorityId;
    type Balance = Balance;
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
    type MarginCallManager = MarginCallManagerMock;
    type AssetGetter = AssetGetterMock;
    type WeightInfo = ();
    type PriceGetter = OracleMock;
    type Aggregates = AggregatesMock;
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
    type Aggregates = AggregatesMock;
    type WeightInfo = ();
    type MarginCallManager = MarginCallManagerMock;
    type SubaccountsManager = SubaccountsManagerMock;

    type AuthorityId = UintAuthorityId;
    type MaxBailsmenToDistribute = MaxBailsmenToDistribute;
    type UnsignedPriority = UnsignedPriority;
    type ValidatorOffchainBatcher = ();
    type QueueLengthWeightConstant = QueueLengthWeightConstant;
}

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
    type OnNewAsset = ();
    type WeightInfo = ();
}

parameter_types! {
    pub InitialMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 100);
    pub MaintenanceMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(25, 1000);
    pub CriticalMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 1000);
    pub MaintenancePeriod: u64 = 86_400u64;
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
    type OrderAggregates = OrderAggregatesMock;
    type AssetGetter = AssetGetterMock;
    type SubaccountsManager = SubaccountsManagerMock;
    type WeightInfo = ();
}

parameter_types! {
    pub const PriceStepCount: u32 = 5;
    pub const PenaltyFee: Balance = 5_000_000_000;
    pub const DexUnsignedPriority: u64 = 100;
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type DeleteOrderOrigin = EnsureRoot<AccountId>;
    type UpdateAssetCorridorOrigin = EnsureRoot<AccountId>;
    type PriceStepCount = PriceStepCount;
    type PenaltyFee = PenaltyFee;
    type DexUnsignedPriority = DexUnsignedPriority;
    type WeightInfo = ();
    type ValidatorOffchainBatcher = eq_rate::Pallet<Test>;
}

pub fn all_orders(asset: Asset, expected_side: OrderSide) -> Vec<Order<AccountId>> {
    ModuleDex::actual_price_chunks(asset)
        .into_iter()
        .flat_map(|chunk_key| ModuleDex::orders_by_asset_and_chunk_key(asset, chunk_key))
        .filter(|order| order.side == expected_side)
        .collect()
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::CRV, FixedI64::saturating_from_integer(10000)),
        (asset::BTC, FixedI64::saturating_from_integer(10000)),
        (asset::EOS, FixedI64::saturating_from_integer(3)),
        (asset::ETH, FixedI64::saturating_from_integer(250)),
        (asset::EQD, FixedI64::saturating_from_integer(1)),
    ]);

    let mut r = frame_system::GenesisConfig::default()
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
    .assimilate_storage(&mut r)
    .unwrap();

    eq_dex::GenesisConfig {
        chunk_corridors: vec![
            (asset::EQD, 5),
            (asset::BTC, 5),
            (asset::ETH, 200),
            (asset::DOT, 5),
            (asset::CRV, 5),
            (asset::EOS, 5),
            (asset::EQ, 5),
            (asset::GENS, 5),
            (asset::DAI, 5),
            (asset::USDT, 5),
            (asset::BUSD, 5),
            (asset::USDC, 5),
        ],
    }
    .assimilate_storage::<Test>(&mut r)
    .unwrap();

    let num_validators = 5;
    let validators = (0..num_validators)
        .map(|x| ((x + 1) * 10 + 1) as AccountId)
        .collect::<Vec<_>>();

    eq_session_manager::GenesisConfig::<Test> {
        validators: validators.clone(),
    }
    .assimilate_storage(&mut r)
    .unwrap();

    eq_balances::GenesisConfig::<Test> {
        balances: vec![(
            TreasuryModuleId::get().into_account_truncating(),
            vec![(100 * ONE_TOKEN, asset::EQD.get_id())],
        )],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(true)),
    }
    .assimilate_storage(&mut r)
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
    .assimilate_storage(&mut r)
    .unwrap();

    r.into()
}
