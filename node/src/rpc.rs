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

//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]

use std::sync::Arc;

use common_runtime::opaque::Block;
use sc_client_api::AuxStore;
use sc_rpc::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};

use eq_balances_rpc::*;
use eq_xdot_pool_rpc::*;
use equilibrium_curve_amm_rpc::*;
use pallet_transaction_payment_rpc::*;
use sc_rpc::dev::*;
use substrate_frame_rpc_system::*;

/// A type representing all RPC extensions.
pub type RpcModule = jsonrpsee::RpcModule<()>;

/// Full client dependencies.
pub struct FullDeps<C, P> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
    /// Whether to deny unsafe calls
    pub deny_unsafe: DenyUnsafe,
}

/// Instantiate all full RPC extensions.
pub fn create_full<C, P>(
    deps: FullDeps<C, P>,
) -> Result<RpcModule, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + sc_client_api::BlockBackend<Block>
        + AuxStore
        + HeaderMetadata<Block, Error = BlockChainError>
        + Send
        + Sync
        + CallApiAt<Block>
        + 'static,
    C::Api: crate::service::RuntimeApiCollection,
    P: TransactionPool + Sync + Send + 'static,
{
    let mut module = RpcModule::new(());
    let FullDeps {
        client,
        pool,
        deny_unsafe,
    } = deps;

    module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;
    module.merge(Dev::new(client.clone(), deny_unsafe).into_rpc())?;
    module.merge(EquilibriumCurveAmm::new(client.clone()).into_rpc())?;
    module.merge(EqXdotPool::new(client.clone()).into_rpc())?;
    module.merge(EqBalances::new(client.clone()).into_rpc())?;

    Ok(module)
}
