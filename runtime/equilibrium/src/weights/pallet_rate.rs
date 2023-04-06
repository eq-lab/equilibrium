
//! Autogenerated weights for `eq_rate`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-12-14, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `muctep-osx-m1.local`, CPU: `<UNKNOWN>`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("eq-dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/eq-node
// benchmark
// pallet
// --chain=eq-dev
// --execution=wasm
// --wasm-execution=compiled
// --pallet=eq_rate
// --extrinsic=*
// --steps=50
// --repeat=20
// --output=./runtime/equilibrium/src/weights/pallet_rate.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_rate`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_rate::WeightInfo for WeightInfo<T> {
	// Storage: EqRate Keys (r:1 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	// Storage: System Account (r:2 w:2)
	// Storage: EqDex AssetWeightByAccountId (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: Oracle PricePoints (r:18 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqMarginCall MaintenanceTimers (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:1 w:1)
	// Storage: FinancialModule Metrics (r:1 w:0)
	// Storage: EqAggregates TotalUserGroups (r:39 w:3)
	// Storage: EqRate AutoReinitEnabled (r:1 w:0)
	// Storage: Authorship Author (r:1 w:0)
	// Storage: System Digest (r:1 w:0)
	// Storage: EqBalances Account (r:1 w:0)
	fn reinit() -> Weight {
		Weight::from_ref_time(3_362_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(77 as u64))
			.saturating_add(T::DbWeight::get().writes(7 as u64))
	}
	// Storage: EqRate LastFeeUpdate (r:1 w:1)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	// Storage: System Account (r:2 w:2)
	// Storage: EqDex AssetWeightByAccountId (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: Oracle PricePoints (r:18 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqMarginCall MaintenanceTimers (r:1 w:0)
	// Storage: FinancialModule Metrics (r:1 w:0)
	// Storage: EqAggregates TotalUserGroups (r:39 w:3)
	// Storage: Authorship Author (r:1 w:0)
	// Storage: System Digest (r:1 w:0)
	// Storage: EqBalances Account (r:1 w:0)
	fn reinit_external() -> Weight {
		Weight::from_ref_time(1_795_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(75 as u64))
			.saturating_add(T::DbWeight::get().writes(7 as u64))
	}
	// Storage: EqRate Keys (r:1 w:0)
	// Storage: Subaccounts OwnerAccount (r:1 w:0)
	// Storage: Subaccounts Subaccount (r:4 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: Oracle PricePoints (r:1 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances Reserved (r:1 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:2)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqBalances Account (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	fn delete_account() -> Weight {
		Weight::from_ref_time(252_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(22 as u64))
			.saturating_add(T::DbWeight::get().writes(7 as u64))
	}
	// Storage: Subaccounts OwnerAccount (r:1 w:0)
	// Storage: Subaccounts Subaccount (r:4 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: Oracle PricePoints (r:1 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances Reserved (r:1 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:2)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqBalances Account (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	fn delete_account_external() -> Weight {
		Weight::from_ref_time(173_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(21 as u64))
			.saturating_add(T::DbWeight::get().writes(7 as u64))
	}
	// Storage: EqRate AutoReinitEnabled (r:0 w:1)
	fn set_auto_reinit_enabled() -> Weight {
		Weight::from_ref_time(4_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
