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
use crate as eq_distribution;
use eq_primitives::asset::{self, Asset, AssetGetter};
use eq_primitives::balance::{DepositReason, EqCurrency, WithdrawReason};
use eq_primitives::mocks::VestingAccountMock;
use frame_support::parameter_types;
use frame_support::traits::{EitherOfDiverse, LockIdentifier, WithdrawReasons};
use frame_support::weights::Weight;
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::ModuleError;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};
use sp_runtime::{DispatchError, DispatchResult};
use std::{cell::RefCell, convert::TryFrom};

parameter_types! {
    pub const ExistentialDeposit: Balance = 1;
    pub const DistributionModuleId: PalletId = PalletId(*b"eq/distr");
    pub const VestingModuleId: PalletId = PalletId(*b"eq/vestn");
}

type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type BlockNumber = u64;

use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::TransferReason;
use sp_runtime::traits::One;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        EqDistribution: eq_distribution::{Pallet, Call, Storage},
        EqMembers: pallet_collective::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>},
    }
);

pub const AMOUNT: Balance = 100;
pub const ACC_ID: AccountId = 1;

thread_local! {
    pub static ADDED_VESTING: RefCell<Option<(DummyValidatorId, (Balance, Balance, u64))>> = RefCell::new(
        Option::None
    );
    pub static TRANSFER: RefCell<Option<((Asset, DummyValidatorId, DummyValidatorId, Balance), ExistenceRequirement)>> = RefCell::new(
        Option::None
    );
    pub static VESTED_TRANSFER: RefCell<Option<((DummyValidatorId, DummyValidatorId, u64), ExistenceRequirement)>> = RefCell::new(
        Option::None
    );
    pub static VESTING_EXISTS: RefCell<bool> = RefCell::new(false);
    pub static CAN_TRANSFER: RefCell<bool> = RefCell::new(true);
}

type DummyValidatorId = u64;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
    pub const BasicCurrencyGet: eq_primitives::asset::Asset = eq_primitives::asset::GENS;
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = BlockNumber;
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

// #[derive(Default)]
// pub struct PositiveImbalanceMock;

// impl TryDrop for PositiveImbalanceMock {
//     fn try_drop(self) -> Result<(), Self> {
//         panic!("Not used");
//     }
// }

// impl Imbalance<u64> for PositiveImbalanceMock {
//     type Opposite = NegativeImbalanceMock;
//     fn zero() -> Self {
//         panic!("Not used");
//     }

//     fn drop_zero(self) -> Result<(), Self> {
//         panic!("Not used");
//     }

//     fn split(self, _amount: u64) -> (Self, Self) {
//         panic!("Not used");
//     }

//     fn merge(self, _other: Self) -> Self {
//         panic!("Not used");
//     }

//     fn subsume(&mut self, _other: Self) {
//         panic!("Not used");
//     }

//     fn offset(self, _other: Self::Opposite) -> Result<Self, Self::Opposite> {
//         panic!("Not used");
//     }

//     fn peek(&self) -> u64 {
//         panic!("Not used");
//     }
// }

// #[derive(Default)]
// pub struct NegativeImbalanceMock;

// impl TryDrop for NegativeImbalanceMock {
//     fn try_drop(self) -> Result<(), Self> {
//         panic!("Not used");
//     }
// }

// impl Imbalance<u64> for NegativeImbalanceMock {
//     type Opposite = PositiveImbalanceMock;

//     fn zero() -> Self {
//         panic!("Not used");
//     }

//     fn drop_zero(self) -> Result<(), Self> {
//         panic!("Not used");
//     }

//     fn split(self, _amount: u64) -> (Self, Self) {
//         panic!("Not used");
//     }

//     fn merge(self, _other: Self) -> Self {
//         panic!("Not used");
//     }

//     fn subsume(&mut self, _other: Self) {
//         panic!("Not used");
//     }

//     fn offset(self, _other: Self::Opposite) -> Result<Self, Self::Opposite> {
//         panic!("Not used");
//     }

//     fn peek(&self) -> u64 {
//         panic!("Not used");
//     }
// }

frame_support::parameter_types! {
    pub const MaxLocks: u32 = 10;
}
pub struct EqCurrencyMock;
impl EqCurrency<DummyValidatorId, Balance> for EqCurrencyMock {
    type Moment = BlockNumber;
    type MaxLocks = MaxLocks;

    fn total_balance(_who: &AccountId, _asset: Asset) -> Balance {
        unimplemented!()
    }

    fn debt(
        _: &DummyValidatorId,
        _: Asset,
    ) -> <CurrencyMock as Currency<DummyValidatorId>>::Balance {
        unimplemented!()
    }

    fn currency_total_issuance(
        _: asset::Asset,
    ) -> <CurrencyMock as Currency<DummyValidatorId>>::Balance {
        unimplemented!()
    }

    fn minimum_balance_value() -> <CurrencyMock as Currency<DummyValidatorId>>::Balance {
        unimplemented!()
    }

    fn free_balance(
        _: &DummyValidatorId,
        _: asset::Asset,
    ) -> <CurrencyMock as Currency<DummyValidatorId>>::Balance {
        unimplemented!()
    }

    fn ensure_can_withdraw(
        _: &DummyValidatorId,
        _: asset::Asset,
        _: <CurrencyMock as Currency<DummyValidatorId>>::Balance,
        _: WithdrawReasons,
        _: <CurrencyMock as Currency<DummyValidatorId>>::Balance,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn currency_transfer(
        transactor: &DummyValidatorId,
        dest: &DummyValidatorId,
        asset: Asset,
        value: <CurrencyMock as Currency<DummyValidatorId>>::Balance,
        existence_requirement: ExistenceRequirement,
        transfer_reason: TransferReason,
        ensure_can_change: bool,
    ) -> DispatchResult {
        let result = CAN_TRANSFER.with(|v| v.borrow().clone())
            && transfer_reason == eq_primitives::TransferReason::Common
            && ensure_can_change;
        if !result {
            Err(DispatchError::Module(ModuleError {
                index: 0,
                error: *b"zero",
                message: Option::None,
            }))
        } else if !AssetGetterMock::get_assets().contains(&asset) {
            Err(DispatchError::Module(ModuleError {
                index: 0,
                error: *b"one1",
                message: Option::None,
            }))
        } else {
            TRANSFER.with(|v| {
                *v.borrow_mut() =
                    Option::Some(((asset, *transactor, *dest, value), existence_requirement))
            });
            Ok(())
        }
    }

    fn deposit_into_existing(
        _: &DummyValidatorId,
        _: Asset,
        _: <CurrencyMock as Currency<DummyValidatorId>>::Balance,
        _: Option<DepositReason>,
    ) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn deposit_creating(
        _: &DummyValidatorId,
        _: Asset,
        _: <CurrencyMock as Currency<DummyValidatorId>>::Balance,
        _: bool,
        _: Option<DepositReason>,
    ) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn withdraw(
        _: &DummyValidatorId,
        _: Asset,
        _: <CurrencyMock as Currency<DummyValidatorId>>::Balance,
        _: bool,
        _: Option<WithdrawReason>,
        _: WithdrawReasons,
        _: ExistenceRequirement,
    ) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn make_free_balance_be(
        _: &DummyValidatorId,
        _: Asset,
        _: eq_primitives::SignedBalance<<CurrencyMock as Currency<DummyValidatorId>>::Balance>,
    ) {
        unimplemented!()
    }

    fn can_be_deleted(_: &DummyValidatorId) -> Result<bool, DispatchError> {
        unimplemented!()
    }

    fn delete_account(_: &DummyValidatorId) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn exchange(
        _: (&DummyValidatorId, &DummyValidatorId),
        _: (&Asset, &Asset),
        _: (
            <CurrencyMock as Currency<DummyValidatorId>>::Balance,
            <CurrencyMock as Currency<DummyValidatorId>>::Balance,
        ),
    ) -> Result<(), (DispatchError, Option<DummyValidatorId>)> {
        unimplemented!()
    }

    fn reserve(
        _: &DummyValidatorId,
        _: Asset,
        _: <CurrencyMock as Currency<DummyValidatorId>>::Balance,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn unreserve(
        _: &DummyValidatorId,
        _: Asset,
        _: <CurrencyMock as Currency<DummyValidatorId>>::Balance,
    ) -> <CurrencyMock as Currency<DummyValidatorId>>::Balance {
        unimplemented!()
    }

    fn xcm_transfer(
        _from: &DummyValidatorId,
        _asset: Asset,
        _amount: Balance,
        _to: eq_primitives::balance::XcmDestination,
    ) -> DispatchResult {
        unimplemented!()
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

    fn reserved_balance(_: &DummyValidatorId, _: Asset) -> Balance {
        unimplemented!()
    }

    fn slash_reserved(
        _: &DummyValidatorId,
        _: Asset,
        _: Balance,
    ) -> (eq_balances::NegativeImbalance<Balance>, Balance) {
        unimplemented!()
    }

    fn repatriate_reserved(
        _: &DummyValidatorId,
        _: &DummyValidatorId,
        _: Asset,
        _: Balance,
        _: frame_support::traits::BalanceStatus,
    ) -> Result<Balance, DispatchError> {
        unimplemented!()
    }
}

pub type CurrencyMock =
    eq_primitives::balance_adapter::BalanceAdapter<Balance, EqCurrencyMock, BasicCurrencyGet>;

pub struct VestingScheduleMock;
impl VestingSchedule<DummyValidatorId> for VestingScheduleMock {
    type Moment = u64;

    type Currency = CurrencyMock;

    fn vesting_balance(
        _who: &DummyValidatorId,
    ) -> Option<<Self::Currency as Currency<DummyValidatorId>>::Balance> {
        let mut result = false;
        VESTING_EXISTS.with(|v| result = v.borrow().clone());
        if result {
            Option::Some(1)
        } else {
            Option::None
        }
    }

    fn add_vesting_schedule(
        who: &DummyValidatorId,
        locked: <Self::Currency as Currency<DummyValidatorId>>::Balance,
        per_block: <Self::Currency as Currency<DummyValidatorId>>::Balance,
        starting_block: Self::Moment,
    ) -> DispatchResult {
        ADDED_VESTING.with(|v| {
            *v.borrow_mut() = Option::Some((*who, (locked, per_block, starting_block)));
        });
        Ok(())
    }

    fn remove_vesting_schedule(_who: &DummyValidatorId, _schedule_index: u32) -> DispatchResult {
        panic!("not used");
    }

    fn can_add_vesting_schedule(
        _who: &DummyValidatorId,
        _locked: <Self::Currency as Currency<DummyValidatorId>>::Balance,
        _per_block: <Self::Currency as Currency<DummyValidatorId>>::Balance,
        _starting_block: Self::Moment,
    ) -> frame_support::dispatch::DispatchResult {
        unimplemented!()
    }
}

pub struct AssetGetterMock;
impl AssetGetter for AssetGetterMock {
    fn get_asset_data(
        _: &eq_primitives::asset::Asset,
    ) -> Result<eq_primitives::asset::AssetData<eq_primitives::asset::Asset>, DispatchError> {
        unimplemented!()
    }

    fn exists(_: eq_primitives::asset::Asset) -> bool {
        unimplemented!()
    }

    fn get_assets_data() -> Vec<eq_primitives::asset::AssetData<eq_primitives::asset::Asset>> {
        unimplemented!()
    }

    fn get_assets_data_with_usd(
    ) -> Vec<eq_primitives::asset::AssetData<eq_primitives::asset::Asset>> {
        unimplemented!()
    }

    fn get_assets() -> Vec<eq_primitives::asset::Asset> {
        vec![asset::GENS, asset::BTC, asset::ETH, asset::HDOT]
    }

    fn get_assets_with_usd() -> Vec<eq_primitives::asset::Asset> {
        unimplemented!()
    }

    fn priority(_: eq_primitives::asset::Asset) -> Option<u64> {
        unimplemented!()
    }

    fn get_main_asset() -> eq_primitives::asset::Asset {
        asset::GENS
    }

    fn collateral_discount(_asset: &Asset) -> EqFixedU128 {
        EqFixedU128::one()
    }
}

parameter_types! {
    pub const MotionDuration: BlockNumberFor<Test> = 1;
    pub const MaxProposals: u32 = 1;
    pub const MaxMembers: u32 = 1;
}

impl pallet_collective::Config<()> for Test {
    type Origin = OriginFor<Test>;
    type Proposal = Call;
    type Event = Event;
    type MotionDuration = MotionDuration;
    type MaxProposals = MaxProposals;
    type MaxMembers = MaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = ();
}

impl Config for Test {
    type PalletId = DistributionModuleId;
    type ManagementOrigin =
        EitherOfDiverse<EnsureRoot<AccountId>, pallet_collective::EnsureMember<AccountId, ()>>;
    type VestingAccountId = VestingAccountMock<AccountId>;
    type VestingSchedule = VestingScheduleMock;
    type AssetGetter = AssetGetterMock;
    type EqCurrency = EqCurrencyMock;
    type WeightInfo = ();
}

pub type ModuleDistribution = Pallet<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let r = frame_system::GenesisConfig::default().build_storage::<Test>();

    r.unwrap().into()
}
