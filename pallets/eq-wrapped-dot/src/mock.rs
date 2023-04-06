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
use eq_primitives::asset::{AssetType, AssetXcmData, OtherReservedData};
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::mocks::{
    ParachainId, TimeZeroDurationMock, UpdateTimeManagerEmptyMock, XcmRouterCachedMessagesMock,
};
use eq_primitives::subaccount::{SubAccType, SubaccountsManager};
use eq_primitives::{BailsmanManager, EqBuyout, SignedBalance};
use eq_xcm::relay_interface::call::{RelayChainCall, RelayChainCallBuilder};
use frame_support::{parameter_types, PalletId, RuntimeDebug};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{AccountIdLookup, BlakeTwo256, IdentityLookup};
use sp_runtime::{DispatchError, DispatchResult, Perbill, Percent, Permill};
use sp_std::cell::RefCell;
use std::marker::PhantomData;
use xcm::DoubleEncoded;

use crate as eq_wrapped_dot;

type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
pub(crate) type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        EqBalances: eq_balances::{Pallet, Call, Storage, Event<T>},
        EqAggregates: eq_aggregates::{Pallet},
        EqAssets: eq_assets::{Pallet, Storage, Call, Event},
        EqWrappedDot: eq_wrapped_dot::{Pallet, Storage, Call},
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
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

impl eq_assets::Config for Test {
    type Event = Event;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
    type OnNewAsset = ();
    type WeightInfo = ();
}

impl eq_aggregates::Config for Test {
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Test>;
}

thread_local! {
    static PRICES: RefCell<Vec<(asset::Asset, FixedI64)>> = RefCell::new(vec![
        (asset::CRV, FixedI64::saturating_from_integer(10000)),
        (asset::BTC, FixedI64::saturating_from_integer(10000)),
        (asset::EOS, FixedI64::saturating_from_integer(3)),
        (asset::ETH, FixedI64::saturating_from_integer(250)),
        (asset::EQD, FixedI64::saturating_from_integer(1)),
        (asset::EQ, FixedI64::saturating_from_integer(1)),
        (asset::DOT, FixedI64::saturating_from_integer(4)),
        (asset::EQDOT, FixedI64::saturating_from_integer(4))
        ]);
    static CURRENT_TIME: RefCell<u64> = RefCell::new(1_598_006_981_634);
}

pub const ACCOUNT_BAILSMAN_1: AccountId = 333;
pub const ACCOUNT_BAILSMAN_2: AccountId = 444;

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
        false
    }

    fn get_subaccount_id(_who: &AccountId, _subacc_type: &SubAccType) -> Option<AccountId> {
        Some(9999_u64)
    }

    fn is_subaccount(_who: &AccountId, _subacc_id: &AccountId) -> bool {
        // hack for not deleting account in transfer
        true
    }

    fn get_owner_id(_subaccount: &AccountId) -> Option<(AccountId, SubAccType)> {
        None
    }

    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        0
    }

    fn is_master(who: &AccountId) -> bool {
        who != &ACCOUNT_BAILSMAN_1 && who != &ACCOUNT_BAILSMAN_2
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

parameter_types! {
    pub const ExistentialDeposit: Balance = 1;
    pub const EpochDuration: u64 = 3;
    pub const ExpectedBlockTime: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
    pub const LendingModuleId: PalletId = PalletId(*b"eq/lendr");
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
    type Event = Event;
    type WeightInfo = ();
    type Aggregates = eq_aggregates::Pallet<Test>;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmanManagerMock;
    type UpdateTimeManager = UpdateTimeManagerEmptyMock<AccountId>;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type LocationInverter = eq_primitives::mocks::LocationInverterMock;
    type OrderAggregates = ();
    type UnixTime = TimeZeroDurationMock;
}

parameter_types! {
    pub const RelayLocation: MultiLocation = Here.into();
    pub const AnyNetwork: NetworkId = NetworkId::Any;
    pub Ancestry: MultiLocation = Here.into();
    pub UnitWeightCost: u64 = 1_000;
    pub const BaseXcmWeight: u64 = 1_000;
    pub CurrencyPerSecond: (AssetId, u128) = (Concrete(RelayLocation::get()), 1);
    pub TrustedAssets: (MultiAssetFilter, MultiLocation) = (All.into(), Here.into());
    pub const MaxInstructions: u32 = 100;
}

thread_local! {
    pub static SENT_XCM: RefCell<Vec<(MultiLocation, Xcm<()>)>> = RefCell::new(Vec::new());
}

/// Sender that never returns error, always sends
pub struct TestSendXcm;
impl SendXcm for TestSendXcm {
    fn send_xcm(dest: impl Into<MultiLocation>, msg: Xcm<()>) -> SendResult {
        SENT_XCM.with(|q| q.borrow_mut().push((dest.into(), msg)));
        Ok(())
    }
}

pub struct TestSendXcmErrX8;
impl SendXcm for TestSendXcmErrX8 {
    fn send_xcm(dest: impl Into<MultiLocation>, msg: Xcm<()>) -> SendResult {
        let dest = dest.into();
        if dest.len() == 8 {
            Err(SendError::Transport("Destination location full"))
        } else {
            SENT_XCM.with(|q| q.borrow_mut().push((dest, msg)));
            Ok(())
        }
    }
}

parameter_types! {
    pub TargetReserve: Permill = Permill::from_percent(15);
    pub MaxReserve: Permill = Permill::from_percent(20);
    pub MinReserve: Permill = Permill::from_percent(10);
    pub const MinStakingDeposit: Balance = 5_000_000_000; //5 DOT
    pub EqDotWithdrawFee: Permill = Permill::from_float(0.98940904738);
    pub const WrappedDotPalletId: PalletId = PalletId(*b"eq/wrdot");
}

#[derive(RuntimeDebug)]
pub struct RelayRuntimeMock;

impl RelaySystemConfig for RelayRuntimeMock {
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type Balance = XcmBalance;
}

impl Config for Test {
    type StakingInitializeOrigin = EnsureRoot<AccountId>;
    type Balance = Balance;
    type Aggregates = eq_aggregates::Pallet<Test>;
    type TargetReserve = TargetReserve;
    type MaxReserve = MaxReserve;
    type MinReserve = MinReserve;
    type MinDeposit = MinStakingDeposit;
    type RelayChainCallBuilder = RelayChainCallBuilder<RelayRuntimeMock, ParachainId>;
    type XcmRouter = XcmRouterCachedMessagesMock;
    type ParachainId = ParachainId;
    type PriceGetter = OracleMock;
    type EqCurrency = EqBalances;
    type WithdrawFee = EqDotWithdrawFee;
    type PalletId = WrappedDotPalletId;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::EQD, FixedI64::saturating_from_integer(1)),
        (asset::EQ, FixedI64::saturating_from_integer(1)),
        (asset::DOT, FixedI64::saturating_from_integer(4)),
        (asset::EQDOT, FixedI64::saturating_from_integer(4)),
    ]);

    let mut r = frame_system::GenesisConfig::default()
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
                Permill::from_rational(2u32, 10u32), // ** FOR TESTS **
                2,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::from_percent(90),
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
                AssetXcmData::OtherReserved(OtherReservedData {
                    multi_location: (1, Here).into(),
                    decimals: 10,
                })
                .encode(),
                Permill::from_rational(2u32, 5u32),
                4,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::EQDOT.get_id(),
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

pub fn assert_extrinsic_sent(call: RelayChainCall<RelayRuntimeMock>) {
    let call = call.encode();
    for (dest, xcm) in XcmRouterCachedMessagesMock::get() {
        assert_eq!(dest, Parent.into());
        let maybe_transaction = xcm
            .0
            .into_iter()
            .filter_map(|i| match i {
                Transact { call, .. } => {
                    DoubleEncoded::<RelayChainCall<RelayRuntimeMock>>::from(call)
                        .take_decoded()
                        .ok()
                }
                _ => None,
            })
            .next();

        if let Some(transaction) = maybe_transaction {
            if transaction.encode() == call {
                return;
            }
        }
    }

    panic!("XCM transaction not found");
}
