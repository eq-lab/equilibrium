
//! Autogenerated weights for `eq_session_manager`
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
// eq_session_manager
// --extrinsic=*
// --steps
// 10
// --repeat
// 5
// --output
// ./runtime/equilibrium/src/weights/pallet_session_manager.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_session_manager`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_session_manager::WeightInfo for WeightInfo<T> {
	// Storage: Session NextKeys (r:1 w:0)
	// Storage: EqSessionManager Validators (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: EqSessionManager IsChanged (r:0 w:1)
	fn add_validator() -> Weight {
		Weight::from_parts(45_983_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: EqSessionManager Validators (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	// Storage: EqSessionManager IsChanged (r:0 w:1)
	fn remove_validator() -> Weight {
		Weight::from_parts(55_232_000 as u64, 0)
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
}
