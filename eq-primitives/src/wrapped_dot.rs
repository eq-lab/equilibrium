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

use sp_arithmetic::traits::{One, Zero};
use sp_arithmetic::{FixedI64, FixedPointNumber};
use sp_std::convert::{TryFrom, TryInto};
use sp_std::fmt::Debug;

pub trait EqDotPrice {
    /// Returns total dot amount in staking, free + bonded
    fn get_price_coeff<FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>>(
    ) -> Option<FixedNumber>;
}

impl EqDotPrice for () {
    fn get_price_coeff<FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>>(
    ) -> Option<FixedNumber> {
        FixedI64::one().try_into().ok()
    }
}
