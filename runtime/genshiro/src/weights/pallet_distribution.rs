
//! Autogenerated weights for `eq_distribution`
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
// eq_distribution
// --extrinsic=*
// --steps
// 10
// --repeat
// 5
// --output
// ./runtime/equilibrium/src/weights/pallet_distribution.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_distribution`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_distribution::WeightInfo for WeightInfo<T> {
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqBalances Account (r:1 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	fn transfer() -> Weight {
		Weight::from_ref_time(139_039_000 as u64)
			.saturating_add(T::DbWeight::get().reads(15 as u64))
			.saturating_add(T::DbWeight::get().writes(5 as u64))
	}
	// Storage: Vesting Vesting (r:1 w:1)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: System Account (r:3 w:3)
	// Storage: Subaccounts OwnerAccount (r:3 w:0)
	// Storage: EqBalances Account (r:2 w:0)
	// Storage: EqAggregates AccountUserGroups (r:9 w:2)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: Vesting Vested (r:1 w:1)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	fn vested_transfer() -> Weight {
		Weight::from_ref_time(255_860_000 as u64)
			.saturating_add(T::DbWeight::get().reads(23 as u64))
			.saturating_add(T::DbWeight::get().writes(9 as u64))
	}
}
