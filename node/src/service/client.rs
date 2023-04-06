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
use super::{FullBackend, FullClient};
use common_runtime::{opaque::*, BlockNumber, Hash};
use sc_client_api::KeyIterator;
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
    Equilibrium(Arc<FullClient<eq_node_runtime::RuntimeApi, super::EquilibriumRuntimeExecutor>>),
    #[cfg(feature = "with-gens-runtime")]
    Genshiro(Arc<FullClient<gens_node_runtime::RuntimeApi, super::GenshiroRuntimeExecutor>>),
}

#[cfg(feature = "with-eq-runtime")]
impl From<Arc<FullClient<eq_node_runtime::RuntimeApi, super::EquilibriumRuntimeExecutor>>>
    for Client
{
    fn from(
        client: Arc<FullClient<eq_node_runtime::RuntimeApi, super::EquilibriumRuntimeExecutor>>,
    ) -> Self {
        Self::Equilibrium(client)
    }
}

#[cfg(feature = "with-gens-runtime")]
impl From<Arc<FullClient<gens_node_runtime::RuntimeApi, super::GenshiroRuntimeExecutor>>>
    for Client
{
    fn from(
        client: Arc<FullClient<gens_node_runtime::RuntimeApi, super::GenshiroRuntimeExecutor>>,
    ) -> Self {
        Self::Genshiro(client)
    }
}

macro_rules! match_client {
    ($self:ident, $method:ident($($param:ident),*)) => {
        match *$self {
            #[cfg(feature = "with-eq-runtime")]
            Self::Equilibrium(ref client) => client.$method($($param),*),
            #[cfg(feature = "with-gens-runtime")]
            Self::Genshiro(ref client) => client.$method($($param),*),
        }
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
        id: &BlockId<Block>,
    ) -> sp_blockchain::Result<Option<Vec<<Block as BlockT>::Extrinsic>>> {
        match_client!(self, block_body(id))
    }

    fn block_indexed_body(
        &self,
        id: &BlockId<Block>,
    ) -> sp_blockchain::Result<Option<Vec<Vec<u8>>>> {
        match_client!(self, block_indexed_body(id))
    }

    fn block(&self, id: &BlockId<Block>) -> sp_blockchain::Result<Option<SignedBlock<Block>>> {
        match_client!(self, block(id))
    }

    fn block_status(&self, id: &BlockId<Block>) -> sp_blockchain::Result<BlockStatus> {
        match_client!(self, block_status(id))
    }

    fn justifications(&self, id: &BlockId<Block>) -> sp_blockchain::Result<Option<Justifications>> {
        match_client!(self, justifications(id))
    }

    fn block_hash(
        &self,
        number: NumberFor<Block>,
    ) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
        match_client!(self, block_hash(number))
    }

    fn indexed_transaction(
        &self,
        hash: &<Block as BlockT>::Hash,
    ) -> sp_blockchain::Result<Option<Vec<u8>>> {
        match_client!(self, indexed_transaction(hash))
    }

    fn has_indexed_transaction(
        &self,
        hash: &<Block as BlockT>::Hash,
    ) -> sp_blockchain::Result<bool> {
        match_client!(self, has_indexed_transaction(hash))
    }

    fn requires_full_sync(&self) -> bool {
        match_client!(self, requires_full_sync())
    }
}

impl sc_client_api::StorageProvider<Block, FullBackend> for Client {
    fn storage(
        &self,
        id: &BlockId<Block>,
        key: &StorageKey,
    ) -> sp_blockchain::Result<Option<StorageData>> {
        match_client!(self, storage(id, key))
    }

    fn storage_keys(
        &self,
        id: &BlockId<Block>,
        key_prefix: &StorageKey,
    ) -> sp_blockchain::Result<Vec<StorageKey>> {
        match_client!(self, storage_keys(id, key_prefix))
    }

    fn storage_hash(
        &self,
        id: &BlockId<Block>,
        key: &StorageKey,
    ) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
        match_client!(self, storage_hash(id, key))
    }

    fn storage_pairs(
        &self,
        id: &BlockId<Block>,
        key_prefix: &StorageKey,
    ) -> sp_blockchain::Result<Vec<(StorageKey, StorageData)>> {
        match_client!(self, storage_pairs(id, key_prefix))
    }

    fn storage_keys_iter<'a>(
        &self,
        id: &BlockId<Block>,
        prefix: Option<&'a StorageKey>,
        start_key: Option<&StorageKey>,
    ) -> sp_blockchain::Result<
        KeyIterator<'a, <FullBackend as sc_client_api::Backend<Block>>::State, Block>,
    > {
        match_client!(self, storage_keys_iter(id, prefix, start_key))
    }

    fn child_storage(
        &self,
        id: &BlockId<Block>,
        child_info: &ChildInfo,
        key: &StorageKey,
    ) -> sp_blockchain::Result<Option<StorageData>> {
        match_client!(self, child_storage(id, child_info, key))
    }

    fn child_storage_keys(
        &self,
        id: &BlockId<Block>,
        child_info: &ChildInfo,
        key_prefix: &StorageKey,
    ) -> sp_blockchain::Result<Vec<StorageKey>> {
        match_client!(self, child_storage_keys(id, child_info, key_prefix))
    }

    fn child_storage_keys_iter<'a>(
        &self,
        id: &BlockId<Block>,
        child_info: ChildInfo,
        prefix: Option<&'a StorageKey>,
        start_key: Option<&StorageKey>,
    ) -> sp_blockchain::Result<
        KeyIterator<'a, <FullBackend as sc_client_api::Backend<Block>>::State, Block>,
    > {
        match_client!(
            self,
            child_storage_keys_iter(id, child_info, prefix, start_key)
        )
    }

    fn child_storage_hash(
        &self,
        id: &BlockId<Block>,
        child_info: &ChildInfo,
        key: &StorageKey,
    ) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
        match_client!(self, child_storage_hash(id, child_info, key))
    }
}

impl sp_blockchain::HeaderBackend<Block> for Client {
    fn header(&self, id: BlockId<Block>) -> sp_blockchain::Result<Option<Header>> {
        let id = &id;
        match_client!(self, header(id))
    }

    fn info(&self) -> sp_blockchain::Info<Block> {
        match_client!(self, info())
    }

    fn status(&self, id: BlockId<Block>) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
        match_client!(self, status(id))
    }

    fn number(&self, hash: Hash) -> sp_blockchain::Result<Option<BlockNumber>> {
        match_client!(self, number(hash))
    }

    fn hash(&self, number: BlockNumber) -> sp_blockchain::Result<Option<Hash>> {
        match_client!(self, hash(number))
    }
}
