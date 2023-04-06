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

use codec::{Decode, Encode, MaxEncodedLen};
use sp_runtime::RuntimeDebug;

#[derive(
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, scale_info::TypeInfo,
)]
pub enum ProxyType {
    Any,
}

impl Default for ProxyType {
    fn default() -> Self {
        ProxyType::Any
    }
}

impl MaxEncodedLen for ProxyType {
    fn max_encoded_len() -> usize {
        core::mem::size_of::<ProxyType>()
    }
}
