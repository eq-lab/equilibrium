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

//! Implementation of stored positive and negative balances

use codec::{Decode, Encode, MaxEncodedLen};
use core::ops::{Add, AddAssign, Sub, SubAssign};
use frame_support::traits::{Imbalance, SignedImbalance};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
// use sp_arithmetic::{FixedI64, FixedPointNumber};
use sp_runtime::{
    traits::{AtLeast32Bit, CheckedAdd, CheckedSub, Zero},
    RuntimeDebug,
};
use sp_std::fmt::Debug;

pub trait EqMember: Sized + Debug + Eq + PartialEq + Clone {}
impl<T: Sized + Debug + Eq + PartialEq + Clone> EqMember for T {}

/// Balance representation based on number generic `Balance`
#[derive(
    Encode, Decode, MaxEncodedLen, Copy, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum SignedBalance<Balance> {
    Positive(Balance),
    Negative(Balance),
}

impl<Balance> Zero for SignedBalance<Balance>
where
    Balance: AtLeast32Bit,
{
    fn zero() -> Self {
        SignedBalance::Positive(Balance::zero())
    }

    fn is_zero(&self) -> bool {
        match self {
            Self::Positive(value) => value.is_zero(),
            Self::Negative(value) => value.is_zero(),
        }
    }
}

impl<Balance> Add for SignedBalance<Balance>
where
    Balance: AtLeast32Bit,
{
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        match rhs {
            SignedBalance::Positive(value) => self.add_balance(&value),
            SignedBalance::Negative(value) => self.sub_balance(&value),
        }
        .unwrap()
    }
}

impl<Balance> Sub for SignedBalance<Balance>
where
    Balance: AtLeast32Bit,
{
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        match rhs {
            SignedBalance::Negative(value) => self.add_balance(&value),
            SignedBalance::Positive(value) => self.sub_balance(&value),
        }
        .unwrap()
    }
}

impl<Balance> AddAssign for SignedBalance<Balance>
where
    Balance: AtLeast32Bit,
{
    fn add_assign(&mut self, rhs: Self) {
        *self = match rhs {
            SignedBalance::Positive(value) => self.add_balance(&value),
            SignedBalance::Negative(value) => self.sub_balance(&value),
        }
        .unwrap()
    }
}

impl<Balance> SubAssign for SignedBalance<Balance>
where
    Balance: AtLeast32Bit,
{
    fn sub_assign(&mut self, rhs: Self) {
        *self = match rhs {
            SignedBalance::Positive(value) => self.sub_balance(&value),
            SignedBalance::Negative(value) => self.add_balance(&value),
        }
        .unwrap()
    }
}

impl<Balance> SignedBalance<Balance>
where
    Balance: AtLeast32Bit,
{
    pub fn sub_balance(&self, other: &Balance) -> Option<Self> {
        match self {
            SignedBalance::Positive(value) => {
                let min_to_remove = value.min(other);
                let new_value = value.checked_sub(&min_to_remove)?;
                let new_other = other.checked_sub(&min_to_remove)?;
                if new_other.is_zero() {
                    Some(SignedBalance::Positive(new_value))
                } else {
                    Some(SignedBalance::Negative(new_other))
                }
            }
            SignedBalance::Negative(value) => {
                let new_value = other.checked_add(value)?;
                Some(SignedBalance::Negative(new_value))
            }
        }
    }

    pub fn add_balance(&self, other: &Balance) -> Option<Self> {
        match self {
            SignedBalance::Negative(value) => {
                let min_to_remove = value.min(other);
                let new_value = value.checked_sub(&min_to_remove)?;
                let new_other = other.checked_sub(&min_to_remove)?;
                if new_value.is_zero() {
                    Some(SignedBalance::Positive(new_other))
                } else {
                    Some(SignedBalance::Negative(new_value))
                }
            }
            SignedBalance::Positive(value) => {
                let new_value = other.checked_add(value)?;
                Some(SignedBalance::Positive(new_value))
            }
        }
    }

    pub fn abs(&self) -> Balance {
        match self {
            SignedBalance::Positive(value) => value.clone(),
            SignedBalance::Negative(value) => value.clone(),
        }
    }

    pub fn negate(&self) -> SignedBalance<Balance> {
        match self {
            SignedBalance::Positive(value) => SignedBalance::Negative(value.clone()),
            SignedBalance::Negative(value) => SignedBalance::Positive(value.clone()),
        }
    }

    pub fn is_positive(&self) -> bool {
        match self {
            SignedBalance::Positive(_) => true,
            SignedBalance::Negative(_) => false,
        }
    }

    pub fn is_negative(&self) -> bool {
        !self.is_positive()
    }

    pub fn map<U: EqMember, F: FnOnce(Balance) -> U>(&self, f: F) -> SignedBalance<U> {
        match self {
            SignedBalance::Positive(value) => SignedBalance::Positive(f(value.clone())),
            SignedBalance::Negative(value) => SignedBalance::Negative(f(value.clone())),
        }
    }
}

impl<Balance> CheckedAdd for SignedBalance<Balance>
where
    Balance: AtLeast32Bit,
{
    fn checked_add(&self, rhs: &SignedBalance<Balance>) -> Option<SignedBalance<Balance>> {
        match (self, rhs) {
            (_, SignedBalance::Positive(value)) => self.add_balance(value),
            (_, SignedBalance::Negative(value)) => self.sub_balance(value),
        }
    }
}

impl<Balance> CheckedSub for SignedBalance<Balance>
where
    Balance: AtLeast32Bit,
{
    fn checked_sub(&self, rhs: &SignedBalance<Balance>) -> Option<SignedBalance<Balance>> {
        match (self, rhs) {
            (_, SignedBalance::Positive(value)) => self.sub_balance(value),
            (_, SignedBalance::Negative(value)) => self.add_balance(value),
        }
    }
}

impl<Balance> SignedBalance<Balance>
where
    Balance: AtLeast32Bit + Into<i128>,
{
    pub fn to_i128(&self) -> i128 {
        match self {
            SignedBalance::Positive(value) => Into::<i128>::into(value.clone()),
            SignedBalance::Negative(value) => -(Into::<i128>::into(value.clone())),
        }
    }
}

impl<Balance> Default for SignedBalance<Balance>
where
    Balance: Default,
{
    fn default() -> SignedBalance<Balance> {
        SignedBalance::Positive(Default::default())
    }
}

impl<B, P: Imbalance<B, Opposite = N>, N: Imbalance<B, Opposite = P>> From<&SignedImbalance<B, P>>
    for SignedBalance<B>
{
    fn from(imbalance: &SignedImbalance<B, P>) -> SignedBalance<B> {
        match imbalance {
            SignedImbalance::Positive(x) => SignedBalance::Positive(x.peek()),
            SignedImbalance::Negative(x) => SignedBalance::Negative(x.peek()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::SignedBalance;

    #[test]
    fn map_works() {
        let positive_balance = SignedBalance::Positive(2u32);
        assert_eq!(
            positive_balance.map(|v| v * 20u32),
            SignedBalance::Positive(40u32)
        );

        let negative_balance = SignedBalance::Negative(5u32);
        assert_eq!(
            negative_balance.map(|v| v * 5u32),
            SignedBalance::Negative(25u32)
        );
    }

    #[test]
    fn add_assign_works() {
        let mut positive_balance = SignedBalance::Positive(2u32);
        positive_balance += SignedBalance::Positive(4u32);
        assert_eq!(positive_balance, SignedBalance::Positive(6u32));

        let mut positive_balance = SignedBalance::Positive(2u32);
        positive_balance += SignedBalance::Negative(4u32);
        assert_eq!(positive_balance, SignedBalance::Negative(2u32));

        let mut negative_balance = SignedBalance::Negative(1u32);
        negative_balance += SignedBalance::Positive(2u32);
        assert_eq!(negative_balance, SignedBalance::Positive(1u32));

        let mut negative_balance = SignedBalance::Negative(1u32);
        negative_balance += SignedBalance::Negative(2u32);
        assert_eq!(negative_balance, SignedBalance::Negative(3u32));
    }

    #[test]
    fn sub_assign_works() {
        let mut positive_balance = SignedBalance::Positive(5u32);
        positive_balance -= SignedBalance::Positive(3u32);
        assert_eq!(positive_balance, SignedBalance::Positive(2u32));

        let mut positive_balance = SignedBalance::Positive(2u32);
        positive_balance -= SignedBalance::Negative(4u32);
        assert_eq!(positive_balance, SignedBalance::Positive(6u32));

        let mut negative_balance = SignedBalance::Negative(1u32);
        negative_balance -= SignedBalance::Positive(2u32);
        assert_eq!(negative_balance, SignedBalance::Negative(3u32));

        let mut negative_balance = SignedBalance::Negative(1u32);
        negative_balance -= SignedBalance::Negative(2u32);
        assert_eq!(negative_balance, SignedBalance::Positive(1u32));
    }
}

impl<B: PartialOrd + Eq + Clone + Debug> PartialOrd for SignedBalance<B> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        match (self, other) {
            (SignedBalance::Positive(a), SignedBalance::Positive(b)) => a.partial_cmp(b),
            (SignedBalance::Positive(_), SignedBalance::Negative(_)) => {
                Some(core::cmp::Ordering::Greater)
            }
            (SignedBalance::Negative(_), SignedBalance::Positive(_)) => {
                Some(core::cmp::Ordering::Less)
            }
            (SignedBalance::Negative(a), SignedBalance::Negative(b)) => b.partial_cmp(a),
        }
    }
}

impl<B: Ord + Eq + Clone + Debug> Ord for SignedBalance<B> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match (self, other) {
            (SignedBalance::Positive(a), SignedBalance::Positive(b)) => a.cmp(b),
            (SignedBalance::Positive(_), SignedBalance::Negative(_)) => {
                core::cmp::Ordering::Greater
            }
            (SignedBalance::Negative(_), SignedBalance::Positive(_)) => core::cmp::Ordering::Less,
            (SignedBalance::Negative(a), SignedBalance::Negative(b)) => b.cmp(a),
        }
    }
}
