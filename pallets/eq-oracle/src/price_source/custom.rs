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

use crate::price_source::http_client;
use crate::price_source::WithUrl;
use crate::price_source::{PriceSource, PriceSourceError};
use crate::regex_offsets::{get_index_offsets, get_url_offset};
use crate::OffchainStorage;
use alloc::string::String;
use eq_primitives::asset::{Asset, AssetData, AssetType};
use eq_primitives::str_asset;
use eq_utils::ok_or_error;
use serde_json as json;
use sp_arithmetic::{FixedI64, FixedPointNumber};
use sp_runtime::traits::One;
use sp_std::vec::Vec;

/// Json price source. Gets prices for assets from setting "oracle::source_assets"
/// or for all assets if no settings specified. Also uses price_strategy from "oracle::source_assets"
/// if specifies. Price strategy define how to interpret value from source (price, reverse)
#[derive(Debug)]
pub struct JsonPriceSource {
    /// Full query, containing url template and path to price in json
    /// example: json(https://ftx.com/api/markets/{$}/USD).result.price
    query: String,
    assets_data: Vec<AssetData<Asset>>,
}

impl JsonPriceSource {
    pub fn new(assets_data: Vec<AssetData<Asset>>) -> Option<Self> {
        Some(JsonPriceSource {
            query: OffchainStorage::get_query()?,
            assets_data,
        })
    }

    /// Fetches a price for an asset from a URL source with the query
    fn fetch_price(asset: &Asset, query: &str) -> Result<FixedI64, PriceSourceError> {
        let url_bytes_offset = get_url_offset(query.as_bytes());

        if let Some((start, end)) = url_bytes_offset {
            // regex is \(.+\)\.
            let url_template = &query[start + 1..end - 2];
            if !url_template.contains("{$}") {
                log::error!("{}:{}. Incorrect query format, doesn't have {{$}}. Query: {}, url template: {:?}.", 
                file!(), line!(), query, url_template);
                Err(PriceSourceError::WrongUrlPattern)
            } else {
                let path_template = &query[end..];
                let (url, path) = asset.get_url(url_template, path_template)?;
                let s = http_client::get(url.as_str()).map_err(|e| {
                    log::error!("{}:{}. Http GET {:?}", file!(), line!(), url);
                    match e {
                        sp_runtime::offchain::http::Error::DeadlineReached => {
                            log::error!("DEADLINE")
                        }
                        sp_runtime::offchain::http::Error::IoError => log::error!("IO_ERROR"),
                        sp_runtime::offchain::http::Error::Unknown => log::error!("UNKNOWN"),
                    };
                    PriceSourceError::HttpError
                })?;

                Self::fetch_price_from_json(s, path.as_str())
            }
        } else {
            log::error!(
                "{}:{}. Incorrect query format, can't parse. Query: {}",
                file!(),
                line!(),
                query
            );

            Err(PriceSourceError::IncorrectQueryFormat)
        }
    }

    /// Fetches a price from a collected JSON
    pub(crate) fn fetch_price_from_json(
        body: String,
        path: &str,
    ) -> Result<FixedI64, PriceSourceError> {
        let mut val: &json::Value = &json::from_str(&body).or_else(|_| {
            log::warn!(
                "{:?}:{:?} {:?}. Cannot deserialize an instance from a string of JSON text.",
                file!(),
                line!(),
                body
            );

            Err(PriceSourceError::DeserializationError)
        })?;

        let indices = path.split(".");
        for index in indices {
            let offsets = get_index_offsets(index.as_bytes());
            if offsets.len() == 0 {
                let option_value = val.get(index);
                val = ok_or_error!(
                    option_value,
                    PriceSourceError::JsonParseError,
                    "{}:{}. Couldn't access a value in a map. Json: {:?}, index: {:?}.",
                    file!(),
                    line!(),
                    val,
                    index
                )?;
            } else {
                // arrays
                for (start, end) in offsets {
                    if start != 0 {
                        let option_value = val.get(&index[..start]);
                        val = ok_or_error!(
                            option_value,
                            PriceSourceError::JsonParseError,
                            "{}:{}. Couldn't access an element of an array. Json: {:?}, index: {:?}.",
                            file!(),
                            line!(),
                            val,
                            &index[..start])?;
                    }
                    let i = &index[start + 1..end - 1]
                        .parse::<usize>()
                        .expect("Expect a number as array index");

                    let option_value = val.get(i);
                    val = ok_or_error!(
                        option_value,
                        PriceSourceError::JsonParseError,
                        "{}:{}. Couldn't access an element of an array. Json: {:?}, index: {:?}.",
                        file!(),
                        line!(),
                        val,
                        i
                    )?;
                }
            }
        }

        let option_price = match val {
            json::Value::Number(v) => Ok(v.as_f64()),
            json::Value::String(v) => Ok(v.parse::<f64>().ok()),
            _ => Err({
                log::error!(
                    "{}:{}. Value received from json not number or string. Value: {:?}.",
                    file!(),
                    line!(),
                    val
                );

                PriceSourceError::JsonValueNotANumber
            }),
        }?;

        let price = ok_or_error!(
            option_price,
            PriceSourceError::JsonPriceConversionError,
            "{}:{}. Couldn't get value as f64. Value: {:?}.",
            file!(),
            line!(),
            val
        )?;

        Ok(FixedI64::from_inner(
            (price * (FixedI64::accuracy() as f64)) as i64,
        ))
    }
}

impl PriceSource for JsonPriceSource {
    fn get_prices(&self) -> Vec<(Asset, Result<FixedI64, PriceSourceError>)> {
        let maybe_asset_settings = OffchainStorage::get_asset_settings();

        let mut asset_prices: Vec<(Asset, Result<FixedI64, PriceSourceError>)> = Vec::new();

        for asset_data in self.assets_data.iter() {
            let asset = asset_data.id;
            // EQD is always unity
            if asset == eq_primitives::asset::EQD {
                continue;
            }

            // Price for this kind of tokens calculates from pool tokens
            // See eq_oracle::Pallet::calc_lp_token_price()
            if let AssetType::Lp(_) = asset_data.asset_type {
                continue;
            }

            if asset == eq_primitives::asset::HDOT {
                continue;
            }

            if asset == eq_primitives::asset::XDOT {
                continue;
            }

            if asset == eq_primitives::asset::XDOT2 {
                continue;
            }

            if asset == eq_primitives::asset::XDOT3 {
                continue;
            }

            if asset == eq_primitives::asset::STDOT {
                continue;
            }

            if asset == eq_primitives::asset::TDOT {
                continue;
            }

            // If specified, do not fetch non available currencies
            let price = if let Some(asset_settings) = &maybe_asset_settings {
                if asset_settings.len() == 0 {
                    OffchainStorage::clear_asset_settings();
                    Self::fetch_price(&asset, &self.query)
                } else {
                    let symbol: String = match str_asset!(asset).map(Into::into) {
                        Ok(s) => s,
                        Err(_) => {
                            asset_prices.push((asset, Err(PriceSourceError::Symbol)));
                            continue;
                        }
                    };

                    match asset_settings.iter().find(|(a, _)| *a == symbol) {
                        Some((_, price_strategy)) => Self::fetch_price(&asset, &self.query)
                            .and_then(|price| match price_strategy.as_str() {
                                "price" => Ok(price),
                                "reverse" => Ok(FixedI64::one() / price),
                                _ => Err(PriceSourceError::UnknownPriceStrategy),
                            }),
                        _ => continue, //skip asset
                    }
                }
            } else {
                Self::fetch_price(&asset, &self.query)
            }
            .map_err(|err| {
                log::error!(
                    "{}:{} Custom price source return error. Asset: {:?}, error: {:?}",
                    file!(),
                    line!(),
                    asset,
                    err,
                );
                err
            });

            asset_prices.push((asset, price));
        }
        asset_prices
    }
}
