
//! Autogenerated weights for `eq_treasury`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-12-05, STEPS: `10`, REPEAT: 5, LOW RANGE: `[]`, HIGH RANGE: `[]`
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
// eq_treasury
// --extrinsic=*
// --steps
// 10
// --repeat
// 5
// --output
// ./runtime/equilibrium/src/weights/pallet_treasury.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_treasury`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_treasury::WeightInfo for WeightInfo<T> {
	// Storage: Oracle PricePoints (r:2 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: Treasury BuyoutLimit (r:1 w:0)
	// Storage: Treasury Buyouts (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:0)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	fn buyout() -> Weight {
		Weight::from_parts(172_807_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(17 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: Treasury BuyoutLimit (r:0 w:1)
	fn update_buyout_limit() -> Weight {
		Weight::from_parts(5_778_000 as u64, 0)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
