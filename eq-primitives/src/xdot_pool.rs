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

use core::convert::TryInto;

use crate::{Balance, ONE_TOKEN};
use sp_runtime::traits::Convert;
use sp_runtime::DispatchError;
use substrate_fixed::types::I64F64;

pub type PoolId = u32;
pub type XdotNumber = I64F64;

pub trait XdotPoolInfoTrait<AssetId, Balance>
where
    AssetId: Default,
    Balance: Default,
{
    /// Returns base asset
    fn base_asset(&self) -> AssetId;
    /// Returns xbase asset
    fn xbase_asset(&self) -> AssetId;
    /// Returns base asset pool balance
    fn base_balance(&self) -> Balance;
    /// Returns xbase asset pool balance
    fn xbase_balance(&self) -> Balance;
    /// Returns sum of lp total supply and xbase pool balance
    fn virtual_xbase_balance(&self) -> Option<Balance>;
}

pub trait XBasePrice<AssetId, Balance, PriceNumber>
where
    AssetId: Default,
    Balance: Default,
{
    type XdotPoolInfo: XdotPoolInfoTrait<AssetId, Balance> + Clone;

    /// Returns xbase price in relation to base for pool with corresponding `pool_id`.
    /// You can provide `custom_time_till_maturity` for price at some point of time.
    fn get_xbase_virtual_price(
        pool_info: &Self::XdotPoolInfo,
        custom_time_till_maturity: Option<u64>,
    ) -> Result<PriceNumber, DispatchError>;

    /// Return LP token price for current `pool_id`.
    /// You can provide `custom_time_till_maturity` for price at some point of time.
    fn get_lp_virtual_price(
        pool_info: &Self::XdotPoolInfo,
        custom_time_till_maturity: Option<u64>,
    ) -> Result<PriceNumber, DispatchError>;

    /// Returns pool info by `pool_id`
    fn get_pool(pool_id: PoolId) -> Result<Self::XdotPoolInfo, DispatchError>;
}

// for tests
impl<AssetId, Balance> XdotPoolInfoTrait<AssetId, Balance> for ()
where
    AssetId: Default,
    Balance: Default,
{
    fn base_asset(&self) -> AssetId {
        Default::default()
    }
    fn xbase_asset(&self) -> AssetId {
        Default::default()
    }
    fn base_balance(&self) -> Balance {
        Default::default()
    }
    fn xbase_balance(&self) -> Balance {
        Default::default()
    }
    fn virtual_xbase_balance(&self) -> Option<Balance> {
        Default::default()
    }
}

pub struct XdotBalanceConvert;
impl Convert<XdotNumber, Option<Balance>> for XdotBalanceConvert {
    fn convert(n: XdotNumber) -> Option<Balance> {
        n.to_bits().try_into().ok().map(|bits: u128| {
            use sp_runtime::traits::Zero;
            if bits == 0 {
                return Balance::zero();
            }
            let right = bits ^ (bits >> 64) << 64;

            (bits >> XdotNumber::INT_NBITS) * ONE_TOKEN
                + ((right * ONE_TOKEN) >> XdotNumber::FRAC_NBITS)
        })
    }
}

#[test]
fn xdot_convert_test() {
    use sp_runtime::traits::Zero;

    let i64f64_max = XdotNumber::from_bits(i128::MAX);
    let neg = XdotNumber::from_bits(-1);
    let one_bits = 2i128.pow(64);
    let one = XdotNumber::from_bits(one_bits);
    let test_numbers = [
        (i64f64_max, Some(9223372036854775807999999999)),
        (neg, None),
        (XdotNumber::from_bits(1), Some(Balance::zero())),
        (one, Some(ONE_TOKEN)),
        (
            XdotNumber::from_bits(one_bits / 1000000000),
            Some(Balance::zero()),
        ),
        (XdotNumber::from_bits(one_bits / 1000000000 + 1), Some(1)),
        (XdotNumber::from_bits(0), Some(Balance::zero())),
    ];
    for (xn, expected) in test_numbers {
        assert_eq!(XdotBalanceConvert::convert(xn), expected);
        // println!("bits                  {:?}", xn.to_bits());
        // println!("xn                    {:?}", xn);
        // println!("from_inner_fixed {:?}", from_inner_fixed(xn));
        // println!("{:?}", XdotFixedNumberConvert::convert(xn));
    }
}

impl Convert<Balance, XdotNumber> for XdotBalanceConvert {
    fn convert(n: Balance) -> XdotNumber {
        XdotNumber::from_num(n) / XdotNumber::from_num(ONE_TOKEN)
    }
}

pub struct XdotFixedNumberConvert;
impl Convert<XdotNumber, Option<u128>> for XdotFixedNumberConvert {
    fn convert(a: XdotNumber) -> Option<u128> {
        (a.checked_mul(XdotNumber::from_num(1_000_000_000_000_000_000i128)))
            .map(|x| x.to_num::<i128>().try_into().ok())
            .flatten()
    }
}
