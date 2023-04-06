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

use crate::balance::Balance;
use crate::ONE_TOKEN;
use equilibrium_curve_amm::traits::CheckedConvert;
use frame_support::codec::{CompactAs, Decode, Encode};
use sp_arithmetic::per_things::Rounding;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::sp_std::convert::{TryFrom, TryInto};
use sp_runtime::traits::Convert;
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub};
use sp_runtime::{PerThing, Permill};
use sp_std::ops::{Add, Div, Mul, Sub};

#[derive(
    Encode,
    Decode,
    CompactAs,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    scale_info::TypeInfo,
)]
pub struct CurveNumber(u128);

const CURVE_NUMBER_DIV: u128 = 1_000_000_000_000_000;

impl CurveNumber {
    pub fn max_value() -> CurveNumber {
        CurveNumber(u128::MAX)
    }

    pub fn min_value() -> CurveNumber {
        CurveNumber(u128::MIN)
    }

    pub fn zero() -> CurveNumber {
        CurveNumber(0u128)
    }

    pub fn one() -> CurveNumber {
        Self::from_inner(Self::accuracy())
    }

    pub fn from_inner(num: u128) -> CurveNumber {
        CurveNumber(num)
    }

    pub fn accuracy() -> u128 {
        CURVE_NUMBER_DIV
    }

    pub fn into_inner(self) -> u128 {
        self.0
    }

    pub fn checked_from_u128(a: u128) -> Option<CurveNumber> {
        a.checked_mul(Self::accuracy()).map(Self::from_inner)
    }

    pub fn saturating_from_u128(a: u128) -> CurveNumber {
        Self::checked_from_u128(a).unwrap_or(Self::max_value())
    }

    pub fn checked_from_rational(n: u128, d: u128) -> Option<CurveNumber> {
        if d == 0 {
            return None;
        }
        multiply_by_rational_with_rounding(n, Self::accuracy(), d, Rounding::Down)
            .map(Self::from_inner)
    }

    pub fn saturating_from_rational(n: u128, d: u128) -> CurveNumber {
        if d == 0 {
            panic!("attempt to divide by zero")
        }
        Self::checked_from_rational(n, d).unwrap_or(Self::max_value())
    }

    pub fn from_perthing<P: PerThing>(p: P) -> CurveNumber {
        let accuracy = P::ACCURACY;
        let value = p.deconstruct();

        Self::saturating_from_rational(value.into(), accuracy.into())
    }
}

impl Add for CurveNumber {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl CheckedAdd for CurveNumber {
    fn checked_add(&self, v: &Self) -> Option<Self> {
        self.0.checked_add(v.0).map(Self)
    }
}

impl Sub for CurveNumber {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl CheckedSub for CurveNumber {
    fn checked_sub(&self, v: &Self) -> Option<Self> {
        self.0.checked_sub(v.0).map(Self)
    }
}

impl Mul for CurveNumber {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        self.checked_mul(&rhs)
            .unwrap_or_else(|| panic!("attempt to multiply with overflow"))
    }
}

impl CheckedMul for CurveNumber {
    fn checked_mul(&self, v: &Self) -> Option<Self> {
        multiply_by_rational_with_rounding(self.0, v.0, Self::accuracy(), Rounding::Down).map(Self)
    }
}

impl Div for CurveNumber {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        if rhs.0 == 0 {
            panic!("attempt to divide by zero")
        }
        self.checked_div(&rhs)
            .unwrap_or_else(|| panic!("attempt to divide with overflow"))
    }
}

impl CheckedDiv for CurveNumber {
    fn checked_div(&self, v: &Self) -> Option<Self> {
        if v.0 == 0 {
            return None;
        }
        multiply_by_rational_with_rounding(self.0, Self::accuracy(), v.0, Rounding::Down).map(Self)
    }
}

pub struct CurveNumberConvert;

impl Convert<Balance, CurveNumber> for CurveNumberConvert {
    fn convert(a: Balance) -> CurveNumber {
        if ONE_TOKEN > CurveNumber::accuracy() {
            let accuracy = ONE_TOKEN / CurveNumber::accuracy();
            CurveNumber::from_inner(a / accuracy)
        } else {
            let accuracy = CurveNumber::accuracy() / ONE_TOKEN;
            CurveNumber::from_inner(a * accuracy)
        }
    }
}

impl Convert<CurveNumber, Balance> for CurveNumberConvert {
    fn convert(a: CurveNumber) -> Balance {
        if ONE_TOKEN > CurveNumber::accuracy() {
            let accuracy = ONE_TOKEN / CurveNumber::accuracy();
            (a.into_inner() * accuracy)
                .try_into()
                .expect("Wrong conversion from CurveNumber to Balance")
        } else {
            let accuracy = CurveNumber::accuracy() / ONE_TOKEN;
            (a.into_inner() / accuracy)
                .try_into()
                .expect("Wrong conversion from CurveNumber to Balance")
        }
    }
}

impl Convert<Permill, CurveNumber> for CurveNumberConvert {
    fn convert(a: Permill) -> CurveNumber {
        CurveNumber::from_perthing(a)
    }
}

impl Convert<u8, CurveNumber> for CurveNumberConvert {
    fn convert(a: u8) -> CurveNumber {
        CurveNumber::saturating_from_u128(a.into())
    }
}

impl CheckedConvert<usize, CurveNumber> for CurveNumberConvert {
    fn convert(a: usize) -> Option<CurveNumber> {
        let a = u128::try_from(a).ok()?;
        CurveNumber::checked_from_u128(a)
    }
}

#[cfg(test)]
mod test {
    use super::CurveNumber;
    use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub};

    #[test]
    fn op_checked_add_overflow_works() {
        let a = CurveNumber::max_value();
        let b = CurveNumber::saturating_from_u128(1);
        assert!(a.checked_add(&b).is_none());
    }

    #[test]
    fn op_add_works() {
        let a = CurveNumber::saturating_from_rational(5, 2);
        let b = CurveNumber::saturating_from_rational(1, 2);

        // Positive case: 6/2 = 3.
        assert_eq!(CurveNumber::saturating_from_u128(3), a + b);
    }

    #[test]
    fn op_checked_sub_underflow_works() {
        let a = CurveNumber::min_value();
        let b = CurveNumber::saturating_from_u128(1);
        assert!(a.checked_sub(&b).is_none());
    }

    #[test]
    fn op_sub_works() {
        let a = CurveNumber::saturating_from_rational(5, 2);
        let b = CurveNumber::saturating_from_rational(1, 2);

        assert_eq!(CurveNumber::saturating_from_u128(2), a - b);
    }

    #[test]
    fn op_checked_mul_overflow_works() {
        let a = CurveNumber::max_value();
        let b = CurveNumber::saturating_from_u128(2);
        assert!(a.checked_mul(&b).is_none());
    }

    #[test]
    fn op_mul_works() {
        let a = CurveNumber::saturating_from_u128(42);
        let b = CurveNumber::saturating_from_u128(2);
        assert_eq!(CurveNumber::saturating_from_u128(84), a * b);
    }

    #[test]
    #[should_panic(expected = "attempt to divide by zero")]
    fn op_div_panics_on_zero_divisor() {
        let a = CurveNumber::saturating_from_u128(1);
        let b = CurveNumber::saturating_from_u128(0);
        let _c = a / b;
    }

    #[test]
    fn op_div_works() {
        let a = CurveNumber::saturating_from_u128(42);
        let b = CurveNumber::saturating_from_u128(2);
        assert_eq!(CurveNumber::saturating_from_u128(21), a / b);
    }

    #[test]
    fn saturating_from_u128_works() {
        let inner_max = u128::MAX;
        let inner_min = u128::MIN;
        let accuracy = CurveNumber::accuracy();

        // Cases where integer fits.
        let a = CurveNumber::saturating_from_u128(42);
        assert_eq!(a.into_inner(), 42 * accuracy);

        // Max/min integers that fit.
        let a = CurveNumber::saturating_from_u128(inner_max / accuracy);
        assert_eq!(a.into_inner(), (inner_max / accuracy) * accuracy);

        let a = CurveNumber::saturating_from_u128(inner_min / accuracy);
        assert_eq!(a.into_inner(), (inner_min / accuracy) * accuracy);

        // Cases where integer doesn't fit, so it saturates.
        let a = CurveNumber::saturating_from_u128(inner_max / accuracy + 1);
        assert_eq!(a.into_inner(), inner_max);

        let a = CurveNumber::saturating_from_u128((inner_min / accuracy).saturating_sub(1));
        assert_eq!(a.into_inner(), inner_min);
    }

    #[test]
    fn checked_from_integer_works() {
        let inner_max = u128::MAX;
        let accuracy = CurveNumber::accuracy();

        // Case where integer fits.
        let a = CurveNumber::checked_from_u128(42).expect("42 * accuracy <= inner_max; qed");
        assert_eq!(a.into_inner(), 42 * accuracy);

        // Max integer that fit.
        let a = CurveNumber::checked_from_u128(inner_max / accuracy)
            .expect("(inner_max / accuracy) * accuracy <= inner_max; qed");
        assert_eq!(a.into_inner(), (inner_max / accuracy) * accuracy);

        // Case where integer doesn't fit, so it returns `None`.
        let a = CurveNumber::checked_from_u128(inner_max / accuracy + 1);
        assert_eq!(a, None);
    }

    #[test]
    fn from_inner_works() {
        let inner_max = u128::MAX;
        let inner_min = u128::MIN;

        assert_eq!(CurveNumber::max_value(), CurveNumber::from_inner(inner_max));
        assert_eq!(CurveNumber::min_value(), CurveNumber::from_inner(inner_min));
    }

    #[test]
    #[should_panic(expected = "attempt to divide by zero")]
    fn saturating_from_rational_panics_on_zero_divisor() {
        let _ = CurveNumber::saturating_from_rational(1, 0);
    }

    #[test]
    fn saturating_from_rational_works() {
        let inner_max = u128::MAX;
        let inner_min = u128::MIN;
        let accuracy = CurveNumber::accuracy();

        let a = CurveNumber::saturating_from_rational(5, 2);

        // Positive case: 2.5
        assert_eq!(a.into_inner(), 25 * accuracy / 10);

        // Max - 1.
        let a = CurveNumber::saturating_from_rational(inner_max - 1, accuracy);
        assert_eq!(a.into_inner(), inner_max - 1);

        // Min + 1.
        let a = CurveNumber::saturating_from_rational(inner_min + 1, accuracy);
        assert_eq!(a.into_inner(), inner_min + 1);

        // Max.
        let a = CurveNumber::saturating_from_rational(inner_max, accuracy);
        assert_eq!(a.into_inner(), inner_max);

        // Min.
        let a = CurveNumber::saturating_from_rational(inner_min, accuracy);
        assert_eq!(a.into_inner(), inner_min);

        // Zero.
        let a = CurveNumber::saturating_from_rational(0, 1);
        assert_eq!(a.into_inner(), 0);

        let a = CurveNumber::saturating_from_rational(inner_max - 1, accuracy);
        assert_eq!(a.into_inner(), inner_max - 1);

        let a = CurveNumber::saturating_from_rational(inner_min + 1, accuracy);
        assert_eq!(a.into_inner(), inner_min + 1);

        let a = CurveNumber::saturating_from_rational(inner_max, 1);
        assert_eq!(a.into_inner(), inner_max);

        let a = CurveNumber::saturating_from_rational(inner_min, 1);
        assert_eq!(a.into_inner(), inner_min);

        let a = CurveNumber::saturating_from_rational(inner_max, inner_max);
        assert_eq!(a.into_inner(), accuracy);

        let a = CurveNumber::saturating_from_rational(inner_max, 3 * accuracy);
        assert_eq!(a.into_inner(), inner_max / 3);

        let a = CurveNumber::saturating_from_rational(inner_min, 2 * accuracy);
        assert_eq!(a.into_inner(), inner_min / 2);

        let a = CurveNumber::saturating_from_rational(inner_min, accuracy / 3);
        assert_eq!(a.into_inner(), inner_min);

        let a = CurveNumber::saturating_from_rational(1, accuracy);
        assert_eq!(a.into_inner(), 1);

        // Out of accuracy.
        let a = CurveNumber::saturating_from_rational(1, accuracy + 1);
        assert_eq!(a.into_inner(), 0);
    }

    #[test]
    fn checked_from_rational_works() {
        let inner_max = u128::MAX;
        let inner_min = u128::MIN;
        let accuracy = CurveNumber::accuracy();

        // Divide by zero => None.
        let a = CurveNumber::checked_from_rational(1, 0);
        assert_eq!(a, None);

        // Max - 1.
        let a = CurveNumber::checked_from_rational(inner_max - 1, accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_max - 1);

        // Min + 1.
        let a = CurveNumber::checked_from_rational(inner_min + 1, accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_min + 1);

        // Max.
        let a = CurveNumber::checked_from_rational(inner_max, accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_max);

        // Min.
        let a = CurveNumber::checked_from_rational(inner_min, accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_min);

        let a = CurveNumber::checked_from_rational(inner_max, 3 * accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_max / 3);

        let a = CurveNumber::checked_from_rational(inner_min, 2 * accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_min / 2);

        let a = CurveNumber::checked_from_rational(1, accuracy).unwrap();
        assert_eq!(a.into_inner(), 1);

        let a = CurveNumber::checked_from_rational(1, accuracy + 1).unwrap();
        assert_eq!(a.into_inner(), 0);
    }

    #[test]
    fn checked_mul_works() {
        let inner_max = u128::MAX;

        let a = CurveNumber::saturating_from_u128(2);

        // Max - 1.
        let b = CurveNumber::from_inner(inner_max - 1);
        assert_eq!(
            a.checked_mul(&(b / CurveNumber::saturating_from_u128(2))),
            Some(b)
        );

        // Max.
        let c = CurveNumber::from_inner(inner_max);
        assert_eq!(
            a.checked_mul(&(c / CurveNumber::saturating_from_u128(2))),
            Some(b)
        );

        // Max + 1 => None.
        let e = CurveNumber::from_inner(1);
        assert_eq!(
            a.checked_mul(&(c / CurveNumber::saturating_from_u128(2) + e)),
            None
        );

        let a = CurveNumber::saturating_from_rational(1, 2);
        let c = CurveNumber::saturating_from_u128(255);

        assert_eq!(
            a.checked_mul(&CurveNumber::saturating_from_u128(42)),
            Some(CurveNumber::saturating_from_u128(21))
        );
        assert_eq!(
            c.checked_mul(&CurveNumber::saturating_from_u128(2)),
            Some(CurveNumber::saturating_from_u128(510))
        );
        assert_eq!(c.checked_mul(&CurveNumber::max_value()), None);
        assert_eq!(
            a.checked_mul(&CurveNumber::max_value()),
            CurveNumber::max_value().checked_div(&CurveNumber::saturating_from_u128(2))
        );
        assert_eq!(
            a.checked_mul(&CurveNumber::min_value()),
            CurveNumber::min_value().checked_div(&CurveNumber::saturating_from_u128(2))
        );
    }

    #[test]
    fn checked_div_works() {
        let inner_max = u128::MAX;
        let inner_min = u128::MIN;

        let a = CurveNumber::from_inner(inner_max);
        let b = CurveNumber::from_inner(inner_min);
        let c = CurveNumber::zero();
        let d = CurveNumber::one();
        let e = CurveNumber::saturating_from_u128(6);
        let f = CurveNumber::saturating_from_u128(5);

        assert_eq!(
            e.checked_div(&CurveNumber::saturating_from_u128(2)),
            Some(CurveNumber::saturating_from_u128(3))
        );
        assert_eq!(
            f.checked_div(&CurveNumber::saturating_from_u128(2)),
            Some(CurveNumber::saturating_from_rational(5, 2))
        );

        assert_eq!(
            a.checked_div(&CurveNumber::saturating_from_u128(inner_max)),
            Some(CurveNumber::saturating_from_u128(1))
        );
        assert_eq!(
            a.checked_div(&CurveNumber::saturating_from_u128(2)),
            Some(CurveNumber::from_inner(inner_max / 2))
        );
        assert_eq!(
            a.checked_div(&CurveNumber::max_value()),
            Some(CurveNumber::saturating_from_u128(1))
        );
        assert_eq!(a.checked_div(&d), Some(a));

        assert_eq!(
            b.checked_div(&CurveNumber::saturating_from_u128(2)),
            Some(CurveNumber::from_inner(inner_min / 2))
        );
        assert_eq!(b.checked_div(&a), Some(CurveNumber::min_value()));
        assert_eq!(
            c.checked_div(&CurveNumber::saturating_from_u128(1)),
            Some(CurveNumber::saturating_from_u128(0))
        );
        assert_eq!(
            d.checked_div(&CurveNumber::saturating_from_u128(1)),
            Some(CurveNumber::saturating_from_u128(1))
        );

        assert_eq!(a.checked_div(&CurveNumber::one()), Some(a));
        assert_eq!(b.checked_div(&CurveNumber::one()), Some(b));
        assert_eq!(c.checked_div(&CurveNumber::one()), Some(c));
        assert_eq!(d.checked_div(&CurveNumber::one()), Some(d));

        assert_eq!(a.checked_div(&CurveNumber::zero()), None);
        assert_eq!(b.checked_div(&CurveNumber::zero()), None);
        assert_eq!(c.checked_div(&CurveNumber::zero()), None);
        assert_eq!(d.checked_div(&CurveNumber::zero()), None);
    }
}
