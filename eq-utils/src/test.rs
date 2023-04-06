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

use sp_arithmetic::FixedPointNumber;

#[macro_export]
macro_rules! fx64 {
    ($i: expr, $f:expr) => {{
        let mut fraq_str = String::from(stringify!($f));
        let existing_zeros_num = fraq_str.len() - fraq_str.trim_end_matches('0').len();

        fraq_str.push_str("000000000");
        let fraq_len = fraq_str[0..9].trim_end_matches('0').len();

        let mut fraq_div = 1i64;

        for _ in 0..existing_zeros_num {
            fraq_div = fraq_div * 10;
        }

        let mut fraq_mul = 1i64;

        for _ in 0..(9 - fraq_len) {
            fraq_mul = fraq_mul * 10;
        }

        FixedI64::from_inner($i * 1_000_000_000i64 + $f / fraq_div * fraq_mul)
    }};
}

#[macro_export]
macro_rules! fx128 {
    ($i: expr, $f:expr) => {{
        let mut fraq_str = String::from(stringify!($f));
        let existing_zeros_num = fraq_str.len() - fraq_str.trim_end_matches('0').len();

        fraq_str.push_str("000000000000000000");
        let fraq_len = fraq_str[0..18].trim_end_matches('0').len();

        let mut fraq_div = 1i128;

        for _ in 0..existing_zeros_num {
            fraq_div = fraq_div * 10;
        }

        let mut fraq_mul = 1i128;

        for _ in 0..(18 - fraq_len) {
            fraq_mul = fraq_mul * 10;
        }

        FixedI128::from_inner($i * 1_000_000_000_000_000_000_i128 + $f / fraq_div * fraq_mul)
    }};
}

#[macro_export]
macro_rules! assert_eq_fx64 {
    ($left:expr, $right:expr, $prec:expr) => {{
        let delta = ($left - $right).into_inner().abs();

        let mut max_delta = 1;

        for _ in 0..(9 - $prec) {
            max_delta = max_delta * 10;
        }

        assert!(
            delta < max_delta,
            "{:?} ({:?}) is not equals to right {:?} ({:?}) with precision {:?}",
            stringify!($left),
            $left,
            stringify!($right),
            $right,
            $prec
        );
    }};
}

#[macro_export]
macro_rules! assert_eq_fx128 {
    ($left:expr, $right:expr, $prec:expr) => {{
        let delta = ($left - $right).into_inner().abs();

        let mut max_delta = 1;

        for _ in 0..(18 - $prec) {
            max_delta = max_delta * 10;
        }

        assert!(
            delta < max_delta,
            "{:?} ({:?}) is not equals to right {:?} ({:?}) with precision {:?}",
            stringify!($left),
            $left,
            stringify!($right),
            $right,
            $prec
        );
    }};
}

pub fn to_prec<T: FixedPointNumber>(n: i32, x: T) -> T {
    let mut y = T::DIV;
    let ten = T::saturating_from_integer(10).into_inner() / T::DIV; // safe unwrap

    for _ in 0..n {
        y = y / ten
    }

    T::from_inner((x.into_inner() / y) * y)
}

mod test {
    #![cfg(test)]

    use crate::converter::FixedU128Convert;
    use crate::fixed::{
        fixedi128_from_balance, fixedi128_from_i64f64, fixedi64_to_i64f64, fixedu128_from_fixedi64,
        i64f64_to_fixedi64,
    };
    use crate::test::to_prec;
    use crate::{fx128, FixedI128};
    use frame_support::sp_runtime::FixedU128;
    use sp_arithmetic::{
        traits::{One, Zero},
        FixedI64, FixedPointNumber, Permill,
    };
    use sp_runtime::traits::Convert;
    use substrate_fixed::types::I64F64;

    #[test]
    fn test_fx64_trailing_zeros() {
        let actual = fx64!(0, 0016000);
        let expected = FixedI64::saturating_from_rational(16, 10000);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_fx64_no_trailing_zeros() {
        let actual = fx64!(0, 0016);
        let expected = FixedI64::saturating_from_rational(16, 10000);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_prec() {
        assert_eq!(to_prec(2, fx64!(0, 1234)), fx64!(0, 12));

        assert_eq!(to_prec(3, fx128!(1, 123456)), to_prec(3, fx128!(1, 123)));

        assert_eq!(
            to_prec(10, fx128!(0, 546372)),
            to_prec(10, fx128!(0, 546372))
        );

        assert_eq!(
            to_prec(4, -fx128!(0, 546372)),
            to_prec(4, -fx128!(0, 546372))
        );

        assert_eq!(
            to_prec(5, FixedU128::from_inner(0_123_456_789_000_000_000)),
            to_prec(5, FixedU128::from_inner(0_123_458_888_999_111_333))
        );

        assert_eq!(
            to_prec(0, FixedU128::from(1)),
            to_prec(0, FixedU128::from(1))
        )
    }

    #[test]
    fn fxi128_from_balance() {
        let balance: u128 = 9_500_000_000;
        let expected = Some(fx128!(9, 5));

        assert_eq!(fixedi128_from_balance(balance), expected);

        let overflows_i128 = i128::MAX as u128 + 1;
        assert_eq!(fixedi128_from_balance(overflows_i128), None);
    }

    #[test]
    fn fxi64_to_i64f64() {
        let zero = FixedI64::zero();
        let negative = fx64!(983158, 123456) * fx64!(-1, 0);
        let positive1 = fx64!(0, 0653);
        let positive2 = fx64!(1987654321, 123456789);

        assert_eq!(fixedi64_to_i64f64(zero), I64F64::from_num(0));
        assert_eq!(
            fixedi64_to_i64f64(negative),
            I64F64::from_num(-983158) + (I64F64::from_num(-123456) / 1_000_000)
        );
        assert_eq!(
            fixedi64_to_i64f64(positive1),
            I64F64::from_num(653) / 10_000
        );
        assert_eq!(
            fixedi64_to_i64f64(positive2),
            I64F64::from_num(1987654321) + (I64F64::from_num(123456789) / 1_000_000_000)
        );
    }

    #[test]
    fn i64f64_to_fxi64() {
        let zero = I64F64::from_num(0);

        // -191919.123456
        let negative = I64F64::from_num(-191919123456_i128) / I64F64::from_num(1_000_000);
        // 0.00653
        let positive1 = I64F64::from_num(653) / I64F64::from_num(100_000);
        // 1987654321.123456789
        let positive2 =
            I64F64::from_num(1987654321123456789_i128) / I64F64::from_num(1_000_000_000);

        assert_eq!(i64f64_to_fixedi64(zero), FixedI64::zero());
        assert_eq!(
            i64f64_to_fixedi64(negative),
            fx64!(191919, 123456) * fx64!(-1, 0)
        );
        assert_eq!(i64f64_to_fixedi64(positive1), fx64!(0, 00653));
        assert_eq!(i64f64_to_fixedi64(positive2), fx64!(1987654321, 123456789));
    }

    #[test]
    fn fixedi128_from_i64f64_test() {
        let zero = I64F64::from_num(0);

        // -191919.123456789123456789
        let negative1 = I64F64::from_num(-191919_i128)
            - I64F64::from_num(123456789123456789i64)
                / I64F64::from_num(1_000_000_000_000_000_000i64);

        // -0.00653
        let negative2 = I64F64::from_num(-653) / I64F64::from_num(100_000);

        // -0.1
        let negative3 = I64F64::from_num(-1) / I64F64::from_num(10);

        // -0.2
        let negative4 = I64F64::from_num(-2) / I64F64::from_num(10);

        // -0.999_999_999_999_999_999
        let negative5 = I64F64::from_num(-999999999999999999i64)
            / I64F64::from_num(1_000_000_000_000_000_000i64);

        // -0.0000000000000000001
        let bits = (I64F64::from_bits(18446744073709551613) - I64F64::from_num(1)).to_bits() + 1;
        let negative6 = I64F64::from_bits(bits);

        // -0.000000000000000001
        let negative7 = I64F64::from_num(-1) / I64F64::from_num(1_000_000_000_000_000_000i128);

        // -5
        let negative_int = I64F64::from_num(-5);

        // 0.00653
        let positive1 = I64F64::from_num(653) / I64F64::from_num(100_000);

        // 0.1
        let positive2 = I64F64::from_num(1) / I64F64::from_num(10);

        // 0.2
        let positive3 = I64F64::from_num(2) / I64F64::from_num(10);

        // 1987654321.123456789
        let positive4 =
            I64F64::from_num(1987654321123456789_i128) / I64F64::from_num(1_000_000_000);

        // 0.999_999_999_999_999_999
        let positive5 = I64F64::from_num(999999999999999999i64)
            / I64F64::from_num(1_000_000_000_000_000_000i64);

        // 0.0000000000000000001
        let positive6 = (I64F64::from_num(1) / I64F64::from_num(1_000_000_000_000_000_000i128))
            / I64F64::from_num(10);

        // 0.000000000000000001
        let positive7 = I64F64::from_num(1) / I64F64::from_num(1_000_000_000_000_000_000i128);

        // 5
        let positive_int = I64F64::from_num(5);

        assert_eq!(
            fixedi128_from_i64f64(negative1),
            fx128!(191919, 123456789123456789) * fx128!(-1, 0)
        );
        assert_eq!(
            fixedi128_from_i64f64(negative2),
            fx128!(0, 00653) * fx128!(-1, 0)
        );
        assert_eq!(
            fixedi128_from_i64f64(negative3),
            fx128!(0, 1) * fx128!(-1, 0)
        );
        assert_eq!(
            fixedi128_from_i64f64(negative4),
            fx128!(0, 2) * fx128!(-1, 0)
        );
        assert_eq!(
            fixedi128_from_i64f64(negative5),
            fx128!(0, 999999999999999999) * fx128!(-1, 0)
        );
        assert_eq!(
            fixedi128_from_i64f64(negative7),
            fx128!(0, 000000000000000001) * fx128!(-1, 0)
        );
        assert_eq!(fixedi128_from_i64f64(negative_int), fx128!(-5, 0));
        assert_eq!(fixedi128_from_i64f64(zero), FixedI128::zero());
        assert_eq!(fixedi128_from_i64f64(positive1), fx128!(0, 00653));
        assert_eq!(fixedi128_from_i64f64(positive2), fx128!(0, 1));
        assert_eq!(fixedi128_from_i64f64(positive3), fx128!(0, 2));
        assert_eq!(
            fixedi128_from_i64f64(positive4),
            fx128!(1987654321, 123456789)
        );
        assert_eq!(
            fixedi128_from_i64f64(positive5),
            fx128!(0, 999999999999999999)
        );
        assert_eq!(fixedi128_from_i64f64(positive6), FixedI128::zero());
        assert_eq!(
            fixedi128_from_i64f64(positive7),
            fx128!(0, 000000000000000001)
        );
        assert_eq!(fixedi128_from_i64f64(positive_int), fx128!(5, 0));
        assert_eq!(fixedi128_from_i64f64(negative6), FixedI128::zero());
    }

    #[test]
    fn convert_u64_to_fxu128() {
        assert_eq!(
            <FixedU128Convert as Convert<u64, FixedU128>>::convert(1_000_000_000u64),
            FixedU128::from(1)
        );
        assert_eq!(
            <FixedU128Convert as Convert<u64, FixedU128>>::convert(0),
            FixedU128::zero()
        );
        assert_eq!(
            <FixedU128Convert as Convert<u64, FixedU128>>::convert(1),
            FixedU128::saturating_from_rational(1, 1_000_000_000)
        );
    }

    #[test]
    fn convert_fxu128_to_u64() {
        assert_eq!(
            <FixedU128Convert as Convert<FixedU128, u64>>::convert(FixedU128::from(1)),
            1_000_000_000u64
        );
        assert_eq!(
            <FixedU128Convert as Convert<FixedU128, u64>>::convert(FixedU128::zero()),
            0u64
        );
        assert_eq!(
            <FixedU128Convert as Convert<FixedU128, u64>>::convert(
                FixedU128::saturating_from_rational(1, 1_000_000_000)
            ),
            1u64
        );
    }

    #[test]
    #[should_panic]
    fn convert_fxu128_to_u64_should_panic() {
        assert_eq!(
            <FixedU128Convert as Convert<FixedU128, u64>>::convert(FixedU128::from(u128::MAX)),
            1_000_000_000u64
        );
    }

    #[test]
    fn convert_permill_to_fxu128() {
        assert_eq!(
            <FixedU128Convert as Convert<Permill, FixedU128>>::convert(Permill::one()),
            FixedU128::one()
        );
        assert_eq!(
            <FixedU128Convert as Convert<Permill, FixedU128>>::convert(Permill::zero()),
            FixedU128::zero()
        );
    }

    #[test]
    fn convert_u8_to_fxu128() {
        assert_eq!(
            <FixedU128Convert as Convert<u8, FixedU128>>::convert(1u8),
            FixedU128::one()
        );
    }

    // #[test]
    // fn convert_usize_to_fxu128() {
    //     assert_eq!(
    //         <FixedU128Convert as CheckedConvert<usize, FixedU128>>::convert(1),
    //         Some(FixedU128::one())
    //     );
    //     assert_eq!(
    //         <FixedU128Convert as CheckedConvert<usize, FixedU128>>::convert(0),
    //         Some(FixedU128::zero())
    //     );
    // }

    #[test]
    fn fxu128_from_fx64() {
        assert_eq!(fixedu128_from_fixedi64(fx64!(-983158, 123456)), None);

        assert_eq!(fixedu128_from_fixedi64(fx64!(1, 0)), Some(FixedU128::one()));
    }
}
