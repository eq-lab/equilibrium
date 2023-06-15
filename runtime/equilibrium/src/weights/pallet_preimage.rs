
//! Autogenerated weights for `pallet_preimage`
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
// pallet_preimage
// --extrinsic=*
// --steps
// 50
// --repeat
// 20
// --output
// ./runtime/equilibrium/src/weights/pallet_preimage.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_preimage`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_preimage::WeightInfo for WeightInfo<T> {
	// Storage: Preimage PreimageFor (r:1 w:1)
	// Storage: Preimage StatusFor (r:1 w:1)
	// Storage: EqBalances Reserved (r:1 w:1)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances TempMigration (r:2 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	/// The range of component `s` is `[0, 2097152]`.
	fn note_preimage(s: u32, ) -> Weight {
		Weight::from_parts(58_203_000 as u64, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_000 as u64, 0).saturating_mul(s as u64))
			.saturating_add(T::DbWeight::get().reads(16 as u64))
			.saturating_add(T::DbWeight::get().writes(6 as u64))
	}
	// Storage: Preimage PreimageFor (r:1 w:1)
	// Storage: Preimage StatusFor (r:1 w:0)
	/// The range of component `s` is `[0, 2097152]`.
	fn note_requested_preimage(s: u32, ) -> Weight {
		Weight::from_parts(0 as u64, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_000 as u64, 0).saturating_mul(s as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Preimage PreimageFor (r:1 w:1)
	// Storage: Preimage StatusFor (r:1 w:0)
	/// The range of component `s` is `[0, 2097152]`.
	fn note_no_deposit_preimage(s: u32, ) -> Weight {
		Weight::from_parts(0 as u64, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_000 as u64, 0).saturating_mul(s as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Preimage StatusFor (r:1 w:1)
	// Storage: EqBalances Reserved (r:1 w:1)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances TempMigration (r:2 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:0)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	// Storage: Preimage PreimageFor (r:0 w:1)
	fn unnote_preimage() -> Weight {
		Weight::from_parts(93_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(15 as u64))
			.saturating_add(T::DbWeight::get().writes(5 as u64))
	}
	// Storage: Preimage StatusFor (r:1 w:1)
	// Storage: Preimage PreimageFor (r:0 w:1)
	fn unnote_no_deposit_preimage() -> Weight {
		Weight::from_parts(17_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Preimage StatusFor (r:1 w:1)
	// Storage: EqBalances Reserved (r:1 w:1)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqBalances TempMigration (r:2 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:0)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	fn request_preimage() -> Weight {
		Weight::from_parts(90_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(15 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: Preimage StatusFor (r:1 w:1)
	fn request_no_deposit_preimage() -> Weight {
		Weight::from_parts(16_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Preimage StatusFor (r:1 w:1)
	fn request_unnoted_preimage() -> Weight {
		Weight::from_parts(14_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Preimage StatusFor (r:1 w:1)
	fn request_requested_preimage() -> Weight {
		Weight::from_parts(7_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Preimage StatusFor (r:1 w:1)
	// Storage: Preimage PreimageFor (r:0 w:1)
	fn unrequest_preimage() -> Weight {
		Weight::from_parts(18_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Preimage StatusFor (r:1 w:1)
	// Storage: Preimage PreimageFor (r:0 w:1)
	fn unrequest_unnoted_preimage() -> Weight {
		Weight::from_parts(15_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Preimage StatusFor (r:1 w:1)
	fn unrequest_multi_referenced_preimage() -> Weight {
		Weight::from_parts(7_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
