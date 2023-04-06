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

use super::*;

pub mod bnc;
pub mod eq;
pub mod eqd;
pub mod gens;

pub const BNCS: u128 = 1_000_000_000_000;
pub const DOLLARS: u128 = BNCS;
pub const CENTS: u128 = DOLLARS / 100;

fn base_tx_fee() -> u128 {
    CENTS / 10
}

pub fn dot_per_second() -> u128 {
    let base_weight = 86_298 * WEIGHT_PER_NANOS;
    let base_tx_per_second = WEIGHT_PER_SECOND / base_weight;
    let fee_per_second = base_tx_per_second * base_tx_fee();
    fee_per_second / 100 * 10 / 100
}

pub fn ksm_per_second() -> u128 {
    let base_weight = 86_298 * WEIGHT_PER_NANOS;
    let base_tx_per_second = WEIGHT_PER_SECOND / base_weight;
    let fee_per_second = base_tx_per_second * base_tx_fee();
    fee_per_second / 100
}

parameter_types! {
    pub const BaseXcmWeight: XcmWeight = 1_000_000_000;
}
