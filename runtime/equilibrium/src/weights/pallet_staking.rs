
//! Autogenerated weights for `eq_staking`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-01-24, STEPS: `1`, REPEAT: 1, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `MacBook-Pro-Maks.local`, CPU: `<UNKNOWN>`
//! EXECUTION: None, WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/eq-node
// benchmark
// pallet
// --chain
// dev
// --pallet=eq_staking
// --extrinsic=*
// --output
// ./runtime/equilibrium/src/weights/pallet_staking.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_staking`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_staking::WeightInfo for WeightInfo<T> {
	// Storage: EqBalances Locked (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqStaking Stakes (r:1 w:1)
	fn stake() -> Weight {
		Weight::from_parts(37_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: EqStaking Rewards (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances TempMigration (r:2 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:0)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	// Storage: EqBalances Locked (r:1 w:1)
	fn reward() -> Weight {
		Weight::from_parts(92_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(17 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: EqStaking Stakes (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqBalances Locked (r:1 w:1)
	fn unlock_stake() -> Weight {
		Weight::from_parts(34_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: EqStaking Rewards (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqBalances Locked (r:1 w:1)
	fn unlock_reward() -> Weight {
		Weight::from_parts(35_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
}
