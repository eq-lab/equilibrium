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

use crate as eq_balances;
use eq_primitives::{
    asset, asset::AssetType, balance_number::EqFixedU128, AccountDistribution, Aggregates,
    TotalAggregates, UserGroup,
};
use frame_support::{
    pallet_prelude::DispatchResult,
    parameter_types,
    traits::{GenesisBuild, OnUnbalanced},
    weights::Weight,
    PalletId,
};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
    generic::Header,
    traits::{BlakeTwo256, IdentityLookup},
    FixedI64, FixedPointNumber, Perbill, Percent,
};
use std::cell::RefCell;
use std::marker::PhantomData;

pub(crate) type Balance = eq_primitives::balance::Balance;

parameter_types! {
    pub const MinimumPeriod: u64 = 1;
    pub const EpochDuration: u64 = 3;
    pub const ExpectedBlockTime: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const TotalIssuance: Balance = 1_000_000_000;
    pub const ExistentialDeposit: Balance = 20;
    pub const ExistentialDepositBasic: Balance = 15;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/resrv");
}

thread_local! {
    pub static EQ_DEPOSIT_BUY_ARGS: RefCell<Option<(AccountId, Balance)>> = RefCell::new(None);
}

type AccountId = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;

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
        EqAssets: eq_assets::{Pallet, Call, Storage, Event}
    }
);

pub fn clear_eq_buyout_args() {
    let _args = EQ_DEPOSIT_BUY_ARGS.with(|args| {
        *args.borrow_mut() = None;
    });
}

pub fn get_eq_buyout_args() -> Option<(AccountId, Balance)> {
    EQ_DEPOSIT_BUY_ARGS.with(|args| *args.borrow())
}

type DummyValidatorId = u64;

parameter_types! {
    pub const BlockHashCount: u32 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u32;
    type BlockNumber = u32;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
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
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub struct BalanceCheckerMock {}

pub const FAIL_ACC: u64 = 666;

impl BalanceChecker<Balance, u64, EqBalances, SubaccountsManagerMock> for BalanceCheckerMock {
    fn need_to_check_impl(
        _who: &AccountId,
        _changes: &Vec<(Asset, SignedBalance<Balance>)>,
    ) -> bool {
        true
    }
    fn can_change_balance_impl(
        who: &u64,
        _change: &Vec<(asset::Asset, SignedBalance<Balance>)>,
        _: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        match who {
            &FAIL_ACC => Err(DispatchError::Other("Expected error")),
            _ => Ok(()),
        }
    }
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
    ) -> Result<AccountDistribution<Balance>, sp_runtime::DispatchError> {
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

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type OnNewAsset = ();
    type MainAsset = MainAsset;
    type WeightInfo = ();
}

thread_local! {
    pub static UNIX_NOW: RefCell<u64> = RefCell::new(0);
}

pub struct TimeMock;

impl TimeMock {
    #[allow(dead_code)]
    pub fn set_secs(secs: u64) {
        UNIX_NOW.with(|now| *now.borrow_mut() = secs)
    }
}

impl UnixTime for TimeMock {
    fn now() -> core::time::Duration {
        UNIX_NOW.with(|now| core::time::Duration::from_secs(*now.borrow()))
    }
}

impl Config for Test {
    type ToggleTransferOrigin = EnsureRoot<AccountId>;
    type ForceXcmTransferOrigin = EnsureRoot<AccountId>;
    type AssetGetter = eq_assets::Pallet<Test>;
    type Balance = Balance;
    type ExistentialDeposit = ExistentialDeposit;
    type ExistentialDepositBasic = ExistentialDepositBasic;

    type BalanceChecker = (
        BalanceCheckerMock,
        locked_balance_checker::CheckLocked<Test>,
    );
    type PriceGetter = OracleMock;
    type RuntimeEvent = RuntimeEvent;
    type Aggregates = AggregatesMock;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmanManagerMock;
    type UpdateTimeManager = RateMock;
    type BailsmanModuleId = BailsmanModuleId;
    type WeightInfo = ();
    type ModuleId = BalancesModuleId;
    type XcmRouter = ();
    type XcmToFee = ();
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type OrderAggregates = ();
    type AccountStore = System;
    type UnixTime = TimeMock;
    type ParachainId = eq_primitives::mocks::ParachainId;
}

thread_local! {
    pub static SLASH: RefCell<Balance> = RefCell::new(0);
}

pub struct SlashMock;

impl SlashMock {
    pub fn balance() -> Balance {
        SLASH.with(|slash| *slash.borrow())
    }
}

impl OnUnbalanced<NegativeImbalance<Balance>> for SlashMock {
    fn on_nonzero_unbalanced(amount: NegativeImbalance<Balance>) {
        SLASH.with(|slash| *slash.borrow_mut() += amount.peek());
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
        _currency: asset::Asset,
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
    ) -> Box<dyn Iterator<Item = (asset::Asset, TotalAggregates<u128>)>> {
        panic!("AggregatesMock not implemented");
    }
    fn get_total(_user_group: UserGroup, _currency: asset::Asset) -> TotalAggregates<u128> {
        TotalAggregates {
            collateral: 1000,
            debt: 10,
        }
    }
}

pub struct RateMock;

impl UpdateTimeManager<u64> for RateMock {
    fn set_last_update(_account_id: &u64) {}
    fn remove_last_update(_accounts_id: &u64) {}
    fn set_last_update_timestamp(_account_id: &u64, _timestamp_ms: u64) {}
}

thread_local! {
    pub static CAN_BE_DELETED: RefCell<bool> = RefCell::new(false);
}

pub type ModuleBalances = Pallet<Test>;

pub const XDOT1: Asset = Asset(2002941027);
pub const XDOT2: Asset = Asset(6450786);

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::BTC, FixedI64::saturating_from_integer(10)),
        (asset::EOS, FixedI64::saturating_from_integer(10)),
        (asset::ETH, FixedI64::saturating_from_integer(10)),
        (asset::EQD, FixedI64::saturating_from_integer(10)),
        (asset::EQ, FixedI64::saturating_from_integer(10)),
        (asset::DOT, FixedI64::saturating_from_integer(10)),
        (asset::CRV, FixedI64::saturating_from_integer(10)),
        (XDOT1, FixedI64::saturating_from_integer(1)),
        (XDOT2, FixedI64::saturating_from_integer(1)),
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
                XDOT1.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                10,
                AssetType::Physical,
                true,
                Percent::zero(),
                Permill::one(),
            ),
            (
                XDOT2.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                11,
                AssetType::Physical,
                true,
                Percent::from_rational(5u32, 10u32),
                Permill::one(),
            )
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    eq_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, vec![(1000_000_000_000 as u128, asset::BTC.get_id())]),
            (2, vec![(2000_000_000_000 as u128, asset::BTC.get_id())]),
            (10, vec![(10_000_000_000 as u128, asset::EQD.get_id())]),
            (20, vec![(20_000_000_000 as u128, asset::EQD.get_id())]),
            (30, vec![(30_000_000_000 as u128, asset::EQD.get_id())]),
            (15, vec![(15 as u128, asset::EQ.get_id())]),
            (16, vec![(13 as u128, asset::EQ.get_id())]),
            (17, vec![(20 as u128, asset::EQ.get_id())]),
        ],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(true)),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    // pub balances: Vec<(T::AccountId, Vec<(T::Balance, u64)>)>,
    t.into()
}
