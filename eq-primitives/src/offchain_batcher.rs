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

#[allow(unused_imports)]
use frame_support::debug;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::Vec;

/// Errors for offchain worker operations
#[derive(RuntimeDebug)]
pub enum OffchainErr {
    /// The signature is invalid
    FailedSigning,
    /// Transaction was not submitted
    SubmitTransaction,
}

/// Result of batcher execute
pub type OffchainResult<T = ()> = Result<T, OffchainErr>;

/// Used for execute batch of operations in offchain worker
/// on validators nodes
pub trait ValidatorOffchainBatcher<AuthorityId, BlockNumber: Copy, AccountId> {
    /// Execute batch operations for every `AuthorityId` key in keys storage
    /// - `block_number` - block number on which offchain worker starts
    /// - `batch_for_single_auth` - `fn` that executes batch with parameters
    ///     - authority_index: u32,
    ///     - key: AuthorityId,
    ///     - block_number: BlockNumber,
    ///     - validators_len: u32,
    /// - `pallet_name` - pallet name, used for logs
    fn execute_batch<F>(
        block_number: BlockNumber,
        batch_for_single_auth: F,
        pallet_name: &str,
    ) -> OffchainResult<()>
    where
        F: Fn(u32, AuthorityId, BlockNumber, u32) -> OffchainResult<()>,
    {
        // get suitable keys from keys storage
        let keys = Self::local_authority_keys();

        let validators_len = Self::get_validators_len();

        let batch_result = keys.into_iter().map(move |(authority_index, key)| {
            batch_for_single_auth(authority_index, key, block_number, validators_len)
        });

        for res in batch_result {
            if let Err(err) = res {
                log::trace!(target: "offchain_batcher", "{:?} offchain_worker:error {:?}", pallet_name, err);
            }
        }

        Ok(())
    }

    fn authority_keys() -> Vec<AuthorityId>;

    fn local_authority_keys() -> Vec<(u32, AuthorityId)>;

    fn get_validators_len() -> u32;

    #[cfg(feature = "runtime-benchmarks")]
    fn set_local_authority_keys(keys: Vec<AuthorityId>);
}

impl<AuthorityId, BlockNumber: Copy, AccountId>
    ValidatorOffchainBatcher<AuthorityId, BlockNumber, AccountId> for ()
{
    fn authority_keys() -> Vec<AuthorityId> {
        Vec::new()
    }

    fn local_authority_keys() -> Vec<(u32, AuthorityId)> {
        Vec::new()
    }

    fn get_validators_len() -> u32 {
        0
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn set_local_authority_keys(_keys: Vec<AuthorityId>) {}
}
