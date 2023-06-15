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
use crate::mock::sp_api_hidden_includes_construct_runtime::hidden_include::traits::GenesisBuild;
use crate::{self as eq_xdot_pool};
use eq_primitives::mocks::TimeZeroDurationMock;
use eq_primitives::xdot_pool::XdotNumber;
use eq_primitives::{
    asset,
    asset::AssetType,
    asset::{Asset, OnNewAsset},
    balance::EqCurrency,
    balance_number::EqFixedU128,
    subaccount::{SubAccType, SubaccountsManager},
    xdot_pool::{XdotBalanceConvert, XdotFixedNumberConvert},
    Aggregates, BailsmanManager, BalanceChange, EqBuyout, MarginCallManager, MarginState,
    OrderChange, PriceGetter, SignedBalance, TransferReason, UserGroup,
};
use frame_support::traits::{ExistenceRequirement, WithdrawReasons};
use frame_support::weights::Weight;
use frame_support::{parameter_types, PalletId};
use frame_system::{self as system, EnsureRoot};
use sp_arithmetic::{FixedI64, Permill};
use sp_core::H256;
use sp_runtime::traits::One;
use sp_runtime::Percent;
use sp_runtime::{
    impl_opaque_keys,
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    DispatchError, DispatchResult, FixedI128, FixedPointNumber, Perbill,
};
use sp_std::convert::TryFrom;
use std::marker::PhantomData;
use substrate_fixed::types::I64F64;
use system::offchain::SendTransactionTypes;

pub struct XdotNumberPriceConvert;
impl Convert<XdotNumber, sp_runtime::FixedI64> for XdotNumberPriceConvert {
    fn convert(n: XdotNumber) -> sp_runtime::FixedI64 {
        eq_utils::fixed::i64f64_to_fixedi64(n)
    }
}

parameter_types! {
    pub const TreasuryPalletId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanPalletId: PalletId = PalletId(*b"eq/bails");
    pub const BalancesPalletId: PalletId = PalletId(*b"eq/balan");
    pub const XdotPalletId: PalletId = PalletId(*b"eq/xdotp");
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const MinimumPeriod: u64 = 1;
    pub const MinSurplus:u64 = 1 * 1000_000_000; // 1 usd
    pub const MinTempBailsman:u64 = 20 * 1000_000_000; // 20 usd
    pub const UnsignedPriority: u64 = 100;
    pub const BasicCurrencyGet: asset::Asset = asset::EQ;
    pub LpTokensDebtWeight: Permill = Permill::from_rational(2u32, 5u32);
    pub const LpTokenBuyoutPriority: u64 = u64::MAX;
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub eq_rate: EqRate,
    }
}

pub type AccountId = u64;
pub type Balance = eq_primitives::balance::Balance;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type DummyValidatorId = u64;
type AssetId = eq_primitives::asset::Asset;

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

impl timestamp::Config for Test {
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

impl pallet_session::Config for Test {
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionManager = ();
    type SessionHandler = (EqRate,);
    type ValidatorId = u64;
    type ValidatorIdOf = sp_runtime::traits::ConvertInto;
    type Keys = SessionKeys;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}

impl eq_session_manager::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorsManagementOrigin = EnsureRoot<AccountId>;
    type ValidatorId = DummyValidatorId;
    type RegistrationChecker = Session;
    type ValidatorIdOf = ();
    type WeightInfo = ();
}
pub struct MarginCallManagerMock;

impl MarginCallManager<AccountId, Balance> for MarginCallManagerMock {
    fn check_margin_with_change(
        _owner: &AccountId,
        _balance_changes: &[BalanceChange<Balance>],
        _order_changes: &[OrderChange],
    ) -> Result<(MarginState, bool), DispatchError> {
        Ok((MarginState::Good, false))
    }

    fn try_margincall(owner: &AccountId) -> Result<MarginState, DispatchError> {
        Self::check_margin(owner)
    }

    fn get_critical_margin() -> EqFixedU128 {
        Default::default()
    }
}

impl authorship::Config for Test {
    type FindAuthor = ();

    type EventHandler = ();
}

test_utils::implement_financial!();

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
    type BailsmanManager = BailsmanManagerMock;
    type AutoReinitToggleOrigin = EnsureRoot<AccountId>;
    type AuthorityId = sp_runtime::testing::UintAuthorityId;
    type Balance = Balance;
    type BalanceGetter = Balances;
    type BalanceRemover = Balances;
    type UnsignedPriority = UnsignedPriority;
    type MinSurplus = MinSurplus;
    type MinTempBailsman = MinTempBailsman;
    type UnixTime = Timestamp;
    type EqBuyout = EqBuyoutMock;
    type EqCurrency = Balances;
    type SubaccountsManager = SubaccountsManagerMock;
    type MarginCallManager = MarginCallManagerMock;
    type AssetGetter = EqAssets;
    type WeightInfo = ();
    type BailsmanModuleId = BailsmanPalletId;
    type PriceGetter = OracleMock;
    type Aggregates = EqAggregates;
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
    type TreasuryModuleId = TreasuryPalletId;
    type LendingModuleId = LendingModuleId;
    type LendingPoolManager = ();
    type LendingAssetRemoval = ();
    type RedistributeWeightInfo = ();
}

pub struct SubaccountsManagerMock;
impl SubaccountsManager<u64> for SubaccountsManagerMock {
    fn create_subaccount_inner(
        _who: &u64,
        _subacc_type: &SubAccType,
    ) -> Result<u64, DispatchError> {
        Ok(9999_u64)
    }
    fn delete_subaccount_inner(
        _who: &u64,
        _subacc_type: &SubAccType,
    ) -> Result<u64, DispatchError> {
        Ok(9999_u64)
    }
    fn has_subaccount(_who: &u64, _subacc_type: &SubAccType) -> bool {
        true
    }
    fn get_subaccount_id(_who: &u64, _subacc_type: &SubAccType) -> Option<u64> {
        Some(9999_u64)
    }
    fn is_subaccount(_who: &u64, _subacc_id: &u64) -> bool {
        false
    }
    fn get_owner_id(_subaccount: &u64) -> Option<(u64, SubAccType)> {
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

pub struct OracleMock;
impl PriceGetter for OracleMock {
    fn get_price<FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>>(
        _currency: &asset::Asset,
    ) -> Result<FixedNumber, sp_runtime::DispatchError> {
        Ok(FixedNumber::one())
    }
}

impl eq_aggregates::Config for Test {
    type Balance = Balance;
    type BalanceGetter = Balances;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
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
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Aggregates = eq_aggregates::Pallet<Test>;
    type TreasuryModuleId = TreasuryPalletId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmanManagerMock;
    type UpdateTimeManager = EqRate;
    type BailsmanModuleId = BailsmanPalletId;
    type ModuleId = BalancesPalletId;
    type OrderAggregates = ();
    type XcmRouter = ();
    type XcmToFee = ();
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type UnixTime = TimeZeroDurationMock;
}

pub struct OnNewAssetMock;

impl OnNewAsset for OnNewAssetMock {
    fn on_new_asset(_asset: Asset, _prices: Vec<FixedI64>) {}
}

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = BasicCurrencyGet;
    type WeightInfo = ();
    type OnNewAsset = OnNewAssetMock;
}

pub struct Assets;

pub const LP_TOKEN: u64 = 1952805748; // b"test"

impl crate::traits::Assets<AssetId, Balance, AccountId> for Assets {
    fn create_lp_asset(pool_id: crate::PoolId) -> Result<AssetId, DispatchError> {
        let asset = Asset(LP_TOKEN + pool_id as u64);

        EqAssets::do_add_asset(
            asset,
            EqFixedU128::from(0),
            FixedI64::from(0),
            Permill::zero(),
            Permill::zero(),
            asset::AssetXcmData::None,
            LpTokensDebtWeight::get(),
            LpTokenBuyoutPriority::get(),
            eq_primitives::asset::AssetType::Lp(eq_primitives::asset::AmmPool::Yield(pool_id)),
            false,
            Percent::zero(),
            Permill::one(),
            vec![],
        )
        .map_err(|e| e.error)?;

        Ok(asset)
    }

    fn mint(asset: AssetId, dest: &AccountId, amount: Balance) -> DispatchResult {
        Balances::deposit_creating(dest, asset, amount, true, None)
    }

    fn burn(asset: AssetId, dest: &AccountId, amount: Balance) -> DispatchResult {
        Balances::withdraw(
            dest,
            asset,
            amount,
            true,
            None,
            WithdrawReasons::empty(),
            ExistenceRequirement::AllowDeath,
        )
    }

    fn transfer(
        asset: AssetId,
        source: &AccountId,
        dest: &AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Balances::currency_transfer(
            source,
            dest,
            asset,
            amount,
            ExistenceRequirement::AllowDeath,
            TransferReason::Common,
            true,
        )
        .into()
    }

    fn balance(asset: AssetId, who: &AccountId) -> Balance {
        Balances::free_balance(who, asset)
    }

    fn total_issuance(asset: AssetId) -> Balance {
        EqAggregates::get_total(UserGroup::Balances, asset).collateral
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type PoolsManagementOrigin = EnsureRoot<AccountId>;
    type WeightInfo = ();
    type FixedNumberBits = i128;
    type XdotNumber = I64F64;
    type NumberConvert = yield_math::YieldConvert;
    type BalanceConvert = XdotBalanceConvert;
    type AssetId = AssetId;
    type Assets = Assets;
    type YieldMath = yield_math::YieldMath<I64F64, yield_math::YieldConvert>;
    type PriceNumber = FixedI64;
    type PriceConvert = XdotNumberPriceConvert;
    type FixedNumberConvert = XdotFixedNumberConvert;
    type AssetChecker = ();
    type OnPoolInitialized = ();
}

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: system::{Pallet, Call, Event<T>},
        EqRate: eq_rate::{Pallet, Storage, Call, ValidateUnsigned},
        Session: pallet_session::{Pallet, Call, Storage, Event},
        EqSessionManager: eq_session_manager::{Pallet, Call, Storage, Event<T>},
        Balances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        EqAggregates: eq_aggregates::{Pallet, Call, Storage},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event},
        Timestamp: timestamp::{Pallet, Call, Storage},
        Xdot: eq_xdot_pool::{Pallet, Call, Storage, Event<T>},
    }
);

pub const XDOT: Asset = Asset(2019848052);

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    eq_assets::GenesisConfig::<Test> {
        _runtime: PhantomData,
        assets: // id, lot, price_step, maker_fee, taker_fee, debt_weight, buyout_priority
        vec![
            (
                asset::EQD.get_id(),
                EqFixedU128::zero(),
                FixedI64::zero(),
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
                EqFixedU128::zero(),
                FixedI64::zero(),
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
                EqFixedU128::zero(),
                FixedI64::zero(),
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
                EqFixedU128::zero(),
                FixedI64::zero(),
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
                EqFixedU128::zero(),
                FixedI64::zero(),
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
                EqFixedU128::zero(),
                FixedI64::zero(),
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
                EqFixedU128::zero(),
                FixedI64::zero(),
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
                XDOT.get_id(),
                EqFixedU128::zero(),
                FixedI64::zero(),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                u64::MAX,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),

        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    eq_balances::GenesisConfig::<Test> {
        balances: vec![],
        is_transfers_enabled: true,
        is_xcm_enabled: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
