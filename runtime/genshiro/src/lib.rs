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

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]
#![forbid(unsafe_code)]
#![deny(warnings)]
// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub use chainbridge;
use codec::Encode;
use core::marker::PhantomData;
pub use eq_assets;
pub use eq_balances;
pub use eq_bridge;
pub use eq_distribution;
pub use eq_lending;
pub use eq_multisig_sudo;
pub use eq_primitives;
use eq_primitives::{balance::EqCurrency, Aggregates, UserGroup};
pub use eq_rate;
pub use eq_treasury;
use eq_utils::XcmBalance;
pub use eq_vesting;
use eq_xcm::ParaId;
use financial_pallet::FinancialSystemTrait;
use financial_primitives::{CalcReturnType, CalcVolatilityType};
use frame_support::pallet_prelude::Get;
use frame_support::traits::UnixTime;
use frame_support::traits::{ExistenceRequirement, WithdrawReasons};
pub use frame_support::{
    construct_runtime, debug,
    dispatch::{DispatchClass, DispatchError, DispatchResult},
    match_types, parameter_types,
    traits::{
        Contains, Everything, Imbalance, KeyOwnerProofSystem, Nothing, Randomness, StorageMapShim,
    },
    weights::{
        constants::{BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_REF_TIME_PER_SECOND},
        ConstantMultiplier, IdentityFee, Weight, WeightToFee,
    },
    PalletId, StorageValue,
};
use frame_system as system;
use frame_system::limits::{BlockLength, BlockWeights};
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use polkadot_parachain::primitives::Sibling;
use polkadot_runtime_common::SlowAdjustingFeeUpdate;
use polkadot_runtime_constants::weights::RocksDbWeight;
use sp_api::impl_runtime_apis;
use sp_arithmetic::{FixedI64, FixedPointNumber, PerThing, Percent};
use sp_consensus_aura::{sr25519::AuthorityId as AuraId, SlotDuration};
use sp_core::ConstU32;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::traits::{
    self, AccountIdConversion, AccountIdLookup, BlakeTwo256, Block as BlockT, Convert, OpaqueKeys,
};
use sp_runtime::transaction_validity::{
    TransactionPriority, TransactionSource, TransactionValidity,
};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::SaturatedConversion;
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys, ApplyExtrinsicResult, FixedI128, Perquintill,
};
pub use sp_runtime::{Perbill, Permill};
use sp_std::{
    convert::{TryFrom, TryInto},
    prelude::*,
};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use system::EnsureRoot;
use transaction_payment::Multiplier;
use xcm::v3::{
    InteriorMultiLocation, Junction::*, Junctions::*, MultiLocation, NetworkId, OriginKind,
    Weight as XcmWeight, Xcm,
};
use xcm_builder::{
    AllowKnownQueryResponses, AllowSubscriptionsFrom, EnsureXcmOrigin, FixedWeightBounds,
    ParentIsPreset, SiblingParachainConvertsVia,
};
use xcm_executor::traits::WithOriginFilter;
use xcm_executor::{traits::ConvertOrigin, Config, XcmExecutor};

use common_runtime::{
    mocks::{BalanceAwareMock, XbasePriceMock},
    *,
};

// Tests for XCM integration
#[cfg(test)]
mod xcm_test;
// Weights used in the runtime.
pub mod weights;

pub const ONE_TOKEN: Balance = eq_utils::ONE_TOKEN as Balance;

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("Gens-parachain"),
    impl_name: create_runtime_str!("Gens-parachain"),
    authoring_version: 10,
    spec_version: 18, // 95a056f737dcc6727fcacf1362f897edc082ea44
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    state_version: 0,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

pub const MILLISECS_PER_BLOCK: u64 = 12000;
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub aura: Aura,
            pub eq_rate: EqRate,
        }
    }
}

parameter_types! {
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
    pub const Offset: BlockNumber = 0;
    pub const Period: BlockNumber = if cfg!(feature = "production") {
        600
    } else {
        10
    };
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = <Self as system::Config>::AccountId;
    type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionManager = eq_session_manager::Pallet<Runtime>;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = opaque::SessionKeys;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
    type PalletsOrigin = OriginCaller;
}

/// We assume that ~10% of the block weight is consumed by `on_initalize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 0.5 seconds of compute with a 6 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
    WEIGHT_REF_TIME_PER_SECOND.saturating_div(2),
    polkadot_primitives::MAX_POV_SIZE as u64,
);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 250;
    pub const Version: RuntimeVersion = VERSION;
    pub RuntimeBlockLength: BlockLength =
        BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
        .base_block(BlockExecutionWeight::get())
        .for_class(DispatchClass::all(), |weights| {
            weights.base_extrinsic = ExtrinsicBaseWeight::get();
        })
        .for_class(DispatchClass::Normal, |weights| {
            weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
        })
        .for_class(DispatchClass::Operational, |weights| {
            weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
            // Operational transactions have some extra reserved space, so that they
            // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
            weights.reserved = Some(
                MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
            );
        })
        .avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
        .build_or_panic();
    pub const SS58Prefix: u8 = 67;
}

pub struct CallFilter;
impl frame_support::traits::Contains<RuntimeCall> for CallFilter {
    #[allow(unused_variables)]
    fn contains(c: &RuntimeCall) -> bool {
        #[cfg(feature = "production")]
        match (eq_migration::Migration::<Runtime>::exists(), c) {
            (false, RuntimeCall::EqMultisigSudo(proposal_call)) => match proposal_call {
                eq_multisig_sudo::Call::propose { call } => match *call.clone() {
                    RuntimeCall::PolkadotXcm(_) => true, // allow send xcm from msig
                    RuntimeCall::Utility(utility_call) => {
                        // allow send xcm batch from msig
                        match utility_call {
                            pallet_utility::Call::batch { calls, .. }
                            | pallet_utility::Call::batch_all { calls, .. } => {
                                calls.iter().all(|call| {
                                    if let RuntimeCall::PolkadotXcm(_) = call {
                                        true
                                    } else {
                                        Self::contains(call)
                                    }
                                })
                            }
                            _ => true,
                        }
                    }
                    _ => Self::contains(&**call),
                },
                _ => true,
            },
            (false, RuntimeCall::Utility(utility_call)) => match utility_call {
                pallet_utility::Call::batch { calls, .. }
                | pallet_utility::Call::batch_all { calls, .. } => {
                    calls.iter().all(|call| Self::contains(call))
                }
                _ => true,
            },
            (false, RuntimeCall::EqBalances(call)) => match call {
                eq_balances::Call::deposit { .. } | eq_balances::Call::burn { .. } => false,
                _ => true,
            },
            // (false, Call::Oracle(eq_oracle::Call::set_fin_metrics_recalc_enabled { .. })) => false,
            (false, RuntimeCall::EqRate(eq_rate::Call::set_now_millis_offset { .. })) => false,
            (false, RuntimeCall::Vesting(eq_vesting::Call::force_vested_transfer { .. })) => false,
            // XCM disallowed
            (_, &RuntimeCall::PolkadotXcm(_)) => false,
            (false, _) => true,

            // only system and sudo are allowed during migration
            (true, &RuntimeCall::ParachainSystem(_)) => true,
            (true, &RuntimeCall::System(_)) => true,
            // (true, &Call::Sudo(_)) => true,
            (true, &RuntimeCall::Timestamp(_)) => true,
            (true, &RuntimeCall::EqMultisigSudo(_)) => true,

            // all other pallets are disallowed during migration
            (true, _) => false,
        }
        #[cfg(not(feature = "production"))]
        true
    }
}

#[allow(unused_parens)]
impl system::Config for Runtime {
    type BaseCallFilter = CallFilter;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = RuntimeBlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = RuntimeBlockLength;
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = Index;
    type BlockNumber = BlockNumber;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = RocksDbWeight;
    type Version = Version;
    type OnNewAccount = (eq_rate::Pallet<Runtime>);
    type OnKilledAccount = (eq_rate::Pallet<Runtime>);
    type AccountData = AccountData<Balance>;
    type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
    type PalletInfo = PalletInfo;
    type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
    pub const MaxAuthorities: u32 = 100_000;
}

impl aura::Config for Runtime {
    type AuthorityId = AuraId;
    type DisabledValidators = ();
    type MaxAuthorities = MaxAuthorities;
}

pub struct FilterPrices;

impl FilterPrices {
    fn filter_prices(who: &AccountId) {
        Oracle::filter_prices_from(who);
    }
}

impl eq_whitelists::OnRemove<AccountId> for FilterPrices {
    fn on_remove(who: &AccountId) {
        Self::filter_prices(who);
    }
}

impl eq_whitelists::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WhitelistManagementOrigin = EnsureRoot<AccountId>;
    type WeightInfo = weights::pallet_whitelists::WeightInfo<Runtime>;
    type OnRemove = FilterPrices;
}

impl eq_assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MainAsset = BasicCurrencyGet;
    type OnNewAsset = FinancialPalletOnNewAsset;
    type AssetManagementOrigin = EnsureRoot<AccountId>;
    type WeightInfo = weights::pallet_assets::WeightInfo<Runtime>;
}

parameter_types! {
    pub const MigrationsPerBlock: u16 = 2_000;
}

impl eq_migration::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MigrationsPerBlock = MigrationsPerBlock;
    type WeightInfo = eq_migration::weights::EqWeight<Runtime>;
}

impl eq_oracle::Config for Runtime {
    type FinMetricsRecalcToggleOrigin = EnsureRoot<AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type AuthorityId = eq_oracle::crypto::AuthId;
    type RuntimeCall = RuntimeCall;
    type Balance = Balance;
    type UnixTime = EqRate;
    type Whitelist = Whitelists;
    type MedianPriceTimeout = MedianPriceTimeout;
    type PriceTimeout = PriceTimeout;
    type OnPriceSet = financial_pallet::Pallet<Runtime>;
    type FinancialSystemTrait = financial_pallet::Pallet<Runtime>;
    type FinancialRecalcPeriodBlocks = FinancialRecalcPeriodBlocks;
    type FinancialAssetRemover = financial_pallet::Pallet<Runtime>;
    type UnsignedPriority = OracleUnsignedPriority;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type Aggregates = EqAggregates;
    type AggregatesAssetRemover = EqAggregates;
    type WeightInfo = weights::pallet_oracle::WeightInfo<Runtime>;
    type CurveAmm = equilibrium_curve_amm::Pallet<Runtime>;
    type LpPriceBlockTimeout = LpPriceBlockTimeout;
    type XBasePrice = XbasePriceMock<Asset, Balance, FixedI64>;
    type UnsignedLifetimeInBlocks = UnsignedLifetimeInBlocks;
    type LendingAssetRemoval = EqLending;
    type EqDotPrice = ();
}

parameter_types! {
    pub const BailsmenUnsignedPriority: TransactionPriority = TransactionPriority::min_value();
    pub const MaxBailsmenToDistribute: u32 = 20;
    pub const QueueLengthWeightConstant: u32 = 5;
}

impl eq_bailsman::Config for Runtime {
    type PalletId = BailsmanModuleId;
    type PriceGetter = Oracle;
    type UnixTime = EqRate;
    type Balance = eq_primitives::balance::Balance;
    type BalanceGetter = EqBalances;
    type EqCurrency = EqBalances;
    type Aggregates = EqAggregates;
    type RuntimeEvent = RuntimeEvent;
    type MinTempBalanceUsd = MinTempBalanceUsd;
    type MinimalCollateral = MinimalCollateral;
    type AssetGetter = EqAssets;
    type WeightInfo = weights::pallet_bailsman::WeightInfo<Runtime>;
    type MarginCallManager = EqMarginCall;
    type SubaccountsManager = Subaccounts;
    type AuthorityId = eq_rate::ed25519::AuthorityId;
    type ValidatorOffchainBatcher = EqRate;
    type UnsignedPriority = BailsmenUnsignedPriority;
    type MaxBailsmenToDistribute = MaxBailsmenToDistribute;
    type QueueLengthWeightConstant = QueueLengthWeightConstant;
}

parameter_types! {
    pub const PriceStepCount: u32 = 5;
    pub const PenaltyFee: Balance = 10 * ONE_TOKEN;
    pub const DexUnsignedPriority: TransactionPriority = TransactionPriority::min_value();
}

impl eq_dex::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DeleteOrderOrigin = EnsureRoot<AccountId>;
    type UpdateAssetCorridorOrigin = EnsureRoot<AccountId>;
    type PriceStepCount = PriceStepCount;
    type PenaltyFee = PenaltyFee;
    type DexUnsignedPriority = DexUnsignedPriority;
    type WeightInfo = weights::pallet_dex::WeightInfo<Runtime>;
    type ValidatorOffchainBatcher = eq_rate::Pallet<Runtime>;
}

parameter_types! {
    pub InitialMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(2, 10);
    pub MaintenanceMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(1, 10);
    pub CriticalMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 100);
    pub MaintenancePeriod: u64 = 60*60*24;
}

impl eq_margin_call::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type UnixTime = eq_rate::Pallet<Runtime>;
    type BailsmenManager = Bailsman;
    type BalanceGetter = EqBalances;
    type PriceGetter = Oracle;
    type InitialMargin = InitialMargin;
    type MaintenanceMargin = MaintenanceMargin;
    type CriticalMargin = CriticalMargin;
    type MaintenancePeriod = MaintenancePeriod;
    type OrderAggregates = EqDex;
    type AssetGetter = EqAssets;
    type SubaccountsManager = Subaccounts;
    type WeightInfo = weights::pallet_margin_call::WeightInfo<Runtime>;
}

parameter_types! {
    pub const MaxSignatories: u32 = 10;
}

impl eq_multisig_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = weights::pallet_multisig_sudo::WeightInfo<Runtime>;
}

parameter_types! {
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Aura;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

pub const EXISTENSIAL_DEPOSIT: Balance = ONE_TOKEN / 10;
pub const EXISTENSIAL_DEPOSIT_BASIC: Balance = 100 * ONE_TOKEN;

parameter_types! {
    pub const ExistentialDeposit: Balance = EXISTENSIAL_DEPOSIT; // 0.1 USD
    pub const ExistentialDepositBasic: Balance = EXISTENSIAL_DEPOSIT_BASIC; // 100 GENS
    pub const BasicCurrencyGet: eq_primitives::asset::Asset = eq_primitives::asset::GENS;
    pub const RelayCurrencyGet: eq_primitives::asset::Asset = eq_primitives::asset::KSM;
}

impl eq_aggregates::Config for Runtime {
    type Balance = Balance;
    type BalanceGetter = EqBalances;
}

pub struct FallbackWeightToFee;
impl sp_runtime::traits::Convert<(Asset, XcmWeight), Option<eq_utils::XcmBalance>>
    for FallbackWeightToFee
{
    fn convert((asset, weight): (Asset, XcmWeight)) -> Option<eq_utils::XcmBalance> {
        use eq_primitives::asset;
        use eq_xcm::fees::*;

        Some(match asset {
            asset::KSM => {
                let weight = multiply_by_rational_weight(
                    weight,
                    polkadot::BaseXcmWeight::get(),
                    crate::BaseXcmWeight::get(),
                );
                kusama::WeightToFee::weight_to_fee(&weight)
            }
            asset::MOVR => {
                let weight = multiply_by_rational_weight(
                    weight,
                    moonbeam::BaseXcmWeight::get(),
                    crate::BaseXcmWeight::get(),
                );
                moonbeam::movr::WeightToFee::weight_to_fee(&weight)
            }
            asset::HKO => {
                let weight = multiply_by_rational_weight(
                    weight,
                    parallel::BaseXcmWeight::get(),
                    crate::BaseXcmWeight::get(),
                );
                parallel::hko::WeightToFee::weight_to_fee(&weight)
            }
            asset::KAR => {
                let weight = multiply_by_rational_weight(
                    weight,
                    acala::BaseXcmWeight::get(),
                    crate::BaseXcmWeight::get(),
                );
                acala::kar::WeightToFee::weight_to_fee(&weight)
            }
            asset::KUSD => {
                let weight = multiply_by_rational_weight(
                    weight,
                    acala::BaseXcmWeight::get(),
                    crate::BaseXcmWeight::get(),
                );
                acala::kusd::WeightToFee::weight_to_fee(&weight)
            }
            asset::KBTC => {
                let weight = multiply_by_rational_weight(
                    weight,
                    interlay::BaseXcmWeight::get(),
                    crate::BaseXcmWeight::get(),
                );
                interlay::kbtc::WeightToFee::weight_to_fee(&weight)
            }
            asset::EQD => crate::fee::XcmWeightToFee::weight_to_fee(&weight),
            asset::GENS => crate::fee::XcmWeightToFee::weight_to_fee(&weight) * 100,
            _ => return None,
        })
    }
}

pub struct XcmToFee;
impl<'xcm, Call>
    Convert<
        (eq_primitives::asset::Asset, MultiLocation, &'xcm Xcm<Call>),
        Option<(eq_primitives::asset::Asset, XcmBalance)>,
    > for XcmToFee
{
    fn convert(
        (asset, destination, message): (
            eq_primitives::asset::Asset,
            MultiLocation,
            &'xcm Xcm<Call>,
        ),
    ) -> Option<(eq_primitives::asset::Asset, XcmBalance)> {
        use eq_primitives::{asset, xcm_origins};
        use eq_xcm::fees::*;
        Some(match (destination, asset) {
            (xcm_origins::RELAY, asset::KSM) => (asset::KSM, kusama::XcmToFee::convert(message)),
            #[cfg(test)]
            (xcm_origins::RELAY, asset::GENS) => (asset::GENS, 0),
            #[cfg(test)]
            (xcm_origins::RELAY, asset::EQD) => (asset::EQD, 0),

            (xcm_origins::ksm::PARACHAIN_MOONRIVER, asset::MOVR) => {
                (asset::MOVR, moonbeam::movr::XcmToFee::convert(message))
            }

            (xcm_origins::ksm::PARACHAIN_HEIKO, asset::HKO) => {
                (asset::HKO, parallel::hko::XcmToFee::convert(message))
            }
            (xcm_origins::ksm::PARACHAIN_HEIKO, asset::GENS) => {
                (asset::GENS, parallel::gens::XcmToFee::convert(message))
            }

            (xcm_origins::ksm::PARACHAIN_KARURA, asset::KAR) => {
                (asset::KAR, acala::kar::XcmToFee::convert(message))
            }
            (xcm_origins::ksm::PARACHAIN_KARURA, asset::KUSD) => {
                (asset::KUSD, acala::kusd::XcmToFee::convert(message))
            }
            (xcm_origins::ksm::PARACHAIN_KARURA, asset::LKSM) => {
                (asset::LKSM, acala::lksm::XcmToFee::convert(message))
            }
            (xcm_origins::ksm::PARACHAIN_KARURA, asset::GENS) => {
                (asset::GENS, acala::gens::XcmToFee::convert(message))
            }
            (xcm_origins::ksm::PARACHAIN_KARURA, asset::EQD) => {
                (asset::EQD, acala::eqd::XcmToFee::convert(message))
            }

            (xcm_origins::ksm::PARACHAIN_KINTSUGI, asset::KBTC) => {
                (asset::KBTC, interlay::kbtc::XcmToFee::convert(message))
            }

            (xcm_origins::ksm::PARACHAIN_SHIDEN, asset::SDN) => {
                (asset::SDN, astar::sdn::XcmToFee::convert(message))
            }
            (xcm_origins::ksm::PARACHAIN_SHIDEN, asset::GENS) => {
                (asset::GENS, astar::gens::XcmToFee::convert(message))
            }
            (xcm_origins::ksm::PARACHAIN_SHIDEN, asset::EQD) => {
                (asset::EQD, astar::eqd::XcmToFee::convert(message))
            }

            (xcm_origins::ksm::PARACHAIN_BIFROST, asset::BNC) => {
                (asset::BNC, bifrost::bnc::XcmToFee::convert(message))
            }
            // Cannot calculate fee so is is hardcoded to 0
            // (xcm_origins::ksm::PARACHAIN_BIFROST, asset::GENS) => {
            //     (asset::GENS, bifrost::gens::XcmToFee::convert(message))
            // }
            // (xcm_origins::ksm::PARACHAIN_BIFROST, asset::EQD) => {
            //     (asset::EQD, bifrost::eqd::XcmToFee::convert(message))
            // }
            _ => return None,
        })
    }
}

impl eq_balances::Config for Runtime {
    type AssetGetter = eq_assets::Pallet<Runtime>;
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;

    // order matters: heavy checks must be at the end
    type BalanceChecker = (
        eq_subaccounts::Pallet<Runtime>,
        eq_balances::locked_balance_checker::CheckLocked<Runtime>,
        eq_lending::Pallet<Runtime>,
        eq_bailsman::Pallet<Runtime>,
    );

    type ExistentialDeposit = ExistentialDeposit;
    type ExistentialDepositBasic = ExistentialDepositBasic;
    type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
    type Aggregates = eq_aggregates::Pallet<Runtime>;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = Subaccounts;
    type BailsmenManager = Bailsman;
    type UpdateTimeManager = eq_rate::Pallet<Runtime>;
    type BailsmanModuleId = BailsmanModuleId;
    type ModuleId = BalancesModuleId;
    type XcmRouter = XcmRouter;
    type XcmToFee = XcmToFee;
    type LocationToAccountId = LocationToAccountId;
    type PriceGetter = Oracle;
    type OrderAggregates = EqDex;
    type AccountStore = System;
    type UnixTime = eq_rate::Pallet<Runtime>;
    type ForceXcmTransferOrigin = EnsureRoot<AccountId>;
    type ToggleTransferOrigin = EnsureRoot<AccountId>;
    type ParachainId = ParachainInfo;
    type UniversalLocation = UniversalLocation;
}

pub type BasicCurrency = eq_primitives::balance_adapter::BalanceAdapter<
    Balance,
    eq_balances::Pallet<Runtime>,
    BasicCurrencyGet,
>;

pub type RelayCurrency = eq_primitives::balance_adapter::BalanceAdapter<
    Balance,
    eq_balances::Pallet<Runtime>,
    RelayCurrencyGet,
>;

parameter_types! {
    pub const TransactionBaseFee: Balance = 1;
    pub const TransactionByteFee: Balance = 1;
    pub const OperationalFeeMultiplier: u8 = 5;
    pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
    /// The adjustment variable of the runtime. Higher values will cause `TargetBlockFullness` to
    /// change the fees more rapidly.
    pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(3, 100_000);
    /// Minimum amount of the multiplier. This value cannot be too low. A test case should ensure
    /// that combined with `AdjustmentVariable`, we can recover from the minimum.
    /// See `multiplier_can_grow_from_zero`.
    pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000u128);
}

type EqImbalance = eq_balances::NegativeImbalance<Balance>;
pub struct DealWithFees<FeeAsset>(PhantomData<FeeAsset>);
impl<FeeAsset: Get<eq_primitives::asset::Asset>> frame_support::traits::OnUnbalanced<EqImbalance>
    for DealWithFees<FeeAsset>
{
    fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = EqImbalance>) {
        if let Some(fees) = fees_then_tips.next() {
            // for fees, 20% to treasury, 80% to author
            let amount = fees.peek();
            let _ = <EqBalances as eq_primitives::balance::EqCurrency<AccountId, Balance>>::deposit_creating(
                &EqTreasury::account_id(),
                FeeAsset::get(),
                amount,
                false,
                None
            );
        }
    }

    fn on_unbalanced(fees: EqImbalance) {
        let amount = fees.peek();
        let _ = <EqBalances as eq_primitives::balance::EqCurrency<AccountId, Balance>>::deposit_creating(
            &EqTreasury::account_id(),
            FeeAsset::get(),
            amount,
            false,
            None
        );
    }
}

/// Fee-related.
pub mod fee {
    use frame_support::weights::{
        WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
    };
    use smallvec::smallvec;
    pub use sp_runtime::Perbill;

    /// The block saturation level. Fees will be updates based on this value.
    pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

    /// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
    /// node's balance type.
    ///
    /// This should typically create a mapping between the following ranges:
    ///   - [0, `MAXIMUM_BLOCK_WEIGHT`]
    ///   - [Balance::min, Balance::max]
    pub struct WeightToFee;
    impl WeightToFeePolynomial for WeightToFee {
        type Balance = crate::Balance;
        fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
            smallvec![WeightToFeeCoefficient {
                degree: 1,
                negative: false,
                coeff_frac: Perbill::from_percent(10),
                coeff_integer: 0,
            }]
        }
    }

    const DOLLAR: crate::XcmBalance = eq_utils::ONE_TOKEN;
    const CENT: crate::XcmBalance = DOLLAR / 100;

    /// Convert Weight of xcm message into USD amount of fee
    pub struct XcmWeightToFee;
    impl WeightToFeePolynomial for XcmWeightToFee {
        type Balance = crate::XcmBalance;
        fn polynomial() -> WeightToFeeCoefficients<crate::XcmBalance> {
            let p = 215 * CENT;
            let q = crate::XcmBalance::from(crate::ExtrinsicBaseWeight::get().ref_time());
            smallvec![WeightToFeeCoefficient {
                coeff_integer: p / q,
                coeff_frac: Perbill::from_rational(p % q, q),
                negative: false,
                degree: 1,
            }]
        }
    }
}

impl transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction =
        transaction_payment::CurrencyAdapter<BasicCurrency, DealWithFees<BasicCurrencyGet>>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type WeightToFee = fee::WeightToFee;
    type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
}

impl authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
    type EventHandler = ();
}

#[cfg(not(feature = "production"))]
impl sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
}

parameter_types! {}

const PRICE_TIMEOUT_IN_SECONDS: u64 = 60; // 1 minute

parameter_types! {
    pub const MedianPriceTimeout: u64 = 60 * 60 * 1; // 1 hours
    pub const PriceTimeout: u64 = PRICE_TIMEOUT_IN_SECONDS;
    pub const MinimalCollateral: Balance = 1000 * ONE_TOKEN; // 1000 USD
    pub const OracleUnsignedPriority: UnsignedPriorityPair = (TransactionPriority::min_value(), 10_000);
    pub const MinSurplus: Balance =  100 * ONE_TOKEN; // 100 Eq
    pub const MinTempBalanceUsd: Balance = 50 * ONE_TOKEN; // 50 USD
    pub const TreasuryModuleId: PalletId = PalletId(*b"eq/trsry");
    pub const BailsmanModuleId: PalletId = PalletId(*b"eq/bails");

    pub const LiquidityFarmingModuleId: PalletId = PalletId(*b"eq/liqfm");
    pub const LendingModuleId: PalletId = PalletId(*b"eq/lendr");
    pub const LpPriceBlockTimeout: u64 = PRICE_TIMEOUT_IN_SECONDS * 1000 / MILLISECS_PER_BLOCK;
    pub const UnsignedLifetimeInBlocks: u32 = 5;
    pub const FinancialRecalcPeriodBlocks: BlockNumber  = (1000 * 60 * 60 * 4) / MILLISECS_PER_BLOCK as BlockNumber; // 4 hours in blocks
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
}

parameter_types! {
    pub BuyFee: Permill = PerThing::from_rational::<u32>(1, 1000);
    pub SellFee: Permill = PerThing::from_rational::<u32>(1, 1000);
    pub const MinAmountToBuyout: Balance = 100 * ONE_TOKEN;
}

impl eq_treasury::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type Balance = Balance;
    type PriceGetter = Oracle;
    type BalanceGetter = EqBalances;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type PalletId = TreasuryModuleId;
    type BuyFee = BuyFee;
    type SellFee = SellFee;
    type UnixTime = eq_rate::Pallet<Runtime>;
    type WeightInfo = weights::pallet_treasury::WeightInfo<Runtime>;
    type MinAmountToBuyout = MinAmountToBuyout;
}

parameter_types! {
    pub const MinVestedTransfer: Balance = 1000 * ONE_TOKEN; // 1000 GENS
    pub const VestingModuleId: PalletId = PalletId(*b"eq/vestn");
}

pub struct VestingAccount;
impl Get<AccountId> for VestingAccount {
    fn get() -> AccountId {
        VestingModuleId::get().into_account_truncating()
    }
}

pub struct BlockNumberToBalance {}

impl sp_runtime::traits::Convert<BlockNumber, Balance> for BlockNumberToBalance {
    fn convert(block_number: BlockNumber) -> Balance {
        block_number as Balance
    }
}

type VestingInstance = eq_vesting::Instance1;
impl eq_vesting::Config<VestingInstance> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = BasicCurrency;
    type BlockNumberToBalance = BlockNumberToBalance;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = weights::pallet_vesting::WeightInfo<Runtime>;
    type PalletId = VestingModuleId;
    type IsTransfersEnabled = eq_balances::Pallet<Runtime>;
}

parameter_types! {
    // Maximum number of points for each asset that Financial Pallet can store
    pub const PriceCount: u32 = 30;
    // Duration of the price period in minutes
    pub const PricePeriod: u32 = 24 * 60;
    // CalcReturnType used by FinancialPallet's calc* extrinsics
    pub const ReturnType: u32 = CalcReturnType::Log.into_u32();
    // CalcVolatilityType used by FinancialPallet's calc* extrinsics
    pub const VolCorType: i64 = CalcVolatilityType::Regular.into_i64();
}

impl financial_pallet::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    // In most cases you should use pallet_timestamp as a UnixTime trait implementation
    type UnixTime = eq_rate::Pallet<Runtime>;

    // Specify parameters here you defined before
    type PriceCount = PriceCount;
    type PricePeriod = PricePeriod;
    type ReturnType = ReturnType;
    type VolCorType = VolCorType;

    // Construct fixed number type that substrate-fixed crate provides.
    // This type defines valid range of values and precision
    // for all calculations Financial Pallet performs.
    type FixedNumber = substrate_fixed::types::I64F64;
    // FixedNumber underlying type should be defined explicitly because
    // rust compiler could not determine it on its own.
    type FixedNumberBits = i128;

    // Specify here system wide type used for balance values.
    // You should also provide convertions to and from FixedNumber
    // which is used for all calculations under the hood.
    type Price = substrate_fixed::types::I64F64;

    // Asset type specific to your system. It can be as simple as
    // enum. See example below.
    type Asset = eq_primitives::asset::Asset;
    // Provide BalanceAware trait implementation.
    // Financial Pallet uses it to check user balances.
    type Balances = BalanceAwareMock<AccountId, eq_primitives::asset::Asset>;
}

pub struct FinancialPalletOnNewAsset;
impl OnNewAsset for FinancialPalletOnNewAsset {
    fn on_new_asset(asset: eq_primitives::asset::Asset, prices: Vec<sp_runtime::FixedI64>) {
        use frame_support::StorageMap;

        if !prices.is_empty() {
            let mut price_log = financial_primitives::capvec::CapVec::new(
                <Runtime as financial_pallet::Config>::PriceCount::get(),
            );

            prices
                .iter()
                .rev()
                .map(|price| eq_utils::fixedi64_to_i64f64(*price))
                .for_each(|price| {
                    price_log.push(price);
                });

            financial_pallet::PriceLogs::<Runtime>::insert(
                asset,
                financial_pallet::PriceLog {
                    latest_timestamp: financial_pallet::Duration::from(
                        eq_rate::Pallet::<Runtime>::now(),
                    ),
                    prices: price_log,
                },
            );
        }

        // May be that new prices are breaking financial pallet recalculation,
        // but it is very rare case
        #[allow(unused_must_use)]
        let _ = Financial::recalc_inner();
    }
}

#[cfg(feature = "runtime-benchmarks")]
type DistriBenchInstance = eq_distribution::Instance16;
#[cfg(feature = "runtime-benchmarks")]
impl eq_distribution::Config<DistriBenchInstance> for Runtime {
    type PalletId = TreasuryModuleId;
    type VestingSchedule = Vesting;
    type VestingAccountId = VestingAccount;
    type AssetGetter = EqAssets;
    type EqCurrency = EqBalances;
    type ManagementOrigin = EnsureRoot<AccountId>;
    type WeightInfo = weights::pallet_distribution::WeightInfo<Runtime>;
}

type TreasuryInstance = eq_distribution::Instance5;
impl eq_distribution::Config<TreasuryInstance> for Runtime {
    type PalletId = TreasuryModuleId;
    type VestingSchedule = Vesting;
    type VestingAccountId = VestingAccount;
    type AssetGetter = EqAssets;
    type EqCurrency = EqBalances;
    type ManagementOrigin = EnsureRoot<AccountId>;
    type WeightInfo = weights::pallet_distribution::WeightInfo<Runtime>;
}

parameter_types! {
    pub const RateUnsignedPriority: TransactionPriority = TransactionPriority::min_value();

    pub TreasuryFee: Permill = Permill::from_percent(1);
    pub const WeightFeeTreasury: u32 = 80;
    pub const WeightFeeValidator: u32 = 20;

    pub BaseBailsmanFee: Permill = Permill::from_percent(1);
    pub BaseLenderFee: Permill =  Permill::from_rational(5u32, 1000u32);
    pub LenderPart: Permill = Permill::from_percent(30);

    pub RiskLowerBound: FixedI128 = FixedI128::saturating_from_rational(1, 2);
    pub RiskUpperBound: FixedI128 = FixedI128::saturating_from_integer(2);
    pub RiskNSigma: FixedI128 = FixedI128::saturating_from_integer(5);
    pub RiskRho: FixedI128 = FixedI128::saturating_from_rational(7, 10);
    pub Alpha: FixedI128 = FixedI128::from(15);
}

/// Special structure that holds weights from pallets
pub struct WeightInfoGetter;
impl eq_primitives::bailsman_redistribute_weight::RedistributeWeightInfo for WeightInfoGetter {
    fn redistribute(z: u32) -> Weight {
        use eq_bailsman::WeightInfo;

        weights::pallet_bailsman::WeightInfo::<Runtime>::redistribute(z)
    }
}

#[allow(unused_parens)]
impl eq_rate::Config for Runtime {
    type Balance = Balance;
    type BalanceGetter = EqBalances;
    type BalanceRemover = EqBalances;
    type AuthorityId = eq_rate::ed25519::AuthorityId;
    type MinSurplus = MinSurplus;
    type BailsmanManager = Bailsman;
    type MinTempBailsman = MinTempBalanceUsd;
    type UnixTime = timestamp::Pallet<Runtime>;
    type EqBuyout = eq_treasury::Pallet<Runtime>;
    type BailsmanModuleId = BailsmanModuleId;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type SubaccountsManager = Subaccounts;
    type MarginCallManager = EqMarginCall;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type UnsignedPriority = RateUnsignedPriority;
    type WeightInfo = weights::pallet_rate::WeightInfo<Runtime>;
    type RedistributeWeightInfo = WeightInfoGetter;
    type PriceGetter = Oracle;
    type Aggregates = EqAggregates;
    type RiskLowerBound = RiskLowerBound;
    type RiskUpperBound = RiskUpperBound;
    type RiskNSigma = RiskNSigma;
    type Alpha = Alpha;
    type Financial = Financial;
    type FinancialStorage = Financial;
    type TreasuryFee = TreasuryFee;
    type WeightFeeTreasury = WeightFeeTreasury;
    type WeightFeeValidator = WeightFeeValidator;
    type BaseBailsmanFee = BaseBailsmanFee;
    type BaseLenderFee = BaseLenderFee;
    type LenderPart = LenderPart;
    type TreasuryModuleId = TreasuryModuleId;
    type LendingModuleId = LendingModuleId;
    type LendingPoolManager = EqLending;
    type LendingAssetRemoval = EqLending;
    type AutoReinitToggleOrigin = EnsureRoot<AccountId>;
}

impl eq_session_manager::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = <Self as system::Config>::AccountId;
    type RegistrationChecker = pallet_session::Pallet<Runtime>;
    type ValidatorIdOf = sp_runtime::traits::ConvertInto;
    type ValidatorsManagementOrigin = EnsureRoot<AccountId>;
    type WeightInfo = weights::pallet_session_manager::WeightInfo<Runtime>;
}

impl eq_subaccounts::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetGetter = EqAssets;
    type BalanceGetter = EqBalances;
    type Aggregates = EqAggregates;
    type EqCurrency = EqBalances;
    type BailsmenManager = Bailsman;
    type PriceGetter = Oracle;
    type Whitelist = Whitelists;
    type UpdateTimeManager = EqRate;
    type WeightInfo = weights::pallet_subaccounts::WeightInfo<Runtime>;
    type IsTransfersEnabled = EqBalances;
}

impl eq_lending::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type Balance = Balance;
    type BalanceGetter = EqBalances;
    type Aggregates = eq_aggregates::Pallet<Runtime>;
    type BailsmanManager = Bailsman;
    type SubaccountsManager = Subaccounts;
    type ModuleId = LendingModuleId;
    type EqCurrency = EqBalances;
    type UnixTime = EqRate;
    type PriceGetter = Oracle;
    type WeightInfo = weights::pallet_lending::WeightInfo<Runtime>;
}

impl system::offchain::SigningTypes for Runtime {
    type Public = <Signature as traits::Verify>::Signer;
    type Signature = Signature;
}

impl<C> system::offchain::SendTransactionTypes<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = UncheckedExtrinsic;
}

impl<LocalCall> system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    fn create_transaction<C: system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: RuntimeCall,
        public: <Signature as traits::Verify>::Signer,
        account: AccountId,
        nonce: Index,
    ) -> Option<(
        RuntimeCall,
        <UncheckedExtrinsic as traits::Extrinsic>::SignaturePayload,
    )> {
        let period = BlockHashCount::get()
            .checked_next_power_of_two()
            .map(|c| c / 2)
            .unwrap_or(2) as u64;

        let current_block = System::block_number()
            .saturated_into::<u64>()
            // The `System::block_number` is initialized with `n+1`,
            // so the actual block number is `n`.
            .saturating_sub(1);

        // let tip = 0;
        let extra: SignedExtra = (
            system::CheckSpecVersion::<Runtime>::new(),
            system::CheckTxVersion::<Runtime>::new(),
            system::CheckGenesis::<Runtime>::new(),
            system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
            system::CheckNonce::<Runtime>::from(nonce),
            system::CheckWeight::<Runtime>::new(),
            transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
            eq_rate::reinit_extension::ReinitAccount::<Runtime, CallsWithReinit>::new(),
            eq_treasury::CheckBuyout::<Runtime>::new(),
        );

        let raw_payload = SignedPayload::new(call, extra)
            .map_err(|e| {
                log::warn!("SignedPayload error: {:?}", e);
            })
            .ok()?;

        let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
        //let address = Indices::unlookup(account);
        let (call, extra, _) = raw_payload.deconstruct();
        let address = sp_runtime::MultiAddress::Id(account);
        Some((call, (address, signature, extra)))
    }
}

parameter_types! {
    pub const ChainId: u8 = 1;
}

impl chainbridge::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = BasicCurrency;
    type Balance = Balance;
    type BalanceGetter = EqBalances;
    type AdminOrigin = system::EnsureRoot<Self::AccountId>;
    type Proposal = RuntimeCall;
    type ChainIdentity = ChainId;
    type WeightInfo = weights::pallet_chainbridge::WeightInfo<Runtime>;
}

parameter_types! {
    pub const GensHashId: chainbridge::ResourceId = [14u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    pub const GensNativeTokenId: chainbridge::ResourceId = [0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x7au8, 0x05u8, 0xc5u8, 0x1fu8, 0x15u8, 0xd3u8, 0x66u8, 0xacu8, 0x77u8, 0xbcu8, 0x86u8, 0x67u8, 0x21u8, 0x66u8, 0x83u8, 0x61u8, 0x00u8];
}

parameter_types! {
    pub const EthHashId: chainbridge::ResourceId = [11u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    pub const EthNativeTokenId: chainbridge::ResourceId = [0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0xe7u8, 0xafu8, 0x8cu8, 0xdbu8, 0xa2u8, 0x34u8, 0xffu8, 0xeeu8, 0xddu8, 0xccu8, 0xbbu8, 0xaau8, 0x34u8, 0x58u8, 0x79u8, 0x87u8, 0x00u8];
}

parameter_types! {
    pub const CrvHashId: chainbridge::ResourceId = [13u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    pub const CrvNativeTokenId: chainbridge::ResourceId = [0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0xe5u8, 0x4du8, 0xd1u8, 0xf1u8, 0x1eu8, 0x2fu8, 0xd2u8, 0x47u8, 0x4au8, 0xf6u8, 0x4fu8, 0x48u8, 0x7eu8, 0x91u8, 0x1bu8, 0x59u8, 0x00u8];
}

impl eq_bridge::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type BridgeOrigin = chainbridge::EnsureBridge<Runtime>;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type BridgeManagementOrigin = EnsureRoot<AccountId>;
    type WeightInfo = weights::pallet_bridge::WeightInfo<Runtime>;
}

//////////////////////////////////////////////////////////////////////////////
// 	Cumulus pallets
//////////////////////////////////////////////////////////////////////////////

impl cumulus_pallet_aura_ext::Config for Runtime {}

parameter_types! {
    pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
    pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
}

impl cumulus_pallet_parachain_system::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SelfParaId = ParachainInfo;
    type DmpMessageHandler = DmpQueue;
    type ReservedDmpWeight = ReservedDmpWeight;
    type OutboundXcmpMessageSource = XcmpQueue;
    type XcmpMessageHandler = XcmpQueue;
    type ReservedXcmpWeight = ReservedXcmpWeight;
    type OnSystemEvent = ();
    type CheckAssociatedRelayNumber = cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;
}

impl parachain_info::Config for Runtime {}

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
    // Convertion to relay-chain sovereign account.
    ParentIsPreset<AccountId>,
    // Convertion to sibling parachain sovereign account.
    SiblingParachainConvertsVia<Sibling, AccountId>,
    // Straight up local `AccountId32` origins just alias directly to `AccountId`.
    // We expect messages only from `NetworkId::Any` and `NetworkId::Kusama`
    eq_xcm::origins::AccountIdConversion<AccountId>,
);

/// Means for transacting assets on this chain or through bridge.
pub type LocalAssetTransactor = eq_xcm::assets::EqCurrencyAdapter<
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // Balance type on chain
    Balance,
    // Operations on balances
    EqBalances,
    // Mathes MultiLocation to Genshiro assets
    EqAssets,
    // Transfers assets to standalone & Get ResourceId from asset
    EqBridge,
    // Do a simple punn to convert an AccountId32 MultiLocation into a native chain account ID:
    LocationToAccountId,
    // We don't track any teleports.
    (),
>;

pub struct TransactIsNotAllowed;

impl<Origin> ConvertOrigin<Origin> for TransactIsNotAllowed {
    fn convert_origin(
        origin: impl Into<MultiLocation>,
        _kind: OriginKind,
    ) -> Result<Origin, MultiLocation> {
        Err(origin.into())
    }
}

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = TransactIsNotAllowed;

parameter_types! {
    // One XCM operation is 1_000_000 weight - almost certainly a conservative estimate.
    pub BaseXcmWeight: XcmWeight = XcmWeight::from_parts(1_000_000, 0);
    pub const MaxInstructions: u32 = 100;
    pub const RelayNetwork: Option<NetworkId> = Some(NetworkId::Kusama);
    pub UniversalLocation: InteriorMultiLocation =
        X2(GlobalConsensus(RelayNetwork::get().unwrap()), Parachain(<ParachainInfo as Get<ParaId>>::get().into()));
}

pub struct TreasuryAccount;
impl Get<AccountId> for TreasuryAccount {
    fn get() -> AccountId {
        TreasuryModuleId::get().into_account_truncating()
    }
}

pub type EqTrader = eq_xcm::assets::EqTrader<
    AccountId,
    Balance,
    EqAssets,
    EqBalances,
    Oracle,
    TreasuryAccount,
    fee::XcmWeightToFee,
    FallbackWeightToFee,
>;

match_types! {
    pub type TrustedOrigins: impl Contains<MultiLocation> = {
        &eq_primitives::xcm_origins::RELAY
        | &eq_primitives::xcm_origins::ksm::PARACHAIN_MOONRIVER
        | &eq_primitives::xcm_origins::ksm::PARACHAIN_HEIKO
        | &eq_primitives::xcm_origins::ksm::PARACHAIN_KARURA
        | &eq_primitives::xcm_origins::ksm::PARACHAIN_SHIDEN
        | &eq_primitives::xcm_origins::ksm::PARACHAIN_BIFROST
    };
}

pub type Barrier = (
    eq_xcm::barrier::AllowReserveAssetDepositedFrom<EqAssets, TrustedOrigins>,
    eq_xcm::barrier::AllowReserveTransferAssetsFromAccountId,
    AllowKnownQueryResponses<PolkadotXcm>,
    AllowSubscriptionsFrom<TrustedOrigins>,
);

pub struct XcmConfig;
impl Config for XcmConfig {
    type RuntimeCall = RuntimeCall;
    type XcmSender = XcmRouter;
    // How to withdraw and deposit an asset.
    type AssetTransactor = LocalAssetTransactor;
    type OriginConverter = XcmOriginToTransactDispatchOrigin;
    type IsReserve = MultiNativeAsset;
    type IsTeleporter = NoTeleport;
    type Barrier = Barrier;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type Trader = EqTrader;
    type ResponseHandler = PolkadotXcm;
    type AssetTrap = PolkadotXcm;
    type AssetClaims = PolkadotXcm;
    type SubscriptionService = PolkadotXcm;
    type PalletInstancesInfo = AllPalletsWithSystem; // QueryPallet don't pass Barrier anyway
    type MaxAssetsIntoHolding = ConstU32<8>;
    type AssetLocker = ();
    type AssetExchanger = ();
    type FeeManager = ();
    type MessageExporter = ();
    type UniversalLocation = UniversalLocation;
    type UniversalAliases = Nothing;
    type CallDispatcher = WithOriginFilter<Nothing>;
    type SafeCallFilter = Nothing;
}

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
#[cfg(not(test))]
pub type XcmRouter = (
    // use UMP to communicate with the relay chain:
    cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm, ()>,
    // use XCMP to comminicate with other parachains via relay:
    XcmpQueue,
);

#[cfg(test)]
pub type XcmRouter = xcm_test::XcmRouterMock;

pub type LocalOriginToLocation = eq_xcm::origins::LocalOriginToLocation<RuntimeOrigin, AccountId>;

impl pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmRouter = XcmRouter;
    type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmExecuteFilter = Nothing;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmTeleportFilter = Nothing;
    type XcmReserveTransferFilter = Everything; // We don't use xcm pallet calls
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    type Currency = BasicCurrency;
    type UniversalLocation = UniversalLocation;
    type AdminOrigin = EnsureRoot<AccountId>;
    type CurrencyMatcher = ();
    type TrustedLockers = Nothing;
    type SovereignAccountOf = LocationToAccountId;
    type MaxLockers = ConstU32<8>;
    type WeightInfo = pallet_xcm::TestWeightInfo;
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type ChannelInfo = ParachainSystem;
    type VersionWrapper = PolkadotXcm;
    type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
    type ControllerOrigin = EnsureRoot<AccountId>;
    type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
    type PriceForSiblingDelivery = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const PotId: PalletId = PalletId(*b"PotStake");
    pub const MaxCandidates: u32 = 1000;
    pub const MinCandidates: u32 = 5;
    pub const SessionLength: BlockNumber = 6 * HOURS;
    pub const MaxInvulnerables: u32 = 100;
}

/// We allow root to execute privileged collator selection operations.
pub type CollatorSelectionUpdateOrigin = EnsureRoot<AccountId>;

impl pallet_collator_selection::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = RelayCurrency;
    type UpdateOrigin = CollatorSelectionUpdateOrigin;
    type PotId = PotId;
    type MaxCandidates = MaxCandidates;
    type MinCandidates = MinCandidates;
    type MaxInvulnerables = MaxInvulnerables;
    // should be a multiple of session or things will get inconsistent
    type KickThreshold = Period;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
    type ValidatorRegistration = Session;
    type WeightInfo = (); // weights::pallet_collator_selection::WeightInfo<Runtime>;
}

use eq_primitives::{
    asset::{Asset, AssetXcmData, OnNewAsset},
    balance::AccountData,
    balance_number::EqFixedU128,
    curve_number::{CurveNumber, CurveNumberConvert},
    TransferReason, UnsignedPriorityPair,
};
use sp_std::prelude::Vec;

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = common_runtime::opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: system::{Pallet, Call, Config, Storage, Event<T>},
        ParachainSystem: cumulus_pallet_parachain_system::{
            Pallet, Call, Config, Storage, Inherent, Event<T>, ValidateUnsigned,
        },
        Utility: pallet_utility::{Pallet, Call, Event},
        Timestamp: timestamp::{Pallet, Call, Storage, Inherent},
        ParachainInfo: parachain_info::{Pallet, Storage, Config},
        EqSessionManager: eq_session_manager::{Pallet, Call, Storage, Event<T>, Config<T>,},

        // Collator support. the order of these 4 are important and shall not change.
        Authorship: authorship::{Pallet, Storage},
        CollatorSelection: pallet_collator_selection::{Pallet, Call, Storage, Event<T>, Config<T>},
        Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>},
        Aura: aura::{Pallet, Config<T>},
        AuraExt: cumulus_pallet_aura_ext::{Pallet, Storage, Config},

        EqAssets: eq_assets::{Pallet, Call, Config<T>, Storage, Event}, // Assets genesis must be built first
        Oracle: eq_oracle::{Pallet, Call, Storage, Event<T>, Config, ValidateUnsigned},
        EqTreasury: eq_distribution::<Instance5>::{Pallet, Call, Storage, Config},
        Treasury: eq_treasury::{Pallet, Call, Storage, Config, Event<T>},
        EqBalances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        // ..... //
        EqRate: eq_rate::{Pallet, Storage, Call, ValidateUnsigned},

        TransactionPayment: transaction_payment::{Pallet, Storage, Event<T>},
        // Sudo: sudo::{Pallet, Call, Config<T>, Storage, Event<T>},
        Bailsman: eq_bailsman::{Pallet, Call, Config<T>, Storage, Event<T>, ValidateUnsigned},
        Whitelists: eq_whitelists::{Pallet, Call, Storage, Event<T>, Config<T>,},

        Vesting: eq_vesting::<Instance1>::{Pallet, Call, Storage, Event<T>, Config<T>},
        EqAggregates: eq_aggregates::{Pallet, Storage},
        Subaccounts: eq_subaccounts::{Pallet, Call, Storage, Event<T>, Config<T>},
        Financial: financial_pallet::{Pallet, Call, Storage, Config<T>, Event<T>},
        ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>, Config<T>},
        EqBridge: eq_bridge::{Pallet, Call, Storage, Event<T>, Config<T>},
        EqMultisigSudo: eq_multisig_sudo::{Pallet, Call, Storage, Config<T>, Event<T>},
        EqMarginCall: eq_margin_call::{Pallet, Call, Storage, Event<T>},
        EqDex: eq_dex::{Pallet, Call, Storage, Event<T>, Config, ValidateUnsigned},
        EqLending: eq_lending::{Pallet, Call, Storage, Event<T>, Config<T>},
        Migration: eq_migration::{Pallet, Call, Storage, Event<T>},
        CurveAmm: equilibrium_curve_amm::{Pallet, Call, Storage, Event<T>},

        // XCM helpers.
        PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Storage, Origin, Config},
        DmpQueue: cumulus_pallet_dmp_queue::{Pallet, Call, Storage, Event<T>},
        XcmpQueue: cumulus_pallet_xcmp_queue::{Pallet, Call, Storage, Event<T>},
    }
);

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    system::CheckSpecVersion<Runtime>,
    system::CheckTxVersion<Runtime>,
    system::CheckGenesis<Runtime>,
    system::CheckEra<Runtime>,
    system::CheckNonce<Runtime>,
    system::CheckWeight<Runtime>,
    transaction_payment::ChargeTransactionPayment<Runtime>,
    eq_rate::reinit_extension::ReinitAccount<Runtime, CallsWithReinit>,
    eq_treasury::CheckBuyout<Runtime>,
);

pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
>;

#[derive(Clone, Eq, PartialEq, scale_info::TypeInfo)]
pub struct CallsWithReinit;
impl Contains<RuntimeCall> for CallsWithReinit {
    fn contains(call: &RuntimeCall) -> bool {
        matches!(call, RuntimeCall::Subaccounts(..))
    }
}

pub struct AssetGenerator;

impl AssetGenerator {
    fn generate_asset_for_pool(pool_id: u32, name_base: Vec<u8>) -> eq_primitives::asset::Asset {
        let zero_in_bytes = '0' as u8;
        let mut pool_id_bytes: Vec<u8> = Vec::new();
        let mut pool_id_for_bytes = pool_id;
        while pool_id_for_bytes > 0 {
            let byte = TryInto::<u8>::try_into(pool_id_for_bytes % 10).unwrap() + zero_in_bytes;
            pool_id_bytes.push(byte);
            pool_id_for_bytes = pool_id_for_bytes / 10;
        }

        // special case for 0, we want lpt0 instead of just lpt
        if pool_id == 0 {
            pool_id_bytes.push(zero_in_bytes);
        }

        pool_id_bytes.reverse();

        let name: Vec<u8> = name_base
            .iter()
            .chain(pool_id_bytes.iter())
            .map(|item| item.clone())
            .collect();
        eq_primitives::asset::Asset::from_bytes(&name)
            .expect("Asset name cannot has wrong symbols!")
    }
}

pub struct EqCurveAssetsAdapter;
type AssetId = eq_primitives::asset::Asset;
impl equilibrium_curve_amm::traits::Assets<AssetId, Balance, AccountId> for EqCurveAssetsAdapter {
    fn create_asset(pool_id: equilibrium_curve_amm::PoolId) -> Result<AssetId, DispatchError> {
        let asset = AssetGenerator::generate_asset_for_pool(pool_id, b"lpt".to_vec());

        EqAssets::do_add_asset(
            asset,
            EqFixedU128::from(0),
            FixedI64::from(0),
            Permill::zero(),
            Permill::zero(),
            AssetXcmData::None,
            LPTokensDebtWeight::get(),
            LpTokenBuyoutPriority::get(),
            eq_primitives::asset::AssetType::Lp(eq_primitives::asset::AmmPool::Curve(pool_id)),
            false,
            Percent::zero(),
            Permill::one(),
            // prices will be set at at OnPoolCreated
            vec![],
        )
        .map_err(|e| e.error)?;

        Ok(asset)
    }

    fn mint(asset: AssetId, dest: &AccountId, amount: Balance) -> DispatchResult {
        EqBalances::deposit_creating(dest, asset, amount, true, None)
    }

    fn burn(asset: AssetId, dest: &AccountId, amount: Balance) -> DispatchResult {
        EqBalances::withdraw(
            dest,
            asset,
            amount,
            true,
            None,
            WithdrawReasons::empty(),
            ExistenceRequirement::AllowDeath,
        )
    }

    fn transfer(
        asset: AssetId,
        source: &AccountId,
        dest: &AccountId,
        amount: Balance,
    ) -> DispatchResult {
        EqBalances::currency_transfer(
            dest,
            source,
            asset,
            amount,
            ExistenceRequirement::AllowDeath,
            TransferReason::Common,
            true,
        )
        .into()
    }

    fn balance(asset: AssetId, who: &AccountId) -> Balance {
        EqBalances::free_balance(who, asset)
    }

    fn total_issuance(asset: AssetId) -> Balance {
        EqAggregates::get_total(UserGroup::Balances, asset).collateral
    }

    fn withdraw_admin_fees(
        pool_id: equilibrium_curve_amm::PoolId,
        amounts: impl Iterator<Item = Balance>,
    ) -> DispatchResult {
        let pool_info = equilibrium_curve_amm::Pallet::<Runtime>::pools(pool_id).ok_or(
            equilibrium_curve_amm::Error::<Runtime>::PoolNotFound, // DispatchError::Module {
                                                                   //     index: 33,
                                                                   //     error: 4,
                                                                   //     message: None,
                                                                   // },
        )?;
        let pool_assets = pool_info.assets;

        let _ = pool_assets.into_iter().zip(amounts).map(|(asset, amount)| {
            <EqBalances as eq_primitives::balance::EqCurrency<AccountId, Balance>>::currency_transfer(
                &CurveAmmModuleId::get().into_account_truncating(),
                &EqTreasury::account_id(),
                asset,
                amount,
                ExistenceRequirement::AllowDeath,
                TransferReason::Common,
                false,
            )
        })
            .collect::<Result<_, DispatchError>>()?;
        Ok(())
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn create_benchmark_asset() -> AssetId {
        use sp_arithmetic::traits::One;

        let assets_number = <EqAssets as eq_primitives::asset::AssetGetter>::get_assets().len() + 1;

        let asset_name_bytes: Vec<u8> = "bench"
            .as_bytes()
            .iter()
            .chain(assets_number.to_string().as_bytes())
            .copied()
            .collect();

        let asset = eq_primitives::asset::Asset::from_bytes(&asset_name_bytes)
            .expect("Asset name cannot has wrong symbols!");

        EqAssets::do_add_asset(
            asset,
            0,
            0,
            0,
            0,
            AssetXcmData::None,
            EqFixedU128::saturating_from_rational(2, 5).into_inner() as u128,
            0,
            eq_primitives::asset::AssetType::Native,
            true,
            EqFixedU128::from(1).into_inner(),
            Permill::one(),
            vec![FixedI64::one()],
        )
        .expect("Benchmark asset not added");

        Oracle::set_the_only_price(asset, FixedI64::one());

        asset
    }
}

pub type CurveUnbalanceHandler = Treasury;

mod curve_utils {
    use super::*;
    use eq_primitives::PriceGetter;
    use eq_utils::{fixedi64_to_i64f64, i64f64_to_fixedi64};
    use equilibrium_curve_amm::traits::CurveAmm;
    use equilibrium_curve_amm::PoolId;
    use financial_pallet::FinancialSystemTrait;
    use financial_pallet::PriceLogs;
    use financial_pallet::{
        get_index_range, get_period_id_range, get_range_intersection, PriceLog,
    };
    use financial_primitives::capvec::CapVec;
    use frame_benchmarking::Zero;
    use frame_support::StorageMap;
    use sp_arithmetic::FixedI64;
    use sp_runtime::traits::Saturating;

    pub struct AssetChecker;

    impl equilibrium_curve_amm::traits::SliceChecker<AssetId> for AssetChecker {
        #[cfg(not(feature = "runtime-benchmarks"))]
        fn check(items: &[AssetId]) -> Result<(), DispatchError> {
            use eq_primitives::financial_storage::FinancialStorage;
            use frame_support::ensure;
            use sp_std::collections::btree_set::BTreeSet;

            let metrics = Financial::get_metrics()
                .ok_or(equilibrium_curve_amm::pallet::Error::<Runtime>::ExternalAssetCheckFailed)?;
            let assets_with_metrcis: BTreeSet<_> = metrics.assets.iter().collect();

            for asset in items {
                let asset_data =
                    <EqAssets as eq_primitives::asset::AssetGetter>::get_asset_data(asset)?;

                use eq_primitives::asset::AssetType;

                match asset_data.asset_type {
                    AssetType::Native | AssetType::Physical | AssetType::Synthetic => Ok(()),
                    AssetType::Lp(_) => Err(
                        equilibrium_curve_amm::pallet::Error::<Runtime>::ExternalAssetCheckFailed,
                    ),
                }?;

                // check that all pool tokens have actual prices + fin metrics
                let _: FixedI64 = Oracle::get_price(asset)?;
                ensure!(
                    assets_with_metrcis.contains(asset),
                    equilibrium_curve_amm::pallet::Error::<Runtime>::ExternalAssetCheckFailed
                );
            }

            Ok(())
        }

        #[cfg(feature = "runtime-benchmarks")]
        fn check(items: &[AssetId]) -> Result<(), DispatchError> {
            for asset in items {
                let asset_data =
                    <EqAssets as eq_primitives::asset::AssetGetter>::get_asset_data(asset)?;

                use eq_primitives::asset::AssetType;

                if let AssetType::Lp(_) = asset_data.asset_type {
                    //skip errors in benchmark version
                    continue;
                };
            }

            Ok(())
        }
    }

    pub struct OnPoolCreated;

    impl equilibrium_curve_amm::traits::OnPoolCreated for OnPoolCreated {
        fn on_pool_created(pool_id: PoolId) {
            let pool = super::CurveAmm::pool(pool_id).expect("pool should be created!");
            let assets = pool.assets;

            // firstly set current lp token price
            let price = calculate_mean_price(
                assets
                    .iter()
                    .map(|asset| {
                        Oracle::get_price(asset).expect("We checked all prices on pool creation!")
                    })
                    .collect(),
            );
            Oracle::set_the_only_price(pool.pool_asset, price);

            // now we need to set historical prices of lp token,
            // we assume that virtual price is 1 in history (no trades)
            let price_period = financial_primitives::PricePeriod(PricePeriod::get());

            let asset_logs: Vec<_> = assets
                .iter()
                .map(|asset| (asset, Financial::price_logs(asset).expect("")))
                .collect();

            let period_id_ranges = asset_logs
                .iter()
                .map(|(_, l)| {
                    get_period_id_range(&price_period, l.prices.len(), l.latest_timestamp)
                        .expect("No overflow for current metrics")
                })
                .collect::<Vec<_>>();

            // Ensure that all correlations calculated for the same period
            let intersection = get_range_intersection(period_id_ranges.iter())
                .expect("current metrics are calculated, but prices are not intersected");

            let mut historical_prices: Vec<Vec<FixedI64>> = (intersection.start..intersection.end)
                .collect::<Vec<_>>()
                .iter()
                .map(|_| Vec::new())
                .collect();

            for ((_, log1), period_id_range1) in asset_logs.iter().zip(period_id_ranges.iter()) {
                let range1 = get_index_range(period_id_range1, &intersection).unwrap();
                log1.prices
                    .iter_range(&range1)
                    .enumerate()
                    .for_each(|(index, price)| {
                        historical_prices[index].push(i64f64_to_fixedi64(*price));
                    });
            }

            let mut lp_price_log =
                CapVec::new(<Runtime as financial_pallet::Config>::PriceCount::get());
            for prices in historical_prices {
                lp_price_log.push(fixedi64_to_i64f64(calculate_mean_price(prices)));
            }

            // TODO: Check historical prices not empty
            PriceLogs::<Runtime>::insert(
                pool.pool_asset,
                PriceLog {
                    latest_timestamp: price_period
                        .get_period_id_start(intersection.end)
                        .unwrap()
                        .into(),
                    prices: lp_price_log,
                },
            );

            // May be that new prices are breaking financial pallet recalculation,
            // but it is very rare case
            #[allow(unused_must_use)]
            let _ = Financial::recalc_inner();
        }
    }

    fn calculate_mean_price(prices: Vec<FixedI64>) -> FixedI64 {
        let prices_len = prices.len();
        if prices_len == 0 {
            FixedI64::zero()
        } else {
            let prices_sum = prices
                .into_iter()
                .fold(FixedI64::zero(), |acc, x| acc.saturating_add(x));

            prices_sum / FixedI64::saturating_from_integer(prices_len as u64)
        }
    }
}

parameter_types! {
    pub const CreationFee: Balance = 100_000 * ONE_TOKEN;
    pub const CurveAmmModuleId: PalletId = PalletId(*b"eq/crvam");
    pub Precision: CurveNumber = CurveNumber::from_inner(1u128);
    pub LPTokensDebtWeight: Permill = Permill::from_rational(2u32, 5u32);
    pub const LpTokenBuyoutPriority: u64 = u64::MAX;
}

/// Configure the pallet equilibrium_curve_amm in pallets/equilibrium_curve_amm.

impl equilibrium_curve_amm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AssetId = AssetId;
    type Balance = Balance;
    type Currency = BasicCurrency;
    type CreationFee = CreationFee;
    type Assets = EqCurveAssetsAdapter;
    type OnUnbalanced = CurveUnbalanceHandler;
    type PalletId = CurveAmmModuleId;
    type AssetChecker = curve_utils::AssetChecker;

    type Number = CurveNumber;
    type Precision = Precision;
    type Convert = CurveNumberConvert;
    type WeightInfo = weights::pallet_curve_amm::WeightInfo<Runtime>;
    type OnPoolCreated = curve_utils::OnPoolCreated;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkingInit = benchmarking::BenchmarkingInitializer;
}

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    use frame_benchmarking::define_benchmarks;

    define_benchmarks!(
        [frame_benchmarking, BaselineBench::<Runtime>]
        [frame_system, SystemBench::<Runtime>]
        [cumulus_pallet_session_benchmarking, SessionBench::<Runtime>]
        // [eq_balances, BalancesBench::<Runtime>]
        // [eq_distribution, DistriBench::<Runtime, DistriBenchInstance>]
        // [eq_vesting, VestingBench::<Runtime, eq_vesting::Instance1>]
        // [eq_treasury, TreasuryBench::<Runtime>]
        // [eq_rate, RateBench::<Runtime>]
        [eq_session_manager, SessionManagerBench::<Runtime>]
        [chainbridge, ChainBridge]
        // [eq_bridge, BridgeBench::<Runtime>]
        [eq_assets, EqAssets]
        [eq_multisig_sudo, EqMultisigSudo]
        // [eq_bailsman, BailsmanBench::<Runtime>]
        // [eq_oracle, OracleBench::<Runtime>]
        // [eq_dex, DexBench::<Runtime>]
        // [eq_margin_call, MarginBench::<Runtime>]
        // [eq_lending, LendingBench::<Runtime>]
        // [eq_wrapped_dot, WrappedDotBench::<Runtime>]
    );
}

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
        fn account_nonce(account: AccountId) -> Index {
            System::account_nonce(account)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(_source: TransactionSource, tx: <Block as BlockT>::Extrinsic, block_hash: <Block as BlockT>::Hash) -> TransactionValidity {
            Executive::validate_transaction(_source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> SlotDuration {
            sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
        }

        fn authorities() -> Vec<AuraId> {
            Aura::authorities().into_inner()
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
        Block,
        Balance,
    > for Runtime {
        fn query_info(uxt: <Block as BlockT>::Extrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
        fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
            ParachainSystem::collect_collation_info(header)
        }
    }

    impl equilibrium_curve_amm_rpc_runtime_api::EquilibriumCurveAmmApi<Block, Balance> for Runtime {
        fn get_dy(
            pool_id: equilibrium_curve_amm::PoolId,
            i: equilibrium_curve_amm::PoolTokenIndex,
            j: equilibrium_curve_amm::PoolTokenIndex,
            dx: Balance
        ) -> Option<Balance> {
            CurveAmm::get_dy(pool_id, i, j, dx).ok()
        }

        fn get_withdraw_one_coin(
            pool_id: equilibrium_curve_amm::PoolId,
            burn_amount: Balance,
            i: equilibrium_curve_amm::PoolTokenIndex
        ) -> Option<Balance> {
            CurveAmm::get_withdraw_one_coin(pool_id, burn_amount, i).ok()
        }

        fn get_virtual_price(
            pool_id: equilibrium_curve_amm::PoolId,
        ) -> Option<Balance> {
            CurveAmm::get_virtual_price(pool_id).ok()
        }
    }

    impl eq_xdot_pool_rpc_runtime_api::EqXdotPoolApi<Block, Balance> for Runtime {
        fn invariant(
            _pool_id: eq_primitives::xdot_pool::PoolId
        ) -> Option<u128> {
            None
        }

        fn fy_token_out_for_base_in(
            _pool_id: eq_primitives::xdot_pool::PoolId,
            _base_amount: Balance
        ) -> Option<Balance> {
            None
        }

        fn base_out_for_fy_token_in(
           _pool_id: eq_primitives::xdot_pool::PoolId,
           _fy_token_amount: Balance
        ) -> Option<Balance> {
            None
        }

        fn fy_token_in_for_base_out(
            _pool_id: eq_primitives::xdot_pool::PoolId,
            _base_amount: Balance,
        ) -> Option<Balance> {
            None
        }

        fn base_in_for_fy_token_out(
            _pool_id: eq_primitives::xdot_pool::PoolId,
            _fy_token_amount: Balance,
        ) -> Option<Balance> {
            None
        }

        fn base_out_for_lp_in(
            _pool_id: eq_primitives::xdot_pool::PoolId,
            _lp_in: Balance
        ) -> Option<Balance> {
            None
        }

        fn base_and_fy_out_for_lp_in(
            _pool_id: eq_primitives::xdot_pool::PoolId,
            _lp_in: Balance
        ) -> Option<(Balance, Balance)> {
            None
        }

        fn max_base_xbase_in_and_out(
            _pool_id: eq_primitives::xdot_pool::PoolId
        ) -> Option<(Balance, Balance, Balance, Balance)> {
            None
        }
    }

    impl eq_balances_rpc_runtime_api::EqBalancesApi<Block, Balance, AccountId> for Runtime {
        fn wallet_balance_in_usd(_account_id: AccountId) -> Option<Balance> {
            None
        }
        fn portfolio_balance_in_usd(_account_id: AccountId) -> Option<Balance> {
            None
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade() -> (Weight, Weight) {
            log::info!("try-runtime::on_runtime_upgrade equilibrium");
            let weight = Executive::try_runtime_upgrade().unwrap();
            (weight, RuntimeBlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            select: frame_try_runtime::TryStateSelect
        ) -> Weight {
            log::info!(
                "try-runtime: executing block {:?} / root checks: {:?} / try-state-select: {:?}",
                block.header.hash(),
                state_root_check,
                select,
            );
            Executive::try_execute_block(block, state_root_check, select).unwrap()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{baseline, Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;
            use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

            // use eq_balances::benchmarking::Pallet as BalancesBench;
            // use eq_bridge::benchmarking::Pallet as BridgeBench;
            // use eq_distribution::benchmarking::Pallet as DistriBench;
            // use eq_vesting::benchmarking::Pallet as VestingBench;
            // use eq_treasury::benchmarking::Pallet as TreasuryBench;
            use eq_session_manager::benchmarking::Pallet as SessionManagerBench;
            // use eq_rate::benchmarking::Pallet as RateBench;
            // use eq_lending::benchmarking::Pallet as LendingBench;
            // use eq_bailsman::benchmarking::Pallet as BailsmanBench;
            // use eq_oracle::benchmarking::Pallet as OracleBench;
            // use eq_dex::benchmarking::Pallet as DexBench;
            // use eq_margin_call::benchmarking::Pallet as MarginBench;
            // use eq_wrapped_dot::benchmarking::Pallet as WrappedDotBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();

            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{add_benchmark, baseline, BenchmarkBatch, Benchmarking, TrackedStorageKey};

            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;
            impl baseline::Config for Runtime {}

            use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
            impl cumulus_pallet_session_benchmarking::Config for Runtime {}

            // use eq_balances::benchmarking::Pallet as BalancesBench;
            // use eq_distribution::benchmarking::Pallet as DistriBench;
            // use eq_bailsman::benchmarking::Pallet as BailsmanBench;
            // use eq_treasury::benchmarking::Pallet as TreasuryBench;
            // use eq_rate::benchmarking::Pallet as RateBench;
            // use eq_dex::benchmarking::Pallet as EqDexBench;
            // use eq_margin_call::benchmarking::Pallet as MarginCallBench;
            use eq_session_manager::benchmarking::Pallet as SessionManagerBench;
            // use eq_lending::benchmarking::Pallet as LendingBench;
            // use eq_bridge::benchmarking::Pallet as BridgeBench;
            // use eq_vesting::benchmarking::Pallet as VestingBench;

            impl frame_system_benchmarking::Config for Runtime {}
            // impl eq_balances::benchmarking::Config for Runtime {}
            // impl eq_distribution::benchmarking::Config<DistriBenchInstance> for Runtime {}
            // impl eq_bailsman::benchmarking::Config for Runtime {}
            // impl eq_treasury::benchmarking::Config for Runtime {}
            // impl eq_rate::benchmarking::Config for Runtime {}
            // impl eq_dex::benchmarking::Config for Runtime {}
            // impl eq_margin_call::benchmarking::Config for Runtime {}
            impl eq_session_manager::benchmarking::Config for Runtime {}
            // impl eq_lending::benchmarking::Config for Runtime {}
            // impl eq_bridge::benchmarking::Config for Runtime {}
            // impl eq_vesting::benchmarking::Config<eq_vesting::Instance1> for Runtime {}

            let whitelist: Vec<TrackedStorageKey> = vec![
                // Block Number
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
                // Execution Phase
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
                // Event Count
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
                // System Events
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),

            ];
            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);

            add_benchmarks!(params, batches);

            if batches.is_empty() {
                return Err("Benchmark not found for this pallet.".into());
            }
            Ok(batches)
        }
    }
}

struct CheckInherents;

impl cumulus_pallet_parachain_system::CheckInherents<Block> for CheckInherents {
    fn check_inherents(
        block: &Block,
        relay_state_proof: &cumulus_pallet_parachain_system::RelayChainStateProof,
    ) -> sp_inherents::CheckInherentsResult {
        let relay_chain_slot = relay_state_proof
            .read_slot()
            .expect("Could not read the relay chain slot from the proof");

        let inherent_data =
            cumulus_primitives_timestamp::InherentDataProvider::from_relay_chain_slot_and_duration(
                relay_chain_slot,
                sp_std::time::Duration::from_secs(6),
            )
            .create_inherent_data()
            .expect("Could not create the timestamp inherent data");

        inherent_data.check_extrinsics(&block)
    }
}

cumulus_pallet_parachain_system::register_validate_block! {
    Runtime = Runtime,
    BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
    CheckInherents = CheckInherents,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_lp_token_asset_works() {
        let id: u32 = 1234u32;
        let name: Vec<u8> = b"lpt"
            .iter()
            .chain(id.to_string().as_bytes().iter())
            .map(|item| item.clone())
            .collect();
        let asset = eq_primitives::asset::Asset::from_bytes(&name).unwrap();
        assert_eq!(
            asset,
            AssetGenerator::generate_asset_for_pool(id, b"lpt".to_vec())
        );
    }

    #[test]
    fn generate_lp_token_asset_works_for0() {
        let id: u32 = 0u32;
        let name: Vec<u8> = b"lpt"
            .iter()
            .chain(id.to_string().as_bytes().iter())
            .map(|item| item.clone())
            .collect();
        let _asset = eq_primitives::asset::Asset::from_bytes(&name).unwrap();
        let asset = eq_primitives::asset::Asset::from_bytes(&name).unwrap();
        assert_eq!(
            asset,
            AssetGenerator::generate_asset_for_pool(id, b"lpt".to_vec())
        );
    }

    #[test]
    fn generate_lp_token_asset_works_for1() {
        let id: u32 = 1u32;
        let name: Vec<u8> = b"lpt"
            .iter()
            .chain(id.to_string().as_bytes().iter())
            .map(|item| item.clone())
            .collect();
        let asset = eq_primitives::asset::Asset::from_bytes(&name).unwrap();
        assert_eq!(
            asset,
            AssetGenerator::generate_asset_for_pool(id, b"lpt".to_vec())
        );
    }
}
