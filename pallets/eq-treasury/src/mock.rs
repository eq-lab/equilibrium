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
use crate as eq_treasury;
use core::marker::PhantomData;
use eq_primitives::{
    asset,
    asset::Asset,
    asset::AssetType,
    subaccount::{SubAccType, SubaccountsManager},
    BailsmanManager, UpdateTimeManager,
};
use frame_support::{
    parameter_types,
    traits::{FindAuthor, WithdrawReasons},
    weights::Weight,
};
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    DispatchError, FixedI64, Perbill, Percent,
};
use std::cell::RefCell;
use system::EnsureRoot;

pub(crate) type AccountId = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type Balance = eq_primitives::balance::Balance;

pub(crate) type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;

use core::convert::{TryFrom, TryInto};

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Authorship: authorship::{Pallet, Storage},
        EqBalances: eq_balances::{Pallet, Call, Storage, Event<T>},
        EqAggregates: eq_aggregates::{Pallet, Call, Storage},
        EqTreasury: eq_treasury::{Pallet, Call, Storage, Event<T>},
        EqAssets: eq_assets::{Pallet, Storage, Call, Event},
    }
);

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
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
    pub const BuyFee:Permill = Permill::from_percent(2);
    pub const SellFee:Permill = Permill::from_percent(10);
    pub const MaxPricePoints: usize = 3;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
    pub const ExistentialDeposit: Balance = 1;
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
}

pub type DummyValidatorId = u64;

impl system::Config for Test {
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

thread_local! {
    static CURRENT_TIME: RefCell<u64> = RefCell::new(1598006981634);
}

pub struct SubaccountsManagerMock;

impl SubaccountsManager<AccountId> for SubaccountsManagerMock {
    fn create_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        Ok(9999)
    }

    fn delete_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        Ok(9999)
    }

    fn has_subaccount(_who: &AccountId, _subacc_type: &SubAccType) -> bool {
        true
    }

    fn get_subaccount_id(_who: &AccountId, _subacc_type: &SubAccType) -> Option<AccountId> {
        Some(9999)
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

    fn redistribute(_who: &AccountId) -> std::result::Result<u32, sp_runtime::DispatchError> {
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
    type Aggregates = eq_aggregates::Pallet<Test>;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmanManagerMock;
    type UpdateTimeManager = RateMock;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type OrderAggregates = ();
    type UnixTime = TimeMock;
}

pub struct RateMock;
impl UpdateTimeManager<AccountId> for RateMock {
    fn set_last_update(_account_id: &AccountId) {}
    fn remove_last_update(_accounts_id: &AccountId) {}
    fn set_last_update_timestamp(_account_id: &AccountId, _timestamp_ms: u64) {}
}

impl eq_aggregates::Config for Test {
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Test>;
}

pub struct BalanceCheckerMock {}

impl
    eq_primitives::balance::BalanceChecker<
        Balance,
        DummyValidatorId,
        EqBalances,
        SubaccountsManagerMock,
    > for BalanceCheckerMock
{
    fn can_change_balance_impl(
        who: &DummyValidatorId,
        changes: &Vec<(Asset, eq_primitives::SignedBalance<Balance>)>,
        _: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        let res = changes.iter().all(|(asset, change)| match change {
            eq_primitives::SignedBalance::Positive(_) => true,
            eq_primitives::SignedBalance::Negative(change_value) => {
                let balance =
                    <ModuleBalances as BalanceGetter<DummyValidatorId, Balance>>::get_balance(
                        who, asset,
                    );
                match balance {
                    eq_primitives::SignedBalance::Negative(_) => false,
                    eq_primitives::SignedBalance::Positive(balance_value) => {
                        balance_value >= *change_value
                    }
                }
            }
        });
        res.then(|| ())
            .ok_or_else(|| DispatchError::Other("Can change balance mock"))
    }
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

pub struct TimeMock {}

impl UnixTime for TimeMock {
    fn now() -> core::time::Duration {
        //core::time::Duration::from_millis(CURRENT_TIME.with(|v| *v.borrow()))
        core::time::Duration::from_millis(1598006981634)
    }
}

parameter_types! {
    pub const MinAmountToBuyout: Balance = 100 * eq_utils::ONE_TOKEN;
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetGetter = eq_assets::Pallet<Test>;
    type Balance = Balance;
    type BuyFee = BuyFee;
    type SellFee = SellFee;
    type PalletId = TreasuryModuleId;
    type PriceGetter = OracleMock;
    type EqCurrency = ModuleBalances;
    type BalanceGetter = ModuleBalances;
    type UnixTime = TimeMock;
    type WeightInfo = ();
    type MinAmountToBuyout = MinAmountToBuyout;
}

pub type ModuleTreasury = Pallet<Test>;
pub type ModuleBalances = eq_balances::Pallet<Test>;
pub type ModuleAggregates = eq_aggregates::Pallet<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::BTC, FixedI64::saturating_from_integer(10000)),
        (asset::EOS, FixedI64::saturating_from_integer(3)),
        (asset::ETH, FixedI64::saturating_from_integer(250)),
        (asset::EQD, FixedI64::saturating_from_integer(1)),
        (asset::EQ, FixedI64::saturating_from_integer(1)),
        (asset::DOT, FixedI64::saturating_from_integer(17)),
    ]);

    let mut r = system::GenesisConfig::default()
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
    r.into()
}
