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

use core::convert::TryFrom;
use core::convert::TryInto;
use core::ops::Div;
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::ONE_TOKEN;
use sp_runtime::FixedI128;
use sp_runtime::FixedI64;
use sp_runtime::FixedPointNumber;
use sp_runtime::FixedU128;
use substrate_fixed::types::I64F64;

pub fn fixedi64_from_balance<B>(balance: B) -> Option<FixedI64>
where
    B: Into<u128>,
{
    Into::<u128>::into(balance)
        .try_into()
        .ok()
        .map(|n| FixedI64::from_inner(n))
}

pub fn fixedi128_from_balance<B>(balance: B) -> Option<FixedI128>
where
    B: Into<u128>,
{
    let accuracy = FixedI128::accuracy() / ONE_TOKEN as i128;

    Into::<u128>::into(balance)
        .try_into()
        .ok()
        .map(|n: i128| FixedI128::from_inner(n * accuracy))
}

pub fn fixedi128_from_fixedi64(value: FixedI64) -> FixedI128 {
    let accuracy = FixedI128::accuracy() / FixedI64::accuracy() as i128;
    FixedI128::from_inner((value.into_inner() as i128) * accuracy)
}

pub fn fixedi64_from_fixedi128(value: FixedI128) -> Option<FixedI64> {
    let accuracy = FixedI128::accuracy() / FixedI64::accuracy() as i128;

    let raw_value = value.into_inner() / accuracy;

    if raw_value > i64::MAX as i128 {
        Option::None
    } else {
        Option::Some(FixedI64::from_inner(raw_value as i64))
    }
}

pub fn fixedi64_from_fixedu128(value: FixedU128) -> Option<FixedI64> {
    let accuracy = FixedU128::accuracy() / FixedI64::accuracy() as u128;

    let raw_value = value.into_inner() / accuracy;

    if raw_value > i64::MAX as u128 {
        Option::None
    } else {
        Option::Some(FixedI64::from_inner(raw_value as i64))
    }
}

pub fn balance_from_fixedi128<B>(value: FixedI128) -> Option<B>
where
    B: TryFrom<u128>,
{
    let accuracy = FixedI128::accuracy() / ONE_TOKEN as i128;
    value
        .into_inner()
        .div(accuracy)
        .try_into()
        .ok()
        .map(|b| TryFrom::<u128>::try_from(b).ok())
        .flatten()
}

pub fn balance_from_fixedi64<B>(value: FixedI64) -> Option<B>
where
    B: From<u128>,
{
    value
        .into_inner()
        .try_into()
        .ok()
        .map(|b| From::<u128>::from(b))
}

pub fn fixedi64_to_i64f64(value: FixedI64) -> I64F64 {
    I64F64::from_num(value.into_inner()) / I64F64::from_num(FixedI64::DIV)
}

pub fn i64f64_to_fixedi64(value: I64F64) -> FixedI64 {
    let corrected = value * I64F64::from_num(FixedI64::DIV);
    let fx64_inner = (corrected.round().to_bits() >> 64) as i64;
    FixedI64::from_inner(fx64_inner)
}

pub fn fixedu128_from_fixedi64(value: FixedI64) -> Option<FixedU128> {
    if value.is_negative() {
        None
    } else {
        Some(FixedU128::from_inner(
            fixedi128_from_fixedi64(value).into_inner() as u128,
        ))
    }
}

pub fn fixedi128_from_i64f64(value: I64F64) -> FixedI128 {
    crate::math::from_inner_fixed(value)
}

pub fn eq_fixedu128_from_balance<B>(balance: B) -> EqFixedU128
where
    B: Into<u128>,
{
    EqFixedU128::from_inner(Into::<u128>::into(balance))
}

pub fn balance_from_eq_fixedu128<B>(value: EqFixedU128) -> Option<B>
where
    B: TryFrom<u128>,
{
    TryFrom::<_>::try_from(value.into_inner()).ok()
}

pub fn fixedi64_from_eq_fixedu128(value: EqFixedU128) -> Option<FixedI64> {
    fixedi64_from_balance(value.into_inner())
}

pub fn fixedi128_from_eq_fixedu128(value: EqFixedU128) -> Option<FixedI128> {
    fixedi128_from_balance(value.into_inner())
}

pub fn eq_fixedu128_from_fixedi64(value: FixedI64) -> Option<EqFixedU128> {
    balance_from_fixedi64(value).map(|b| EqFixedU128::from_inner(b))
}

pub fn eq_fixedu128_from_fixedi128(value: FixedI128) -> Option<EqFixedU128> {
    balance_from_fixedi128(value).map(|b| EqFixedU128::from_inner(b))
}

pub fn eq_fixedu128_from_i64f64(value: I64F64) -> Option<EqFixedU128> {
    let result = value;
    let corrected = result * I64F64::from_num(EqFixedU128::DIV);
    (corrected.round().to_bits() >> 64)
        .try_into()
        .ok()
        .map(|inner| EqFixedU128::from_inner(inner))
}

#[cfg(test)]
mod tests {
    use crate::fixed::eq_fixedu128_from_i64f64;

    #[test]
    fn eq_fixedu128_from_i64f64_test() {
        let negative = substrate_fixed::types::I64F64::from_bits(-4731971676530249);
        let positive = substrate_fixed::types::I64F64::from_bits(4731971676530249);

        let eq_fixed_u218_from_abs = eq_fixedu128_from_i64f64(negative.abs());
        let eq_fixed_u218_from_pos = eq_fixedu128_from_i64f64(positive);
        // println!("{:?} {:?}", negative, eq_fixed_u218_from_neg);
        // println!(" {:?} {:?}", positive, eq_fixed_u218_from_pos);
        assert_eq!(eq_fixed_u218_from_abs, eq_fixed_u218_from_pos);
    }
}
