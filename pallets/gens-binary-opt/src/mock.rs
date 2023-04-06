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

use crate as gens_binary_opt;
use eq_primitives::{
    asset,
    subaccount::{SubAccType, SubaccountsManager},
    Aggregates, BailsmanManager, EqBuyout, SignedBalance, TotalAggregates, UpdateTimeManager,
    UserGroup,
};
use eq_utils::ONE_TOKEN;
use eq_whitelists::CheckWhitelisted;
use frame_support::{
    parameter_types, sp_io,
    traits::{Hooks, LockIdentifier, WithdrawReasons},
};
use sp_core::H256;
use sp_runtime::FixedU128;
use sp_runtime::{
    testing::Header,
    traits::AccountIdConversion,
    traits::{BlakeTwo256, IdentityLookup},
    DispatchError, DispatchResult, FixedI128, FixedI64, FixedPointNumber, ModuleId, Perbill,
    Perquintill,
};
use std::{cell::RefCell, collections::HashMap};

pub const ONE_THIRD_TOKEN: u64 = 0_333_333_333;
pub const ONE_TENTH_TOKEN: u64 = 0_100_000_000;
pub const ONE_HUNDREDTH_TOKEN: u64 = 0_010_000_000;

pub const PROPER_ASSET: Asset = Asset(0x41414141); // "AAAA"
pub const TARGET_ASSET: Asset = Asset(0x42424242); // "BBBB"
pub const UNKNOWN_ASSET: Asset = Asset(0x43434343); // "CCCC"

pub const OLD_TARGET_PRICE: FixedI64 = FixedI64::from_inner(1_000_000_000);
pub const NEW_TARGET_PRICE: FixedI64 = FixedI64::from_inner(2_000_000_000);
pub const TARGET_PRICE_0: FixedI64 = FixedI64::from_inner(0_500_000_000);
pub const TARGET_PRICE_1: FixedI64 = FixedI64::from_inner(1_500_000_000);

pub const USER_0: AccountId = 1;
pub const USER_1: AccountId = 2;
pub const USER_2: AccountId = 3;
pub const USER_3: AccountId = 4;

pub const BINARY_ID_0: BinaryId = 1;
pub const BINARY_ID_1: BinaryId = 2;
pub const BINARY_ID_2: BinaryId = 4;

pub const MINIMAL_DEPOSIT: Balance = ONE_HUNDREDTH_TOKEN;

thread_local! {
    pub static ASSET_PRICE: RefCell<FixedI64> = RefCell::new(OLD_TARGET_PRICE);
    pub static BENCHMARK_TIMESTAMP: RefCell<Time> = RefCell::new(0);
    pub static BALANCES: RefCell<HashMap<(AccountId, Asset), Balance>> = RefCell::new([
        ((USER_0, PROPER_ASSET), 10 * ONE_TOKEN),
        ((USER_1, PROPER_ASSET), 10 * ONE_TOKEN),
        ((USER_2, PROPER_ASSET), 10 * ONE_TOKEN),
        ((USER_3, PROPER_ASSET), 10 * ONE_TOKEN),
        ((get_treasury_account(), PROPER_ASSET), 0 * ONE_TOKEN),
        ((get_pallet_account(), PROPER_ASSET), 0 * ONE_TOKEN),
    ].iter().cloned().collect());
}

pub type Time = u64;
pub type Balance = Balance;
pub type BinaryId = u64;
pub type AccountId = u64;
pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub type ModuleTimestamp = timestamp::Module<Test>;
pub type ModuleSystem = frame_system::Module<Test>;
pub type ModuleBinaries = crate::Module<Test>;

// Test runtime

use core::convert::{TryFrom, TryInto};

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        GensBinaryOpt: gens_binary_opt::{Pallet, Call, Storage, Event<T>},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event},
        EqBalances: eq_balances::{Pallet, Call, Storage, Event<T>},
        Timestamp: timestamp::{Pallet, Call, Storage},
    }
);

// Impl configs

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

parameter_types! {
    pub const BinaryOpModuleId: ModuleId = ModuleId(*b"eq/binop");
    pub const BalancesModuleId: ModuleId = ModuleId(*b"eq/balan");
    pub const BailsmanModuleId: ModuleId = ModuleId(*b"eq/bails");
    pub const TreasuryModuleId: ModuleId = ModuleId(*b"eq/trsry");
}

parameter_types! {
    pub const DepositOffset: u64 = 10;
    pub const Penalty: Perquintill = Perquintill::from_percent(5);
}

impl Config for Test {
    type Event = Event;
    type Balance = Balance;
    type BinaryId = BinaryId;
    type AssetGetter = AssetGetterMock;
    type PriceGetter = OracleMock;
    type UnixTime = timestamp::Module<Test>;
    type EqCurrency = EqCurrencyMock;
    type DepositOffset = DepositOffset;
    type Penalty = Penalty;
    type ModuleId = BinaryOpModuleId;
    type TreasuryModuleId = TreasuryModuleId;
    type WeightInfo = ();
}

parameter_types! {
    pub const BasicCurrencyGet: asset::Asset = PROPER_ASSET;
}

impl eq_assets::Config for Test {
    type Event = Event;
    type MainAsset = BasicCurrencyGet;
    type OnNewAsset = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
    pub const BlockHashCount: u64 = 250;
    pub const MinimumPeriod: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const MinSurplus: u64 = 1 * ONE_TOKEN; // 1 usd
    pub const MinTempBailsman: u64 = 20 * ONE_TOKEN; // 20 usd
    pub const UnsignedPriority: u64 = 100;
    pub const DepositEq: u64 = 0;
    pub const ExistentialDeposit: Balance = 1;
    pub CriticalLtv: FixedI128 = FixedI128::saturating_from_rational(105, 100);
    pub const MinimalCollateral: Balance = 10 * ONE_TOKEN;
    pub RiskLowerBound: FixedI128 = FixedI128::saturating_from_rational(1, 2);
    pub RiskUpperBound: FixedI128 = FixedI128::saturating_from_integer(2);
    pub RiskNSigma: FixedI128 = FixedI128::saturating_from_integer(10);
    pub RiskRho: FixedI128 = FixedI128::saturating_from_rational(7, 10);
    pub Alpha: FixedI128 = FixedI128::from(15);
    pub const MinTempBalanceUsd: Balance = 0; // always reinit
}

impl eq_balances::Config for Test {
    type ParachainId = eq_primitives::mocks::ParachainId;
    type AssetGetter = eq_assets::Pallet<Test>;
    type AccountStore = System;
    type Balance = Balance;
    type ExistentialDeposit = ExistentialDeposit;
    type ExistentialDepositBasic = ExistentialDeposit;
    type BalanceChecker = BalanceCheckerMock;
    type PriceGetter = OracleMock;
    type Event = Event;
    type WeightInfo = ();
    type Aggregates = AggregatesMock;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = SubaccountsManagerMock;
    type BailsmenManager = BailsmenManagerMock;
    type UpdateTimeManager = RateMock;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = eq_primitives::mocks::XcmRouterErrMock;
    type XcmToFee = eq_primitives::mocks::XcmToFeeZeroMock;
    type LocationToAccountId = ();
    type LocationInverter = eq_primitives::mocks::LocationInverterMock;
    type OrderAggregates = ();
}

impl timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

// parameter_types! {
//     pub const LpPriceBlockTimeout: u64 = 10u64;
//     pub const UnsignedLifetimeInBlocks: u32 = 5;
//     pub const FinancialRecalcPeriodBlocks: u64  = (1000 * 60 * 60 * 4) as u64 / 6000;
//     pub const PriceTimeout: u64 = 1;
//     pub const MedianPriceTimeout: u64 = 60 * 60 * 2;
// }

// impl eq_oracle::Config for Test {
//     type FinancialRecalcPeriodBlocks = FinancialRecalcPeriodBlocks;
//     type AssetGetter = AssetGetterMock;
//     type AuthorityId = crypto::TestAuthId;
//     type Event = Event;
//     type Call = Call;
//     type Whitelist = WhiteListMock;
//     type UnixTime = TimeMock;
//     type MedianPriceTimeout = MedianPriceTimeout;
//     type PriceTimeout = PriceTimeout;
//     type UnsignedPriority = UnsignedPriority;
//     type Balance = Balance;
//     type OnPriceSet = FinancialMock;
//     type FinancialSystemTrait = FinancialMock;
//     type CurveAmm = CurveAmmStub;
//     type WeightInfo = ();
//     type LpPriceBlockTimeout = LpPriceBlockTimeout;
//     type UnsignedLifetimeInBlocks = UnsignedLifetimeInBlocks;
// }

// Mocks

pub struct AssetGetterMock;

impl AssetGetter<DebtWeightType> for AssetGetterMock {
    fn get_asset_data(
        asset: &Asset,
    ) -> Result<asset::AssetData<Asset, DebtWeightType>, DispatchError> {
        if Self::exists(asset.clone()) {
            Ok(asset::AssetData::<Asset, DebtWeightType> {
                id: asset.clone(),
                lot: FixedU128::from_inner(0),
                price_step: FixedU128::from_inner(0),
                maker_fee: FixedU128::from_inner(0),
                taker_fee: FixedU128::from_inner(0),
                multi_asset: None,
                multi_location: None,
                debt_weight: DebtWeightType::from_inner(0),
                buyout_priority: 0,
                asset_type: asset::AssetType::Physical,
                is_dex_enabled: false,
                collateral_enabled: false,
            })
        } else {
            Err(eq_assets::Error::<Test>::AssetNotExists.into())
        }
    }

    fn exists(asset: Asset) -> bool {
        match asset {
            TARGET_ASSET | PROPER_ASSET => true,
            _ => false,
        }
    }

    fn get_assets_data() -> Vec<asset::AssetData<Asset, DebtWeightType>> {
        todo!()
    }

    fn get_assets_data_with_usd() -> Vec<asset::AssetData<Asset, DebtWeightType>> {
        todo!()
    }

    fn get_assets() -> Vec<Asset> {
        vec![PROPER_ASSET, TARGET_ASSET]
    }

    fn get_assets_with_usd() -> Vec<Asset> {
        Self::get_assets()
    }

    fn priority(_asset: Asset) -> Option<u64> {
        todo!()
    }

    fn get_main_asset() -> Asset {
        PROPER_ASSET
    }

    fn get_collateral_enabled_assets() -> Vec<Asset> {
        Self::get_assets()
    }

    fn is_allowed_as_collateral(_asset: &Asset) -> bool {
        true
    }

    fn collateral_discount(_asset: &Asset) -> EqFixedU128 {
        EqFixedU128::one()
    }
}

pub struct BalanceCheckerMock {}

pub const FAIL_ACC: u64 = 666;

impl eq_primitives::balance::BalanceChecker<u64, u64, EqBalances, SubaccountsManagerMock>
    for BalanceCheckerMock
{
    fn can_change_balance_impl(
        who: &u64,
        changes: &Vec<(Asset, eq_primitives::SignedBalance<Balance>)>,
        _reason: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        let all_positive = change.iter().all(|sb| match sb {
            SignedBalance::Positive(_) => true,
            SignedBalance::Negative(_) => false,
        });
        if all_positive {
            return Ok(());
        }
        match who {
            &FAIL_ACC => Err(DispatchError::Other("Expected error")),
            _ => Ok(()),
        }
    }
}

pub struct BailsmenManagerMock;

impl BailsmanManager<AccountId, u64> for BailsmenManagerMock {
    fn register_bailsman(_who: &AccountId) -> Result<(), sp_runtime::DispatchError> {
        unimplemented!()
    }

    fn unregister_bailsman(_who: &AccountId) -> Result<(), sp_runtime::DispatchError> {
        unimplemented!()
    }

    fn receive_position(
        _who: &AccountId,
        _is_deleting_position: bool,
    ) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn reinit() -> Result<bool, sp_runtime::DispatchError> {
        unimplemented!()
    }

    fn should_unreg_bailsman(
        _who: &AccountId,
        _assets: &[Asset],
        _amounts: &[SignedBalance<u64>],
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
}

pub struct RateMock;

impl UpdateTimeManager<AccountId> for RateMock {
    fn set_last_update(_account_id: &u64) {}
    fn remove_last_update(_accounts_id: &u64) {}
    fn set_last_update_timestamp(_account_id: &u64, _timestamp_ms: u64) {}
}

pub struct EqBuyoutMock;

impl EqBuyout<u64, u64> for EqBuyoutMock {
    fn eq_buyout(who: &u64, amount: u64) -> sp_runtime::DispatchResult {
        let native_asset =
            <eq_assets::Module<Test> as AssetGetter<DebtWeightType>>::get_main_asset();
        <eq_balances::Module<Test> as EqCurrency<u64, u64>>::currency_transfer(
            &TreasuryModuleId::get().into_account_truncating(),
            who,
            native_asset,
            amount,
            ExistenceRequirement::AllowDeath,
            eq_primitives::TransferReason::Common,
            false,
        )?;

        <eq_balances::Module<Test> as EqCurrency<u64, u64>>::currency_transfer(
            who,
            &TreasuryModuleId::get().into_account_truncating(),
            asset::EOS,
            ONE_TOKEN,
            ExistenceRequirement::AllowDeath,
            eq_primitives::TransferReason::Common,
            false,
        )?;

        Ok(())
    }
    fn is_enough(_asset: Asset, _amount: u64, _amount_buyout: u64) -> Result<bool, DispatchError> {
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
    fn set_usergroup(account_id: &AccountId, user_group: UserGroup, _is_in: &bool) {
        USER_GROUPS.with(|v| {
            v.borrow_mut()
                .push((user_group.clone(), account_id.clone()));
        })
    }

    fn update_total(
        _account_id: &AccountId,
        _asset: Asset,
        _prev_balance: &SignedBalance<u64>,
        _delta_balance: &SignedBalance<u64>,
    ) {
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
    ) -> Box<dyn Iterator<Item = (Asset, TotalAggregates<u64>)>> {
        panic!("AggregatesMock not implemented");
    }
    fn get_total(_user_group: UserGroup, _asset: Asset) -> TotalAggregates<u64> {
        TotalAggregates {
            collateral: 0,
            debt: 0,
        }
    }
}

pub struct OracleMock;

impl PriceGetter for OracleMock {
    fn get_price(asset: &Asset) -> Result<FixedI64, DispatchError> {
        match *asset {
            TARGET_ASSET => Ok(ASSET_PRICE.with(|price| *price.borrow())),
            PROPER_ASSET => Ok(FixedI64::one()),
            _ => Err(DispatchError::Other("Unknown asset")),
        }
    }
}

pub struct EqCurrencyMock;

impl EqCurrency<AccountId, Balance> for EqCurrencyMock {
    fn total_balance(asset: Asset, who: &AccountId) -> Balance {
        BALANCES.with(|balances| {
            balances
                .borrow()
                .get(&(who.clone(), asset))
                .cloned()
                .unwrap_or(0)
        })
    }

    fn debt(_asset: Asset, _who: &AccountId) -> Balance {
        unimplemented!()
    }

    fn currency_total_issuance(_asset: Asset) -> Balance {
        unimplemented!()
    }

    fn minimum_balance_value() -> Balance {
        unimplemented!()
    }

    fn free_balance(_asset: Asset, _who: &AccountId) -> Balance {
        unimplemented!()
    }

    fn ensure_can_withdraw(
        _asset: Asset,
        _who: &AccountId,
        _amount: Balance,
        _reasons: frame_support::traits::WithdrawReasons,
        _new_balance: Balance,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn currency_transfer(
        asset: Asset,
        transactor: &AccountId,
        dest: &AccountId,
        value: Balance,
        _existence_requirement: ExistenceRequirement,
        _transfer_reason: TransferReason,
        _ensure_can_change: bool,
    ) -> DispatchResult {
        if transactor == dest {
            Ok(().into())
        } else {
            BALANCES.with(|balances| {
                let balances = &mut *balances.borrow_mut();

                let trans_balance = balances.entry((*transactor, asset)).or_insert(0);
                if *trans_balance < value {
                    println!("{} < {}", trans_balance, value);
                    Err(DispatchError::Other("Not enought"))
                } else {
                    *trans_balance -= value;
                    *balances.entry((*dest, asset)).or_insert(0) += value;
                    Ok(().into())
                }
            })
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
    }

    fn deposit_into_existing(
        _asset: Asset,
        _who: &AccountId,
        _value: Balance,
        _: Option<DepositReason>,
    ) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn deposit_creating(
        _asset: Asset,
        _who: &AccountId,
        _value: Balance,
        _ensure_can_change: bool,
    ) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn withdraw(
        _asset: Asset,
        _who: &AccountId,
        _value: Balance,
        _reasons: frame_support::traits::WithdrawReasons,
        _liveness: ExistenceRequirement,
        _ensure_can_change: bool,
    ) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn make_free_balance_be(
        _asset: Asset,
        _who: &AccountId,
        _value: eq_primitives::SignedBalance<Balance>,
    ) {
        unimplemented!()
    }

    fn can_be_deleted(_who: &AccountId) -> Result<bool, DispatchError> {
        unimplemented!()
    }

    fn delete_account(_account_id: &AccountId) -> Result<(), DispatchError> {
        unimplemented!()
    }

    fn exchange(
        _accounts: (&AccountId, &AccountId),
        _assets: (&Asset, &Asset),
        _values: (Balance, Balance),
    ) -> Result<(), (DispatchError, Option<AccountId>)> {
        unimplemented!()
    }

    fn reserve(_who: &AccountId, _asset: Asset, _amount: Balance) -> DispatchResult {
        unimplemented!()
    }

    fn unreserve(_who: &AccountId, _asset: Asset, _amount: Balance) -> Balance {
        unimplemented!()
    }
}

thread_local! {
    pub static SUBACCOUNTS: RefCell<Vec<(AccountId, SubAccType, AccountId)>> = RefCell::new(vec![(1, SubAccType::Trader, 101)]);
}

pub struct SubaccountsManagerMock;

impl SubaccountsManager<u64> for SubaccountsManagerMock {
    fn create_subaccount_inner(who: &u64, subacc_type: &SubAccType) -> Result<u64, DispatchError> {
        let subaccount_id = who + 100;
        SUBACCOUNTS.with(|v| {
            let mut vec = v.borrow_mut();
            vec.push((*who, *subacc_type, subaccount_id));
        });

        Ok(subaccount_id)
    }
    fn delete_subaccount_inner(who: &u64, subacc_type: &SubAccType) -> Result<u64, DispatchError> {
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
    fn has_subaccount(who: &u64, _subacc_type: &SubAccType) -> bool {
        match &who {
            0u64 => false,
            _ => true,
        }
    }
    fn get_subaccount_id(who: &u64, subacc_type: &SubAccType) -> Option<u64> {
        let mut subaccount_id: Option<u64> = None;
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
    fn is_subaccount(_who: &AccountId, _subacc_id: &u64) -> bool {
        false
    }
    fn get_owner_id(_subaccount: &AccountId) -> Option<(u64, SubAccType)> {
        None
    }
    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        0
    }
}

pub struct WhiteListMock;

impl CheckWhitelisted<AccountId> for WhiteListMock {
    fn in_whitelist(_account_id: &AccountId) -> bool {
        true
    }
    /// Gets a vector of all whitelisted accounts
    fn accounts() -> Vec<AccountId> {
        Vec::new()
    }
}

// Benchmarks runtime

pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into()
}

pub fn time_move(secs: u64) {
    const SECS_PER_BLOCK: u64 = 6;

    BENCHMARK_TIMESTAMP.with(|time| {
        *time.borrow_mut() += secs;
        ModuleTimestamp::set_timestamp(*time.borrow() * 1_000);

        let end_block = *time.borrow() / SECS_PER_BLOCK;

        while ModuleSystem::block_number() < end_block {
            if ModuleSystem::block_number() > 1 {
                ModuleBinaries::on_finalize(ModuleSystem::block_number());
                ModuleSystem::on_finalize(ModuleSystem::block_number());
            }
            ModuleSystem::set_block_number(ModuleSystem::block_number() + 1);
            ModuleSystem::on_initialize(ModuleSystem::block_number());
            ModuleBinaries::on_initialize(ModuleSystem::block_number());
        }
    });
}

pub fn get_balances() -> HashMap<(AccountId, Asset), Balance> {
    BALANCES.with(|balances| (&*balances.borrow()).clone())
}

pub fn set_target_price(to_set: FixedI64) {
    ASSET_PRICE.with(|price| *price.borrow_mut() = to_set)
}

pub fn get_pallet_account() -> AccountId {
    BinaryOpModuleId::get().into_account_truncating()
}

pub fn get_treasury_account() -> AccountId {
    TreasuryModuleId::get().into_account_truncating()
}
