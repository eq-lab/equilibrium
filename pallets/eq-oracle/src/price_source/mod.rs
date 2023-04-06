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

pub(crate) mod custom;
pub(crate) mod pancake;

use alloc::format;
use alloc::string::String;
use eq_primitives::asset::{self, Asset};

use serde_json as json;
use sp_arithmetic::FixedI64;
use sp_std::vec::Vec;

/// Source of received price data points
#[derive(Debug)]
pub enum SourceType {
    /// Sources that provides prices throw http get requests
    Custom,
    /// Special source for calculation price of LP token of PancakeSwap
    Pancake,
}

impl SourceType {
    /// Converts `resource_type` string into `SourceType`
    pub fn from(resource_type: String) -> Option<SourceType> {
        match resource_type.as_str() {
            "custom" => Some(SourceType::Custom),
            "pancake" => Some(SourceType::Pancake),
            _ => Option::None,
        }
    }
}

/// Price source abstraction. Settings of price source stored in offchain local storage.
pub trait PriceSource {
    /// Returns collection of (asset, price result)
    fn get_prices(&self) -> Vec<(Asset, Result<FixedI64, PriceSourceError>)>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum PriceSourceError {
    HttpError,
    WrongUrlPattern,
    IncorrectQueryFormat,
    DeserializationError,
    JsonParseError,
    JsonValueNotANumber,
    JsonPriceConversionError,
    CallContractError,
    OverflowError,
    StorageValueDoesNotExists,
    UnknownPriceStrategy,
    Symbol,
}

mod http_client {
    use sp_runtime::offchain::{http, Duration};

    use super::*;

    /// Send get request
    pub fn get(url: &str) -> Result<String, http::Error> {
        let request = http::Request::get(url);
        execute_request(request)
    }

    ///Send post request with `body` and header Content-Type: application/json
    pub fn post(url: &str, body: Vec<&[u8]>) -> Result<String, http::Error> {
        let mut request = http::Request::post(url, body);
        request = request.add_header("Content-type", "application/json");

        execute_request(request)
    }

    fn execute_request<T: Default + IntoIterator<Item = I>, I: AsRef<[u8]>>(
        request: http::Request<T>,
    ) -> Result<String, http::Error> {
        let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(5_000));

        let url = request.url.clone();
        let pending = request.deadline(deadline).send().map_err(|e| {
            log::error!(
                "{}:{}. Error sending request. Request: {:?}, deadline: {:?}.",
                file!(),
                line!(),
                url,
                deadline
            );
            match e {
                sp_runtime::offchain::HttpError::DeadlineReached => http::Error::DeadlineReached,
                sp_runtime::offchain::HttpError::IoError => http::Error::IoError,
                sp_runtime::offchain::HttpError::Invalid => http::Error::Unknown,
            }
        })?;
        // no response or a timeout
        let response = pending
            .try_wait(deadline)
            .map_err(|_| {
                log::error!(
                    "{}:{}. Didn't receive response. Deadline: {:?}.",
                    file!(),
                    line!(),
                    deadline
                );
                http::Error::DeadlineReached
            })?
            .map_err(|e| {
                log::error!("RESPONSE {:?}", e);
                e
            })?;
        if response.code != 200 {
            log::error!(
                "{}:{}. Unexpected status code: {}",
                file!(),
                line!(),
                response.code
            );
            return Err(http::Error::Unknown);
        }
        let body = response.body();
        let str = String::from_utf8(body.collect()).unwrap_or(String::new());
        Ok(str)
    }
}

/// Getter of a URL for an asset price
pub(super) trait WithUrl {
    /// Gets a URL and JSON path for an asset price
    fn get_url(
        &self,
        url_template: &str,
        path_template: &str,
    ) -> Result<(String, String), PriceSourceError>;
}

impl WithUrl for Asset {
    /// Gets a URL
    ///
    /// Put self string identifier in `url_template` and `path_template` instead of `{$}`
    fn get_url(
        &self,
        url_template: &str,
        path_template: &str,
    ) -> Result<(String, String), PriceSourceError> {
        let is_upper_case = url_template.find("USD").is_some();
        let symbol = {
            let is_kraken = url_template.contains("api.kraken.com");
            let symbol = self.get_symbol(is_kraken)?;

            if is_upper_case {
                symbol.to_uppercase()
            } else {
                symbol.to_lowercase()
            }
        };

        Ok((
            url_template.replace("{$}", &symbol),
            path_template.replace("{$}", &symbol),
        ))
    }
}

/// Returns a symbolic ticker for query
trait AsQuerySymbol {
    fn get_symbol(self, is_kraken: bool) -> Result<String, PriceSourceError>;
}

/// Returns a symbolic ticker
impl AsQuerySymbol for Asset {
    fn get_symbol(self, is_kraken: bool) -> Result<String, PriceSourceError> {
        match (is_kraken, self) {
            (true, asset::ETH | asset::MXETH) => Ok("xethz".into()),
            (true, asset::BTC | asset::WBTC | asset::MXWBTC | asset::IBTC | asset::KBTC) => {
                Ok("xxbtz".into())
            }
            (true, asset::USDT) => Ok("usdtz".into()),
            (_, asset::ETH | asset::MXETH) => Ok("eth".into()),
            (_, asset::BTC | asset::WBTC | asset::MXWBTC | asset::IBTC | asset::KBTC) => {
                Ok("btc".into())
            }
            (_, asset::USDC | asset::MXUSDC) => Ok("usdc".into()),
            (_, _) => Ok(self.to_str()),
        }
    }
}
