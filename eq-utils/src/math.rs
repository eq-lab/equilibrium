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

#![allow(clippy::result_unit_err)]
use super::*;
#[allow(unused_imports)]
use frame_support::debug;
use sp_arithmetic::traits::One;
use substrate_fixed::transcendental;

type InnerFixed = substrate_fixed::FixedI128<substrate_fixed::types::extra::U64>;

fn to_inner_fixed(x: FixedI128) -> InnerFixed {
    let sign = if x.is_zero() {
        FixedI128::one()
    } else {
        x / x.saturating_abs()
    };
    let sign = sign.into_inner() / FixedI128::DIV;

    let (a, b) = (
        x.saturating_abs().floor().into_inner(),
        x.frac().into_inner(),
    );
    let int_part = (a / FixedI128::DIV) << 64;
    let frac_part = (b << 64) / FixedI128::DIV;

    InnerFixed::from_bits(int_part | frac_part) * InnerFixed::from_num(sign)
}

pub(crate) fn from_inner_fixed(x: InnerFixed) -> FixedI128 {
    let bits = x.to_bits();

    if bits == 0 {
        return FixedI128::zero();
    }

    let right = bits ^ (bits >> 64) << 64;
    let correction = if bits > 0 && right > 17 {
        // 17 in the right side is 10e-18 for positive numbers
        1
    } else if bits < 0 && right > 18446744073709551598 {
        // 18446744073709551598 in the right side is 10e-18 for negative numbers
        1
    } else {
        0
    };
    FixedI128::from_inner(
        (bits >> 64) * FixedI128::DIV + ((right * FixedI128::DIV) >> 64) + correction,
    )
}

pub trait MathUtils
where
    Self: Sized,
{
    fn sqrt(self) -> Result<Self, ()>;
    fn ln(self) -> Result<Self, ()>;

    fn sqr(self) -> Self; // add Option or result?

    fn exp(self) -> Result<Self, ()>;

    fn pow(self, y: Self) -> Result<Self, ()>;
}

impl MathUtils for FixedI128 {
    fn sqrt(self) -> Result<Self, ()> {
        let result = transcendental::sqrt(to_inner_fixed(self)).map_err(|_| ())?;
        Ok(from_inner_fixed(result))
    }

    fn sqr(self) -> Self {
        self.saturating_mul(self)
    }

    fn ln(self) -> Result<Self, ()> {
        let result = transcendental::ln(to_inner_fixed(self))?;
        Ok(from_inner_fixed(result))
    }

    fn exp(self) -> Result<Self, ()> {
        let result = transcendental::exp(to_inner_fixed(self))?;
        Ok(from_inner_fixed(result))
    }

    fn pow(self, y: Self) -> Result<Self, ()> {
        let result = transcendental::pow(to_inner_fixed(self), to_inner_fixed(y))?;
        Ok(from_inner_fixed(result))
    }
}

#[cfg(test)]
use frame_support::assert_ok;

// ------------------------------------------ Sqrt tests ------------------------------------------

#[test]
fn sqrt_for_integers() {
    assert_ok!(
        MathUtils::sqrt(FixedI128::saturating_from_integer(25)),
        FixedI128::saturating_from_integer(5)
    );
    assert_ok!(
        MathUtils::sqrt(FixedI128::saturating_from_integer(1729225)),
        FixedI128::saturating_from_integer(1315)
    );
}

#[test]
fn sqrt_for_floats() {
    let sqrt = MathUtils::sqrt(fx128!(123455, 224880000)).unwrap();
    let expected_sqrt = fx128!(351, 361957076);
    assert_eq_fx128!(sqrt, expected_sqrt, 9);

    let sqrt = MathUtils::sqrt(fx128!(98745, 023550000)).unwrap();
    let expected_sqrt = fx128!(314, 237209048);
    assert_eq_fx128!(sqrt, expected_sqrt, 9);

    let sqrt = MathUtils::sqrt(fx128!(632987450333673, 023550000123345567)).unwrap();
    let expected_sqrt = fx128!(25159241, 8474);
    assert_eq_fx128!(sqrt, expected_sqrt, 4);
}

#[test]
fn sqrt_fails() {
    assert!(MathUtils::sqrt(FixedI128::saturating_from_integer(-1)).is_err());
}

#[test]
fn sqrt_test_small_num() {
    let actual = MathUtils::sqrt(fx128!(0, 001886109)).unwrap();
    let expected = fx128!(0, 043429356);
    assert_eq_fx128!(actual, expected, 5);
}

// ------------------------------------------- ln tests -------------------------------------------

#[test]
fn ln_test() {
    let ln = FixedI128::from_inner(1).ln().unwrap();
    let expected_ln = -fx128!(41, 4465316739);
    assert_eq_fx128!(ln, expected_ln, 1);

    let ln = FixedI128::saturating_from_integer(25).ln().unwrap();
    let expected_ln = fx128!(3, 21887582487);
    assert_eq_fx128!(ln, expected_ln, 9);

    let ln = FixedI128::saturating_from_integer(97965987).ln().unwrap();
    let expected_ln = fx128!(18, 400130905);
    assert_eq_fx128!(ln, expected_ln, 9);

    let ln = fx128!(123215, 4254).ln().unwrap();
    let expected_ln = fx128!(11, 7216895284);
    assert_eq_fx128!(ln, expected_ln, 9);

    let ln = fx128!(846841, 412120).ln().unwrap();
    let expected_ln = fx128!(13, 6492687213);
    assert_eq_fx128!(ln, expected_ln, 9);

    let ln = fx128!(5155355, 121353000).ln().unwrap();
    let expected_ln = fx128!(15, 4555465618);
    assert_eq_fx128!(ln, expected_ln, 9);

    let ln = fx128!(5155355467555098, 121353000987654321).ln().unwrap();
    let expected_ln = fx128!(36, 178812465881);
    assert_eq_fx128!(ln, expected_ln, 4);
}

#[test]
fn ln_fails() {
    assert!(FixedI128::saturating_from_integer(-1).ln().is_err());
}

#[test]
fn ln_test_e() {
    let e = fx128!(2, 718281828);

    let actual = e.ln().unwrap();
    let expected = fx128!(1, 0);
    assert_eq_fx128!(actual, expected, 9);
}

// ------------------------------------------- exp tests -------------------------------------------

#[test]
fn exp_test() {
    assert_eq_fx128!(fx128!(1, 0).exp().unwrap(), fx128!(2, 718281828), 7)
}

// ------------------------------------------- pow tests -------------------------------------------

#[test]
fn pow_test() {
    let x = fx128!(10, 0);
    let y = fx128!(0, 75);

    let actual = x.pow(y).unwrap();
    let expected = fx128!(5, 623413252);
    assert_eq_fx128!(actual, expected, 6);
}

#[test]
fn pow_test_pow_0() {
    let x = fx128!(33, 0);
    let y = fx128!(0, 0);

    let actual = x.pow(y).unwrap();
    let expected = fx128!(1, 0);
    assert_eq_fx128!(actual, expected, 6);
}

#[test]
fn pow_test_0_base() {
    let x = FixedI128::zero();
    let y = fx128!(3, 0);

    let actual = x.pow(y).unwrap();
    let expected = FixedI128::zero();
    assert_eq_fx128!(actual, expected, 6);
}

#[test]
fn pow_test_pow_1() {
    let x = fx128!(10_000_001, 1234567);
    let y = fx128!(1, 0);

    let actual = x.pow(y).unwrap();
    let expected = fx128!(10_000_001, 1234567);

    assert_eq_fx128!(actual, expected, 6);
}

#[test]
fn pow_test_0_pow_0() {
    // Decided that we expect 0^0 to be 0, as in used math library

    let x = FixedI128::zero();
    let y = FixedI128::zero();

    let actual = x.pow(y).unwrap();
    assert_eq_fx128!(actual, FixedI128::zero(), 6);
}

#[test]
fn pow_test_e() {
    let e = fx128!(2, 718281828);
    let y = fx128!(1, 0);

    let actual = e.pow(y).unwrap();
    let expected = fx128!(2, 718281828);
    assert_eq_fx128!(actual, expected, 6);
}

#[test]
fn pow_test_neg_power() {
    let e = fx128!(2, 0);
    let y = fx128!(-2, 0);

    let actual = e.pow(y).unwrap();
    let expected = fx128!(0, 25);
    assert_eq_fx128!(actual, expected, 6);
}

#[test]
fn pow_complex_result() {
    let base = fx128!(-1, 0);
    let pow = fx128!(0, 5);

    let result = base.pow(pow);
    assert!(result.is_err());
}

// ------------------------------------------ Other tests ------------------------------------------

#[test]
fn inner_fixed_conversions() {
    let num = fx128!(12, 345);
    let inner_fixed = to_inner_fixed(num);
    let actual = from_inner_fixed(inner_fixed);

    assert_eq_fx128!(actual, num, 17);
}
