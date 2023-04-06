
//! Autogenerated weights for `eq_wrapped_dot`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-01-09, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `muctep-osx-m1.local`, CPU: `<UNKNOWN>`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/production/eq-node
// benchmark
// pallet
// --chain=dev
// --execution=wasm
// --wasm-execution=compiled
// --pallet
// eq_wrapped_dot
// --extrinsic=*
// --steps
// 50
// --repeat
// 20
// --output
// ./runtime/equilibrium/src/weights/pallet_wrapped_dot.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_wrapped_dot`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_wrapped_dot::WeightInfo for WeightInfo<T> {
	// Storage: EqBalances TempMigration (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: EqAggregates AccountUserGroups (r:3 w:0)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqWrappedDot CurrentBalance (r:1 w:1)
	fn deposit() -> Weight {
		Weight::from_ref_time(78_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(9 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqWrappedDot CurrentBalance (r:1 w:1)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances TempMigration (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: EqAggregates AccountUserGroups (r:3 w:0)
	fn withdraw() -> Weight {
		Weight::from_ref_time(74_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(9 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	// Storage: EqWrappedDot CurrentBalance (r:1 w:0)
	// Storage: ParachainInfo ParachainId (r:1 w:0)
	// Storage: EqWrappedDot WithdrawQueue (r:1 w:1)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances TempMigration (r:2 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: Subaccounts OwnerAccount (r:1 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	fn withdraw_unbond() -> Weight {
		Weight::from_ref_time(89_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(16 as u64))
			.saturating_add(T::DbWeight::get().writes(5 as u64))
	}
	// Storage: EqWrappedDot CurrentBalance (r:1 w:1)
	// Storage: EqWrappedDot RelayStakingInfo (r:1 w:0)
	// Storage: EqWrappedDot LastWithdrawEra (r:1 w:1)
	// Storage: ParachainInfo ParachainId (r:1 w:0)
	// Storage: EqWrappedDot StakingRoutinePeriodicity (r:1 w:0)
	// Storage: EqWrappedDot WithdrawQueue (r:1 w:1)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances TempMigration (r:2 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: EqAggregates AccountUserGroups (r:6 w:0)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	/// The range of component `c` is `[1, 50]`.
	fn on_initialize(c: u32, ) -> Weight {
		Weight::from_ref_time(62_036_000 as u64)
			// Standard Error: 30_000
			.saturating_add(Weight::from_ref_time(24_230_000 as u64).saturating_mul(c as u64))
			.saturating_add(T::DbWeight::get().reads(14 as u64))
			.saturating_add(T::DbWeight::get().reads((5 as u64).saturating_mul(c as u64)))
			.saturating_add(T::DbWeight::get().writes(6 as u64))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(c as u64)))
	}
	// Storage: ParachainInfo ParachainId (r:1 w:0)
	// Storage: EqWrappedDot RelayStakingInfo (r:0 w:1)
	fn on_finalize() -> Weight {
		Weight::from_ref_time(5_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
