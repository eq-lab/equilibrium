
//! Autogenerated weights for `eq_bridge`
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
// eq_bridge
// --extrinsic=*
// --steps
// 10
// --repeat
// 5
// --output
// ./runtime/equilibrium/src/weights/pallet_bridge.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `eq_bridge`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> eq_bridge::WeightInfo for WeightInfo<T> {
	// Storage: ChainBridge ChainNonces (r:1 w:1)
	// Storage: ChainBridge DisabledChains (r:1 w:0)
	// Storage: EqBridge EnabledWithdrawals (r:1 w:0)
	// Storage: EqBridge MinimumTransferAmount (r:1 w:0)
	// Storage: EqBridge Resources (r:1 w:0)
	// Storage: ChainBridge Fees (r:1 w:0)
	// Storage: Subaccounts OwnerAccount (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: EqAggregates AccountUserGroups (r:3 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	fn transfer_native() -> Weight {
		Weight::from_ref_time(133_506_000 as u64)
			.saturating_add(T::DbWeight::get().reads(13 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: EqBridge Resources (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: Oracle PricePoints (r:2 w:0)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqBalances Account (r:1 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	// Storage: EqAggregates TotalUserGroups (r:2 w:2)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	fn transfer() -> Weight {
		Weight::from_ref_time(261_189_000 as u64)
			.saturating_add(T::DbWeight::get().reads(19 as u64))
			.saturating_add(T::DbWeight::get().writes(6 as u64))
	}
	// Storage: EqBridge Resources (r:1 w:0)
	// Storage: EqAssets Assets (r:1 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: Subaccounts OwnerAccount (r:2 w:0)
	// Storage: EqBalances Account (r:1 w:0)
	// Storage: EqAggregates AccountUserGroups (r:6 w:1)
	// Storage: EqAggregates TotalUserGroups (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: EqRate NowMillisOffset (r:1 w:0)
	// Storage: EqRate LastFeeUpdate (r:0 w:1)
	fn transfer_basic() -> Weight {
		Weight::from_ref_time(145_939_000 as u64)
			.saturating_add(T::DbWeight::get().reads(16 as u64))
			.saturating_add(T::DbWeight::get().writes(5 as u64))
	}
	fn remark() -> Weight {
		Weight::from_ref_time(16_978_000 as u64)
	}
	// Storage: EqBridge Resources (r:0 w:1)
	// Storage: EqBridge AssetResource (r:0 w:1)
	fn set_resource() -> Weight {
		Weight::from_ref_time(8_414_000 as u64)
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: EqBridge EnabledWithdrawals (r:1 w:1)
	fn enable_withdrawals() -> Weight {
		Weight::from_ref_time(23_057_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: EqBridge EnabledWithdrawals (r:1 w:1)
	fn disable_withdrawals() -> Weight {
		Weight::from_ref_time(24_146_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: ChainBridge ChainNonces (r:1 w:0)
	// Storage: EqBridge Resources (r:1 w:0)
	// Storage: EqBridge MinimumTransferAmount (r:0 w:1)
	fn set_minimum_transfer_amount() -> Weight {
		Weight::from_ref_time(29_159_000 as u64)
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
