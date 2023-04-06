
//! Autogenerated weights for `eq_assets`
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
// eq_assets
// --extrinsic=*
// --steps
// 50
// --repeat
// 20
// --output
// ./runtime/equilibrium/src/weights/pallet_assets.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_assets`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_assets::WeightInfo for WeightInfo<T> {
	// Storage: EqAssets Assets (r:1 w:1)
	// Storage: FinancialModule PriceLogs (r:15 w:0)
	// Storage: FinancialModule Metrics (r:0 w:1)
	fn add_asset() -> Weight {
		Weight::from_ref_time(27_519_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(16 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: EqAssets Assets (r:1 w:1)
	// Storage: EqAssets AssetsToRemove (r:1 w:1)
	fn remove_asset() -> Weight {
		Weight::from_ref_time(17_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: EqAssets Assets (r:1 w:1)
	fn update_asset() -> Weight {
		Weight::from_ref_time(16_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
