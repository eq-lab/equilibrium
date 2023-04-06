
//! Autogenerated weights for `eq_balances`
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
// eq_balances
// --extrinsic=*
// --steps
// 50
// --repeat
// 20
// --output
// ./runtime/equilibrium/src/weights/pallet_balances.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_balances`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_balances::WeightInfo for WeightInfo<T> {
	// Storage: EqBalances IsTransfersEnabled (r:0 w:1)
	fn enable_transfers() -> Weight {
		Weight::from_ref_time(4_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: EqBalances IsTransfersEnabled (r:0 w:1)
	fn disable_transfers() -> Weight {
		Weight::from_ref_time(4_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: EqBalances IsTransfersEnabled (r:1 w:0)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances TempMigration (r:2 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	fn transfer() -> Weight {
		Weight::from_ref_time(82_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(15 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqBalances XcmNativeTransfers (r:1 w:1)
	/// The range of component `z` is `[1, 100]`.
	fn allow_xcm_transfers_native_for(z: u32, ) -> Weight {
		Weight::from_ref_time(7_607_000 as u64)
			// Standard Error: 2_000
			.saturating_add(Weight::from_ref_time(1_740_000 as u64).saturating_mul(z as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(z as u64)))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(z as u64)))
	}
	// Storage: EqBalances XcmNativeTransfers (r:0 w:1)
	/// The range of component `z` is `[1, 100]`.
	fn forbid_xcm_transfers_native_for(z: u32, ) -> Weight {
		Weight::from_ref_time(3_381_000 as u64)
			// Standard Error: 2_000
			.saturating_add(Weight::from_ref_time(873_000 as u64).saturating_mul(z as u64))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(z as u64)))
	}
	// Storage: EqBalances DailyXcmLimit (r:0 w:1)
	fn update_xcm_transfer_native_limit() -> Weight {
		Weight::from_ref_time(4_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: EqBalances IsXcmTransfersEnabled (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances TempMigration (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: EqAggregates AccountUserGroups (r:3 w:0)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	fn xcm_transfer_native() -> Weight {
		Weight::from_ref_time(58_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(8 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: EqBalances IsXcmTransfersEnabled (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: ParachainInfo ParachainId (r:1 w:0)
	// Storage: EqBalances TempMigration (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: EqAggregates AccountUserGroups (r:3 w:0)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	fn xcm_transfer() -> Weight {
		Weight::from_ref_time(63_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(9 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: EqBalances MigrationToggle (r:1 w:0)
	// Storage: EqBalances Account (r:3 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	/// The range of component `a` is `[1, 100]`.
	fn on_initialize(a: u32, ) -> Weight {
		Weight::from_ref_time(0 as u64)
			// Standard Error: 48_000
			.saturating_add(Weight::from_ref_time(47_510_000 as u64).saturating_mul(a as u64))
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().reads((3 as u64).saturating_mul(a as u64)))
			.saturating_add(T::DbWeight::get().writes((3 as u64).saturating_mul(a as u64)))
	}
}
