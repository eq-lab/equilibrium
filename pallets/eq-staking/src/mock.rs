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

use core::marker::PhantomData;

use super::*;
use crate as eq_staking;
use eq_primitives::{
    asset::{Asset, AssetType},
    balance_number::EqFixedU128,
    mocks::{
        TimeZeroDurationMock, UniversalLocationMock, UpdateTimeManagerEmptyMock, XcmRouterErrMock,
        XcmToFeeZeroMock,
    },
    subaccount::{SubAccType, SubaccountsManager},
    Aggregates, BailsmanManager, TotalAggregates, UserGroup,
};
pub use eq_utils::ONE_TOKEN;
use frame_support::{
    parameter_types,
    traits::{ConstU16, ConstU64, GenesisBuild, Get},
    PalletId,
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    DispatchError, FixedI64, Percent, Permill,
};
use system::EnsureRoot;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type AccountId = u64;
pub(crate) type Balance = eq_primitives::balance::Balance;
pub(crate) type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: system::{Pallet, Call, Event<T>},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event},
        EqBalances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        EqStaking: eq_staking::{Pallet, Call, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage},
    }
);

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

parameter_types! {
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub const ExistentialDeposit: Balance = 1;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
    pub const MinimumPeriod: u64 = 1;
    pub const MaxStakesCount: u32 = 10;
    pub const RewardsLockPeriod: crate::StakePeriod = crate::StakePeriod::Twelve;
}

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
    type OnNewAsset = ();
    type WeightInfo = ();
}

pub struct AggregatesMock;

impl Aggregates<AccountId, Balance> for AggregatesMock {
    fn in_usergroup(_account_id: &AccountId, _user_group: UserGroup) -> bool {
        true
    }
    fn set_usergroup(
        _account_id: &AccountId,
        _user_group: UserGroup,
        _is_in: bool,
    ) -> DispatchResult {
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

    fn iter_account(_user_group: UserGroup) -> Box<dyn Iterator<Item = AccountId>> {
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

pub struct SubaccountsManagerMock;
impl SubaccountsManager<AccountId> for SubaccountsManagerMock {
    fn create_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        unimplemented!()
    }

    fn delete_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        unimplemented!()
    }

    fn has_subaccount(_who: &AccountId, _subacc_type: &SubAccType) -> bool {
        unimplemented!()
    }

    fn get_subaccount_id(_who: &AccountId, _subacc_type: &SubAccType) -> Option<AccountId> {
        unimplemented!()
    }

    fn is_subaccount(_who: &AccountId, _subaccount_id: &AccountId) -> bool {
        unimplemented!()
    }

    fn get_owner_id(_subaccount: &AccountId) -> Option<(AccountId, SubAccType)> {
        unimplemented!()
    }

    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        unimplemented!()
    }

    fn is_master(_who: &u64) -> bool {
        true
    }
}

pub struct BailsmenManagerMock;

impl BailsmanManager<AccountId, Balance> for BailsmenManagerMock {
    fn register_bailsman(_who: &AccountId) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn unregister_bailsman(_who: &AccountId) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn receive_position(
        _who: &AccountId,
        _is_deleting_position: bool,
    ) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn should_unreg_bailsman(
        _who: &AccountId,
        _amounts: &[(Asset, SignedBalance<Balance>)],
        _: Option<(Balance, Balance)>,
    ) -> Result<bool, sp_runtime::DispatchError> {
        unimplemented!()
    }

    fn bailsmen_count() -> u32 {
        0
    }

    fn distribution_queue_len() -> u32 {
        0
    }

    fn redistribute(_who: &AccountId) -> Result<u32, DispatchError> {
        unimplemented!()
    }

    fn get_account_distribution(
        _who: &AccountId,
    ) -> Result<eq_primitives::AccountDistribution<Balance>, DispatchError> {
        unimplemented!()
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
    type BalanceChecker = eq_balances::locked_balance_checker::CheckLocked<Test>;
    type PriceGetter = OracleMock;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Aggregates = AggregatesMock;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmenManagerMock;
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

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

pub struct TreasuryAccount;
impl Get<AccountId> for TreasuryAccount {
    fn get() -> AccountId {
        use sp_runtime::traits::AccountIdConversion;
        TreasuryModuleId::get().into_account_truncating()
    }
}

parameter_types! {
    pub const MaxRewardExternalIdsCount: u32 = 1000;
}

impl eq_staking::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type EqCurrency = EqBalances;
    type BalanceGetter = EqBalances;
    type LockGetter = EqBalances;
    type UnixTime = pallet_timestamp::Pallet<Test>;
    type MaxStakesCount = MaxStakesCount;
    type RewardManagementOrigin = EnsureRoot<AccountId>;
    type LiquidityAccount = TreasuryAccount;
    type LiquidityAccountCustom = TreasuryAccount;
    type RewardsLockPeriod = RewardsLockPeriod;
    type WeightInfo = ();
    type MaxRewardExternalIdsCount = MaxRewardExternalIdsCount;
}

pub const ACCOUNT_1: AccountId = 1234;
pub const ACCOUNT_2: AccountId = 2345;
pub const ACCOUNT_3: AccountId = 3456;
pub const BALANCE: Balance = 10_000 * ONE_TOKEN;
pub const EXTERNAL_ID: u64 = 112233;

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
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
            )
		]
	}
    .assimilate_storage(&mut storage)
    .unwrap();

    eq_balances::GenesisConfig::<Test> {
        balances: vec![
            (ACCOUNT_1, vec![(BALANCE, asset::EQ.get_id())]),
            (ACCOUNT_2, vec![(BALANCE, asset::EQ.get_id())]),
            (ACCOUNT_3, vec![(BALANCE, asset::EQ.get_id())]),
        ],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(eq_primitives::XcmMode::Xcm(false)),
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    storage.into()
}
