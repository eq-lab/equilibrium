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

use std::marker::PhantomData;

use super::Call as ClaimsCall;
use super::*;
use crate as eq_claim;
use crate::secp_utils::*;
use crate::EthereumAddress;
use codec::Encode;
use eq_primitives::mocks::TimeZeroDurationMock;
use eq_primitives::mocks::VestingAccountMock;
use eq_primitives::XcmMode;
use eq_primitives::{
    asset,
    asset::{Asset, AssetType},
    balance::BalanceGetter,
    balance_number::EqFixedU128,
    subaccount::{SubAccType, SubaccountsManager},
    BailsmanManager, BlockNumberToBalance, EqBuyout, SignedBalance, UpdateTimeManager,
};
use frame_support::traits::WithdrawReasons;
use frame_support::weights::Weight;
use frame_support::{
    assert_err, assert_noop, assert_ok,
    dispatch::DispatchError::BadOrigin,
    dispatch::{GetDispatchInfo, Pays},
    ord_parameter_types, parameter_types,
    traits::{ExistenceRequirement, GenesisBuild},
    PalletId,
};
use frame_system::EnsureRoot;
use hex_literal::hex;
use sp_core::H256;
use sp_io::hashing::keccak_256;
use sp_runtime::FixedI64;
use sp_runtime::Percent;
use sp_runtime::{
    generic::Header,
    traits::{BlakeTwo256, IdentityLookup, One},
    DispatchError,
};

pub(crate) type Balance = eq_primitives::balance::Balance;

type AccountId = u64;
type OracleMock = eq_primitives::price::mock::OracleMock<AccountId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

use core::convert::{TryFrom, TryInto};
use sp_arithmetic::Permill;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Claims: eq_claim::{Pallet, Call, Storage, Event<T>, Config<T>, ValidateUnsigned},
        EqVesting: eq_vesting::{Pallet, Call, Storage, Event<T>},
        EqBalances: eq_balances::{Pallet, Call, Storage, Event<T>},
        EqAggregates: eq_aggregates::{Pallet, Call, Storage},
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
    pub const ExistentialDeposit: Balance = 1;
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
}

parameter_types! {
    pub const BlockHashCount: u32 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
    pub const MinimumPeriod: u64 = 1;
    pub const UnsignedPriority: u64 = 100;
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
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

pub struct EqBuyoutMock;

impl EqBuyout<AccountId, Balance> for EqBuyoutMock {
    fn eq_buyout(_who: &AccountId, _amount: Balance) -> DispatchResult {
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
    ) -> Result<eq_primitives::AccountDistribution<Balance>, sp_runtime::DispatchError> {
        unimplemented!()
    }

    fn should_unreg_bailsman(
        _: &AccountId,
        _: &[(Asset, SignedBalance<Balance>)],
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
    type XcmRouter = ();
    type XcmToFee = ();
    type LocationToAccountId = ();
    type UniversalLocation = eq_primitives::mocks::UniversalLocationMock;
    type OrderAggregates = ();
    type UnixTime = TimeZeroDurationMock;
    type ToggleTransferOrigin = EnsureRoot<AccountId>;
    type ForceXcmTransferOrigin = EnsureRoot<AccountId>;
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

//type DummyValidatorId = u64;

pub struct BalanceCheckerMock {}

impl eq_primitives::balance::BalanceChecker<Balance, AccountId, EqBalances, SubaccountsManagerMock>
    for BalanceCheckerMock
{
    fn can_change_balance_impl(
        who: &AccountId,
        changes: &Vec<(Asset, eq_primitives::SignedBalance<Balance>)>,
        _: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        let res = changes.iter().all(|(asset, change)| match change {
            eq_primitives::SignedBalance::Positive(_) => true,
            eq_primitives::SignedBalance::Negative(change_value) => {
                let balance =
                    <Balances as BalanceGetter<AccountId, Balance>>::get_balance(who, asset);
                match balance {
                    eq_primitives::SignedBalance::Negative(_) => false,
                    eq_primitives::SignedBalance::Positive(balance_value) => {
                        balance_value >= *change_value
                    }
                }
            }
        });

        res.then(|| ())
            .ok_or_else(|| DispatchError::Other("Expected error"))
    }
}

parameter_types! {
    pub const MinVestedTransfer: Balance = 1_000_000_000;
    pub const BasicCurrencyGet: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
    pub const VestingModuleId: PalletId = PalletId(*b"eq/vestn");
}
pub type BasicCurrency = eq_primitives::balance_adapter::BalanceAdapter<
    Balance,
    eq_balances::Pallet<Test>,
    BasicCurrencyGet,
>;
impl eq_vesting::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Currency = BasicCurrency;
    type VestingAsset = BasicCurrencyGet;
    type BlockNumberToBalance = BlockNumberToBalance;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = ();
    type PalletId = VestingModuleId;
    type IsTransfersEnabled = Balances;
}

parameter_types! {
    pub Prefix: &'static [u8] = b"Pay RUSTs to the TEST account:";
    pub ClaimUnsignedPriority: u64 = 100;
}
ord_parameter_types! {
    pub const Six: u64 = 6;
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Vesting = Vesting;
    type Prefix = Prefix;
    type MoveClaimOrigin = frame_system::EnsureSignedBy<Six, u64>;
    type VestingAccountId = VestingAccountMock<AccountId>;
    type WeightInfo = ();
    type UnsignedPriority = ClaimUnsignedPriority;
    type Currency = BasicCurrency;
}

type Balances = eq_balances::Pallet<Test>;
type Vesting = eq_vesting::Pallet<Test>;

fn alice() -> secp256k1::SecretKey {
    secp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}
fn bob() -> secp256k1::SecretKey {
    secp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}
fn dave() -> secp256k1::SecretKey {
    secp256k1::SecretKey::parse(&keccak_256(b"Dave")).unwrap()
}
fn eve() -> secp256k1::SecretKey {
    secp256k1::SecretKey::parse(&keccak_256(b"Eve")).unwrap()
}
fn frank() -> secp256k1::SecretKey {
    secp256k1::SecretKey::parse(&keccak_256(b"Frank")).unwrap()
}

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
pub fn new_test_ext() -> sp_io::TestExternalities {
    OracleMock::init(vec![(asset::EQ, FixedI64::one())]);

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
            ],
        }
    .assimilate_storage(&mut t)
    .unwrap();

    // We use default for brevity, but you can configure as desired if needed.
    eq_balances::GenesisConfig::<Test> {
        balances: vec![],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(true)),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    eq_claim::GenesisConfig::<Test> {
        claims: vec![
            (eth(&alice()), 100, None, false),
            (eth(&dave()), 200, None, true),
            (eth(&eve()), 300, Some(42), true),
            (eth(&frank()), 400, Some(43), false),
        ],
        vesting: vec![(eth(&alice()), (50, 10, 1))],
    }
    .assimilate_storage(&mut t)
    .unwrap();
    t.into()
}

fn total_claims() -> Balance {
    100 + 200 + 300 + 400
}

#[test]
fn basic_setup_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(Claims::total(), total_claims());
        assert_eq!(Claims::claims(&eth(&alice())), Some(100));
        assert_eq!(Claims::claims(&eth(&dave())), Some(200));
        assert_eq!(Claims::claims(&eth(&eve())), Some(300));
        assert_eq!(Claims::claims(&eth(&frank())), Some(400));
        assert_eq!(Claims::claims(&EthereumAddress::default()), None);
        assert_eq!(Claims::vesting(&eth(&alice())), Some((50, 10, 1)));
    });
}

#[test]
fn serde_works() {
    let x = EthereumAddress(hex!["0123456789abcdef0123456789abcdef01234567"]);
    let y = serde_json::to_string(&x).unwrap();
    assert_eq!(y, "\"0x0123456789abcdef0123456789abcdef01234567\"");
    let z: EthereumAddress = serde_json::from_str(&y).unwrap();
    assert_eq!(x, z);
}

#[test]
fn claiming_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        assert_ok!(Claims::claim(
            RuntimeOrigin::none(),
            42,
            sig::<Test>(&alice(), &42u64.encode(), &[][..])
        ));
        assert_eq!(BasicCurrency::free_balance(&42), 50);
        assert_eq!(Vesting::vesting_balance(&42), Some(50));
        assert_eq!(Claims::total(), total_claims() - 100);
    });
}

#[test]
fn basic_claim_moving_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        assert_noop!(
            Claims::move_claim(RuntimeOrigin::signed(1), eth(&alice()), eth(&bob()), None),
            BadOrigin
        );
        assert_ok!(Claims::move_claim(
            RuntimeOrigin::signed(6),
            eth(&alice()),
            eth(&bob()),
            None
        ));
        assert_noop!(
            Claims::claim(
                RuntimeOrigin::none(),
                42,
                sig::<Test>(&alice(), &42u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim
        );
        assert_ok!(Claims::claim(
            RuntimeOrigin::none(),
            42,
            sig::<Test>(&bob(), &42u64.encode(), &[][..])
        ));
        assert_eq!(BasicCurrency::free_balance(&42), 50);
        assert_eq!(BasicCurrency::free_balance(&Vesting::account_id()), 50);
        assert_eq!(Vesting::vesting_balance(&42), Some(50));
        assert_eq!(Claims::total(), total_claims() - 100);
    });
}

#[test]
fn claim_attest_moving_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claims::move_claim(
            RuntimeOrigin::signed(6),
            eth(&dave()),
            eth(&bob()),
            None
        ));
        let s = sig::<Test>(&bob(), &42u64.encode(), get_statement_text());
        assert_ok!(Claims::claim_attest(
            RuntimeOrigin::none(),
            42,
            s,
            get_statement_text().to_vec()
        ));
        assert_eq!(BasicCurrency::free_balance(&42), 200);
    });
}

#[test]
fn attest_moving_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claims::move_claim(
            RuntimeOrigin::signed(6),
            eth(&eve()),
            eth(&bob()),
            Some(42)
        ));
        assert_ok!(Claims::attest(
            RuntimeOrigin::signed(42),
            get_statement_text().to_vec()
        ));
        assert_eq!(BasicCurrency::free_balance(&42), 300);
    });
}

#[test]
fn claiming_does_not_bypass_signing() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claims::claim(
            RuntimeOrigin::none(),
            42,
            sig::<Test>(&alice(), &42u64.encode(), &[][..])
        ));
        assert_noop!(
            Claims::claim(
                RuntimeOrigin::none(),
                42,
                sig::<Test>(&dave(), &42u64.encode(), &[][..])
            ),
            Error::<Test>::InvalidStatement,
        );
        assert_noop!(
            Claims::claim(
                RuntimeOrigin::none(),
                42,
                sig::<Test>(&eve(), &42u64.encode(), &[][..])
            ),
            Error::<Test>::InvalidStatement,
        );
        assert_ok!(Claims::claim(
            RuntimeOrigin::none(),
            42,
            sig::<Test>(&frank(), &42u64.encode(), &[][..])
        ));
    });
}

#[test]
fn attest_claiming_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        let s = sig::<Test>(&dave(), &42u64.encode(), &get_statement_text()[1..]);
        let r = Claims::claim_attest(
            RuntimeOrigin::none(),
            42,
            s.clone(),
            get_statement_text()[1..].to_vec(),
        );
        assert_noop!(r, Error::<Test>::InvalidStatement);

        let r = Claims::claim_attest(RuntimeOrigin::none(), 42, s, get_statement_text().to_vec());
        assert_noop!(r, Error::<Test>::SignerHasNoClaim);
        // ^^^ we use ecdsa_recover, so an invalid signature just results in a random signer id
        // being recovered, which realistically will never have a claim.

        let s = sig::<Test>(&dave(), &42u64.encode(), get_statement_text());
        assert_ok!(Claims::claim_attest(
            RuntimeOrigin::none(),
            42,
            s,
            get_statement_text().to_vec()
        ));
        assert_eq!(BasicCurrency::free_balance(&42), 200);
        assert_eq!(Claims::total(), total_claims() - 200);

        let s = sig::<Test>(&dave(), &42u64.encode(), get_statement_text());
        let r = Claims::claim_attest(RuntimeOrigin::none(), 42, s, get_statement_text().to_vec());
        assert_noop!(r, Error::<Test>::SignerHasNoClaim);
    });
}

#[test]
fn attesting_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        assert_noop!(
            Claims::attest(RuntimeOrigin::signed(69), get_statement_text().to_vec()),
            Error::<Test>::SenderHasNoClaim
        );
        assert_noop!(
            Claims::attest(
                RuntimeOrigin::signed(42),
                get_statement_text()[1..].to_vec()
            ),
            Error::<Test>::InvalidStatement
        );
        assert_ok!(Claims::attest(
            RuntimeOrigin::signed(42),
            get_statement_text().to_vec()
        ));
        assert_eq!(BasicCurrency::free_balance(&42), 300);
        assert_eq!(Claims::total(), total_claims() - 300);
    });
}

#[test]
fn claim_cannot_clobber_preclaim() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        // Alice's claim is 100
        assert_ok!(Claims::claim(
            RuntimeOrigin::none(),
            42,
            sig::<Test>(&alice(), &42u64.encode(), &[][..])
        ));
        assert_eq!(BasicCurrency::free_balance(&42), 50);
        assert_eq!(BasicCurrency::free_balance(&Vesting::account_id()), 50);
        assert_eq!(Vesting::vesting_balance(&42), Some(50));
        // Eve's claim is 300 through Account 42
        assert_ok!(Claims::attest(
            RuntimeOrigin::signed(42),
            get_statement_text().to_vec()
        ));
        assert_eq!(BasicCurrency::free_balance(&42), 50 + 300);
        assert_eq!(Claims::total(), total_claims() - 400);
    });
}

#[test]
fn valid_attest_transactions_are_free() {
    new_test_ext().execute_with(|| {
        let p = PrevalidateAttests::<Test>::new();
        let c = RuntimeCall::Claims(ClaimsCall::attest {
            statement: get_statement_text().to_vec(),
        });
        let di = c.get_dispatch_info();
        assert_eq!(di.pays_fee, Pays::No);
        let r = p.validate(&42, &c, &di, 20);
        assert_eq!(r, TransactionValidity::Ok(ValidTransaction::default()));
    });
}

#[test]
fn invalid_attest_transactions_are_recognised() {
    new_test_ext().execute_with(|| {
        let p = PrevalidateAttests::<Test>::new();
        let c = RuntimeCall::Claims(ClaimsCall::attest {
            statement: get_statement_text()[1..].to_vec(),
        });
        let di = c.get_dispatch_info();
        let r = p.validate(&42, &c, &di, 20);
        assert!(r.is_err());
        let c = RuntimeCall::Claims(ClaimsCall::attest {
            statement: get_statement_text()[1..].to_vec(),
        });
        let di = c.get_dispatch_info();
        let r = p.validate(&69, &c, &di, 20);
        assert!(r.is_err());
    });
}

#[test]
fn cannot_bypass_attest_claiming() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        let s = sig::<Test>(&dave(), &42u64.encode(), &[]);
        let r = Claims::claim(RuntimeOrigin::none(), 42, s.clone());
        assert_noop!(r, Error::<Test>::InvalidStatement);
    });
}

#[test]
fn add_claim_works() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Claims::mint_claim(RuntimeOrigin::signed(42), eth(&bob()), 200, None, false),
            sp_runtime::traits::BadOrigin,
        );
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        assert_noop!(
            Claims::claim(
                RuntimeOrigin::none(),
                69,
                sig::<Test>(&bob(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim,
        );
        assert_ok!(Claims::mint_claim(
            RuntimeOrigin::root(),
            eth(&bob()),
            200,
            None,
            false
        ));
        assert_eq!(Claims::total(), total_claims() + 200);
        assert_ok!(Claims::claim(
            RuntimeOrigin::none(),
            69,
            sig::<Test>(&bob(), &69u64.encode(), &[][..])
        ));
        assert_eq!(BasicCurrency::free_balance(&69), 200);
        assert_eq!(Vesting::vesting_balance(&69), None);
        assert_eq!(Claims::total(), total_claims());
    });
}

#[test]
fn add_claim_with_vesting_works() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Claims::mint_claim(
                RuntimeOrigin::signed(42),
                eth(&bob()),
                200,
                Some((50, 10, 1)),
                false
            ),
            sp_runtime::traits::BadOrigin,
        );
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        assert_noop!(
            Claims::claim(
                RuntimeOrigin::none(),
                69,
                sig::<Test>(&bob(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim,
        );
        assert_ok!(Claims::mint_claim(
            RuntimeOrigin::root(),
            eth(&bob()),
            200,
            Some((50, 10, 1)),
            false
        ));
        assert_ok!(Claims::claim(
            RuntimeOrigin::none(),
            69,
            sig::<Test>(&bob(), &69u64.encode(), &[][..])
        ));
        assert_eq!(BasicCurrency::free_balance(&69), 150);
        assert_eq!(BasicCurrency::free_balance(&Vesting::account_id()), 50);
        assert_eq!(Vesting::vesting_balance(&69), Some(50));

        // Make sure we can not transfer the vested balance.
        assert_err!(
            BasicCurrency::transfer(&69, &80, 180, ExistenceRequirement::AllowDeath),
            DispatchError::Other("Expected error")
        );
    });
}

#[test]
fn add_claim_with_statement_works() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Claims::mint_claim(RuntimeOrigin::signed(42), eth(&bob()), 200, None, true),
            sp_runtime::traits::BadOrigin,
        );
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        let signature = sig::<Test>(&bob(), &69u64.encode(), get_statement_text());
        assert_noop!(
            Claims::claim_attest(
                RuntimeOrigin::none(),
                69,
                signature.clone(),
                get_statement_text().to_vec()
            ),
            Error::<Test>::SignerHasNoClaim
        );
        assert_ok!(Claims::mint_claim(
            RuntimeOrigin::root(),
            eth(&bob()),
            200,
            None,
            true
        ));
        assert_noop!(
            Claims::claim_attest(RuntimeOrigin::none(), 69, signature.clone(), vec![],),
            Error::<Test>::SignerHasNoClaim
        );
        assert_ok!(Claims::claim_attest(
            RuntimeOrigin::none(),
            69,
            signature.clone(),
            get_statement_text().to_vec()
        ));
        assert_eq!(BasicCurrency::free_balance(&69), 200);
    });
}

#[test]
fn origin_signed_claiming_fail() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        assert_err!(
            Claims::claim(
                RuntimeOrigin::signed(42),
                42,
                sig::<Test>(&alice(), &42u64.encode(), &[][..])
            ),
            sp_runtime::traits::BadOrigin,
        );
    });
}

#[test]
fn double_claiming_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        assert_ok!(Claims::claim(
            RuntimeOrigin::none(),
            42,
            sig::<Test>(&alice(), &42u64.encode(), &[][..])
        ));
        assert_noop!(
            Claims::claim(
                RuntimeOrigin::none(),
                42,
                sig::<Test>(&alice(), &42u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim
        );
    });
}

#[test]
fn claiming_while_vested_doesnt_work() {
    new_test_ext().execute_with(|| {
        // A user is already vested
        assert_ok!(<Test as Config>::Vesting::add_vesting_schedule(
            &69,
            total_claims(),
            100,
            10
        ));
        <Test as Config>::Currency::make_free_balance_be(&69, total_claims());
        assert_eq!(BasicCurrency::free_balance(&69), total_claims());
        assert_ok!(Claims::mint_claim(
            RuntimeOrigin::root(),
            eth(&bob()),
            200,
            Some((50, 10, 1)),
            false
        ));
        // New total
        assert_eq!(Claims::total(), total_claims() + 200);

        // They should not be able to claim
        assert_noop!(
            Claims::claim(
                RuntimeOrigin::none(),
                69,
                sig::<Test>(&bob(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::VestedBalanceExists,
        );
    });
}

#[test]
fn mint_claim_correct_error() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Claims::mint_claim(
                RuntimeOrigin::root(),
                eth(&bob()),
                1,
                Some((50, 10, 1)),
                false
            ),
            Error::<Test>::InvalidStatement
        );
    });
}

#[test]
fn non_sender_sig_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        assert_noop!(
            Claims::claim(
                RuntimeOrigin::none(),
                42,
                sig::<Test>(&alice(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim
        );
    });
}

#[test]
fn non_claimant_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_eq!(BasicCurrency::free_balance(&42), 0);
        assert_noop!(
            Claims::claim(
                RuntimeOrigin::none(),
                42,
                sig::<Test>(&bob(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim
        );
    });
}

#[test]
fn real_eth_sig_works() {
    new_test_ext().execute_with(|| {
			// "Pay RUSTs to the TEST account:2a00000000000000"
			let sig = hex!["444023e89b67e67c0562ed0305d252a5dd12b2af5ac51d6d3cb69a0b486bc4b3191401802dc29d26d586221f7256cd3329fe82174bdf659baea149a40e1c495d1c"];
			let sig = EcdsaSignature(sig);
			let who = 42u64.using_encoded(to_ascii_hex);
			let signer = Claims::eth_recover(&sig, &who, &[][..]).unwrap();
			assert_eq!(signer.0, hex!["6d31165d5d932d571f3b44695653b46dcc327e84"]);
		});
}

#[test]
fn ecdsa_sig_comparison() {
    let sig_a = EcdsaSignature(hex!["444023e89b67e67c0562ed0305d252a5dd12b2af5ac51d6d3cb69a0b486bc4b3191401802dc29d26d586221f7256cd3329fe82174bdf659baea149a40e1c495d1c"]);
    let sig_b = EcdsaSignature(hex!["444023e89b67e67c0562ed0305d252a5dd12b2af5ac51d6d3cb69a0b486bc4b3191401802dc29d26d586221f7256cd3329fe82174bdf659baea149a40e1c495d1c"]);
    let sig_c = EcdsaSignature(hex!["ab4e9ab31a149aa456765c26decf553ddb3aba15f2cae7e3c002040c97e261777f861c89a0d340ffa6185645e2bbdf4396b77b18da9d226cb9232c5f59403e431b"]);

    let eq_check = sig_a == sig_b;
    let diff_check = sig_a != sig_c;

    assert!(eq_check);
    assert!(diff_check);
}

#[test]
fn ecdsa_sig_as_ref() {
    let sig= EcdsaSignature(hex!["ab4e9ab31a149aa456765c26decf553ddb3aba15f2cae7e3c002040c97e261777f861c89a0d340ffa6185645e2bbdf4396b77b18da9d226cb9232c5f59403e431b"]);
    let sig_ref_a: &[u8; 65] = sig.as_ref();
    let sig_ref_b: &[u8] = sig.as_ref();

    assert_eq!(sig_ref_a, &sig.0);
    assert_eq!(sig_ref_b, &sig.0[..]);
}

#[test]
fn real_eth_sig_works_ci() {
    new_test_ext().execute_with(|| {
			// "Pay RUSTs to the TEST account:2a00000000000000"
			let sig = hex!["ab4e9ab31a149aa456765c26decf553ddb3aba15f2cae7e3c002040c97e261777f861c89a0d340ffa6185645e2bbdf4396b77b18da9d226cb9232c5f59403e431b"];
			let sig = EcdsaSignature(sig);
			let who = [100, 52, 51, 53, 57, 51, 99, 55, 49, 53, 102, 100, 100, 51, 49, 99, 54, 49, 49, 52, 49, 97, 98, 100, 48, 52, 97, 57, 57, 102, 100, 54, 56, 50, 50, 99, 56, 53, 53, 56, 56, 53, 52, 99, 99, 100, 101, 51, 57, 97, 53, 54, 56, 52, 101, 55, 97, 53, 54, 100, 97, 50, 55, 100];
			let signer = Claims::eth_recover(&sig, &who, &[][..]).unwrap();
			assert_eq!(signer.0, hex!["5A4447BB16Ae41B00051feda82990F88da7EC2A9"]);
		});
}

#[test]
fn validate_unsigned_works() {
    use sp_runtime::traits::ValidateUnsigned;
    let source = sp_runtime::transaction_validity::TransactionSource::External;

    new_test_ext().execute_with(|| {
        let current_block = 2u32;
        System::set_block_number(current_block);

        let expected_priority = UnsignedPriority::get();

        assert_eq!(
            <Pallet<Test>>::validate_unsigned(
                source,
                &ClaimsCall::claim {
                    dest: 1,
                    ethereum_signature: sig::<Test>(&alice(), &1u64.encode(), &[][..])
                }
            ),
            Ok(ValidTransaction {
                priority: expected_priority,
                requires: vec![],
                provides: vec![("claims", eth(&alice())).encode()],
                longevity: TransactionLongevity::max_value(),
                propagate: true,
            })
        );
        assert_eq!(
            <Pallet<Test>>::validate_unsigned(
                source,
                &ClaimsCall::claim {
                    dest: 0,
                    ethereum_signature: EcdsaSignature([0; 65])
                }
            ),
            InvalidTransaction::Custom(ValidityError::InvalidEthereumSignature.into()).into(),
        );
        assert_eq!(
            <Pallet<Test>>::validate_unsigned(
                source,
                &ClaimsCall::claim {
                    dest: 1,
                    ethereum_signature: sig::<Test>(&bob(), &1u64.encode(), &[][..])
                }
            ),
            InvalidTransaction::Custom(ValidityError::SignerHasNoClaim.into()).into(),
        );
        let s = sig::<Test>(&dave(), &1u64.encode(), get_statement_text());
        let call = ClaimsCall::claim_attest {
            dest: 1,
            ethereum_signature: s,
            statement: get_statement_text().to_vec(),
        };
        assert_eq!(
            <Pallet<Test>>::validate_unsigned(source, &call),
            Ok(ValidTransaction {
                priority: expected_priority,
                requires: vec![],
                provides: vec![("claims", eth(&dave())).encode()],
                longevity: TransactionLongevity::max_value(),
                propagate: true,
            })
        );
        assert_eq!(
            <Pallet<Test>>::validate_unsigned(
                source,
                &ClaimsCall::claim_attest {
                    dest: 1,
                    ethereum_signature: EcdsaSignature([0; 65]),
                    statement: get_statement_text().to_vec()
                }
            ),
            InvalidTransaction::Custom(ValidityError::InvalidEthereumSignature.into()).into(),
        );

        let s = sig::<Test>(&bob(), &1u64.encode(), get_statement_text());
        let call = ClaimsCall::claim_attest {
            dest: 1,
            ethereum_signature: s,
            statement: get_statement_text().to_vec(),
        };
        assert_eq!(
            <Pallet<Test>>::validate_unsigned(source, &call),
            InvalidTransaction::Custom(ValidityError::SignerHasNoClaim.into()).into(),
        );

        let s = sig::<Test>(&dave(), &1u64.encode(), get_statement_text());
        let call = ClaimsCall::claim_attest {
            dest: 1,
            ethereum_signature: s,
            statement: get_statement_text()[1..].to_vec(),
        };
        assert_eq!(
            <Pallet<Test>>::validate_unsigned(source, &call),
            InvalidTransaction::Custom(ValidityError::SignerHasNoClaim.into()).into(),
        );

        let s = sig::<Test>(&dave(), &1u64.encode(), &get_statement_text()[1..]);
        let call = ClaimsCall::claim_attest {
            dest: 1,
            ethereum_signature: s,
            statement: get_statement_text()[1..].to_vec(),
        };
        assert_eq!(
            <Pallet<Test>>::validate_unsigned(source, &call),
            InvalidTransaction::Custom(ValidityError::InvalidStatement.into()).into(),
        );
    });
}

#[test]
fn err_conversion_to_u8() {
    let err_0 = u8::from(ValidityError::InvalidEthereumSignature);
    let err_2 = u8::from(ValidityError::InvalidStatement);

    assert_eq!(0_u8, err_0);
    assert_eq!(2_u8, err_2);
}

#[test]
fn prevalidate_attest_default() {
    let x = PrevalidateAttests::<Test>::default();
    assert_eq!(x, PrevalidateAttests::<Test>(sp_std::marker::PhantomData))
}
