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

use codec::Codec;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorCode},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_rpc::number::NumberOrHex;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay};
use std::convert::TryInto;
use std::sync::Arc;

pub use eq_balances_rpc_runtime_api::EqBalancesApi as EqBalancesRuntimeApi;

#[rpc(client, server)]
pub trait EqBalancesApi<Balance, AccountId> {
    // Check: // Return type as String because u128/i128 doesn't supported
    #[method(name = "eqbalances_walletBalanceInUsd")]
    fn wallet_balance_in_usd(&self, account_id: AccountId) -> RpcResult<Balance>;

    #[method(name = "eqbalances_portfolioBalanceInUsd")]
    fn portfolio_balance_in_usd(&self, account_id: AccountId) -> RpcResult<Balance>;
}

pub struct EqBalances<C, M> {
    client: Arc<C>,
    // Check: deny unsafe behaviour
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> EqBalances<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
impl<C, Block, Balance, AccountId> EqBalancesApiServer<Balance, AccountId> for EqBalances<C, Block>
where
    Block: BlockT,
    C: 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: EqBalancesRuntimeApi<Block, Balance, AccountId>,
    Balance: Codec + TryInto<NumberOrHex> + MaybeDisplay,
    AccountId: Codec + MaybeDisplay,
{
    fn wallet_balance_in_usd(&self, account_id: AccountId) -> RpcResult<Balance> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();

        api.wallet_balance_in_usd(at, account_id)
            .ok()
            .flatten()
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }

    fn portfolio_balance_in_usd(&self, account_id: AccountId) -> RpcResult<Balance> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();
        api.portfolio_balance_in_usd(at, account_id)
            .ok()
            .flatten()
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }
}
