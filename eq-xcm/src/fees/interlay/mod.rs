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

pub mod ibtc;
pub mod intr;
pub mod kbtc;

fn base_tx_in_dot() -> u128 {
    10_000_000_000 / 5706
}

pub fn dot_per_second() -> u128 {
    let base_weight = 86_298 * WEIGHT_PER_NANOS;
    let base_tx_per_second = (WEIGHT_PER_SECOND as u128) / base_weight;
    base_tx_per_second * base_tx_in_dot()
}

parameter_types! {
    pub const BaseXcmWeight: XcmWeight = Weight::from_parts(200_000_000, 0u64);
}
