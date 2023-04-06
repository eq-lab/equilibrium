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

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

use eq_primitives::DECIMALS;
use sp_arithmetic::traits::{Saturating, Zero};
use sp_arithmetic::*;
pub mod converter;
pub mod ensure;
pub mod fixed;
pub mod math;
pub mod offchain;
pub mod ok_or_error;
pub mod test;
pub use eq_primitives::XcmBalance;
pub use eq_primitives::ONE_TOKEN;

pub mod vec_map {
    pub use eq_primitives::vec_map::{SortedVec, VecMap};
}

/// Returns a * b / c and (a * b) % c (wrapping to 128 bits) or None in the case of overflow and c = 0.
/// Always rounding down.
pub fn multiply_by_rational(
    a: impl Into<u128>,
    b: impl Into<u128>,
    c: impl Into<u128>,
) -> Option<u128> {
    // return a * b / c
    sp_runtime::helpers_128bit::multiply_by_rational_with_rounding(
        a.into(),
        b.into(),
        c.into(),
        sp_arithmetic::per_things::Rounding::Down,
    )
}

pub fn balance_from_xcm<Balance>(value: XcmBalance, decimals: u8) -> Option<Balance>
where
    Balance: sp_std::convert::TryFrom<XcmBalance>,
{
    let value = balance_swap_decimals(value, decimals, DECIMALS)?;
    Balance::try_from(value).ok()
}

pub fn balance_into_xcm<Balance>(value: Balance, decimals: u8) -> Option<XcmBalance>
where
    Balance: sp_std::convert::TryInto<XcmBalance>,
{
    let value = value.try_into().ok()?;
    balance_swap_decimals(value, DECIMALS, decimals)
}

pub fn balance_swap_decimals(value: XcmBalance, from: u8, to: u8) -> Option<XcmBalance> {
    const POW_TEN: [u128; 24] = [
        1,
        10,
        100,
        1_000,
        10_000,
        100_000,
        1_000_000,
        10_000_000,
        100_000_000,
        1_000_000_000,
        10_000_000_000,
        100_000_000_000,
        1_000_000_000_000,
        10_000_000_000_000,
        100_000_000_000_000,
        1_000_000_000_000_000,
        10_000_000_000_000_000,
        100_000_000_000_000_000,
        1_000_000_000_000_000_000,
        10_000_000_000_000_000_000,
        100_000_000_000_000_000_000,
        1_000_000_000_000_000_000_000,
        10_000_000_000_000_000_000_000,
        100_000_000_000_000_000_000_000,
    ];
    use sp_std::cmp::Ordering::*;
    Some(match from.cmp(&to) {
        Less => value.checked_mul(*POW_TEN.get((to - from) as usize)?)?,
        Equal => value,
        Greater => value.checked_div(*POW_TEN.get((from - to) as usize)?)?,
    })
}

use xcm::latest::{
    Junction::*,
    Junctions::{self, *},
};
use xcm::v1::MultiLocation;

pub fn chain_part(this: &MultiLocation) -> Option<MultiLocation> {
    match (this.parents, this.first_interior()) {
        // sibling parachain
        (1, Some(Parachain(id))) => Some(MultiLocation::new(1, X1(Parachain(*id)))),
        // parent
        (1, _) => Some(MultiLocation::parent()),
        // children parachain
        (0, Some(Parachain(id))) => Some(MultiLocation::new(0, X1(Parachain(*id)))),
        _ => None,
    }
}

pub fn non_chain_part(this: &MultiLocation) -> Junctions {
    let mut junctions = this.interior().clone();
    while let Some(Parachain(_)) = junctions.first() {
        let _ = junctions.take_first();
    }

    junctions
}

#[test]
fn balance_from_xcm_balance_test() {
    let relay_balance: u128 = 123456789999;
    assert_eq!(
        123456789,
        balance_from_xcm::<u64>(relay_balance, 12).unwrap()
    );
    let large_relay_balance: u128 = (u64::MAX as u128 + 1) * 100000;
    assert!(balance_from_xcm::<u64>(large_relay_balance, 12).is_none());

    for i in 0..100 {
        assert_eq!(0, balance_from_xcm::<u64>(i, 12).unwrap());
    }

    assert_eq!(1, balance_from_xcm::<u64>(1000, 12).unwrap());
}

#[test]
fn xcm_balance_from_balance_test() {
    let balance: u64 = 123_456_789_999;
    assert_eq!(Some(123_456_789_999_000), balance_into_xcm(balance, 12));
    let large_balance: u64 = u64::MAX;
    assert_eq!(
        Some(u64::MAX as u128 * 1000),
        balance_into_xcm(large_balance, 12)
    );
}

#[test]
fn xcm_balance_with_custom_decimals() {
    assert_eq!(Some(123), balance_into_xcm(123_456_789_999_u64, 0));
    assert_eq!(Some(123_456), balance_into_xcm(123_456_789_999_u64, 3));
    assert_eq!(
        Some(1_234_567_899_990),
        balance_into_xcm(123_456_789_999_u64, 10)
    );
    assert_eq!(
        Some(123_456_789_999_000_000_000),
        balance_into_xcm(123_456_789_999_u64, 18)
    );

    assert_eq!(
        Some(123_456_789_999_u64),
        balance_from_xcm(123_456_789_999_000_000_000, 18)
    );
    assert_eq!(
        Some(123_456_789_999_u64),
        balance_from_xcm(1_234_567_899_990, 10)
    );
    assert_eq!(Some(123_456_000_000_u64), balance_from_xcm(123_456, 3));
    assert_eq!(Some(123_000_000_000_u64), balance_from_xcm(123, 0));

    let out_of_u64 = 1_000_000_000_000_000_000_000_u128;
    assert_eq!(Option::<u64>::None, balance_from_xcm(out_of_u64, 3));
    assert_eq!(Option::<u64>::None, balance_from_xcm(out_of_u64, 6));
    assert_eq!(Option::<u64>::None, balance_from_xcm(out_of_u64, 9));
    assert_eq!(
        Some(1_000_000_000_000_000_000_u64),
        balance_from_xcm(out_of_u64, 12)
    );
    assert_eq!(
        Some(1_000_000_000_000_000_u64),
        balance_from_xcm(out_of_u64, 15)
    );
}
