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

pub mod eq;
pub mod eqd;
pub mod pha;

pub const PHA_UNIT: XcmBalance = 1_000_000_000_000;
pub const PHA_PER_SEC_MUL: XcmBalance = 80;
pub const PHA_PER_SEC: XcmBalance = PHA_PER_SEC_MUL * PHA_UNIT;
pub const PHA_PRICE: XcmBalance = 200_000_000_000;

parameter_types! {
    pub const BaseXcmWeight: XcmWeight = 1_000_000_000;
}
