#![cfg(test)]

use super::*;

use crate as gens_binary_opt;
use eq_balances::MaxLocks;
use eq_primitives::{
    asset,
    balance::{Balance, DepositReason, WithdrawReason},
    balance_number::EqFixedU128,
    mocks::TimeZeroDurationMock,
    subaccount::{SubAccType, SubaccountsManager},
    Aggregates, BailsmanManager, EqBuyout, SignedBalance, TotalAggregates, UpdateTimeManager,
    UserGroup,
};
use eq_utils::ONE_TOKEN;
use eq_whitelists::CheckWhitelisted;
use frame_support::{
    parameter_types, sp_io,
    traits::{Currency, Hooks, LockIdentifier, WithdrawReasons},
    PalletId,
};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::AccountIdConversion,
    traits::{BlakeTwo256, IdentityLookup, Zero},
    DispatchError, DispatchResult, FixedI128, FixedI64, FixedPointNumber, Perbill, Percent,
};
use sp_runtime::{traits::One, Permill};
use std::{cell::RefCell, collections::HashMap};

pub const ONE_THIRD_TOKEN: Balance = 0_333_333_333;
pub const ONE_TENTH_TOKEN: Balance = 0_100_000_000;
pub const ONE_HUNDREDTH_TOKEN: Balance = 0_010_000_000;

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

pub const ZERO_FEE: Permill = Permill::zero();
pub const TEN_PERCENT_FEE: Permill = Permill::from_parts(100_000);
pub const DEPOSIT_OFFSET: u64 = 10;
pub const PENALTY: Permill = Permill::from_percent(5);

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
pub type BinaryId = u64;
pub type AccountId = u64;
type DummyValidatorId = u64;
type BlockNumber = u64;
pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub type ModuleTimestamp = pallet_timestamp::Pallet<Test>;
pub type ModuleSystem = frame_system::Pallet<Test>;
pub type ModuleBinaries = crate::Pallet<Test>;
pub type CurrencyMock =
    eq_primitives::balance_adapter::BalanceAdapter<Balance, EqCurrencyMock, BasicCurrencyGet>;

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
        Timestamp: pallet_timestamp::{Pallet, Call, Storage},
    }
);

// Impl configs

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = BlockNumber;
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

parameter_types! {
    pub const BinaryOpModuleId: PalletId = PalletId(*b"eq/binop");
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
}

pub struct TreasuryAccount;
impl Get<AccountId> for TreasuryAccount {
    fn get() -> AccountId {
        TreasuryModuleId::get().into_account_truncating()
    }
}
pub struct UpdateOnceInBlocks;
impl Get<BlockNumber> for UpdateOnceInBlocks {
    fn get() -> BlockNumber {
        BlockNumber::one()
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ToggleBinaryCreateOrigin = EnsureRoot<AccountId>;
    type Balance = Balance;
    type AssetGetter = AssetGetterMock;
    type PriceGetter = OracleMock;
    type UnixTime = pallet_timestamp::Pallet<Test>;
    type EqCurrency = EqCurrencyMock;
    type PalletId = BinaryOpModuleId;
    type TreasuryModuleId = TreasuryAccount;
    type WeightInfo = ();
    type UpdateOnceInBlocks = UpdateOnceInBlocks;
}

parameter_types! {
    pub const BasicCurrencyGet: asset::Asset = PROPER_ASSET;
}

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MainAsset = BasicCurrencyGet;
    type OnNewAsset = ();
    type WeightInfo = ();
    type AssetManagementOrigin = EnsureRoot<AccountId>;
}

parameter_types! {
    pub const Period: u64 = 1;
    pub const Offset: u64 = 0;
    pub const BlockHashCount: u64 = 250;
    pub const MinimumPeriod: u64 = 1;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(16);
    pub const MinSurplus: Balance = 1 * ONE_TOKEN; // 1 usd
    pub const MinTempBailsman: Balance = 20 * ONE_TOKEN; // 20 usd
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
    type RuntimeEvent = RuntimeEvent;
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
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type OrderAggregates = ();
    type ToggleTransferOrigin = EnsureRoot<AccountId>;
    type ForceXcmTransferOrigin = EnsureRoot<AccountId>;
    type UnixTime = TimeZeroDurationMock;
}

impl pallet_timestamp::Config for Test {
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
//     type RuntimeEvent = RuntimeEvent;
//     type RuntimeCall = RuntimeCall;
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

impl AssetGetter for AssetGetterMock {
    fn get_asset_data(asset: &Asset) -> Result<asset::AssetData<Asset>, DispatchError> {
        if Self::exists(asset.clone()) {
            Ok(asset::AssetData::<Asset> {
                id: asset.clone(),
                lot: EqFixedU128::from_inner(0),
                price_step: FixedI64::from_inner(0),
                maker_fee: Permill::zero(),
                taker_fee: Permill::zero(),
                debt_weight: Permill::zero(),
                buyout_priority: 0,
                asset_type: asset::AssetType::Physical,
                is_dex_enabled: false,
                asset_xcm_data: eq_primitives::asset::AssetXcmData::None,
                lending_debt_weight: Permill::one(),
                collateral_discount: Percent::one(),
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

    fn get_assets_data() -> Vec<asset::AssetData<Asset>> {
        todo!()
    }

    fn get_assets_data_with_usd() -> Vec<asset::AssetData<Asset>> {
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

    fn collateral_discount(_asset: &Asset) -> EqFixedU128 {
        EqFixedU128::one()
    }
}

pub struct BalanceCheckerMock {}

pub const FAIL_ACC: u64 = 666;

impl eq_primitives::balance::BalanceChecker<Balance, AccountId, EqBalances, SubaccountsManagerMock>
    for BalanceCheckerMock
{
    fn can_change_balance_impl(
        who: &u64,
        changes: &Vec<(Asset, SignedBalance<Balance>)>,
        _reason: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        let all_positive = changes.iter().all(|(_, sb)| match sb {
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

impl BailsmanManager<AccountId, Balance> for BailsmenManagerMock {
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

    fn redistribute(_who: &AccountId) -> Result<u32, sp_runtime::DispatchError> {
        todo!()
    }

    fn get_account_distribution(
        _who: &AccountId,
    ) -> Result<eq_primitives::AccountDistribution<Balance>, sp_runtime::DispatchError> {
        todo!()
    }
}

pub struct RateMock;

impl UpdateTimeManager<AccountId> for RateMock {
    fn set_last_update(_account_id: &u64) {}
    fn remove_last_update(_accounts_id: &u64) {}
    fn set_last_update_timestamp(_account_id: &u64, _timestamp_ms: u64) {}
}

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
            ONE_TOKEN,
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
    ) -> Result<(), sp_runtime::DispatchError> {
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
    ) -> Result<(), sp_runtime::DispatchError> {
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

pub struct OracleMock;

impl PriceGetter for OracleMock {
    fn get_price<FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>>(
        asset: &Asset,
    ) -> Result<FixedNumber, DispatchError> {
        match *asset {
            TARGET_ASSET => {
                let result: Result<FixedNumber, _> =
                    ASSET_PRICE.with(|price| *price.borrow()).try_into();
                match result {
                    Ok(price) => Ok(price),
                    Err(_) => Err(DispatchError::Other("Couldn't convert")),
                }
            }
            PROPER_ASSET => Ok(FixedNumber::one()),
            _ => Err(DispatchError::Other("Unknown asset")),
        }
    }
}

pub struct EqCurrencyMock;

impl EqCurrency<AccountId, Balance> for EqCurrencyMock {
    type Moment = Block;
    type MaxLocks = MaxLocks;

    fn total_balance(who: &AccountId, asset: Asset) -> Balance {
        BALANCES.with(|balances| {
            balances
                .borrow()
                .get(&(who.clone(), asset))
                .cloned()
                .unwrap_or(0)
        })
    }

    fn debt(_who: &AccountId, _asset: Asset) -> Balance {
        unimplemented!()
    }

    fn currency_total_issuance(_asset: Asset) -> Balance {
        unimplemented!()
    }

    fn minimum_balance_value() -> Balance {
        unimplemented!()
    }

    fn free_balance(_who: &AccountId, _asset: Asset) -> Balance {
        unimplemented!()
    }

    fn ensure_can_withdraw(
        _who: &AccountId,
        _asset: Asset,
        _amount: Balance,
        _reasons: frame_support::traits::WithdrawReasons,
        _new_balance: Balance,
    ) -> DispatchResult {
        unimplemented!()
    }

    fn currency_transfer(
        transactor: &AccountId,
        dest: &AccountId,
        asset: Asset,
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
    }

    fn deposit_into_existing(
        _who: &AccountId,
        _asset: Asset,
        _value: Balance,
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
        _who: &AccountId,
        _asset: Asset,
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

    fn reserved_balance(_who: &AccountId, _asset: Asset) -> Balance {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn slash_reserved(
        _who: &AccountId,
        _asset: Asset,
        _value: Balance,
    ) -> (eq_balances::NegativeImbalance<Balance>, Balance) {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn repatriate_reserved(
        _slashed: &AccountId,
        _beneficiary: &AccountId,
        _asset: Asset,
        _value: Balance,
        _status: frame_support::traits::BalanceStatus,
    ) -> Result<Balance, DispatchError> {
        panic!("{}:{} - should not be called", file!(), line!())
    }

    fn xcm_transfer(
        _from: &AccountId,
        _asset: Asset,
        _amount: Balance,
        _kind: eq_primitives::balance::XcmDestination,
    ) -> DispatchResult {
        panic!("{}:{} - should not be called", file!(), line!())
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
