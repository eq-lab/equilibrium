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
    core::{async_trait, RpcResult as Result},
    proc_macros::rpc,
    types::error::{CallError, ErrorCode},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_rpc::number::NumberOrHex;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay};
use std::convert::TryInto;
use std::sync::Arc;

pub use eq_xdot_pool_rpc_runtime_api::EqXdotPoolApi as EqXdotPoolRuntimeApi;

#[rpc(client, server)]
pub trait EqXdotPoolApi<Balance> {
    // Return type as String because u128/i128 doesn't supported
    #[method(name = "xdot_invariant")]
    fn invariant(&self, pool_id: u32) -> Result<String>;

    #[method(name = "xdot_fyTokenOutForBaseIn")]
    fn fy_token_out_for_base_in(&self, pool_id: u32, base_amount: Balance) -> Result<Balance>;

    #[method(name = "xdot_baseOutForFyTokenIn")]
    fn base_out_for_fy_token_in(&self, pool_id: u32, fy_token_amount: Balance) -> Result<Balance>;

    #[method(name = "xdot_fyTokenInForBaseOut")]
    fn fy_token_in_for_base_out(&self, pool_id: u32, base_amount: Balance) -> Result<Balance>;

    #[method(name = "xdot_baseInForFyTokenOut")]
    fn base_in_for_fy_token_out(&self, pool_id: u32, fy_token_amount: Balance) -> Result<Balance>;

    #[method(name = "xdot_baseOutForLpIn")]
    fn base_out_for_lp_in(&self, pool_id: u32, lp_in: Balance) -> Result<Balance>;

    #[method(name = "xdot_baseAndFyOutForLpIn")]
    fn base_and_fy_out_for_lp_in(&self, pool_id: u32, lp_in: Balance)
        -> Result<(Balance, Balance)>;

    #[method(name = "xdot_maxBaseXbaseInAndOut")]
    fn max_base_xbase_in_and_out(
        &self,
        pool_id: u32,
    ) -> Result<(Balance, Balance, Balance, Balance)>;
}

pub struct EqXdotPool<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> EqXdotPool<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
impl<C, Block, Balance> EqXdotPoolApiServer<Balance> for EqXdotPool<C, Block>
where
    Block: BlockT,
    C: 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: EqXdotPoolRuntimeApi<Block, Balance>,
    Balance: Codec + TryInto<NumberOrHex> + MaybeDisplay,
{
    fn invariant(&self, pool_id: u32) -> Result<String> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();

        api.invariant(at, pool_id)
            .ok()
            .flatten()
            .map(|r| r.to_string())
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }

    fn fy_token_out_for_base_in(&self, pool_id: u32, base_amount: Balance) -> Result<Balance> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();
        api.fy_token_out_for_base_in(at, pool_id, base_amount)
            .ok()
            .flatten()
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }

    fn base_out_for_fy_token_in(&self, pool_id: u32, fy_token_amount: Balance) -> Result<Balance> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();
        api.base_out_for_fy_token_in(at, pool_id, fy_token_amount)
            .ok()
            .flatten()
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }

    fn fy_token_in_for_base_out(&self, pool_id: u32, base_amount: Balance) -> Result<Balance> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();
        api.fy_token_in_for_base_out(at, pool_id, base_amount)
            .ok()
            .flatten()
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }

    fn base_in_for_fy_token_out(&self, pool_id: u32, fy_token_amount: Balance) -> Result<Balance> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();
        api.base_in_for_fy_token_out(at, pool_id, fy_token_amount)
            .ok()
            .flatten()
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }

    fn base_out_for_lp_in(&self, pool_id: u32, lp_in: Balance) -> Result<Balance> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();
        api.base_out_for_lp_in(at, pool_id, lp_in)
            .ok()
            .flatten()
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }

    fn base_and_fy_out_for_lp_in(
        &self,
        pool_id: u32,
        lp_in: Balance,
    ) -> Result<(Balance, Balance)> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();
        api.base_and_fy_out_for_lp_in(at, pool_id, lp_in)
            .ok()
            .flatten()
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }

    fn max_base_xbase_in_and_out(
        &self,
        pool_id: u32,
    ) -> Result<(Balance, Balance, Balance, Balance)> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();
        api.max_base_xbase_in_and_out(at, pool_id)
            .ok()
            .flatten()
            .ok_or_else(|| CallError::Custom(ErrorCode::InvalidRequest.into()).into())
    }
}
