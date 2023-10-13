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

#![allow(unused_imports)]
use super::{ParachainBackend, ParachainClient};
use common_runtime::{opaque::*, BlockNumber, Hash};
use sc_client_api::{
    execution_extensions::ExecutionExtensions, Backend, CallExecutor, ExecutorProvider, KeysIter,
    PairsIter,
};
use sp_api::NumberFor;
use sp_consensus::BlockStatus;
use sp_runtime::{
    generic::{BlockId, SignedBlock},
    traits::Block as BlockT,
    Justifications,
};
use sp_storage::{ChildInfo, StorageData, StorageKey};
use std::sync::Arc;

#[derive(Clone)]
pub enum Client {
    #[cfg(feature = "with-eq-runtime")]
    Equilibrium(Arc<ParachainClient<eq_node_runtime::RuntimeApi>>),
    #[cfg(feature = "with-gens-runtime")]
    Genshiro(Arc<ParachainClient<gens_node_runtime::RuntimeApi>>),
}

#[macro_export]
macro_rules! match_client {
    ($self:ident, $method:ident($($param:ident),*)) => {
        match *$self {
            #[cfg(feature = "with-eq-runtime")]
            Self::Equilibrium(ref client) => client.$method($($param),*),
            #[cfg(feature = "with-gens-runtime")]
            Self::Genshiro(ref client) => client.$method($($param),*),
        }
    };
    ($self:ident, $field:ident) => {{
        match *$self {
            #[cfg(feature = "with-eq-runtime")]
            Self::Equilibrium(ref client) => client.$field,
            #[cfg(feature = "with-gens-runtime")]
            Self::Genshiro(ref client) => client.$field
        }
    }};
}

#[cfg(feature = "with-eq-runtime")]
impl From<Arc<ParachainClient<eq_node_runtime::RuntimeApi>>> for Client {
    fn from(client: Arc<ParachainClient<eq_node_runtime::RuntimeApi>>) -> Self {
        Self::Equilibrium(client)
    }
}

#[cfg(feature = "with-gens-runtime")]
impl From<Arc<ParachainClient<gens_node_runtime::RuntimeApi>>> for Client {
    fn from(client: Arc<ParachainClient<gens_node_runtime::RuntimeApi>>) -> Self {
        Self::Genshiro(client)
    }
}

impl sc_client_api::UsageProvider<Block> for Client {
    fn usage_info(&self) -> sc_client_api::ClientInfo<Block> {
        match_client!(self, usage_info())
    }
}

impl sc_client_api::BlockBackend<Block> for Client {
    fn block_body(
        &self,
        hash: Hash,
    ) -> sp_blockchain::Result<Option<Vec<<Block as BlockT>::Extrinsic>>> {
        match_client!(self, block_body(hash))
    }

    fn block_indexed_body(&self, hash: Hash) -> sp_blockchain::Result<Option<Vec<Vec<u8>>>> {
        match_client!(self, block_indexed_body(hash))
    }

    fn block(&self, hash: Hash) -> sp_blockchain::Result<Option<SignedBlock<Block>>> {
        match_client!(self, block(hash))
    }

    fn block_status(&self, hash: Hash) -> sp_blockchain::Result<BlockStatus> {
        match_client!(self, block_status(hash))
    }

    fn justifications(&self, hash: Hash) -> sp_blockchain::Result<Option<Justifications>> {
        match_client!(self, justifications(hash))
    }

    fn block_hash(
        &self,
        number: NumberFor<Block>,
    ) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
        match_client!(self, block_hash(number))
    }

    fn indexed_transaction(&self, hash: Hash) -> sp_blockchain::Result<Option<Vec<u8>>> {
        match_client!(self, indexed_transaction(hash))
    }

    fn has_indexed_transaction(&self, hash: Hash) -> sp_blockchain::Result<bool> {
        match_client!(self, has_indexed_transaction(hash))
    }

    fn requires_full_sync(&self) -> bool {
        match_client!(self, requires_full_sync())
    }
}

impl sc_client_api::StorageProvider<Block, ParachainBackend> for Client {
    fn storage(&self, hash: Hash, key: &StorageKey) -> sp_blockchain::Result<Option<StorageData>> {
        match_client!(self, storage(hash, key))
    }

    fn storage_keys(
        &self,
        hash: Hash,
        prefix: Option<&StorageKey>,
        start_key: Option<&StorageKey>,
    ) -> sp_blockchain::Result<
        KeysIter<<ParachainBackend as sc_client_api::Backend<Block>>::State, Block>,
    > {
        match_client!(self, storage_keys(hash, prefix, start_key))
    }

    fn storage_hash(
        &self,
        hash: Hash,
        key: &StorageKey,
    ) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
        match_client!(self, storage_hash(hash, key))
    }

    fn storage_pairs(
        &self,
        hash: Hash,
        prefix: Option<&StorageKey>,
        start_key: Option<&StorageKey>,
    ) -> sp_blockchain::Result<PairsIter<<ParachainBackend as Backend<Block>>::State, Block>> {
        match_client!(self, storage_pairs(hash, prefix, start_key))
    }

    fn child_storage(
        &self,
        hash: Hash,
        child_info: &ChildInfo,
        key: &StorageKey,
    ) -> sp_blockchain::Result<Option<StorageData>> {
        match_client!(self, child_storage(hash, child_info, key))
    }

    fn child_storage_keys(
        &self,
        hash: Hash,
        child_info: ChildInfo,
        prefix: Option<&StorageKey>,
        start_key: Option<&StorageKey>,
    ) -> sp_blockchain::Result<KeysIter<<ParachainBackend as Backend<Block>>::State, Block>> {
        match_client!(
            self,
            child_storage_keys(hash, child_info, prefix, start_key)
        )
    }

    fn child_storage_hash(
        &self,
        hash: Hash,
        child_info: &ChildInfo,
        key: &StorageKey,
    ) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
        match_client!(self, child_storage_hash(hash, child_info, key))
    }
}

impl sp_blockchain::HeaderBackend<Block> for Client {
    fn header(&self, hash: Hash) -> sp_blockchain::Result<Option<Header>> {
        match_client!(self, header(hash))
    }

    fn info(&self) -> sp_blockchain::Info<Block> {
        match_client!(self, info())
    }

    fn status(&self, hash: Hash) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
        match_client!(self, status(hash))
    }

    fn number(&self, hash: Hash) -> sp_blockchain::Result<Option<BlockNumber>> {
        match_client!(self, number(hash))
    }

    fn hash(&self, number: BlockNumber) -> sp_blockchain::Result<Option<Hash>> {
        match_client!(self, hash(number))
    }
}
