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

use frame_support::traits::{Imbalance, SameOrOther, TryDrop};
use sp_runtime::traits::{Saturating, Zero};
use sp_std::mem;

/// Opaque, move-only struct with private fields that serves as a token denoting that
/// funds have been created without any equal and opposite accounting.
#[must_use]
#[derive(Default)]
pub struct PositiveImbalance<Balance>(Balance);

impl<Balance> PositiveImbalance<Balance> {
    /// Create a new positive imbalance from a balance.
    pub fn new(amount: Balance) -> Self {
        PositiveImbalance(amount)
    }
}

/// Opaque, move-only struct with private fields that serves as a token denoting that
/// funds have been destroyed without any equal and opposite accounting.
#[must_use]
#[derive(Default)]
pub struct NegativeImbalance<Balance>(Balance);

impl<Balance> NegativeImbalance<Balance> {
    /// Create a new negative imbalance from a balance.
    pub fn new(amount: Balance) -> Self {
        NegativeImbalance(amount)
    }
}

impl<Balance> TryDrop for PositiveImbalance<Balance>
where
    Balance: Zero,
{
    fn try_drop(self) -> Result<(), Self> {
        if self.0.is_zero() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl<Balance> Imbalance<Balance> for PositiveImbalance<Balance>
where
    Balance: Copy + Default + Zero + Ord + Saturating,
{
    type Opposite = NegativeImbalance<Balance>;

    fn zero() -> Self {
        Self(Zero::zero())
    }

    fn drop_zero(self) -> Result<(), Self> {
        self.try_drop()
    }

    fn split(self, amount: Balance) -> (Self, Self) {
        let first = self.0.min(amount);
        let second = self.0.saturating_sub(first);

        mem::forget(self);
        (Self(first), Self(second))
    }

    fn merge(mut self, other: Self) -> Self {
        self.0 = self.0.saturating_add(other.0);
        mem::forget(other);

        self
    }

    fn subsume(&mut self, other: Self) {
        self.0 = self.0.saturating_add(other.0);
        mem::forget(other);
    }

    fn offset(self, other: Self::Opposite) -> SameOrOther<Self, NegativeImbalance<Balance>> {
        let (a, b) = (self.0, other.0);
        mem::forget((self, other));

        if a >= b {
            SameOrOther::Same(Self(a.saturating_sub(b)))
        } else {
            SameOrOther::Other(NegativeImbalance::new(b.saturating_sub(a)))
        }
    }

    fn peek(&self) -> Balance {
        self.0
    }
}

impl<Balance> TryDrop for NegativeImbalance<Balance>
where
    Balance: Zero,
{
    fn try_drop(self) -> Result<(), Self> {
        if self.0.is_zero() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl<Balance> Imbalance<Balance> for NegativeImbalance<Balance>
where
    Balance: Copy + Default + Zero + Ord + Saturating,
{
    type Opposite = PositiveImbalance<Balance>;

    fn zero() -> Self {
        Self(Zero::zero())
    }

    fn drop_zero(self) -> Result<(), Self> {
        self.try_drop()
    }

    fn split(self, amount: Balance) -> (Self, Self) {
        let first = self.0.min(amount);
        let second = self.0.saturating_sub(first);

        mem::forget(self);
        (Self(first), Self(second))
    }

    fn merge(mut self, other: Self) -> Self {
        self.0 = self.0.saturating_add(other.0);
        mem::forget(other);

        self
    }

    fn subsume(&mut self, other: Self) {
        self.0 = self.0.saturating_add(other.0);
        mem::forget(other);
    }

    fn offset(self, other: Self::Opposite) -> SameOrOther<Self, PositiveImbalance<Balance>> {
        let (a, b) = (self.0, other.0);
        mem::forget((self, other));

        if a >= b {
            SameOrOther::Same(Self(a.saturating_sub(b)))
        } else {
            SameOrOther::Other(PositiveImbalance::new(b.saturating_sub(a)))
        }
    }

    fn peek(&self) -> Balance {
        self.0
    }
}

impl<Balance> Drop for PositiveImbalance<Balance> {
    /// Basic drop handler will just square up the total issuance.
    fn drop(&mut self) {}
}

impl<Balance> Drop for NegativeImbalance<Balance> {
    /// Basic drop handler will just square up the total issuance.
    fn drop(&mut self) {}
}
