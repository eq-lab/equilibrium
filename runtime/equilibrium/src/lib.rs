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
use core::convert::{TryFrom, TryInto};
pub use eq_assets;
pub use eq_balances;
use eq_balances::NegativeImbalance;
pub use eq_bridge;
pub use eq_claim;
pub use eq_distribution;
pub use eq_multisig_sudo;
use eq_primitives::asset::{self, Asset, AssetGetter};
use eq_primitives::balance::{AccountData, DebtCollateralDiscounted, EqCurrency};
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::subaccount::SubAccType;
use eq_primitives::xcm_origins::{dot::*, RELAY};
use frame_support::traits::tokens::imbalance::SplitTwoWays;
use frame_support::weights::WeightToFee;
use frame_support::StorageMap;
use mocks::{BalanceAwareMock, XbasePriceMock};

pub use eq_primitives;
use eq_primitives::asset::{AssetXcmData, OnNewAsset};
use eq_primitives::curve_number::{CurveNumber, CurveNumberConvert};
use eq_primitives::BlockNumberToBalance;
use eq_primitives::{Aggregates, TransferReason, UnsignedPriorityPair, UserGroup};
pub use eq_rate;
pub use eq_subaccounts;
pub use eq_treasury;
use eq_utils::{XcmBalance, ONE_TOKEN};
pub use eq_vesting;
use eq_whitelists;
pub use equilibrium_curve_amm;
use financial_pallet::{self, FinancialSystemTrait};
use financial_primitives::{CalcReturnType, CalcVolatilityType};
use frame_support::traits::{
    Contains, ExistenceRequirement, InstanceFilter, Nothing, OnUnbalanced, UnixTime,
};
pub use frame_support::{
    construct_runtime, debug,
    dispatch::{DispatchError, DispatchResult},
    match_types, parameter_types,
    traits::{Imbalance, KeyOwnerProofSystem, Randomness, StorageMapShim, WithdrawReasons},
    weights::{
        constants::{BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_PER_SECOND},
        ConstantMultiplier, DispatchClass, IdentityFee, Weight,
    },
    PalletId, StorageValue,
};
use frame_system as system;
use frame_system::limits::BlockLength;
use frame_system::limits::BlockWeights;
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use sp_api::impl_runtime_apis;
use sp_arithmetic::FixedPointNumber;
use sp_arithmetic::PerThing;
use sp_consensus_aura::{sr25519::AuthorityId as AuraId, SlotDuration};
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::{
    self, AccountIdLookup, BlakeTwo256, Block as BlockT, Convert, OpaqueKeys, Zero,
};
use sp_runtime::transaction_validity::{
    TransactionPriority, TransactionSource, TransactionValidity,
};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{create_runtime_str, generic, impl_opaque_keys, ApplyExtrinsicResult, FixedI128};
pub use sp_runtime::{FixedI64, Perbill, Permill};
use sp_runtime::{Percent, SaturatedConversion};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use substrate_fixed;
use system::EnsureRoot;

// Polkadot imports
use codec::Encode;
use frame_support::pallet_prelude::Get;
use polkadot_parachain::primitives::Sibling;
use polkadot_runtime_common::SlowAdjustingFeeUpdate;
use polkadot_runtime_constants::weights::RocksDbWeight;
use xcm::latest::{Junction, MultiAsset, OriginKind, Weight as XcmWeight, Xcm};
use xcm::v1::MultiLocation;
use xcm_builder::{
    EnsureXcmOrigin, FixedWeightBounds, LocationInverter, ParentIsPreset,
    SiblingParachainConvertsVia,
};
use xcm_executor::traits::{ConvertOrigin, FilterAssetLocation};
use xcm_executor::{Config, XcmExecutor};

// All common features
use common_runtime::*;

#[cfg(feature = "runtime-benchmarks")]
use codec::alloc::string::ToString;
// Tests for XCM integration
#[cfg(test)]
mod xcm_test;

// Weights used in the runtime.
pub mod weights;

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("Equilibrium-parachain"),
    impl_name: create_runtime_str!("Equilibrium-parachain"),
    authoring_version: 10,
    spec_version: 27,
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
pub const HOURS: BlockNumber = 60 * MINUTES;
pub const DAYS: BlockNumber = 24 * HOURS;
pub const WEEKS: BlockNumber = 7 * DAYS;

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
    type Event = Event;
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
    type Event = Event;
    type Call = Call;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
    type PalletsOrigin = OriginCaller;
}

/// We assume that ~10% of the block weight is consumed by `on_initalize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10); // may be need to be changed once
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 0.5 seconds of compute with a 12 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = WEIGHT_PER_SECOND.saturating_div(2);

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
    pub const SS58Prefix: u8 = 68;
}

/// Call filter for exctrinsics
/// XCM extrinsics aren't allowed in prod
pub struct CallFilter;
impl frame_support::traits::Contains<Call> for CallFilter {
    #[allow(unused_variables)]
    fn contains(c: &Call) -> bool {
        #[cfg(feature = "production")]
        match (eq_migration::Migration::<Runtime>::exists(), c) {
            (_, Call::EqWrappedDot(eq_wrapped_dot::Call::initialize { .. })) => false,

            // no migration, custom filter
            // (false, Call::Sudo(sudo_call)) => match sudo_call {
            //     sudo::Call::sudo { call }
            //     | sudo::Call::sudo_as { call, .. }
            //     | sudo::Call::sudo_unchecked_weight { call, .. } => {
            //         match **call {
            //             Call::PolkadotXcm(_) => true, // allow send xcm from sudo
            //             _ => Self::contains(&**call),
            //         }
            //     }
            //     _ => true,
            // },
            (false, Call::EqMultisigSudo(proposal_call)) => match proposal_call {
                eq_multisig_sudo::Call::propose { call } => match *call.clone() {
                    Call::PolkadotXcm(_) => true, // allow send xcm from msig
                    Call::Utility(utility_call) => {
                        // allow send xcm batch from msig
                        match utility_call {
                            pallet_utility::Call::batch { calls, .. }
                            | pallet_utility::Call::batch_all { calls, .. } => {
                                calls.iter().all(|call| {
                                    if let Call::PolkadotXcm(_) = call {
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
            (false, Call::Utility(utility_call)) => match utility_call {
                pallet_utility::Call::batch { calls, .. }
                | pallet_utility::Call::batch_all { calls, .. } => {
                    calls.iter().all(|call| Self::contains(call))
                }
                _ => true,
            },
            (false, Call::EqBalances(call)) => match call {
                eq_balances::Call::deposit { .. } | eq_balances::Call::burn { .. } => false,
                _ => true,
            },
            (false, Call::Oracle(eq_oracle::Call::set_fin_metrics_recalc_enabled { .. })) => false,
            (false, Call::EqRate(eq_rate::Call::set_now_millis_offset { .. })) => false,
            (false, Call::Vesting(eq_vesting::Call::force_vested_transfer { .. })) => false,
            (false, Call::Vesting2(eq_vesting::Call::force_vested_transfer { .. })) => false,
            (false, Call::Xdot(eq_xdot_pool::Call::remove_pool { .. })) => false,
            // XCM disallowed
            (_, &Call::PolkadotXcm(_)) => false,
            (false, _) => true,

            // only system and sudo are allowed during migration
            (true, &Call::ParachainSystem(_)) => true,
            (true, &Call::System(_)) => true,
            // (true, &Call::Sudo(_)) => true,
            (true, &Call::Timestamp(_)) => true,
            (true, &Call::EqMultisigSudo(_)) => true,

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
    type Origin = Origin;
    type Call = Call;
    type Index = Index;
    type BlockNumber = BlockNumber;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    type Event = Event;
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
    type Event = Event;
    type WhitelistManagementOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type WeightInfo = weights::pallet_whitelists::WeightInfo<Runtime>;
    type OnRemove = FilterPrices;
}

impl eq_assets::Config for Runtime {
    type Event = Event;
    type AssetManagementOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type MainAsset = BasicCurrencyGet;
    type OnNewAsset = FinancialPalletOnNewAsset;
    type WeightInfo = weights::pallet_assets::WeightInfo<Runtime>;
}

//----------- eq-multisig-sudo ------------------
parameter_types! {
    pub const MaxSignatories: u32 = 10;
}

impl eq_multisig_sudo::Config for Runtime {
    type Event = Event;
    type Call = Call;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = weights::pallet_multisig_sudo::WeightInfo<Runtime>;
}
//------------ eq-margin-call -------------------
parameter_types! {
    // TODO: change values
    pub InitialMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(2, 10);
    pub MaintenanceMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(1, 10);
    pub CriticalMargin: EqFixedU128 = EqFixedU128::saturating_from_rational(5, 100);
    pub MaintenancePeriod: u64 = 60*60*24;
}

impl eq_margin_call::Config for Runtime {
    type Event = Event;
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
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Aura;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

pub const EXISTENTIAL_DEPOSIT_USD: Balance = ONE_TOKEN / 10;
pub const EXISTENTIAL_DEPOSIT_BASIC: Balance = 100 * ONE_TOKEN;

parameter_types! {
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT_USD; // 0.1 USD
    pub const ExistentialDepositBasic: Balance = EXISTENTIAL_DEPOSIT_BASIC; // 100 EQ
    pub const BasicCurrencyGet: eq_primitives::asset::Asset = eq_primitives::asset::EQ;
}

impl eq_aggregates::Config for Runtime {
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Runtime>;
}

pub struct FallbackWeightToFee;
impl sp_runtime::traits::Convert<(Asset, XcmWeight), Option<eq_utils::XcmBalance>>
    for FallbackWeightToFee
{
    fn convert((asset, weight): (Asset, XcmWeight)) -> Option<eq_utils::XcmBalance> {
        use eq_xcm::fees::*;
        Some(match asset {
            asset::DOT => {
                let weight =
                    (weight * polkadot::BaseXcmWeight::get()) / crate::BaseXcmWeight::get();
                polkadot::WeightToFee::weight_to_fee(&Weight::from_ref_time(weight))
            }
            asset::GLMR => {
                let weight =
                    (weight * moonbeam::BaseXcmWeight::get()) / crate::BaseXcmWeight::get();
                moonbeam::glmr::WeightToFee::weight_to_fee(&Weight::from_ref_time(weight))
            }
            asset::PARA => {
                let weight =
                    (weight * parallel::BaseXcmWeight::get()) / crate::BaseXcmWeight::get();
                parallel::para::WeightToFee::weight_to_fee(&Weight::from_ref_time(weight))
            }
            asset::ACA => {
                let weight = (weight * acala::BaseXcmWeight::get()) / crate::BaseXcmWeight::get();
                acala::aca::WeightToFee::weight_to_fee(&Weight::from_ref_time(weight))
            }
            asset::AUSD => {
                let weight = (weight * acala::BaseXcmWeight::get()) / crate::BaseXcmWeight::get();
                acala::ausd::WeightToFee::weight_to_fee(&Weight::from_ref_time(weight))
            }
            asset::IBTC => {
                let weight =
                    (weight * interlay::BaseXcmWeight::get()) / crate::BaseXcmWeight::get();
                interlay::ibtc::WeightToFee::weight_to_fee(&Weight::from_ref_time(weight))
            }
            asset::EQD => crate::fee::XcmWeightToFee::weight_to_fee(&Weight::from_ref_time(weight)),
            asset::USDT => {
                crate::fee::XcmWeightToFee::weight_to_fee(&Weight::from_ref_time(weight)) / 1_000
            }
            asset::EQ => {
                crate::fee::XcmWeightToFee::weight_to_fee(&Weight::from_ref_time(weight)) / 10
            }
            _ => return None,
        })
    }
}

pub struct XcmToFee;
impl<'xcm, Call>
    Convert<
        (
            eq_primitives::asset::Asset,
            xcm::v1::MultiLocation,
            &'xcm Xcm<Call>,
        ),
        Option<(eq_primitives::asset::Asset, XcmBalance)>,
    > for XcmToFee
{
    fn convert(
        (asset, destination, message): (
            eq_primitives::asset::Asset,
            xcm::v1::MultiLocation,
            &'xcm Xcm<Call>,
        ),
    ) -> Option<(eq_primitives::asset::Asset, XcmBalance)> {
        use eq_xcm::fees::*;
        Some(match (destination, asset) {
            (RELAY, asset::DOT) => (asset::DOT, polkadot::XcmToFee::convert(message)),

            (PARACHAIN_MOONBEAM, asset::GLMR) => {
                (asset::GLMR, moonbeam::glmr::XcmToFee::convert(message))
            }
            (PARACHAIN_MOONBEAM, asset::EQ) => {
                (asset::EQ, moonbeam::eq::XcmToFee::convert(message))
            }
            (PARACHAIN_MOONBEAM, asset::EQD) => {
                (asset::EQ, moonbeam::eq::XcmToFee::convert(message))
            }
            (PARACHAIN_MOONBEAM, asset::MATIC) => {
                (asset::EQ, moonbeam::eq::XcmToFee::convert(message))
            }
            (PARACHAIN_MOONBEAM, asset::MXETH) => {
                (asset::EQ, moonbeam::eq::XcmToFee::convert(message))
            }
            (PARACHAIN_MOONBEAM, asset::MXUSDC) => {
                (asset::EQ, moonbeam::eq::XcmToFee::convert(message))
            }
            (PARACHAIN_MOONBEAM, asset::MXWBTC) => {
                (asset::EQ, moonbeam::eq::XcmToFee::convert(message))
            }

            (PARACHAIN_PARALLEL, asset::EQ) => {
                (asset::EQ, parallel::eq::XcmToFee::convert(message))
            }
            (PARACHAIN_PARALLEL, asset::EQD) => {
                (asset::EQD, parallel::eqd::XcmToFee::convert(message))
            }
            (PARACHAIN_PARALLEL, asset::PARA) => {
                (asset::PARA, parallel::para::XcmToFee::convert(message))
            }
            (PARACHAIN_PARALLEL, asset::CDOT613) => (
                asset::CDOT613,
                parallel::cdot613::XcmToFee::convert(message),
            ),
            (PARACHAIN_PARALLEL, asset::CDOT714) => (
                asset::CDOT714,
                parallel::cdot714::XcmToFee::convert(message),
            ),
            (PARACHAIN_PARALLEL, asset::CDOT815) => (
                asset::CDOT815,
                parallel::cdot815::XcmToFee::convert(message),
            ),

            (PARACHAIN_ACALA, asset::EQ) => (asset::EQ, acala::eq::XcmToFee::convert(message)),
            (PARACHAIN_ACALA, asset::EQD) => (asset::EQD, acala::eqd::XcmToFee::convert(message)),
            (PARACHAIN_ACALA, asset::ACA) => (asset::ACA, acala::aca::XcmToFee::convert(message)),
            (PARACHAIN_ACALA, asset::AUSD) => {
                (asset::AUSD, acala::ausd::XcmToFee::convert(message))
            }

            (PARACHAIN_INTERLAY, asset::IBTC) => {
                (asset::IBTC, interlay::ibtc::XcmToFee::convert(message))
            }
            (PARACHAIN_INTERLAY, asset::INTR) => {
                (asset::INTR, interlay::intr::XcmToFee::convert(message))
            }

            (PARACHAIN_STATEMINT, asset::USDT) => {
                (asset::DOT, statemint::XcmToFee::convert(message))
            }

            (PARACHAIN_ASTAR, asset::EQ) => (asset::EQ, astar::eq::XcmToFee::convert(message)),
            (PARACHAIN_ASTAR, asset::EQD) => (asset::EQD, astar::eqd::XcmToFee::convert(message)),
            (PARACHAIN_ASTAR, asset::ASTR) => {
                (asset::ASTR, astar::astr::XcmToFee::convert(message))
            }

            (PARACHAIN_CRUST, asset::EQD) => (asset::EQD, crust::eqd::XcmToFee::convert(message)),
            (PARACHAIN_CRUST, asset::CRU) => (asset::CRU, crust::cru::XcmToFee::convert(message)),

            (PARACHAIN_BIFROST, asset::EQ) => (asset::EQ, bifrost::eq::XcmToFee::convert(message)),
            (PARACHAIN_BIFROST, asset::EQD) => {
                (asset::EQD, bifrost::eq::XcmToFee::convert(message))
            }
            (PARACHAIN_BIFROST, asset::BNC) => {
                (asset::BNC, bifrost::eq::XcmToFee::convert(message))
            }

            (PARACHAIN_PHALA, asset::PHA) => (asset::PHA, phala::pha::XcmToFee::convert(message)),
            (PARACHAIN_PHALA, asset::EQD) => (asset::EQD, phala::eqd::XcmToFee::convert(message)),
            (PARACHAIN_PHALA, asset::EQ) => (asset::EQ, phala::eq::XcmToFee::convert(message)),
            #[cfg(test)]
            (eq_primitives::xcm_origins::ksm::PARACHAIN_HEIKO, asset::EQ) => {
                (asset::EQ, parallel::gens::XcmToFee::convert(message))
            }
            #[cfg(test)]
            (RELAY, asset::EQD) => (asset::EQ, 0),
            #[cfg(test)]
            (PARACHAIN_ACALA, asset::DOT) => (asset::DOT, 0),
            #[cfg(test)]
            (xcm_test::CURSED_MULTI_LOCATION, asset::DOT) => (asset::DOT, 0),
            _ => return None,
        })
    }
}

impl eq_balances::Config for Runtime {
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type ToggleTransferOrigin = EnsureRootOrHalfTechnicalCommittee;
    type ForceXcmTransferOrigin = EnsureRootOrTwoThirdsCouncil;
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type Event = Event;

    // order matters: heavy checks must be at the end
    type BalanceChecker = (
        eq_subaccounts::Pallet<Runtime>,
        eq_balances::locked_balance_checker::CheckLocked<Runtime>,
        eq_lending::Pallet<Runtime>,
        eq_bailsman::Pallet<Runtime>,
    );

    type ExistentialDeposit = ExistentialDeposit;
    type ExistentialDepositBasic = ExistentialDepositBasic;
    type PriceGetter = Oracle;
    type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
    type Aggregates = eq_aggregates::Pallet<Runtime>;
    type TreasuryModuleId = TreasuryModuleId;
    type SubaccountsManager = eq_subaccounts::Pallet<Runtime>;
    type BailsmenManager = Bailsman;
    type UpdateTimeManager = eq_rate::Pallet<Runtime>;
    type BailsmanModuleId = BailsmanModuleId;
    type AccountStore = System;
    type ModuleId = BalancesModuleId;
    type XcmRouter = XcmRouter;
    type XcmToFee = XcmToFee;
    type LocationToAccountId = LocationToAccountId;
    type LocationInverter = LocationInverter<Ancestry>;
    type OrderAggregates = EqDex;
    type ParachainId = ParachainInfo;
    type UnixTime = EqRate;
}

pub type BasicCurrency = eq_primitives::balance_adapter::BalanceAdapter<
    Balance,
    eq_balances::Pallet<Runtime>,
    BasicCurrencyGet,
>;

parameter_types! {
    pub const TransactionByteFee: Balance = 1;
    pub const OperationalFeeMultiplier: u8 = 5;
}

pub struct Author;
impl OnUnbalanced<NegativeImbalance<Balance>> for Author {
    fn on_nonzero_unbalanced(amount: NegativeImbalance<Balance>) {
        let benificiary = Authorship::author()
            .unwrap_or_else(|| TreasuryModuleId::get().into_account_truncating());
        let _ = EqBalances::deposit_creating(
            &benificiary,
            EqAssets::get_main_asset(),
            amount.peek(),
            false,
            None,
        );
    }
}

pub type DealWithFees = SplitTwoWays<Balance, NegativeImbalance<Balance>, Treasury, Author, 80, 20>;

/// Fee-related.
pub mod fee {
    use super::*;
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

    const DOLLAR: crate::XcmBalance = crate::ONE_TOKEN;
    const MILLI: crate::XcmBalance = DOLLAR / 1_000;

    /// Convert Weight of xcm message into USD amount of fee
    pub struct XcmWeightToFee;
    impl WeightToFeePolynomial for XcmWeightToFee {
        type Balance = crate::XcmBalance;
        fn polynomial() -> WeightToFeeCoefficients<crate::XcmBalance> {
            let p = 25 * MILLI;
            let q = crate::XcmBalance::from(BaseXcmWeight::get());
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
    type Event = Event;
    type OnChargeTransaction = transaction_payment::CurrencyAdapter<BasicCurrency, DealWithFees>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type WeightToFee = fee::WeightToFee;
    type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
}

parameter_types! {
    pub const UncleGenerations: BlockNumber = 5;
}

impl authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
    type UncleGenerations = UncleGenerations;
    type FilterUncle = ();
    type EventHandler = ();
}

#[cfg(not(feature = "production"))]
impl sudo::Config for Runtime {
    type Event = Event;
    type Call = Call;
}

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
    pub const RepublicModuleId: PalletId = PalletId(*b"eq/repub");
    pub const InvestorsModuleId: PalletId = PalletId(*b"eq/invst");
    pub const LiquidityFarmingModuleId: PalletId = PalletId(*b"eq/liqfm");
    pub const LendingModuleId: PalletId = PalletId(*b"eq/lendr");
    pub const BalancesModuleId: PalletId = PalletId(*b"eq/balan");
    pub const LpPriceBlockTimeout: u64 = PRICE_TIMEOUT_IN_SECONDS * 1000 / MILLISECS_PER_BLOCK;
    pub const UnsignedLifetimeInBlocks: u32 = 5;
    pub const FinancialRecalcPeriodBlocks: BlockNumber  = (1000 * 60 * 60 * 4) / MILLISECS_PER_BLOCK as BlockNumber; // 4 hours in blocks
}

parameter_types! {
    pub BuyFee: Permill = PerThing::from_rational::<u32>(1, 100);
    pub SellFee: Permill = PerThing::from_rational::<u32>(15, 100);
    pub const MinAmountToBuyout: Balance = 100 * ONE_TOKEN; // 100 Eq
}

impl eq_treasury::Config for Runtime {
    type Event = Event;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type Balance = Balance;
    type PriceGetter = Oracle;
    type BalanceGetter = eq_balances::Pallet<Runtime>;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type PalletId = TreasuryModuleId;
    type BuyFee = BuyFee;
    type SellFee = SellFee;
    type UnixTime = eq_rate::Pallet<Runtime>;
    type WeightInfo = weights::pallet_treasury::WeightInfo<Runtime>;
    type MinAmountToBuyout = MinAmountToBuyout;
}

parameter_types! {
    pub const MinVestedTransfer: Balance = 1 * ONE_TOKEN; // 1 Eq
    pub const VestingModuleId: PalletId = PalletId(*b"eq/vestn");
    pub Prefix: &'static [u8] = if cfg!(feature = "production") {
        b"Pay EQ to the account:"
    } else {
        b"Pay TEST EQ to the TEST account:"
    };
    pub const ClaimUnsignedPriorityPair: TransactionPriority = TransactionPriority::min_value();
}

pub struct VestingAccount;
impl Get<AccountId> for VestingAccount {
    fn get() -> AccountId {
        VestingModuleId::get().into_account_truncating()
    }
}

type VestingInstance1 = eq_vesting::Instance1;
impl eq_vesting::Config<VestingInstance1> for Runtime {
    type Event = Event;
    type Currency = BasicCurrency;
    type BlockNumberToBalance = BlockNumberToBalance;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = weights::pallet_vesting::WeightInfo<Runtime>;
    type PalletId = VestingModuleId;
    type IsTransfersEnabled = eq_balances::Pallet<Runtime>;
}

parameter_types! {
    pub const Vesting2ModuleId: PalletId = PalletId(*b"eq/vest2");
}

type VestingInstance2 = eq_vesting::Instance2;
impl eq_vesting::Config<VestingInstance2> for Runtime {
    type Event = Event;
    type Currency = BasicCurrency;
    type BlockNumberToBalance = BlockNumberToBalance;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = weights::pallet_vesting::WeightInfo<Runtime>;
    type PalletId = Vesting2ModuleId;
    type IsTransfersEnabled = eq_balances::Pallet<Runtime>;
}

impl eq_claim::Config for Runtime {
    type Event = Event;
    type VestingSchedule = Vesting;
    type Prefix = Prefix;
    type MoveClaimOrigin = system::EnsureNever<Self::AccountId>;
    type VestingAccountId = VestingAccount;
    type WeightInfo = weights::pallet_claim::WeightInfo<Runtime>;
    type UnsignedPriority = ClaimUnsignedPriorityPair;
}

parameter_types! {
    pub const LockdropModuleId: PalletId = PalletId(*b"eq/lkdrp");
    pub const LockPeriod: u64 = 90 * 24 * 60 * 60;
    pub const MinLockAmount: Balance = 10 * ONE_TOKEN;
    pub const LockDropUnsignedPriorityPair: TransactionPriority = TransactionPriority::min_value();
}

impl eq_lockdrop::Config for Runtime {
    type Event = Event;
    type PalletId = LockdropModuleId;
    type LockPeriod = LockPeriod;
    type Vesting = Vesting;
    type ValidatorOffchainBatcher = eq_rate::Pallet<Runtime>;
    type MinLockAmount = MinLockAmount;
    type LockDropUnsignedPriority = LockDropUnsignedPriorityPair;
    type WeightInfo = weights::pallet_lockdrop::WeightInfo<Runtime>;
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
    type Event = Event;

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
        if !prices.is_empty() {
            let mut price_log = financial_primitives::capvec::CapVec::new(
                <Runtime as financial_pallet::Config>::PriceCount::get(),
            );

            prices
                .iter()
                .rev()
                .map(|price| eq_utils::fixed::fixedi64_to_i64f64(*price))
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
    type ManagementOrigin = system::EnsureRoot<AccountId>;
    type PalletId = TreasuryModuleId;
    type VestingSchedule = Vesting;
    type VestingAccountId = VestingAccount;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type WeightInfo = weights::pallet_distribution::WeightInfo<Runtime>;
}

type TreasuryInstance = eq_distribution::Instance5;
impl eq_distribution::Config<TreasuryInstance> for Runtime {
    type ManagementOrigin = EnsureRootOrTwoThirdsCouncil;
    type PalletId = TreasuryModuleId;
    type VestingSchedule = Vesting;
    type VestingAccountId = VestingAccount;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type WeightInfo = weights::pallet_distribution::WeightInfo<Runtime>;
}

type RepublicInstance = eq_distribution::Instance2;
impl eq_distribution::Config<RepublicInstance> for Runtime {
    type ManagementOrigin = EnsureRootOrTwoThirdsCouncil;
    type PalletId = RepublicModuleId;
    type VestingSchedule = Vesting;
    type VestingAccountId = VestingAccount;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type WeightInfo = weights::pallet_distribution::WeightInfo<Runtime>;
}

type Investors = eq_distribution::Instance3;
impl eq_distribution::Config<Investors> for Runtime {
    type ManagementOrigin = EnsureRootOrTwoThirdsCouncil;
    type PalletId = InvestorsModuleId;
    type VestingSchedule = Vesting;
    type VestingAccountId = VestingAccount;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type WeightInfo = weights::pallet_distribution::WeightInfo<Runtime>;
}

type LiquidityFarmingD = eq_distribution::Instance4;
impl eq_distribution::Config<LiquidityFarmingD> for Runtime {
    type ManagementOrigin = EnsureRootOrTwoThirdsCouncil;
    type PalletId = LiquidityFarmingModuleId;
    type VestingSchedule = Vesting;
    type VestingAccountId = VestingAccount;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type EqCurrency = eq_balances::Pallet<Runtime>;
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
    type AutoReinitToggleOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Runtime>;
    type BalanceRemover = eq_balances::Pallet<Runtime>;
    type AuthorityId = eq_rate::ed25519::AuthorityId;
    type MinSurplus = MinSurplus;
    type BailsmanManager = Bailsman;
    type MinTempBailsman = MinTempBalanceUsd;
    type UnixTime = timestamp::Pallet<Runtime>;
    type EqBuyout = eq_treasury::Pallet<Runtime>;
    type BailsmanModuleId = BailsmanModuleId;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type SubaccountsManager = eq_subaccounts::Pallet<Runtime>;
    type MarginCallManager = EqMarginCall;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type UnsignedPriority = RateUnsignedPriority;
    type WeightInfo = weights::pallet_rate::WeightInfo<Runtime>;
    type RedistributeWeightInfo = WeightInfoGetter;
    type RiskLowerBound = RiskLowerBound;
    type RiskUpperBound = RiskUpperBound;
    type RiskNSigma = RiskNSigma;
    type Alpha = Alpha;
    type Financial = Financial;
    type FinancialStorage = Financial;
    type PriceGetter = Oracle;
    type Aggregates = EqAggregates;
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
}

impl eq_session_manager::Config for Runtime {
    type ValidatorsManagementOrigin = EnsureRootOrTwoThirdsCouncil;
    type Event = Event;
    type ValidatorId = <Self as system::Config>::AccountId;
    type RegistrationChecker = pallet_session::Pallet<Runtime>;
    type ValidatorIdOf = sp_runtime::traits::ConvertInto;
    type WeightInfo = weights::pallet_session_manager::WeightInfo<Runtime>;
}

impl eq_subaccounts::Config for Runtime {
    type Event = Event;
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
    type Event = Event;
    type Balance = Balance;
    type BalanceGetter = EqBalances;
    type Aggregates = EqAggregates;
    type AssetGetter = EqAssets;
    type PriceGetter = Oracle;
    type SubaccountsManager = Subaccounts;
    type ModuleId = LendingModuleId;
    type EqCurrency = EqBalances;
    type BailsmanManager = Bailsman;
    type UnixTime = EqRate;
    type WeightInfo = weights::pallet_lending::WeightInfo<Runtime>;
}

impl system::offchain::SigningTypes for Runtime {
    type Public = <Signature as traits::Verify>::Signer;
    type Signature = Signature;
}

impl<C> system::offchain::SendTransactionTypes<C> for Runtime
where
    Call: From<C>,
{
    type OverarchingCall = Call;
    type Extrinsic = UncheckedExtrinsic;
}

impl<LocalCall> system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
    Call: From<LocalCall>,
{
    fn create_transaction<C: system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: Call,
        public: <Signature as traits::Verify>::Signer,
        account: AccountId,
        nonce: Index,
    ) -> Option<(
        Call,
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
            eq_claim::PrevalidateAttests::<Runtime>::new(),
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
    pub const ChainId: u8 = 7;
}

impl chainbridge::Config for Runtime {
    type Event = Event;
    type Currency = BasicCurrency;
    type Balance = Balance;
    type BalanceGetter = eq_balances::Pallet<Runtime>;
    type AdminOrigin = system::EnsureRoot<Self::AccountId>;
    type Proposal = Call;
    type ChainIdentity = ChainId;
    type WeightInfo = weights::pallet_chainbridge::WeightInfo<Runtime>;
}

impl eq_bridge::Config for Runtime {
    type BridgeManagementOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type Event = Event;
    type BridgeOrigin = chainbridge::EnsureBridge<Runtime>;
    type EqCurrency = eq_balances::Pallet<Runtime>;
    type AssetGetter = eq_assets::Pallet<Runtime>;
    type WeightInfo = weights::pallet_bridge::WeightInfo<Runtime>;
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
            source,
            dest,
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
            EqFixedU128::from(0),
            FixedI64::from(0),
            Permill::zero(),
            Permill::zero(),
            AssetXcmData::None,
            Permill::from_rational(2u32, 5u32),
            0,
            eq_primitives::asset::AssetType::Native,
            true,
            Percent::one(),
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
    use eq_utils::fixed::{fixedi64_to_i64f64, i64f64_to_fixedi64};
    use equilibrium_curve_amm::traits::CurveAmm;
    use equilibrium_curve_amm::PoolId;
    use financial_pallet::FinancialSystemTrait;
    use financial_pallet::PriceLogs;
    use financial_pallet::{
        get_index_range, get_period_id_range, get_range_intersection, PriceLog,
    };
    use financial_primitives::capvec::CapVec;
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

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking {
    use super::*;
    use eq_primitives::asset::AssetGetter;
    use eq_primitives::PriceSetter;
    use frame_system::RawOrigin;
    use sp_arithmetic::FixedI64;
    use sp_runtime::traits::{AccountIdConversion, One};

    pub struct BenchmarkingInitializer;
    const SEED: u32 = 0;

    impl equilibrium_curve_amm::traits::BenchmarkingInit for BenchmarkingInitializer {
        fn init_withdraw_admin_fees() {
            // initialize prices for all assets
            let price_setter: AccountId = frame_benchmarking::account("price_setter", 0, SEED);
            Whitelists::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone()).unwrap();

            for asset in EqAssets::get_assets_with_usd() {
                <Oracle as PriceSetter<_>>::set_price(price_setter.clone(), asset, FixedI64::one())
                    .unwrap();
            }

            //initialize fee holder
            let fee_holder: AccountId = frame_benchmarking::account("fee_holder", 0, SEED);

            //deposit money
            let basic_asset = EqAssets::get_main_asset();
            let budget = ONE_TOKEN * 10_000_000;

            <EqBalances as eq_primitives::balance::EqCurrency<AccountId, Balance>>::deposit_creating(
                &fee_holder,
                basic_asset,
                budget,
                false,
                None
            )
            .unwrap();

            <EqBalances as eq_primitives::balance::EqCurrency<AccountId, Balance>>::deposit_creating(
                &PalletId(*b"eq/trsry").into_account_truncating(),
                basic_asset,
                budget,
                false,
                None
            )
            .unwrap();
        }
    }

    pub struct XcmRouterBench;
    impl xcm::latest::SendXcm for XcmRouterBench {
        fn send_xcm(
            _destination: impl Into<MultiLocation>,
            _message: Xcm<()>,
        ) -> xcm::latest::SendResult {
            Ok(())
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
    type Event = Event;
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

parameter_types! {
    pub const CurveDistributionModuleId: PalletId = PalletId(*b"eq/crvds");
    pub MinCurveFee: Balance = 20_000_000_000; // TODO
    pub AdminFeeContract: Vec<u8> = if cfg!(feature = "production") {
        Vec::new() // TODO
    } else {
        // Rinkeby FeeDistributor 0x839F5490C12878db4E9C41b1B0aCB4AA70F5b4CB
        // js oneliner to split address into rust byte array:
        // '839F5490C12878db4E9C41b1B0aCB4AA70F5b4CB'.match(/.{2}/g).map(x => `0x${x}u8`).join(', ')
        vec![0x83u8, 0x9Fu8, 0x54u8, 0x90u8, 0xC1u8, 0x28u8, 0x78u8, 0xdbu8, 0x4Eu8, 0x9Cu8, 0x41u8, 0xb1u8, 0xB0u8, 0xaCu8, 0xB4u8, 0xAAu8, 0x70u8, 0xF5u8, 0xb4u8, 0xCBu8]
    };
    pub const DestinationId: chainbridge::ChainId = 0;
}

parameter_types! {
    pub const PriceStepCount: u32 = 5;
    pub const PenaltyFee: Balance = 10 * ONE_TOKEN;
    pub const DexUnsignedPriority: TransactionPriority = TransactionPriority::min_value();
}

parameter_types! {
    pub const MultisigDepositBase: Balance = 10 * ONE_TOKEN;
    pub const MultisigDepositFactor: Balance = ONE_TOKEN;
    pub const MultisigMaxSignatories: u16 = 10;
}

impl pallet_multisig::Config for Runtime {
    type Event = Event;
    type Call = Call;
    type Currency = BasicCurrency;
    type DepositBase = MultisigDepositBase;
    type DepositFactor = MultisigDepositFactor;
    type MaxSignatories = MultisigMaxSignatories;
    type WeightInfo = ();
}

parameter_types! {
    pub const ProxyDepositBase: Balance = 10 * ONE_TOKEN;
    pub const ProxyDepositFactor: Balance = 1 * ONE_TOKEN;
    pub const MaxProxies: u16 = 20;
    pub const AnnouncementDepositBase: Balance = 10 * ONE_TOKEN;
    pub const AnnouncementDepositFactor: Balance = 1 * ONE_TOKEN;
    pub const MaxPending: u32 = 20;
}

impl InstanceFilter<Call> for ProxyType {
    fn filter(&self, _c: &Call) -> bool {
        true
    }

    fn is_superset(&self, o: &Self) -> bool {
        matches!((self, o), (ProxyType::Any, _))
    }
}

impl pallet_proxy::Config for Runtime {
    type Event = Event;
    type Call = Call;
    type Currency = BasicCurrency;
    type ProxyType = ProxyType;
    type ProxyDepositBase = ProxyDepositBase;
    type ProxyDepositFactor = ProxyDepositFactor;
    type MaxProxies = MaxProxies;
    type MaxPending = MaxPending;
    type AnnouncementDepositBase = AnnouncementDepositBase;
    type AnnouncementDepositFactor = AnnouncementDepositFactor;
    type CallHasher = BlakeTwo256;
    type WeightInfo = ();
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

use eq_primitives::proxy::ProxyType;

//////////////////////////////////////////////////////////////////////////////
// 	Cumulus pallets
//////////////////////////////////////////////////////////////////////////////

impl cumulus_pallet_aura_ext::Config for Runtime {}

parameter_types! {
    pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
    pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
}

impl cumulus_pallet_parachain_system::Config for Runtime {
    type Event = Event;
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

parameter_types! {
    pub Ancestry: MultiLocation = Junction::Parachain(ParachainInfo::parachain_id().into()).into();
}

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
    // Convertion to relay-chain sovereign account.
    ParentIsPreset<AccountId>,
    // Convertion to sibling parachain sovereign account.
    SiblingParachainConvertsVia<Sibling, AccountId>,
    // Straight up local `AccountId32` origins just alias directly to `AccountId`.
    // We expect messages only from `NetworkId::Any`
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
    // One XCM operation is 200_000_000 weight - litle overcharging estimate.
    pub const BaseXcmWeight: XcmWeight = 200_000_000;
    pub const MaxInstructions: u32 = 100;
}

pub struct NoTeleport;
impl FilterAssetLocation for NoTeleport {
    fn filter_asset_location(_asset: &MultiAsset, _origin: &MultiLocation) -> bool {
        false
    }
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
        &RELAY |
        &PARACHAIN_MOONBEAM |
        &PARACHAIN_PARALLEL |
        &PARACHAIN_ACALA |
        &PARACHAIN_INTERLAY |
        &PARACHAIN_STATEMINT |
        &PARACHAIN_ASTAR |
        &PARACHAIN_CRUST |
        &PARACHAIN_PHALA |
        &PARACHAIN_LITENTRY |
        &PARACHAIN_POLKADEX |
        &PARACHAIN_COMPOSABLE
    };
}

pub type Barrier = (
    eq_xcm::barrier::AllowReserveAssetDepositedFrom<EqAssets, TrustedOrigins>,
    eq_xcm::barrier::AllowReserveTransferAssetsFromAccountId,
);

pub type Weigher = FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;

pub struct XcmConfig;
impl Config for XcmConfig {
    type Call = Call;
    type XcmSender = XcmRouter;
    // How to withdraw and deposit an asset.
    type AssetTransactor = LocalAssetTransactor;
    type OriginConverter = XcmOriginToTransactDispatchOrigin;
    type IsReserve = eq_xcm::assets::NativeAsset;
    type IsTeleporter = NoTeleport;
    type LocationInverter = LocationInverter<Ancestry>;
    type Barrier = Barrier;
    type Weigher = Weigher;
    type Trader = EqTrader;
    type ResponseHandler = (); // Don't handle responses for now.
    type AssetTrap = PolkadotXcm;
    type AssetClaims = PolkadotXcm;
    type SubscriptionService = PolkadotXcm;
}

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
#[cfg(not(any(test, feature = "runtime-benchmarks")))]
pub type XcmRouter = (
    // use UMP to communicate with the relay chain:
    cumulus_primitives_utility::ParentAsUmp<ParachainSystem, ()>,
    // use XCMP to comminicate with other parachains via relay:
    XcmpQueue,
);

#[cfg(test)]
pub type XcmRouter = xcm_test::XcmRouterMock;

#[cfg(feature = "runtime-benchmarks")]
pub type XcmRouter = benchmarking::XcmRouterBench;

pub type LocalOriginToLocation = eq_xcm::origins::LocalOriginToLocation<Origin, AccountId>;

impl pallet_xcm::Config for Runtime {
    type Event = Event;
    type SendXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
    type XcmRouter = XcmRouter;
    type ExecuteXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
    type XcmExecuteFilter = Nothing;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmTeleportFilter = Nothing;
    type XcmReserveTransferFilter = Nothing; // We don't use xcm pallet calls
    type Weigher = FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
    type LocationInverter = LocationInverter<Ancestry>;
    type Origin = Origin;
    type Call = Call;
    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
    type Event = Event;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
    type Event = Event;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type ChannelInfo = ParachainSystem;
    type VersionWrapper = PolkadotXcm;
    type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
    type ControllerOrigin = EnsureRoot<AccountId>;
    type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
    type WeightInfo = ();
}

/// We allow root to execute privileged collator selection operations.
pub type CollatorSelectionUpdateOrigin = EnsureRoot<AccountId>;

parameter_types! {
    pub const MigrationsPerBlock: u16 = 2_000;
}

impl eq_migration::Config for Runtime {
    type Event = Event;
    type MigrationsPerBlock = MigrationsPerBlock;
    type WeightInfo = eq_migration::weights::EqWeight<Runtime>;
}

impl eq_oracle::Config for Runtime {
    type FinMetricsRecalcToggleOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type Event = Event;
    type AuthorityId = eq_oracle::crypto::AuthId;
    type Call = Call;
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
    type EqDotPrice = EqWrappedDot;
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
    type Event = Event;
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

impl eq_dex::Config for Runtime {
    type Event = Event;
    type DeleteOrderOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type UpdateAssetCorridorOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type PriceStepCount = PriceStepCount;
    type PenaltyFee = PenaltyFee;
    type DexUnsignedPriority = DexUnsignedPriority;
    type WeightInfo = weights::pallet_dex::WeightInfo<Runtime>;
    type ValidatorOffchainBatcher = eq_rate::Pallet<Runtime>;
}

pub struct XdotAssetsAdapter;
impl eq_xdot_pool::traits::Assets<AssetId, Balance, AccountId> for XdotAssetsAdapter {
    fn create_lp_asset(
        pool_id: eq_primitives::xdot_pool::PoolId,
    ) -> Result<AssetId, DispatchError> {
        let asset = AssetGenerator::generate_asset_for_pool(pool_id, b"xlpt".to_vec());

        EqAssets::do_add_asset(
            asset,
            // TODO hardcode for now, change after dex
            EqFixedU128::from(0),
            FixedI64::from(0),
            Permill::zero(),
            Permill::zero(),
            asset::AssetXcmData::None,
            LPTokensDebtWeight::get(),
            LpTokenBuyoutPriority::get(),
            asset::AssetType::Lp(asset::AmmPool::Yield(pool_id)),
            false,
            Percent::zero(),
            Permill::one(),
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
            source,
            dest,
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
        <EqAggregates as eq_primitives::Aggregates<_, _>>::get_total(
            eq_primitives::UserGroup::Balances,
            asset,
        )
        .collateral
    }
}

mod xdot_utils {
    use super::*;
    use eq_primitives::asset::Asset;
    use eq_primitives::xdot_pool::XBasePrice;
    use eq_utils::fixed::{fixedi64_to_i64f64, i64f64_to_fixedi64};
    use financial_pallet::FinancialSystemTrait;

    pub struct AssetChecker;
    impl eq_xdot_pool::traits::AssetChecker<Asset> for AssetChecker {
        fn check(base_asset: Asset, xbase_asset: Asset) -> Result<(), DispatchError> {
            <EqAssets as asset::AssetGetter>::get_asset_data(&base_asset)?;

            Financial::price_logs(base_asset)
                .ok_or(eq_xdot_pool::pallet::Error::<Runtime>::ExternalAssetCheckFailed)?;

            <EqAssets as asset::AssetGetter>::get_asset_data(&xbase_asset)?;

            Ok(())
        }
    }

    pub struct OnPoolInitialized;
    impl eq_xdot_pool::traits::OnPoolInitialized for OnPoolInitialized {
        fn on_initalize(pool_id: eq_primitives::xdot_pool::PoolId) -> Result<(), DispatchError> {
            let pool = super::Xdot::get_pool(pool_id)?;

            let base_asset_log = Financial::price_logs(pool.base_asset)
                .ok_or(eq_xdot_pool::pallet::Error::<Runtime>::ExternalAssetCheckFailed)?;

            let one_period =
                (<Runtime as financial_pallet::Config>::PricePeriod::get() * 60) as u64;
            let price_count = (<Runtime as financial_pallet::Config>::PriceCount::get()) as u64;

            let time_till_maturity =
                Xdot::time_till_maturity(pool.maturity + (price_count - 1) * one_period)?;
            let mut lp_prices = financial_primitives::capvec::CapVec::new(
                <Runtime as financial_pallet::Config>::PriceCount::get(),
            );
            let mut xbase_prices = financial_primitives::capvec::CapVec::new(
                <Runtime as financial_pallet::Config>::PriceCount::get(),
            );
            let mut period = 0;
            for base_price in base_asset_log.prices.iter() {
                let ttm = time_till_maturity - period * one_period;
                let lp_price = fixedi64_to_i64f64(Xdot::get_lp_virtual_price(&pool, Some(ttm))?);
                let xbase_price =
                    fixedi64_to_i64f64(Xdot::get_xbase_virtual_price(&pool, Some(ttm))?);
                lp_prices.push(base_price.saturating_mul(lp_price));
                xbase_prices.push(base_price.saturating_mul(xbase_price));
                period += 1;
            }
            Oracle::set_the_only_price(
                pool.pool_asset,
                i64f64_to_fixedi64(*lp_prices.last().expect("Non empty")),
            );
            Oracle::set_the_only_price(
                pool.xbase_asset,
                i64f64_to_fixedi64(*xbase_prices.last().expect("Non empty")),
            );
            financial_pallet::PriceLogs::<Runtime>::insert(
                pool.pool_asset,
                financial_pallet::PriceLog {
                    latest_timestamp: base_asset_log.latest_timestamp,
                    prices: lp_prices,
                },
            );

            financial_pallet::PriceLogs::<Runtime>::insert(
                pool.base_asset,
                financial_pallet::PriceLog {
                    latest_timestamp: base_asset_log.latest_timestamp,
                    prices: xbase_prices,
                },
            );

            // May be that new prices are breaking financial pallet recalculation,
            // but it is very rare case
            #[allow(unused_must_use)]
            let _ = Financial::recalc_inner();

            Ok(())
        }
    }
}

pub struct XdotNumberPriceConvert;
impl Convert<XdotNumber, sp_runtime::FixedI64> for XdotNumberPriceConvert {
    fn convert(n: XdotNumber) -> sp_runtime::FixedI64 {
        eq_utils::fixed::i64f64_to_fixedi64(n)
    }
}

use eq_primitives::xdot_pool::{XdotBalanceConvert, XdotFixedNumberConvert, XdotNumber};

impl eq_xdot_pool::Config for Runtime {
    type Event = Event;
    type PoolsManagementOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type WeightInfo = ();
    type FixedNumberBits = i128;
    type XdotNumber = XdotNumber;
    type NumberConvert = eq_xdot_pool::yield_math::YieldConvert;
    type BalanceConvert = XdotBalanceConvert;
    type AssetId = AssetId;
    type Assets = XdotAssetsAdapter;
    type YieldMath =
        eq_xdot_pool::yield_math::YieldMath<XdotNumber, eq_xdot_pool::yield_math::YieldConvert>;
    type PriceNumber = sp_runtime::FixedI64;
    type PriceConvert = XdotNumberPriceConvert;
    type FixedNumberConvert = XdotFixedNumberConvert;
    type OnPoolInitialized = xdot_utils::OnPoolInitialized;
    type AssetChecker = xdot_utils::AssetChecker;
}

use eq_xcm::relay_interface::{call::RelayChainCallBuilder, config::RelayRuntime};

parameter_types! {
    pub TargetReserve: Permill = Permill::from_percent(15);
    pub MaxReserve: Permill = Permill::from_percent(20);
    pub MinReserve: Permill = Permill::from_percent(10);
    pub EqDotWithdrawFee: Permill = Permill::from_rational(989_409_u32, 1_000_000_u32); // (1/(1+14.9%)^(28/365.25) = 0.98940904738)
    pub const MinStakingDeposit: Balance = 1 * ONE_TOKEN;
    pub const WrappedDotPalletId: PalletId = PalletId(*b"eq/wrdot");
}

impl eq_wrapped_dot::Config for Runtime {
    type StakingInitializeOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type Balance = Balance;
    type Aggregates = EqAggregates;
    type TargetReserve = TargetReserve;
    type MaxReserve = MaxReserve;
    type MinReserve = MinReserve;
    type MinDeposit = MinStakingDeposit;
    type RelayChainCallBuilder = RelayChainCallBuilder<RelayRuntime, ParachainInfo>;
    type XcmRouter = XcmRouter;
    type ParachainId = ParachainInfo;
    type PriceGetter = Oracle;
    type EqCurrency = EqBalances;
    type WithdrawFee = EqDotWithdrawFee;
    type PalletId = WrappedDotPalletId;
    type WeightInfo = weights::pallet_wrapped_dot::WeightInfo<Runtime>;
}

impl eq_market_maker::Config for Runtime {
    type Event = Event;
    type DexWeightInfo = weights::pallet_dex::WeightInfo<Runtime>;
    type OrderManagement = EqDex;
}

parameter_types! {
    pub const PreimageMaxSize: u32 = 2097152; // 2MB
    pub const PreimageBaseDeposit: Balance = 100 * ONE_TOKEN;
    pub const PreimageByteDeposit: Balance = 100 * ONE_TOKEN;
}

impl pallet_preimage::Config for Runtime {
    type Event = Event;
    type Currency = BasicCurrency;
    type MaxSize = PreimageMaxSize;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type BaseDeposit = PreimageBaseDeposit;
    type ByteDeposit = PreimageByteDeposit;
    type WeightInfo = weights::pallet_preimage::WeightInfo<Runtime>;
}

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * RuntimeBlockWeights::get().max_block;
    pub const MaxScheduledPerBlock: u32 = 10;
    pub const NoPreimagePostponement: Option<BlockNumber> = None;
}

impl pallet_scheduler::Config for Runtime {
    type Event = Event;
    type Origin = Origin;
    type PalletsOrigin = OriginCaller;
    type Call = Call;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
    type PreimageProvider = Preimage;
    type NoPreimagePostponement = NoPreimagePostponement;
    type WeightInfo = weights::pallet_scheduler::WeightInfo<Runtime>;
}

pub mod high_privilege_origins;
pub use high_privilege_origins::*;

// General council setup
parameter_types! {
    pub const CouncilMotionDuration: BlockNumber = if cfg!(feature = "production") {
        3 * DAYS
    } else {
        1 * HOURS
    };
    pub const CouncilMaxProposals: u32 = 100;
    pub const CouncilMaxMembers: u32 = 100;
}

pub type CouncilInstance = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilInstance> for Runtime {
    type Origin = Origin;
    type Proposal = Call;
    type Event = Event;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = weights::pallet_collective::WeightInfo<Runtime>;
}

pub type CouncilMembershipOrigin = EnsureRootOrThreeForthsCouncil;
pub type CouncilMembershipInstance = pallet_membership::Instance1;
impl pallet_membership::Config<CouncilMembershipInstance> for Runtime {
    type Event = Event;
    type MaxMembers = CouncilMaxMembers;
    type AddOrigin = CouncilMembershipOrigin;
    type RemoveOrigin = CouncilMembershipOrigin;
    type SwapOrigin = CouncilMembershipOrigin;
    type ResetOrigin = CouncilMembershipOrigin;
    type PrimeOrigin = CouncilMembershipOrigin;
    type MembershipInitialized = Council;
    type MembershipChanged = Council;
    type WeightInfo = weights::pallet_membership::WeightInfo<Runtime>;
}

// Technical committee setup
parameter_types! {
    pub const TechnicalCommitteeMotionDuration: BlockNumber = if cfg!(feature = "production") {
        3 * DAYS
    } else {
        1 * HOURS
    };
    pub const TechnicalCommitteeMaxProposals: u32 = 100;
    pub const TechnicalCommitteeMaxMembers: u32 = 100;
}

pub type TechnicalCommitteeInstance = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCommitteeInstance> for Runtime {
    type Origin = Origin;
    type Proposal = Call;
    type Event = Event;
    type MotionDuration = TechnicalCommitteeMotionDuration;
    type MaxProposals = TechnicalCommitteeMaxProposals;
    type MaxMembers = TechnicalCommitteeMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = weights::pallet_collective::WeightInfo<Runtime>;
}

pub type TechnicalCommitteeMembershipOrigin = EnsureRootOrTwoThirdsCouncil;
pub type TechnicalCommitteeMembershipInstance = pallet_membership::Instance2;
impl pallet_membership::Config<TechnicalCommitteeMembershipInstance> for Runtime {
    type Event = Event;
    type MaxMembers = TechnicalCommitteeMaxMembers;
    type AddOrigin = TechnicalCommitteeMembershipOrigin;
    type RemoveOrigin = TechnicalCommitteeMembershipOrigin;
    type SwapOrigin = TechnicalCommitteeMembershipOrigin;
    type ResetOrigin = TechnicalCommitteeMembershipOrigin;
    type PrimeOrigin = TechnicalCommitteeMembershipOrigin;
    type MembershipInitialized = TechnicalCommittee;
    type MembershipChanged = TechnicalCommittee;
    type WeightInfo = weights::pallet_membership::WeightInfo<Runtime>;
}

parameter_types! {
    pub const LaunchPeriod: BlockNumber = if cfg!(feature = "production") {
        1 * WEEKS
    } else {
        10 * MINUTES
    };
    pub const VotingPeriod: BlockNumber = if cfg!(feature = "production") {
        2 * WEEKS
    } else {
        10 * MINUTES
    };
    pub const EnactmentPeriod: BlockNumber = if cfg!(feature = "production") {
        2 * DAYS
    } else {
        5 * MINUTES
    };
    pub const VoteLockingPeriod: BlockNumber = if cfg!(feature = "production") {
        2 * WEEKS
    } else {
        10 * MINUTES
    };

    pub const MaxVotes: u32 = 100;
    pub const MaxProposals: u32 = 100;

    pub const FastTrackVotingPeriod: BlockNumber = 3 * HOURS;

    pub const InstantAllowed: bool = true;

    pub const MinimumDeposit: Balance = 1000 * ONE_TOKEN;

    pub const CooloffPeriod: BlockNumber = 7 * DAYS;
}

impl pallet_democracy::Config for Runtime {
    type Proposal = Call;
    type Event = Event;
    type Currency = BasicCurrency;
    type PalletsOrigin = OriginCaller;
    type Scheduler = Scheduler;
    type Slash = Treasury;

    type MinimumDeposit = MinimumDeposit;
    type MaxVotes = MaxVotes;
    type MaxProposals = MaxProposals;

    type LaunchPeriod = LaunchPeriod;
    type VotingPeriod = VotingPeriod;
    type EnactmentPeriod = EnactmentPeriod;
    type VoteLockingPeriod = VoteLockingPeriod;

    type ExternalOrigin = EnsureRootOrHalfCouncil;
    type ExternalMajorityOrigin = EnsureRootOrHalfCouncil;
    type ExternalDefaultOrigin = EnsureRootOrAllCouncil;

    type FastTrackOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
    type FastTrackVotingPeriod = FastTrackVotingPeriod;

    type InstantOrigin = EnsureRootOrAllTechnicalCommittee;
    type InstantAllowed = InstantAllowed;

    type CancellationOrigin = EnsureRootOrTwoThirdsCouncil;
    type BlacklistOrigin = frame_system::EnsureRoot<AccountId>;
    type CancelProposalOrigin = EnsureRootOrAllTechnicalCommittee;

    type VetoOrigin = pallet_collective::EnsureMember<AccountId, CouncilInstance>;
    type CooloffPeriod = CooloffPeriod;

    type OperationalPreimageOrigin =
        pallet_collective::EnsureMember<AccountId, TechnicalCommitteeInstance>;
    type PreimageByteDeposit = PreimageByteDeposit;

    type WeightInfo = weights::pallet_democracy::WeightInfo<Runtime>;
}

parameter_types! {
    pub const MaxStakesCount: u32 = 10;
    pub const EqStakingModuleId: PalletId = PalletId(*b"eq/stkng");
    pub const RewardsLockPeriod: eq_staking::StakePeriod = eq_staking::StakePeriod::Six;
    pub const MaxRewardExternalIdsCount: u32 = 1000;
}

pub struct LiquidityAccount;
impl Get<AccountId> for LiquidityAccount {
    fn get() -> AccountId {
        EqStakingModuleId::get().into_account_truncating()
    }
}

pub struct LiquidityAccountCustom;
impl Get<AccountId> for LiquidityAccountCustom {
    fn get() -> AccountId {
        RepublicModuleId::get().into_account_truncating()
    }
}

impl eq_staking::Config for Runtime {
    type Event = Event;
    type Balance = Balance;
    type EqCurrency = EqBalances;
    type BalanceGetter = EqBalances;
    type LockGetter = EqBalances;
    type UnixTime = EqRate;
    type MaxStakesCount = MaxStakesCount;
    type RewardManagementOrigin = EnsureRootOrTwoThirdsCouncil;
    type LiquidityAccount = LiquidityAccount;
    type LiquidityAccountCustom = LiquidityAccountCustom;
    type RewardsLockPeriod = RewardsLockPeriod;
    type MaxRewardExternalIdsCount = MaxRewardExternalIdsCount;
    type WeightInfo = ();
}

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = common_runtime::opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        ParachainSystem: cumulus_pallet_parachain_system::{
            Pallet, Call, Config, Storage, Inherent, Event<T>, ValidateUnsigned,
        } = 1,
        Utility: pallet_utility::{Pallet, Call, Event} = 2,
        Timestamp: timestamp::{Pallet, Call, Storage, Inherent} = 4,
        ParachainInfo: parachain_info::{Pallet, Storage, Config} = 5,
        EqSessionManager: eq_session_manager::{Pallet, Call, Storage, Event<T>, Config<T>,} = 6,

        // Collator support. the order of these 4 are important and shall not change.
        Authorship: authorship::{Pallet, Call, Storage, Inherent} = 7,
        Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>} = 8,
        Aura: aura::{Pallet, Config<T>} = 9,
        AuraExt: cumulus_pallet_aura_ext::{Pallet, Storage, Config} = 10,

        EqAssets: eq_assets::{Pallet, Call, Config<T>, Storage, Event} = 11, // Assets genesis must be built first
        Oracle: eq_oracle::{Pallet, Call, Storage, Event<T>, Config, ValidateUnsigned} = 12,
        EqTreasury: eq_distribution::<Instance5>::{Pallet, Call, Storage, Config} = 13,
        Treasury: eq_treasury::{Pallet, Call, Storage, Config, Event<T>} = 14,
        EqBalances: eq_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 15,

        TransactionPayment: transaction_payment::{Pallet, Storage, Event<T>} = 16,
        Sudo: sudo::{Pallet, Call, Config<T>, Storage, Event<T>} = 17,
        Bailsman: eq_bailsman::{Pallet, Call, Config<T>, Storage, Event<T>, ValidateUnsigned} = 18,
        Whitelists: eq_whitelists::{Pallet, Call, Storage, Event<T>, Config<T>,} = 19,
        EqRate: eq_rate::{Pallet, Storage, Call, ValidateUnsigned} = 20,
        Republic: eq_distribution::<Instance2>::{Pallet, Call, Storage, Config} = 21,
        EqInvestors: eq_distribution::<Instance3>::{Pallet, Call, Storage, Config} = 22,

        EqLiquidityFarming: eq_distribution::<Instance4>::{Pallet, Call, Storage, Config} = 23,

        Vesting: eq_vesting::<Instance1>::{Pallet, Call, Storage, Event<T, Instance1>, Config<T, Instance1>} = 24,
        Vesting2: eq_vesting::<Instance2>::{Pallet, Call, Storage, Event<T, Instance2>, Config<T, Instance2>} = 25,
        Claims: eq_claim::{Pallet, Call, Storage, Event<T>, Config<T>, ValidateUnsigned} = 26,
        EqAggregates: eq_aggregates::{Pallet, Storage} = 27,
        Subaccounts: eq_subaccounts::{Pallet, Call, Storage, Event<T>, Config<T>} = 28,
        Financial: financial_pallet::{Pallet, Call, Storage, Config<T>, Event<T>} = 29,
        ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>, Config<T>} = 30,
        EqBridge: eq_bridge::{Pallet, Call, Storage, Event<T>, Config<T>} = 31,
        EqMultisigSudo: eq_multisig_sudo::{Pallet, Call, Storage, Config<T>, Event<T>} = 32,
        EqMarginCall: eq_margin_call::{Pallet, Call, Storage, Event<T>} = 33,
        EqDex: eq_dex::{Pallet, Call, Storage, Event<T>, Config, ValidateUnsigned} = 34,
        EqLending: eq_lending::{Pallet, Call, Storage, Event<T>, Config<T>} = 35,
        EqLockdrop: eq_lockdrop::{Pallet, Call, Storage, Event<T>, ValidateUnsigned, Config<T>} = 36,
        Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 37,
        Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>} = 38,
        EqMarketMaker: eq_market_maker::{Pallet, Call, Storage, Event<T>} = 39,
        Xdot: eq_xdot_pool::{Pallet, Call, Storage, Event<T>} = 40,
        Migration: eq_migration::{Pallet, Call, Storage, Event<T>} = 41,
        CurveAmm: equilibrium_curve_amm::{Pallet, Call, Storage, Event<T>} = 42,

        // XCM helpers.
        PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Storage, Origin, Config} = 43,
        DmpQueue: cumulus_pallet_dmp_queue::{Pallet, Call, Storage, Event<T>} = 44,
        XcmpQueue: cumulus_pallet_xcmp_queue::{Pallet, Call, Storage, Event<T>} = 45,

        EqWrappedDot: eq_wrapped_dot::{Pallet, Call, Storage, Config} = 46,

        // Governance
        Preimage: pallet_preimage = 60,
        Scheduler: pallet_scheduler = 61,

        Council: pallet_collective::<Instance1> = 62,
        CouncilMembership: pallet_membership::<Instance1> = 63,
        TechnicalCommittee: pallet_collective::<Instance2> = 64,
        TechnicalCommitteeMembership: pallet_membership::<Instance2> = 65,

        Democracy: pallet_democracy = 66,

        EqStaking: eq_staking::{Pallet, Call, Event<T>, Storage } = 67
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
    eq_claim::PrevalidateAttests<Runtime>,
    eq_treasury::CheckBuyout<Runtime>,
);

pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    CustomOnRuntimeUpgrade,
>;

#[derive(Clone, Eq, PartialEq, scale_info::TypeInfo)]
pub struct CallsWithReinit;
impl Contains<Call> for CallsWithReinit {
    fn contains(call: &Call) -> bool {
        matches!(call, Call::Subaccounts(..))
    }
}

pub struct CustomOnRuntimeUpgrade;

impl frame_support::traits::OnRuntimeUpgrade for CustomOnRuntimeUpgrade {
    fn on_runtime_upgrade() -> Weight {
        // eqBalances -> MigrationToggle
        let migration_toggle_prefix =
            hex_literal::hex!("276c90850b9de2c495875fe945d2a9c788461163b1bd03a8e4846c4f8ae5a3e8");
        // eqBalances -> TempMigration
        let temp_migration_prefix = hex_literal::hex!("276c90850b9de2c495875fe945d2a9c7b29bbb8274c33e88482bdf21e664a0932094525aa4a9b4e64503a0b0c04c66c04829b1e41449bd2cc7f04df856052f4d439f2f3e7f346c9702b94928ddf04707");
        let _ =
            frame_support::storage::unhashed::clear_prefix(&migration_toggle_prefix, None, None);
        let _ = frame_support::storage::unhashed::clear_prefix(&temp_migration_prefix, None, None);
        Weight::from_ref_time(1)
    }
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
        [eq_balances, BalancesBench::<Runtime>]
        [eq_distribution, DistriBench::<Runtime, DistriBenchInstance>]
        [eq_vesting, VestingBench::<Runtime, eq_vesting::Instance1>]
        [eq_subaccounts, SubaccountsBench::<Runtime>]
        [eq_treasury, TreasuryBench::<Runtime>]
        [eq_claim, Claims]
        [eq_whitelists, WhitelistsBench::<Runtime>]
        [eq_rate, RateBench::<Runtime>]
        [eq_lockdrop, LockdropBench::<Runtime>]
        [eq_session_manager, SessionManagerBench::<Runtime>]
        [equilibrium_curve_amm, CurveAmmBench::<Runtime>]
        [chainbridge, ChainBridge]
        [eq_bridge, BridgeBench::<Runtime>]
        [eq_assets, EqAssets]
        [eq_multisig_sudo, EqMultisigSudo]
        [eq_bailsman, BailsmanBench::<Runtime>]
        [eq_oracle, OracleBench::<Runtime>]
        [eq_dex, DexBench::<Runtime>]
        [eq_margin_call, MarginBench::<Runtime>]
        [eq_lending, LendingBench::<Runtime>]
        [eq_wrapped_dot, WrappedDotBench::<Runtime>]
        [pallet_preimage, Preimage]
        [pallet_scheduler, Scheduler]
        [pallet_collective, Council]
        [pallet_membership, CouncilMembership]
        [pallet_democracy, Democracy]
        [eq_staking, EqStakingBench::<Runtime>]
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

    impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
        fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
            ParachainSystem::collect_collation_info(header)
        }
    }

    impl eq_xdot_pool_rpc_runtime_api::EqXdotPoolApi<Block, Balance> for Runtime {
        fn invariant(
            pool_id: eq_primitives::xdot_pool::PoolId
        ) -> Option<u128> {
            Xdot::invariant(pool_id).ok()
        }

        fn fy_token_out_for_base_in(
            pool_id: eq_primitives::xdot_pool::PoolId,
            base_amount: Balance
        ) -> Option<Balance> {
            Xdot::fy_token_out_for_base_in(
                pool_id,
                base_amount
            ).ok()
        }

        fn base_out_for_fy_token_in(
           pool_id: eq_primitives::xdot_pool::PoolId,
           fy_token_amount: Balance
        ) -> Option<Balance> {
            Xdot::base_out_for_fy_token_in(
                pool_id,
                fy_token_amount
            ).ok()
        }

        fn fy_token_in_for_base_out(
            pool_id: eq_primitives::xdot_pool::PoolId,
            base_amount: Balance,
        ) -> Option<Balance> {
            Xdot::fy_token_in_for_base_out(
                pool_id,
                base_amount
            ).ok()
        }

        fn base_in_for_fy_token_out(
            pool_id: eq_primitives::xdot_pool::PoolId,
            fy_token_amount: Balance,
        ) -> Option<Balance> {
            Xdot::base_in_for_fy_token_out(
                pool_id,
                fy_token_amount
            ).ok()
        }

        fn base_out_for_lp_in(
            pool_id: eq_primitives::xdot_pool::PoolId,
            lp_in: Balance
        ) -> Option<Balance> {
                Xdot::base_out_for_lp_in(
                    pool_id,
                    lp_in
            ).ok()
        }

        fn base_and_fy_out_for_lp_in(
            pool_id: eq_primitives::xdot_pool::PoolId,
            lp_in: Balance
        ) -> Option<(Balance, Balance)> {
                Xdot::base_and_fy_out_for_lp_in(
                    pool_id,
                    lp_in
            ).ok()
        }

        fn max_base_xbase_in_and_out(
            pool_id: eq_primitives::xdot_pool::PoolId
        ) -> Option<(Balance, Balance, Balance, Balance)> {
            Xdot::max_base_xbase_in_and_out(
                pool_id
            ).ok()
        }
    }

    impl eq_balances_rpc_runtime_api::EqBalancesApi<Block, Balance, AccountId> for Runtime {
        fn wallet_balance_in_usd(account_id: AccountId) -> Option<Balance> {
            use eq_primitives::balance::BalanceGetter;

            let DebtCollateralDiscounted {debt, collateral, discounted_collateral: _} = EqBalances::get_debt_and_collateral(&account_id).ok()?;
            collateral.checked_sub(debt)
        }
        fn portfolio_balance_in_usd(account_id: AccountId) -> Option<Balance> {
            use eq_primitives::{balance::BalanceGetter, subaccount::SubaccountsManager};

            let DebtCollateralDiscounted { mut debt, mut collateral, discounted_collateral: _} = EqBalances::get_debt_and_collateral(&account_id).ok()?;
            for subacc_type in SubAccType::iterator() {
                if let Some(subacc_id) = Subaccounts::get_subaccount_id(&account_id, &subacc_type) {
                    let DebtCollateralDiscounted { debt: subacc_debt, collateral: subacc_collateral, discounted_collateral: _ } = EqBalances::get_debt_and_collateral(&subacc_id).ok()?;
                    debt = debt.saturating_add(subacc_debt);
                    collateral = collateral.saturating_add(subacc_collateral);
                }
            }
            collateral.checked_sub(debt)
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

            use eq_balances::benchmarking::Pallet as BalancesBench;
            use eq_bridge::benchmarking::Pallet as BridgeBench;
            use eq_distribution::benchmarking::Pallet as DistriBench;
            use eq_vesting::benchmarking::Pallet as VestingBench;
            use eq_subaccounts::benchmarking::Pallet as SubaccountsBench;
            use eq_treasury::benchmarking::Pallet as TreasuryBench;
            use eq_whitelists::benchmarking::Pallet as WhitelistsBench;
            use eq_rate::benchmarking::Pallet as RateBench;
            use eq_lockdrop::benchmarking::Pallet as LockdropBench;
            use eq_session_manager::benchmarking::Pallet as SessionManagerBench;
            use eq_bailsman::benchmarking::Pallet as BailsmanBench;
            use equilibrium_curve_amm::benchmarking::Pallet as CurveAmmBench;
            use eq_oracle::benchmarking::Pallet as OracleBench;
            use eq_dex::benchmarking::Pallet as DexBench;
            use eq_margin_call::benchmarking::Pallet as MarginBench;
            use eq_lending::benchmarking::Pallet as LendingBench;
            use eq_wrapped_dot::benchmarking::Pallet as WrappedDotBench;
            use eq_staking::benchmarking::Pallet as EqStakingBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();

            (list, storage_info)
        }


        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{add_benchmark, baseline, Benchmarking, BenchmarkBatch, TrackedStorageKey};

            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;

            impl frame_system_benchmarking::Config for Runtime {}
            impl baseline::Config for Runtime {}

            use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
            impl cumulus_pallet_session_benchmarking::Config for Runtime {}

            use eq_balances::benchmarking::Pallet as BalancesBench;
            impl eq_balances::benchmarking::Config for Runtime {}

            use eq_bridge::benchmarking::Pallet as BridgeBench;
            impl eq_bridge::benchmarking::Config for Runtime {}

            use eq_distribution::benchmarking::Pallet as DistriBench;
            impl eq_distribution::benchmarking::Config<DistriBenchInstance> for Runtime {}

            use eq_subaccounts::benchmarking::Pallet as SubaccountsBench;
            impl eq_subaccounts::benchmarking::Config for Runtime {}

            use eq_treasury::benchmarking::Pallet as TreasuryBench;
            impl eq_treasury::benchmarking::Config for Runtime {}

            use eq_whitelists::benchmarking::Pallet as WhitelistsBench;
            impl eq_whitelists::benchmarking::Config for Runtime {}

            use eq_rate::benchmarking::Pallet as RateBench;
            impl eq_rate::benchmarking::Config for Runtime {}

            use eq_lockdrop::benchmarking::Pallet as LockdropBench;
            impl eq_lockdrop::benchmarking::Config for Runtime {}

            use eq_session_manager::benchmarking::Pallet as SessionManagerBench;
            impl eq_session_manager::benchmarking::Config for Runtime {}

            use eq_bailsman::benchmarking::Pallet as BailsmanBench;
            impl eq_bailsman::benchmarking::Config for Runtime {}

            use eq_oracle::benchmarking::Pallet as OracleBench;
            impl eq_oracle::benchmarking::Config for Runtime {}

            use eq_vesting::benchmarking::Pallet as VestingBench;
            impl eq_vesting::benchmarking::Config<eq_vesting::Instance1> for Runtime {}

            use equilibrium_curve_amm::benchmarking::Pallet as CurveAmmBench;
            impl equilibrium_curve_amm::benchmarking::Config for Runtime {}

            use eq_dex::benchmarking::Pallet as DexBench;
            impl eq_dex::benchmarking::Config for Runtime {}

            use eq_margin_call::benchmarking::Pallet as MarginBench;
            impl eq_margin_call::benchmarking::Config for Runtime {}

            use eq_lending::benchmarking::Pallet as LendingBench;
            impl eq_lending::benchmarking::Config for Runtime {}

            use eq_wrapped_dot::benchmarking::Pallet as WrappedDotBench;
            impl eq_wrapped_dot::benchmarking::Config for Runtime {}

            use eq_staking::benchmarking::Pallet as EqStakingBench;
            impl eq_staking::benchmarking::Config for Runtime {}

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
    #[test]
    fn t() {
        let assets = [
            25969,
            6382433,
            6450786,
            6582132,
            6648164,
            1635087204,
            1651864420,
            1735159154,
            1768060003,
            2019848052,
            2036625268,
            517081101362,
            517081101363,
            1970496628,
            1970496611,
            2002941027,
            6648936,
        ];
        for a in assets {
            let asset = Asset::new(a).unwrap();
            println!("{:?} {:?}", eq_primitives::str_asset!(asset).unwrap(), a);
        }
    }
}
