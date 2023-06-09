
//! Autogenerated weights for `eq_oracle`
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
// eq_oracle
// --extrinsic=*
// --steps
// 50
// --repeat
// 20
// --output
// ./runtime/equilibrium/src/weights/pallet_oracle.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_oracle`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_oracle::WeightInfo for WeightInfo<T> {
	// Storage: Whitelists WhiteList (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: Oracle PricePoints (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: FinancialModule Updates (r:1 w:1)
	// Storage: FinancialModule PriceLogs (r:1 w:0)
	/// The range of component `b` is `[1, 20]`.
	fn set_price(b: u32, ) -> Weight {
		Weight::from_parts(42_914_000 as u64, 0)
			// Standard Error: 7_000
			.saturating_add(Weight::from_parts(156_000 as u64, 0).saturating_mul(b as u64))
			.saturating_add(T::DbWeight::get().reads(6 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
}
