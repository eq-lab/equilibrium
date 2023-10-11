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

use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sp_runtime::{DispatchError, RuntimeDebug};

/// Types of subaccounts. Every master account can have only one subaccount of
/// each type
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    Hash,
    scale_info::TypeInfo,
    Serialize,
    Deserialize,
)]
#[repr(u8)]
pub enum SubAccType {
    /// Subaccount for with balances used to register and operate as a bailsman
    Bailsman,
    /// Subaccount for with balances used to generate debt and dex trading
    Trader,
    /// Same as trader
    Borrower,
}

impl SubAccType {
    /// Returns iterator over all variants of `SubAccType`
    pub fn iterator() -> impl Iterator<Item = SubAccType> {
        IntoIterator::into_iter([
            SubAccType::Bailsman,
            SubAccType::Trader,
            SubAccType::Borrower,
        ])
    }
}

/// Methods for creating, deleting and checking subaccounts
pub trait SubaccountsManager<AccountId> {
    /// Creates subaccount, generating random `AccountId`, and storing it into pallet's
    /// `OwnerAccount` and `Subaccount` storage.
    fn create_subaccount_inner(
        who: &AccountId,
        subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError>;

    /// Deletes subaccount of given `subacc_type`. Reads Id from storage, then
    /// removes storage values from `Subaccount` and `OwnerAccount`. Returns deleted
    /// subaccount `AccountId`
    fn delete_subaccount_inner(
        who: &AccountId,
        subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError>;

    /// Returns `true` if `who` has a subaccount of given `subacc_type`
    fn has_subaccount(who: &AccountId, subacc_type: &SubAccType) -> bool;

    /// Returns `Some(AccountId)` of given subaccount type if exist and `None` otherwise
    fn get_subaccount_id(who: &AccountId, subacc_type: &SubAccType) -> Option<AccountId>;

    /// Returns `true` if `subaccount_id` is subaccount of `who`
    fn is_subaccount(who: &AccountId, subaccount_id: &AccountId) -> bool;

    /// Returns Some with tuple: master account and current subaccount type.
    /// If account is master returns None.
    fn get_owner_id(subaccount: &AccountId) -> Option<(AccountId, SubAccType)>;

    fn is_master(who: &AccountId) -> bool {
        Self::get_owner_id(who).is_none()
    }

    /// Returns amount of subaccounts for `who` account
    fn get_subaccounts_amount(who: &AccountId) -> usize;
}

impl<AccountId> SubaccountsManager<AccountId> for () {
    fn create_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        Err(DispatchError::Other("Subaccounts not implemented"))
    }

    fn delete_subaccount_inner(
        _who: &AccountId,
        _subacc_type: &SubAccType,
    ) -> Result<AccountId, DispatchError> {
        Err(DispatchError::Other("Subaccounts not implemented"))
    }

    fn has_subaccount(_who: &AccountId, _subacc_type: &SubAccType) -> bool {
        false
    }

    fn get_subaccount_id(_who: &AccountId, _subacc_type: &SubAccType) -> Option<AccountId> {
        None
    }

    fn is_subaccount(_who: &AccountId, _subaccount_id: &AccountId) -> bool {
        false
    }

    fn get_owner_id(_subaccount: &AccountId) -> Option<(AccountId, SubAccType)> {
        None
    }

    fn get_subaccounts_amount(_who: &AccountId) -> usize {
        0
    }
}
