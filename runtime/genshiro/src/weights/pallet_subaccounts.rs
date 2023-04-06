
//! Autogenerated weights for `eq_subaccounts`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-10-17, STEPS: `10`, REPEAT: 5, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `ivan-GP76`, CPU: `11th Gen Intel(R) Core(TM) i7-11800H @ 2.30GHz`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/eq-node
// benchmark
// pallet
// --chain=dev
// --execution=wasm
// --wasm-execution=compiled
// --pallet
// eq_subaccounts
// --extrinsic=*
// --steps
// 10
// --repeat
// 5
// --output
// ./runtime/equilibrium/src/weights/pallet_subaccounts.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_subaccounts`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_subaccounts::WeightInfo for WeightInfo<T> {
	// Storage: EqBalances IsTransfersEnabled (r:1 w:0)
	// Storage: Subaccounts Subaccount (r:1 w:1)
	// Storage: Whitelists WhiteList (r:1 w:0)
	// Storage: unknown [0x3a65787472696e7369635f696e646578] (r:1 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: EqAggregates AccountUserGroups (r:6 w:2)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances Account (r:1 w:0)
	// Storage: Subaccounts OwnerAccount (r:1 w:1)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: Oracle PricePoints (r:1 w:0)
	// Storage: Bailsman BailsmenCount (r:1 w:1)
	// Storage: Bailsman DistributionQueue (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	// Storage: Bailsman LastDistribution (r:0 w:1)
	fn transfer_to_bailsman_register() -> Weight {
		Weight::from_ref_time(219_442_000 as u64)
			.saturating_add(T::DbWeight::get().reads(22 as u64))
			.saturating_add(T::DbWeight::get().writes(11 as u64))
	}
	// Storage: EqBalances IsTransfersEnabled (r:1 w:0)
	// Storage: Subaccounts Subaccount (r:1 w:1)
	// Storage: Whitelists WhiteList (r:1 w:0)
	// Storage: unknown [0x3a65787472696e7369635f696e646578] (r:1 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: EqAggregates AccountUserGroups (r:6 w:2)
	// Storage: EqBalances Account (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: Subaccounts OwnerAccount (r:1 w:1)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	fn transfer_to_borrower_register() -> Weight {
		Weight::from_ref_time(187_264_000 as u64)
			.saturating_add(T::DbWeight::get().reads(19 as u64))
			.saturating_add(T::DbWeight::get().writes(9 as u64))
	}
	// Storage: EqBalances IsTransfersEnabled (r:1 w:0)
	// Storage: Subaccounts Subaccount (r:1 w:1)
	// Storage: Whitelists WhiteList (r:1 w:0)
	// Storage: unknown [0x3a65787472696e7369635f696e646578] (r:1 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: System Account (r:3 w:2)
	// Storage: EqAggregates AccountUserGroups (r:6 w:2)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances Account (r:2 w:0)
	// Storage: Subaccounts OwnerAccount (r:1 w:1)
	// Storage: Oracle PricePoints (r:2 w:0)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqLending OnlyBailsmanTill (r:1 w:0)
	// Storage: EqLending LendersAggregates (r:1 w:0)
	// Storage: EqDex AssetWeightByAccountId (r:1 w:0)
	// Storage: EqMarginCall MaintenanceTimers (r:1 w:0)
	// Storage: Bailsman BailsmenCount (r:1 w:1)
	// Storage: Bailsman DistributionQueue (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	// Storage: Bailsman LastDistribution (r:0 w:1)
	/// The range of component `r` is `[0, 50]`.
	fn transfer_to_bailsman_and_redistribute(r: u32, ) -> Weight {
		Weight::from_ref_time(532_745_000 as u64)
			// Standard Error: 703_000
			.saturating_add(Weight::from_ref_time(2_142_000 as u64).saturating_mul(r as u64))
			.saturating_add(T::DbWeight::get().reads(38 as u64))
			.saturating_add(T::DbWeight::get().writes(11 as u64))
	}
	// Storage: EqBalances IsTransfersEnabled (r:1 w:0)
	// Storage: Subaccounts Subaccount (r:1 w:1)
	// Storage: Whitelists WhiteList (r:1 w:0)
	// Storage: unknown [0x3a65787472696e7369635f696e646578] (r:1 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: System Account (r:3 w:2)
	// Storage: EqAggregates AccountUserGroups (r:6 w:2)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances Account (r:2 w:0)
	// Storage: Subaccounts OwnerAccount (r:1 w:1)
	// Storage: Oracle PricePoints (r:2 w:0)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqLending OnlyBailsmanTill (r:1 w:0)
	// Storage: EqLending LendersAggregates (r:1 w:0)
	// Storage: EqDex AssetWeightByAccountId (r:1 w:0)
	// Storage: EqMarginCall MaintenanceTimers (r:1 w:0)
	// Storage: Bailsman BailsmenCount (r:1 w:1)
	// Storage: Bailsman DistributionQueue (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	// Storage: Bailsman LastDistribution (r:0 w:1)
	fn transfer_to_subaccount() -> Weight {
		Weight::from_ref_time(375_832_000 as u64)
			.saturating_add(T::DbWeight::get().reads(29 as u64))
			.saturating_add(T::DbWeight::get().writes(11 as u64))
	}
	// Storage: EqBalances IsTransfersEnabled (r:1 w:0)
	// Storage: Subaccounts Subaccount (r:1 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	// Storage: Bailsman LastDistribution (r:1 w:1)
	// Storage: Bailsman DistributionQueue (r:1 w:0)
	// Storage: Oracle PricePoints (r:1 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: System Account (r:3 w:2)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqLending OnlyBailsmanTill (r:1 w:0)
	// Storage: EqLending LendersAggregates (r:1 w:0)
	// Storage: EqBalances Account (r:1 w:0)
	// Storage: EqDex AssetWeightByAccountId (r:1 w:0)
	// Storage: EqMarginCall MaintenanceTimers (r:1 w:0)
	// Storage: Bailsman BailsmenCount (r:1 w:1)
	fn transfer_from_subaccount() -> Weight {
		Weight::from_ref_time(321_648_000 as u64)
			.saturating_add(T::DbWeight::get().reads(27 as u64))
			.saturating_add(T::DbWeight::get().writes(7 as u64))
	}
	// Storage: EqBalances IsTransfersEnabled (r:1 w:0)
	// Storage: Subaccounts Subaccount (r:1 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	// Storage: Bailsman LastDistribution (r:1 w:1)
	// Storage: Bailsman DistributionQueue (r:1 w:0)
	// Storage: Oracle PricePoints (r:1 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: System Account (r:3 w:2)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqLending OnlyBailsmanTill (r:1 w:0)
	// Storage: EqLending LendersAggregates (r:1 w:0)
	// Storage: EqBalances Account (r:1 w:0)
	// Storage: EqDex AssetWeightByAccountId (r:1 w:0)
	// Storage: EqMarginCall MaintenanceTimers (r:1 w:0)
	// Storage: Bailsman BailsmenCount (r:1 w:1)
	/// The range of component `r` is `[0, 50]`.
	fn transfer_from_subaccount_redistribute(r: u32, ) -> Weight {
		Weight::from_ref_time(1_700_719_000 as u64)
			// Standard Error: 6_411_000
			.saturating_add(Weight::from_ref_time(26_414_000 as u64).saturating_mul(r as u64))
			.saturating_add(T::DbWeight::get().reads(60 as u64))
			.saturating_add(T::DbWeight::get().writes(28 as u64))
	}
}
