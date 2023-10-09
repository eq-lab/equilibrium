// This file is part of Equilibrium.

// Copyright (C) 2023 EQ Lab.
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]
#![forbid(unsafe_code)]
#![deny(warnings)]

pub use eq_primitives::balance::Balance;
use frame_support::{traits::ContainsPair, weights::Weight};
pub use sp_runtime::{
    generic::DigestItem,
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    MultiSignature,
};
pub use sp_std::prelude::*;
use xcm::v3::{AssetId, Junction::Parachain, Junctions::X1, MultiAsset, MultiLocation};

pub mod mocks;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

pub const MILLISECS_PER_BLOCK: u64 = 12000;
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;
pub const WEEKS: BlockNumber = 7 * DAYS;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;
    use sp_runtime::generic;
    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// A Block signed with a Justification
    pub type SignedBlock = generic::SignedBlock<Block>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;
}

pub struct NoTeleport;
impl ContainsPair<MultiAsset, MultiLocation> for NoTeleport {
    fn contains(_asset: &MultiAsset, _origin: &MultiLocation) -> bool {
        false
    }
}

pub fn multiply_by_rational_weight(a: Weight, b: Weight, c: Weight) -> Weight {
    Weight::from_parts(
        (a.ref_time() * b.ref_time()).saturating_div(c.ref_time()),
        0, // zero proof size till it start working
           // (a.proof_size() * b.proof_size()).saturating_div(c.proof_size()),
    )
}

pub trait Reserve {
    /// Returns assets reserve location.
    fn reserve(&self) -> Option<MultiLocation>;
}

// Takes the chain part of a MultiAsset
impl Reserve for MultiAsset {
    fn reserve(&self) -> Option<MultiLocation> {
        if let AssetId::Concrete(location) = self.id.clone() {
            let first_interior = location.first_interior();
            let parents = location.parent_count();
            match (parents, first_interior.clone()) {
                (0, Some(Parachain(id))) => Some(MultiLocation::new(0, X1(Parachain(id.clone())))),
                (1, Some(Parachain(id))) => Some(MultiLocation::new(1, X1(Parachain(id.clone())))),
                (1, _) => Some(MultiLocation::parent()),
                _ => None,
            }
        } else {
            None
        }
    }
}

pub struct MultiNativeAsset;
impl ContainsPair<MultiAsset, MultiLocation> for MultiNativeAsset {
    fn contains(asset: &MultiAsset, origin: &MultiLocation) -> bool {
        if let Some(ref reserve) = asset.reserve() {
            if reserve == origin {
                return true;
            }
        }
        false
    }
}

/// Maximum number of blocks simultaneously accepted by the Runtime, not yet included
/// into the relay chain.
pub const UNINCLUDED_SEGMENT_CAPACITY: u32 = 1;
/// How many parachain blocks are processed by the relay chain per parent. Limits the
/// number of blocks authored per slot.
pub const BLOCK_PROCESSING_VELOCITY: u32 = 1;
/// Relay chain slot duration, in milliseconds.
pub const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6000;
