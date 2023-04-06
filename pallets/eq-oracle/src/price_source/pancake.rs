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

use crate::price_source::{PriceSource, PriceSourceError};
use crate::OffchainStorage;
use alloc::string::String;
use eq_primitives::asset::Asset;
use eq_utils::fixed::{fixedi64_from_fixedu128, fixedu128_from_fixedi64};
use sp_arithmetic::FixedI64;
use sp_std::{vec, vec::Vec};

/// Pancake price source.
/// Gets data from pancake smart contract and calculate contract lp token price.
/// Price source requires all pool in "oracle::pool_assets" setting.
#[derive(Debug)]
pub struct PancakePriceSource {
    /// Contract address of PancakeSwap.
    /// Example: 0x58f876857a02d6762e0101bb5c46a8c1ed44dc16  contract address in BSC on pair WBNB/BUSD
    /// https://bscscan.com/address/0x58f876857a02d6762e0101bb5c46a8c1ed44dc16
    contract: String,
    /// token_0 contract address
    token_0: String,
    /// token_1 contract token contract address
    token_1: String,
    /// Url of BSC node api.
    /// Example: https://scn1.equilab.io/bsc/mainnet/rpc/
    node_url: String,
    /// token_0 price from our oracle
    token_0_price: FixedI64,
    /// token_1 price from our oracle
    token_1_price: FixedI64,
    /// Target lp token
    asset: Asset,
}

impl PancakePriceSource {
    pub fn new(
        token_0_price: FixedI64,
        token_1_price: FixedI64,
        lp_token: Asset,
    ) -> Result<Self, PriceSourceError> {
        let contract = OffchainStorage::get_contract_address()
            .ok_or(PriceSourceError::StorageValueDoesNotExists)?;
        let node_url =
            OffchainStorage::get_node_url().ok_or(PriceSourceError::StorageValueDoesNotExists)?;
        let token_0_address = pancake_contract::token_0(&node_url, &contract)?;
        let token_1_address = pancake_contract::token_1(&node_url, &contract)?;

        Ok(PancakePriceSource {
            contract,
            token_0: token_0_address,
            token_1: token_1_address,
            node_url,
            token_0_price,
            token_1_price,
            asset: lp_token,
        })
    }
}

impl PriceSource for PancakePriceSource {
    fn get_prices(&self) -> Vec<(Asset, Result<FixedI64, PriceSourceError>)> {
        let get_price = || -> Result<FixedI64, PriceSourceError> {
            let total_supply = pancake_contract::total_supply(&self.node_url, &self.contract)?;

            let gens_balance =
                pancake_contract::balance_of(&self.node_url, &self.contract, &self.token_0)?;
            let busd_balance =
                pancake_contract::balance_of(&self.node_url, &self.contract, &self.token_1)?;

            let price_u128 = (gens_balance * fixedu128_from_fixedi64(self.token_0_price).unwrap()
                + busd_balance * fixedu128_from_fixedi64(self.token_1_price).unwrap())
                / total_supply;

            fixedi64_from_fixedu128(price_u128).ok_or(PriceSourceError::OverflowError)
        };

        vec![(self.asset, get_price())]
    }
}

/// Provides read methods of pancake swap smart-contract
mod pancake_contract {
    use super::*;
    use crate::alloc::string::ToString;
    use crate::price_source::*;
    use sp_arithmetic::{FixedPointNumber, FixedU128};

    const ETH_ACCURACY: u128 = 1_000_000_000_000_000_000_u128;

    /// Execute `eth_call` method
    fn call_contract(url: &str, contract: &str, data: &str) -> Result<String, PriceSourceError> {
        let body_str = format!("{{\"jsonrpc\":\"2.0\", \"method\":\"eth_call\", \"params\":[{{\"to\": \"{}\",\"data\": \"{}\"}},\"latest\"],\"id\":1}}",
                               contract,
                               data);

        let response = http_client::post(url, vec![body_str.as_bytes()])
            .map_err(|_| PriceSourceError::HttpError)?;

        let json_value = json::from_str::<json::Value>(&response)
            .map_err(|_| PriceSourceError::JsonParseError)?;

        match json_value
            .get("result")
            .ok_or_else(||{
                log::error!(
                    "{}:{}. Error response from call_contract. Error:{:?} url: {:?}, contract: {:?}, data {:?}.",
                    file!(),
                    line!(),
                    response,
                    url,
                    contract,
                    data
                );

                PriceSourceError::CallContractError
            })?
        {
            json::Value::String(result) => Ok(result.to_string()),
            _ => Err(PriceSourceError::JsonParseError)
        }
    }

    /// Remove leading zeros from response
    pub fn convert_to_address(response: &str) -> String {
        const ADDRESS_LENGTH: usize = 40;
        format!(
            "0x{}",
            response[response.len() - ADDRESS_LENGTH..].to_string()
        )
    }

    /// Returns total supply of LP token
    pub fn total_supply(url: &str, contract: &str) -> Result<FixedU128, PriceSourceError> {
        // keccak256('totalSupply()') = "0x18160ddd7f15c72528c2f94fd8dfe3c8d5aa26e2c50c7d81f4bc7bee8d4b7932"
        // get first 4 bytes of keccak hash 18160ddd and make data parameter
        const TOTAL_SUPPLY_DATA: &'static str =
            "0x18160ddd0000000000000000000000000000000000000000000000000000000000000000";

        let result = call_contract(url, contract, TOTAL_SUPPLY_DATA)?;

        //received balance in Wei, convert it to units
        u128::from_str_radix(&result.trim_start_matches("0x"), 16)
            .map(|v| FixedU128::saturating_from_rational(v, ETH_ACCURACY))
            .map_err(|_| PriceSourceError::JsonParseError)
    }

    /// Returns balance of `token_contract` on `contract`
    pub fn balance_of(
        url: &str,
        contract: &str,
        token_contract: &str,
    ) -> Result<FixedU128, PriceSourceError> {
        // keccak256('balance_of(address)') = "0x70a08231b98ef4ca268c9cc3f6b4590e4bfec28280db06bb5d45e689f2a360be"
        // get first 4 bytes of keccak hash `70a08231` and make data parameter, last bytes of data should be `contract`
        let mut balance_of_data = String::from(
            "0x70a082310000000000000000000000000000000000000000000000000000000000000000",
        );
        //contract should be in format "0x......."
        //replace last chars of `balance_of_data` by contract number without leading "0x"
        balance_of_data.replace_range(balance_of_data.len() - contract.len() + 2.., &contract[2..]);

        let result_str = call_contract(url, token_contract, balance_of_data.as_str())?;

        u128::from_str_radix(&result_str.trim_start_matches("0x"), 16)
            .map(|v| FixedU128::saturating_from_rational(v, ETH_ACCURACY))
            .map_err(|_| PriceSourceError::JsonParseError)
    }

    /// Returns token0 address
    pub fn token_0(url: &str, contract: &str) -> Result<String, PriceSourceError> {
        //keccak256('token0()') = "0x0dfe16819b2523f68151ea44c4f107305bfeb85c4b18e7e63ac6f999d8cf9a0e"
        //get first 4 bytes of keccak hash 0dfe1681
        const TOKEN_0_DATA: &'static str =
            "0x0dfe16810000000000000000000000000000000000000000000000000000000000000000";

        call_contract(url, contract, TOKEN_0_DATA).map(|s| convert_to_address(s.as_str()))
    }

    /// Returns token1 address
    pub fn token_1(url: &str, contract: &str) -> Result<String, PriceSourceError> {
        //keccak256('token1()') = "0xd21220a7b5fcd6706feb17ecf897df81a823584a161e849e09b1ff66574ed2cb"
        //get first 4 bytes of keccak hash d21220
        const TOKEN_1_DATA: &'static str =
            "0xd21220a70000000000000000000000000000000000000000000000000000000000000000";
        call_contract(url, contract, TOKEN_1_DATA).map(|s| convert_to_address(s.as_str()))
    }
}
