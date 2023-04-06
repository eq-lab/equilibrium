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

use eq_primitives::balance::Balance;
use frame_support::pallet_prelude::*;
use sp_runtime::traits::{AccountIdLookup, MaybeDisplay, StaticLookup};
use sp_std::fmt::Debug;

pub trait RelaySystemConfig {
    /// The user account identifier type for the runtime.
    type AccountId: Parameter
        + Member
        + MaybeSerializeDeserialize
        + Debug
        + MaybeDisplay
        + Ord
        + MaxEncodedLen;

    /// Converting trait to take a source type and convert to `AccountId`.
    ///
    /// Used to define the type and conversion mechanism for referencing accounts in
    /// transactions. It's perfectly reasonable for this to be an identity conversion (with the
    /// source type being `AccountId`), but other pallets (e.g. Indices pallet) may provide more
    /// functional/efficient alternatives.
    type Lookup: StaticLookup<Target = Self::AccountId>;

    /// Just the `Currency::Balance` type; we have this item to allow us to constrain it to
    /// `From<u64>`.
    type Balance: sp_runtime::traits::AtLeast32BitUnsigned
        + codec::FullCodec
        + Copy
        + MaybeSerializeDeserialize
        + sp_std::fmt::Debug
        + Default
        + From<u64>
        + TypeInfo
        + MaxEncodedLen;
}

pub type AccountId = sp_runtime::AccountId32;

#[derive(RuntimeDebug)]
pub struct RelayRuntime;

impl RelaySystemConfig for RelayRuntime {
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type Balance = Balance;
}
