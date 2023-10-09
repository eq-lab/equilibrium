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
use crate as eq_oracle;
use core::marker::PhantomData;
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::{asset, asset::AssetType};
use eq_primitives::{SignedBalance, TotalAggregates};
use equilibrium_curve_amm::traits::CurveAmm as CurveAmmTrait;
use equilibrium_curve_amm::PoolInfo;
use financial_primitives::OnPriceSet;
use frame_support::traits::Everything;
use frame_support::weights::Weight;
use frame_support::{dispatch::DispatchError, parameter_types};
use frame_system::EnsureRoot;
use sp_core::{sr25519::Signature, H256};
use sp_runtime::{
    testing::{Header, TestXt},
    traits::{BlakeTwo256, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify},
};
use sp_runtime::{Percent, Permill};
use std::cell::RefCell;
use std::collections::HashMap;
use substrate_fixed::types::I64F64;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type Balance = eq_primitives::balance::Balance;

use core::convert::{TryFrom, TryInto};

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        EqOracle: eq_oracle::{Pallet, Call, Storage, Event<T>},
        EqWhitelists: eq_whitelists::{Pallet, Call, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage},
        EqAssets: eq_assets::{Pallet, Call, Storage, Event},
        Financial: financial_pallet::{Pallet, Call, Storage, Event<T>},
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
    pub const MinimumPeriod: u64 = 1;
    pub const UnsignedPriority: eq_primitives::UnsignedPriorityPair = (0, 1_000_000);
}
impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
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

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl eq_whitelists::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WhitelistManagementOrigin = EnsureRoot<AccountId>;
    type OnRemove = ();
    type WeightInfo = ();
}

type Extrinsic = TestXt<RuntimeCall, ()>;
type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

impl frame_system::offchain::SigningTypes for Test {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Test
where
    RuntimeCall: From<LocalCall>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = Extrinsic;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
where
    RuntimeCall: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: RuntimeCall,
        _public: <Signature as Verify>::Signer,
        _account: AccountId,
        nonce: u64,
    ) -> Option<(RuntimeCall, <Extrinsic as ExtrinsicT>::SignaturePayload)> {
        Some((call, (nonce, ())))
    }
}

parameter_types! {
    pub const PriceTimeout: u64 = 1;
    pub const MedianPriceTimeout: u64 = 60 * 60 * 2;
    pub const MainAsset: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
}

pub struct FinancialMock;
impl OnPriceSet for FinancialMock {
    type Asset = Asset;
    type Price = I64F64;
    fn on_price_set(_asset: Self::Asset, _value: Self::Price) -> Result<(), DispatchError> {
        Ok(())
    }
}

pub struct CurveAmmStub;
impl CurveAmmTrait for CurveAmmStub {
    type AssetId = Asset;
    type Number = ();
    type Balance = eq_primitives::balance::Balance;
    type AccountId = AccountId;

    fn pool_count() -> u32 {
        0
    }

    fn pool(
        _id: u32,
    ) -> Option<PoolInfo<Self::AccountId, Self::AssetId, Self::Number, Self::Balance>> {
        None
    }

    fn create_pool(
        _who: &Self::AccountId,
        _assets: Vec<Self::AssetId>,
        _amplification: Self::Number,
        _fee: Permill,
        _admin_fee: Permill,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn add_liquidity(
        _who: &Self::AccountId,
        _pool_id: u32,
        _amounts: Vec<Self::Balance>,
        _min_mint_amount: Self::Balance,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn exchange(
        _who: &Self::AccountId,
        _pool_id: u32,
        _i: u32,
        _j: u32,
        _dx: Self::Balance,
        _min_dy: Self::Balance,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn remove_liquidity(
        _who: &Self::AccountId,
        _pool_id: u32,
        _amount: Self::Balance,
        _min_amounts: Vec<Self::Balance>,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn remove_liquidity_imbalance(
        _who: &Self::AccountId,
        _pool_id: u32,
        _amounts: Vec<Self::Balance>,
        _max_burn_amount: Self::Balance,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn remove_liquidity_one_coin(
        _who: &Self::AccountId,
        _pool_id: u32,
        _token_amount: Self::Balance,
        _i: u32,
        _min_amount: Self::Balance,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn get_dy(
        _pool_id: u32,
        _i: u32,
        _j: u32,
        _dx: Self::Balance,
    ) -> Result<Self::Balance, DispatchError> {
        Ok(Self::Balance::default())
    }

    fn get_virtual_price(_pool_id: u32) -> Result<Self::Balance, DispatchError> {
        Ok(Self::Balance::default())
    }

    fn withdraw_admin_fees(_who: &Self::AccountId, _pool_id: u32) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn set_enable_state(_pool_id: u32, _is_enabled: bool) -> DispatchResultWithPostInfo {
        Ok(().into())
    }
}

impl eq_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type MainAsset = MainAsset;
    type OnNewAsset = ();
    type WeightInfo = ();
}

impl FinancialSystemTrait for FinancialMock {
    type Asset = Asset;
    type AccountId = AccountId;
    fn recalc_inner() -> Result<(), DispatchError> {
        Ok(())
    }
    fn recalc_asset_inner(_asset: Self::Asset) -> Result<(), DispatchError> {
        Ok(())
    }
    fn recalc_portfolio_inner(
        _account_id: Self::AccountId,
        _z_score: u32,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

parameter_types! {
    pub const LpPriceBlockTimeout: u64 = 10u64;
    pub const UnsignedLifetimeInBlocks: u32 = 5;
    pub const FinancialRecalcPeriodBlocks: u64  = (1000 * 60 * 60 * 4) as u64 / 6000;
}

pub struct XbasePriceMock;
impl eq_primitives::xdot_pool::XBasePrice<Asset, Balance, FixedI64> for XbasePriceMock {
    type XdotPoolInfo = ();

    /// Returns xbase price in relation to base for pool with corresponding `pool_id`.
    fn get_xbase_virtual_price(
        _pool_info: &Self::XdotPoolInfo,
        _ttm: Option<u64>,
    ) -> Result<FixedI64, DispatchError> {
        Ok(FixedI64::one())
    }

    fn get_lp_virtual_price(
        _pool_info: &Self::XdotPoolInfo,
        _ttm: Option<u64>,
    ) -> Result<FixedI64, DispatchError> {
        Ok(FixedI64::one())
    }

    fn get_pool(
        _pool_id: eq_primitives::xdot_pool::PoolId,
    ) -> Result<Self::XdotPoolInfo, DispatchError> {
        Ok(())
    }
}

thread_local! {
    pub static USER_GROUPS: RefCell<Vec<(UserGroup,AccountId)>>  = Default::default();
    pub static BALANCES: RefCell<HashMap<(AccountId, Asset), substrate_fixed::types::I64F64>> = RefCell::new(HashMap::new());
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
    ) -> DispatchResult {
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
    ) -> DispatchResult {
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
    fn get_total(_user_group: UserGroup, asset: Asset) -> TotalAggregates<Balance> {
        let collateral = if asset == asset::BTC { 100 } else { 0 };

        TotalAggregates {
            collateral,
            debt: 0,
        }
    }
}

pub struct Balances;

impl financial_primitives::BalanceAware for Balances {
    type AccountId = AccountId;
    type Asset = Asset;
    type Balance = substrate_fixed::types::I64F64;

    fn balances(
        account_id: &Self::AccountId,
        assets: &[Asset],
    ) -> Result<Vec<Self::Balance>, DispatchError> {
        BALANCES.with(|b| {
            Ok(assets
                .iter()
                .map(|&a| {
                    b.borrow()
                        .get(&(*account_id, a))
                        .copied()
                        .unwrap_or(substrate_fixed::types::I64F64::default())
                })
                .collect())
        })
    }
}

impl financial_pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type UnixTime = pallet_timestamp::Pallet<Test>;
    type PriceCount = ();
    type PricePeriod = ();
    type ReturnType = ();
    type VolCorType = ();
    type Asset = Asset;
    type FixedNumberBits = i128;
    type FixedNumber = substrate_fixed::types::I64F64;
    type Price = substrate_fixed::types::I64F64;
    type Balances = Balances;
}

impl Config for Test {
    type FinancialRecalcPeriodBlocks = FinancialRecalcPeriodBlocks;
    type AssetGetter = eq_assets::Pallet<Test>;
    type AuthorityId = crypto::TestAuthId;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Whitelist = eq_whitelists::Pallet<Self>;
    type UnixTime = pallet_timestamp::Pallet<Self>;
    type FinMetricsRecalcToggleOrigin = EnsureRoot<AccountId>;
    type MedianPriceTimeout = MedianPriceTimeout;
    type PriceTimeout = PriceTimeout;
    type UnsignedPriority = UnsignedPriority;
    type Balance = eq_primitives::balance::Balance;
    type OnPriceSet = FinancialMock;
    type FinancialSystemTrait = FinancialMock;
    type FinancialAssetRemover = financial_pallet::Pallet<Test>;
    type CurveAmm = CurveAmmStub;
    type WeightInfo = ();
    type LpPriceBlockTimeout = LpPriceBlockTimeout;
    type UnsignedLifetimeInBlocks = UnsignedLifetimeInBlocks;
    type XBasePrice = XbasePriceMock;
    type EqDotPrice = ();
    type Aggregates = AggregatesMock;
    type AggregatesAssetRemover = ();
    type LendingAssetRemoval = ();
}

pub type ModuleOracle = Pallet<Test>;

pub type ModuleTimestamp = pallet_timestamp::Pallet<Test>;
pub type ModuleWhitelist = eq_whitelists::Pallet<Test>;
pub type ModuleSystem = frame_system::Pallet<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
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
    .assimilate_storage(&mut t)
    .unwrap();

    eq_oracle::GenesisConfig {
        prices: vec![],
        update_date: 0,
    }
    .assimilate_storage::<Test>(&mut t)
    .unwrap();

    t.into()
}
