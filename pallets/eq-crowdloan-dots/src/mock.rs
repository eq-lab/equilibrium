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
use crate as eq_crowdloan_dots;
use core::marker::PhantomData;
use eq_primitives::asset::{self, AssetType};
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::mocks::{
    TimeZeroDurationMock, UniversalLocationMock, UpdateTimeManagerEmptyMock, XcmRouterErrMock,
    XcmToFeeZeroMock,
};
use eq_primitives::subaccount::{SubAccType, SubaccountsManager};
use eq_primitives::{AccountDistribution, Aggregates, BailsmanManager, TotalAggregates, UserGroup};
pub use eq_utils::ONE_TOKEN;
use frame_support::traits::{ConstU16, ConstU64, GenesisBuild};
use frame_support::{parameter_types, PalletId};
use frame_system as system;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{DispatchError, FixedI64, FixedPointNumber, Percent, Permill};
use system::EnsureRoot;

pub(crate) type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
pub(crate) type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;

pub type ModuleBalances = eq_balances::Pallet<Test>;
pub type ModuleLending = eq_lending::Pallet<Test>;
pub type ModuleCrowdloanDots = Pallet<Test>;
type DummyValidatorId = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

parameter_types! {
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub const ExistentialDeposit: Balance = 1;
    pub const AccountsToMigratePerBlock: u32 = 2;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
    pub const LendingModuleId: PalletId = PalletId(*b"eq/lendr");
}

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: system::{Pallet, Call, Event<T>},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event},
        EqBalances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        EqCrowdLoanDots: eq_crowdloan_dots::{Pallet, Call, Storage},
        EqLending: eq_lending::{Pallet, Call, Storage, Event<T>, Config<T>}
    }
);

pub struct AggregatesMock;
pub struct BailsmanManagerMock;
pub struct SubaccountsManagerMock;

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

    fn get_subaccount_id(_who: &AccountId, subacc_type: &SubAccType) -> Option<AccountId> {
        match subacc_type {
            SubAccType::Borrower => Some(9999_u64),
            _ => None,
        }
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
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = eq_primitives::balance::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
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
    type BalanceChecker = eq_balances::locked_balance_checker::CheckLocked<Test>;
    type PriceGetter = OracleMock;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Aggregates = AggregatesMock;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmanManagerMock;
    type UpdateTimeManager = UpdateTimeManagerEmptyMock<AccountId>;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = XcmRouterErrMock;
    type XcmToFee = XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type UniversalLocation = UniversalLocationMock;
    type OrderAggregates = ();
    type UnixTime = TimeZeroDurationMock;
}

impl eq_crowdloan_dots::Config for Test {
    type AllowCrowdloanOrigin = EnsureRoot<AccountId>;
    type Balance = Balance;
    type EqCurrency = EqBalances;
    type BalanceGetter = EqBalances;
    type SubaccountsManager = SubaccountsManagerMock;
    type IsTransfersEnabled = EqBalances;
    type LendingPoolManager = ModuleLending;
}

impl eq_lending::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type BalanceGetter = ModuleBalances;
    type Aggregates = AggregatesMock;
    type AssetGetter = eq_assets::Pallet<Test>;
    type PriceGetter = OracleMock;
    type SubaccountsManager = SubaccountsManagerMock;
    type ModuleId = LendingModuleId;
    type EqCurrency = ModuleBalances;
    type BailsmanManager = BailsmanManagerMock;
    type UnixTime = TimeZeroDurationMock;
    type AccountsToMigratePerBlock = AccountsToMigratePerBlock;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![
        (asset::EQ, FixedI64::saturating_from_integer(1)),
        (asset::XDOT, FixedI64::saturating_from_integer(1)),
        (asset::CDOT613, FixedI64::saturating_from_integer(1)),
    ]);

    let mut storage = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    eq_assets::GenesisConfig::<Test> {
		_runtime: PhantomData,
        assets: // id, lot, price_step, maker_fee, taker_fee, debt_weight, buyout_priority
        vec![
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
                asset::XDOT.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                12,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::XDOT2.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                12,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::XDOT3.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                12,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::CDOT613.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                13,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::CDOT714.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                13,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::CDOT815.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                vec![],
                Permill::zero(),
                13,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            )
		]
	}
    .assimilate_storage(&mut storage)
    .unwrap();

    eq_balances::GenesisConfig::<Test> {
        balances: vec![(1, vec![(ONE_TOKEN, asset::EQ.get_id())])],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(eq_primitives::XcmMode::Xcm(false)),
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    storage.into()
}
