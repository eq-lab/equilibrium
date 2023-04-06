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

#![allow(dead_code)]
#![allow(unused_imports)]

use super::*;
use crate as eq_margin_call;
use core::fmt::Debug;
use core::marker::PhantomData;
use eq_primitives::{
    asset,
    asset::AssetType,
    balance::EqCurrency,
    balance_number::EqFixedU128,
    financial_storage::FinancialStorage,
    mocks::TimeZeroDurationMock,
    subaccount::{SubAccType, SubaccountsManager},
    Aggregates, EqBuyout, PriceGetter, SignedBalance, TotalAggregates, TransferReason,
    UpdateTimeManager, UserGroup,
};
use financial_pallet::{AssetMetrics, Duration, Financial, FinancialMetrics};
use financial_primitives::{CalcReturnType, CalcVolatilityType, OnPriceSet};
use frame_support::traits::{ExistenceRequirement, GenesisBuild, WithdrawReasons};
use frame_support::{parameter_types, traits::Everything, PalletId};
use frame_support::{sp_runtime::FixedU128, weights::Weight};
use sp_arithmetic::{FixedI128, FixedI64, FixedPointNumber, Permill};
use sp_core::H256;
use sp_runtime::{
    impl_opaque_keys,
    testing::{Header, TestXt, UintAuthorityId},
    traits::{BlakeTwo256, ConvertInto, IdentityLookup},
    DispatchError, DispatchResult, Perbill,
};
use sp_std::ops::Range;
use std::borrow::{Borrow, BorrowMut};
use std::{cell::RefCell, collections::HashMap};
use substrate_fixed::types::I64F64;
use system::EnsureRoot;

type AccountId = u64;
pub(crate) type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
pub type Balance = u128;

use core::convert::{TryFrom, TryInto};
use sp_runtime::traits::One;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: system::{Pallet, Call, Event<T>},
        Balances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        EqBailsman: eq_bailsman::{Pallet, Call, Storage, Event<T>},
        EqMarginCall: eq_margin_call::{Pallet, Storage, Call, Event<T>},
        EqAssets: eq_assets::{Pallet, Storage, Call, Event},
        Timestamp: timestamp::{Pallet, Call, Storage},
    }
);

impl eq_assets::Config for Test {
    type Event = Event;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
    type OnNewAsset = ();
    type WeightInfo = ();
}

/* -------------------- frame_system --------------------- */
impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
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

thread_local! {
    pub static ACCOUNTS: RefCell<HashMap<AccountId, Vec<(Asset, SignedBalance<Balance>)>>> = RefCell::new(HashMap::new());
    pub static EQ_BUYOUT_ARGS: RefCell<Option<(AccountId, Balance)>> = RefCell::new(None);
    pub static FEE: RefCell<Balance> = RefCell::new(2 * 1000_000_000 ); // 2 usd
    pub static ORDER_AGGREGATES: RefCell<VecMap<Asset, OrderAggregateBySide>> = Default::default();
}

pub struct OrderAggregatesMock;

impl OrderAggregatesMock {
    pub(crate) fn set_order_aggregates(weights: Vec<(Asset, OrderAggregateBySide)>) {
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

thread_local! {
    pub static ASSET_DATA: RefCell<AssetData<Asset>> = RefCell::new(AssetData {
        asset_type: AssetType::Physical,
        id: eq_primitives::asset::EQD,
        is_dex_enabled: true,
        price_step: FixedI64::from(1),
        buyout_priority: 1,
        lot: EqFixedU128::from(1),
        maker_fee: Permill::from_rational(5u32, 10_000u32),
        taker_fee: Permill::from_rational(1u32, 1_000u32),
        debt_weight: Permill::zero(),
        asset_xcm_data: eq_primitives::asset::AssetXcmData::None,
        collateral_discount: Percent::one(),
        lending_debt_weight: Permill::one(),
    });

    pub static COLLATERAL_DISCOUNT: RefCell<EqFixedU128> = RefCell::new(EqFixedU128::from(1));
}

/* ------------------ crate config -------------------*/
parameter_types! {
    pub InitialMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 100);
    pub MaintenanceMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(25, 1000);
    pub CriticalMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 1000);
    pub MaintenancePeriod: u64 = 86_400;
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/resrv");
}

impl Config for Test {
    type Event = Event;
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
    type AssetGetter = eq_assets::Pallet<Test>;
    type SubaccountsManager = SubaccountsManagerMock;
    type WeightInfo = ();
}

impl_opaque_keys! {
    pub struct SessionKeys {

    }
}

pub type Extrinsic = TestXt<Call, ()>;

impl<LocalCall> system::offchain::SendTransactionTypes<LocalCall> for Test
where
    Call: From<LocalCall>,
{
    type OverarchingCall = Call;
    type Extrinsic = Extrinsic;
}

/* ----------------------- eq_subaccounts -------------------- */

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
    type ExistentialDepositBasic = ExistentialDeposit;
    type BalanceChecker = eq_bailsman::Pallet<Test>;
    type PriceGetter = OracleMock;
    type Event = Event;
    type WeightInfo = ();
    type Aggregates = AggregatesMock;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = eq_bailsman::Pallet<Test>;
    type UpdateTimeManager = eq_margin_call::Pallet<Test>;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type LocationInverter = eq_primitives::mocks::LocationInverterMock;
    type OrderAggregates = ();
    type UnixTime = TimeZeroDurationMock;
}

// Some boilerplate for testing purposes,
// specifically `eq-balances` module requires `eq-rate` as an UpdateTimeManager module
// and we do not want to depend on eq-rate
impl<T: Config> UpdateTimeManager<T::AccountId> for Pallet<T> {
    fn set_last_update(_accounts: &T::AccountId) {
        //
    }

    fn remove_last_update(_accounts: &T::AccountId) {
        //
    }

    #[cfg(not(feature = "production"))]
    fn set_last_update_timestamp(_account_id: &T::AccountId, _timestamp_ms: u64) {
        //
    }
}

pub struct AggregatesMock;

impl Aggregates<u64, Balance> for AggregatesMock {
    fn in_usergroup(_account_id: &DummyValidatorId, user_group: UserGroup) -> bool {
        user_group != UserGroup::Bailsmen
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
    ) -> Box<dyn Iterator<Item = (Asset, TotalAggregates<Balance>)>> {
        panic!("AggregatesMock not implemented");
    }

    fn get_total(_user_group: UserGroup, _currency: Asset) -> TotalAggregates<Balance> {
        panic!("AggregatesMock not implemented");
    }
}

type DummyValidatorId = u64;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
}

/* -------------- timestamp ---------------------------------- */
impl timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

pub fn set_fee(fee: Balance) {
    FEE.with(|a| {
        *a.borrow_mut() = fee;
    });
}

// ---------------------- financial ------------------------------

impl Financial for Pallet<Test> {
    type Asset = Asset;
    type Price = I64F64;
    type AccountId = u64;
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

/* -------------- eq-bailsman --------------- */
parameter_types! {
    pub const MinimumPeriod: u64 = 1;
    pub const EpochDuration: u64 = 3;
    pub const ExpectedBlockTime: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const MaxPricePoints: usize = 3;
    pub CriticalLtv: FixedI128 = FixedI128::saturating_from_rational(105, 100);
    pub const MinimalCollateral: Balance = 10 * 1_000_000_000;
    pub const TotalIssuance: Balance = 1_000_000_000;
    pub const ExistentialDeposit: Balance = 1;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
}

parameter_types! {
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
    pub const UnsignedPriority: u64 = 100;
    pub const MinSurplus: u64 = 1 * 1000_000_000; // 1 usd
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const MinTempBalanceUsd: Balance = 0; // always reinit
    pub const MaxBailsmenToDistribute: u32 = 1;
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
    type UnixTime = ModuleTimestamp;
    type PalletId = BailsmanModuleId;
    type Aggregates = AggregatesMock;
    type WeightInfo = ();
    type MarginCallManager = Pallet<Test>;
    type SubaccountsManager = SubaccountsManagerMock;

    type AuthorityId = UintAuthorityId;
    type MaxBailsmenToDistribute = MaxBailsmenToDistribute;
    type UnsignedPriority = UnsignedPriority;
    type ValidatorOffchainBatcher = ();
    type QueueLengthWeightConstant = QueueLengthWeightConstant;
}

// -------------- eq-buyout -----------------------------------------

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
        _asset: Asset,
        _amount: Balance,
        _amount_buyout: Balance,
    ) -> Result<bool, DispatchError> {
        Ok(true)
    }
}

pub type ModuleMarginCall = eq_margin_call::Pallet<Test>;
pub type ModuleSystem = system::Pallet<Test>;
pub type ModuleTimestamp = timestamp::Pallet<Test>;
pub type ModuleBailsman = eq_bailsman::Pallet<Test>;
pub type ModuleBalances = eq_balances::Pallet<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::BTC, FixedI64::saturating_from_integer(10000)),
        (asset::EOS, FixedI64::saturating_from_integer(3)),
        (asset::ETH, FixedI64::saturating_from_integer(250)),
        (asset::EQD, FixedI64::saturating_from_integer(1)),
        (asset::EQ, FixedI64::saturating_from_integer(1)),
        (asset::DOT, FixedI64::saturating_from_integer(4)),
        (asset::USDC, FixedI64::saturating_from_integer(1)),
        (asset::USDT, FixedI64::saturating_from_integer(1)),
    ]);

    let num_validators = 5;
    let _validators = (0..num_validators)
        .map(|x| ((x + 1) * 10 + 1) as AccountId)
        .collect::<Vec<_>>();

    let mut t = system::GenesisConfig::default()
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
                EqFixedU128::saturating_from_rational(1, 100),
                FixedI64::one(),
                Permill::from_rational(1u32, 1000u32),
                Permill::from_rational(2u32, 1000u32),
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

    // eq_bailsman::GenesisConfig::<Test> { bailsmen: vec![] }
    //     .assimilate_storage(&mut t)
    //     .unwrap();

    t.into()
}
