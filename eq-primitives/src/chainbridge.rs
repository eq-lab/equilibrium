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

use frame_support::PalletId;

/// AccountId for the pallet
pub const MODULE_ID: PalletId = PalletId(*b"eq/bridg");
/// AccountId to which the fees will be transferred
pub const FEE_MODULE_ID: PalletId = PalletId(*b"eq/feebr");

/// All chains have unique ID
pub type ChainId = u8;
/// Transaction counter
pub type DepositNonce = u64;
/// All tokens have unique ResourceId
pub type ResourceId = [u8; 32];

use frame_support::dispatch::DispatchResultWithPostInfo;
use sp_std::vec::Vec;

use crate::asset::Asset;

pub trait Bridge<AccountId, Balance, ChainId, ResourceId> {
    /// Transfer `amount` of asset associated with `resource_id`
    /// from `source` account to `recipient` on chain with `dest_id`
    fn transfer_native(
        source: AccountId,
        amount: Balance,
        recipient: Vec<u8>,
        dest_id: ChainId,
        resource_id: ResourceId,
    ) -> DispatchResultWithPostInfo;

    fn get_fee(dest_id: ChainId) -> Balance;
}

pub trait ResourceGetter<ResourceId> {
    fn get_resource_by_asset(asset: Asset) -> Option<ResourceId>;

    fn get_asset_by_resource(resource_id: ResourceId) -> Option<Asset>;
}
