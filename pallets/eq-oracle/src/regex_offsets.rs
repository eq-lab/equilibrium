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

use alloc::vec;
use regex_automata::{DenseDFA, Regex, SparseDFA};
use sp_std::prelude::*;

use lazy_static::lazy_static;

/// Get string as bytes and return match of "\\(.+\\)\\." .
/// Returns start and end offset of the leftmost first match.
/// If no match exists, then None is returned.
pub fn get_url_offset(query: &[u8]) -> Option<(usize, usize)> {
    lazy_static! {
        // precompiled and serialized in std DFAs for "\(.+\)\." regex
        static ref FORWARD_BYTES: Vec<u8> = vec![114, 117, 115, 116, 45, 114, 101, 103, 101, 120, 45, 97, 117, 116, 111, 109, 97, 116, 97, 45, 100, 102, 97, 0, 255, 254, 1, 0, 2, 0, 1, 0, 55, 2, 0, 0, 0, 0, 0, 0, 37, 0, 0, 0, 0, 0, 0, 0, 21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 4, 5, 5, 5, 5, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 11, 11, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 13, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 15, 16, 16, 17, 18, 18, 18, 19, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 76, 2, 0, 0, 76, 2, 76, 2, 97, 2, 76, 2, 76, 2, 76, 2, 0, 0, 0, 0, 0, 0, 0, 0, 118, 2, 139, 2, 160, 2, 181, 2, 160, 2, 202, 2, 223, 2, 244, 2, 0, 0, 210, 0, 55, 2, 210, 0, 231, 0, 210, 0, 210, 0, 210, 0, 210, 0, 0, 0, 0, 0, 0, 0, 0, 0, 252, 0, 17, 1, 38, 1, 59, 1, 38, 1, 80, 1, 101, 1, 122, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 55, 2, 55, 2, 55, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 63, 0, 63, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 63, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 105, 0, 105, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 105, 0, 105, 0, 105, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 105, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 210, 0, 55, 2, 210, 0, 231, 0, 143, 1, 210, 0, 210, 0, 210, 0, 0, 0, 0, 0, 0, 0, 0, 0, 252, 0, 17, 1, 38, 1, 59, 1, 38, 1, 80, 1, 101, 1, 122, 1, 0, 0, 210, 0, 55, 2, 210, 0, 231, 0, 143, 1, 210, 0, 210, 0, 210, 0, 0, 0, 0, 0, 0, 0, 0, 0, 164, 1, 185, 1, 206, 1, 227, 1, 206, 1, 248, 1, 13, 2, 34, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 210, 0, 210, 0, 210, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 252, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 252, 0, 252, 0, 252, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 252, 0, 252, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 38, 1, 38, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 38, 1, 38, 1, 38, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 38, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 210, 0, 55, 2, 210, 0, 231, 0, 143, 1, 210, 0, 21, 0, 210, 0, 0, 0, 0, 0, 0, 0, 0, 0, 252, 0, 17, 1, 38, 1, 59, 1, 38, 1, 80, 1, 101, 1, 122, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 210, 0, 210, 0, 210, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 164, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 164, 1, 164, 1, 164, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 164, 1, 164, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 206, 1, 206, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 206, 1, 206, 1, 206, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 206, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 55, 2, 55, 2, 55, 2, 42, 0, 55, 2, 55, 2, 55, 2, 55, 2, 0, 0, 0, 0, 0, 0, 0, 0, 63, 0, 84, 0, 105, 0, 126, 0, 105, 0, 147, 0, 168, 0, 189, 0, 0, 0, 76, 2, 0, 0, 76, 2, 76, 2, 97, 2, 76, 2, 76, 2, 76, 2, 0, 0, 0, 0, 0, 0, 0, 0, 118, 2, 139, 2, 160, 2, 181, 2, 160, 2, 202, 2, 223, 2, 244, 2, 0, 0, 76, 2, 0, 0, 76, 2, 76, 2, 97, 2, 76, 2, 21, 0, 76, 2, 0, 0, 0, 0, 0, 0, 0, 0, 118, 2, 139, 2, 160, 2, 181, 2, 160, 2, 202, 2, 223, 2, 244, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 76, 2, 76, 2, 76, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 118, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 118, 2, 118, 2, 118, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 118, 2, 118, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 160, 2, 160, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 160, 2, 160, 2, 160, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 160, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        static ref REVERSE_BYTES: Vec<u8> = vec![114, 117, 115, 116, 45, 114, 101, 103, 101, 120, 45, 97, 117, 116, 111, 109, 97, 116, 97, 45, 100, 102, 97, 0, 255, 254, 1, 0, 2, 0, 3, 0, 210, 0, 0, 0, 0, 0, 0, 0, 11, 0, 0, 0, 0, 0, 0, 0, 21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 4, 5, 5, 5, 5, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 11, 11, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 13, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 15, 16, 16, 17, 18, 18, 18, 19, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 84, 0, 0, 0, 84, 0, 21, 0, 84, 0, 84, 0, 84, 0, 84, 0, 105, 0, 105, 0, 105, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 84, 0, 0, 0, 84, 0, 84, 0, 84, 0, 84, 0, 84, 0, 84, 0, 105, 0, 105, 0, 105, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 84, 0, 0, 0, 84, 0, 21, 0, 84, 0, 84, 0, 84, 0, 84, 0, 105, 0, 105, 0, 105, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 126, 0, 126, 0, 147, 0, 0, 0, 84, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 168, 0, 189, 0, 189, 0, 0, 0, 0, 0, 0, 0, 84, 0, 84, 0, 84, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 168, 0, 189, 0, 189, 0, 0, 0, 0, 0, 84, 0, 84, 0, 0, 0, 84, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 84, 0, 84, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 84, 0, 84, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        static ref RE: Regex<DenseDFA<&'static [u16], u16>> = {
            let forward: DenseDFA<&'static [u16], u16> = unsafe {
                DenseDFA::from_bytes(&FORWARD_BYTES)
            };
            let reverse: DenseDFA<&'static [u16], u16> = unsafe {
                DenseDFA::from_bytes(&REVERSE_BYTES)
            };
            Regex::from_dfas(forward, reverse)
        };
    }
    // normal regex that can be used in std is "^json\((?P<url>.+)\)(?P<path>(\.{1}[\w\[\]\d]+)+)$"
    // here we just search "\(.+\)" after "json"
    RE.find_at(query, 4)
}

/// Get string as bytes and return vector of matches of "\[\d+\]".
/// Each match is start and end offset of the leftmost first match.
pub fn get_index_offsets(item: &[u8]) -> Vec<(usize, usize)> {
    lazy_static! {
        // precompiled and serialized in std DFAs for "\[\d+\]" regex
        static ref FORWARD_BYTES: Vec<u8> = vec![114, 117, 115, 116, 45, 114, 101, 103, 101, 120, 45, 97, 117, 116, 111, 109, 97, 116, 97, 45, 115, 112, 97, 114, 115, 101, 45, 100, 102, 97, 0, 255, 254, 1, 0, 2, 0, 0, 0, 104, 3, 0, 0, 0, 0, 0, 0, 33, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15, 16, 17, 18, 18, 19, 19, 20, 21, 22, 23, 24, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 63, 63, 64, 64, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 65, 66, 67, 68, 69, 69, 69, 70, 71, 72, 73, 73, 73, 73, 73, 73, 73, 73, 74, 75, 75, 76, 77, 78, 79, 80, 80, 80, 81, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 82, 0, 0, 0, 0, 22, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 6, 65, 65, 66, 66, 67, 67, 68, 68, 69, 69, 70, 70, 71, 71, 72, 72, 73, 73, 74, 74, 75, 75, 76, 76, 77, 77, 78, 78, 79, 79, 80, 80, 81, 81, 104, 3, 136, 0, 104, 3, 4, 0, 104, 3, 94, 0, 234, 0, 94, 0, 248, 0, 94, 0, 6, 1, 16, 1, 122, 1, 106, 0, 196, 1, 106, 0, 112, 0, 106, 0, 246, 1, 4, 2, 124, 0, 130, 0, 1, 0, 7, 63, 104, 3, 1, 0, 35, 63, 94, 0, 1, 0, 7, 63, 94, 0, 1, 0, 7, 34, 94, 0, 1, 0, 20, 63, 106, 0, 1, 0, 7, 63, 106, 0, 1, 0, 7, 19, 106, 0, 24, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 65, 65, 66, 66, 67, 67, 68, 68, 69, 69, 70, 70, 71, 71, 72, 72, 73, 73, 74, 74, 75, 75, 76, 76, 77, 77, 78, 78, 79, 79, 80, 80, 81, 81, 104, 3, 136, 0, 104, 3, 4, 0, 104, 3, 2, 0, 104, 3, 94, 0, 234, 0, 94, 0, 248, 0, 94, 0, 6, 1, 16, 1, 122, 1, 106, 0, 196, 1, 106, 0, 112, 0, 106, 0, 246, 1, 4, 2, 124, 0, 130, 0, 3, 0, 7, 34, 35, 43, 44, 63, 104, 3, 136, 0, 104, 3, 3, 0, 7, 49, 50, 59, 60, 63, 104, 3, 136, 0, 104, 3, 2, 0, 7, 15, 16, 63, 136, 0, 104, 3, 26, 0, 35, 38, 39, 39, 40, 40, 41, 41, 42, 42, 43, 43, 44, 44, 45, 45, 46, 46, 47, 47, 48, 48, 49, 49, 50, 50, 51, 51, 52, 52, 53, 53, 54, 54, 55, 55, 56, 56, 57, 57, 58, 58, 59, 59, 60, 60, 61, 61, 62, 62, 63, 63, 94, 0, 26, 3, 94, 0, 26, 3, 94, 0, 26, 3, 94, 0, 26, 3, 94, 0, 26, 3, 94, 0, 26, 3, 94, 0, 26, 3, 94, 0, 26, 3, 94, 0, 26, 3, 94, 0, 26, 3, 94, 0, 2, 3, 94, 0, 2, 3, 234, 0, 94, 0, 18, 0, 7, 7, 8, 8, 9, 9, 10, 33, 34, 34, 35, 35, 36, 38, 39, 39, 40, 40, 41, 41, 42, 43, 44, 44, 45, 46, 47, 47, 48, 48, 49, 50, 51, 51, 52, 63, 94, 0, 6, 1, 2, 3, 94, 0, 234, 0, 2, 3, 94, 0, 72, 3, 94, 0, 2, 3, 94, 0, 86, 3, 94, 0, 2, 3, 248, 0, 94, 0, 86, 3, 94, 0, 12, 0, 7, 26, 27, 27, 28, 36, 37, 37, 38, 38, 39, 40, 41, 41, 42, 42, 43, 43, 44, 48, 49, 49, 50, 63, 94, 0, 234, 0, 94, 0, 2, 3, 6, 1, 94, 0, 50, 3, 94, 0, 2, 3, 94, 0, 248, 0, 94, 0, 3, 0, 7, 61, 62, 62, 63, 63, 94, 0, 2, 3, 94, 0, 9, 0, 20, 20, 21, 21, 22, 24, 25, 25, 26, 31, 32, 32, 33, 33, 34, 34, 35, 63, 42, 2, 64, 2, 106, 0, 178, 2, 106, 0, 200, 2, 214, 2, 244, 2, 106, 0, 5, 0, 7, 21, 22, 22, 23, 53, 54, 54, 55, 63, 94, 0, 234, 0, 94, 0, 248, 0, 94, 0, 28, 0, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 13, 14, 14, 15, 16, 17, 17, 18, 20, 21, 21, 22, 22, 23, 23, 24, 27, 28, 28, 29, 29, 30, 30, 31, 31, 32, 36, 37, 37, 38, 38, 39, 39, 40, 50, 51, 51, 52, 54, 55, 55, 56, 56, 57, 63, 94, 0, 26, 3, 94, 0, 248, 0, 40, 3, 94, 0, 2, 3, 94, 0, 248, 0, 94, 0, 2, 3, 94, 0, 2, 3, 94, 0, 2, 3, 94, 0, 6, 1, 248, 0, 94, 0, 234, 0, 94, 0, 2, 3, 94, 0, 2, 3, 94, 0, 2, 3, 234, 0, 94, 0, 5, 0, 7, 42, 43, 43, 44, 46, 47, 47, 48, 63, 94, 0, 234, 0, 94, 0, 2, 3, 94, 0, 3, 0, 7, 33, 34, 34, 35, 63, 94, 0, 16, 3, 94, 0, 7, 0, 7, 11, 12, 12, 13, 16, 17, 17, 18, 38, 39, 39, 40, 63, 94, 0, 6, 1, 94, 0, 248, 0, 94, 0, 2, 3, 94, 0, 3, 0, 7, 48, 49, 49, 50, 63, 94, 0, 248, 0, 94, 0, 3, 0, 7, 19, 20, 28, 29, 63, 104, 3, 136, 0, 104, 3, 2, 0, 7, 18, 19, 63, 104, 3, 136, 0, 3, 0, 7, 39, 40, 49, 50, 63, 104, 3, 136, 0, 104, 3, 2, 0, 7, 55, 56, 63, 104, 3, 136, 0, 5, 0, 7, 19, 20, 28, 29, 49, 50, 59, 60, 63, 104, 3, 136, 0, 104, 3, 136, 0, 104, 3, 3, 0, 7, 12, 13, 19, 20, 63, 104, 3, 136, 0, 104, 3, 4, 0, 7, 15, 16, 19, 20, 28, 29, 63, 136, 0, 104, 3, 136, 0, 104, 3, 11, 0, 0, 2, 3, 3, 4, 6, 65, 70, 71, 71, 72, 75, 76, 76, 77, 78, 79, 79, 80, 80, 81, 81, 104, 3, 4, 0, 104, 3, 94, 0, 100, 0, 106, 0, 112, 0, 106, 0, 118, 0, 124, 0, 130, 0];
        static ref REVERSE_BYTES: Vec<u8> = vec![114, 117, 115, 116, 45, 114, 101, 103, 101, 120, 45, 97, 117, 116, 111, 109, 97, 116, 97, 45, 115, 112, 97, 114, 115, 101, 45, 100, 102, 97, 0, 255, 254, 1, 0, 2, 0, 2, 0, 138, 2, 0, 0, 0, 0, 0, 0, 34, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15, 16, 17, 18, 18, 19, 19, 20, 21, 22, 23, 24, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 63, 63, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 65, 66, 67, 68, 68, 68, 69, 70, 71, 72, 72, 72, 72, 72, 72, 72, 72, 73, 74, 74, 74, 74, 75, 76, 77, 77, 77, 77, 77, 77, 77, 77, 77, 77, 77, 77, 77, 77, 77, 0, 0, 0, 0, 13, 0, 1, 1, 7, 12, 13, 15, 16, 18, 19, 19, 20, 28, 29, 34, 35, 39, 40, 43, 44, 49, 50, 55, 56, 59, 60, 63, 58, 0, 116, 0, 146, 0, 180, 0, 186, 0, 196, 0, 14, 1, 20, 1, 54, 1, 128, 1, 178, 1, 216, 1, 254, 1, 14, 0, 1, 1, 3, 3, 7, 12, 13, 15, 16, 18, 19, 19, 20, 28, 29, 34, 35, 39, 40, 43, 44, 49, 50, 55, 56, 59, 60, 63, 58, 0, 2, 0, 116, 0, 146, 0, 180, 0, 186, 0, 196, 0, 14, 1, 20, 1, 54, 1, 128, 1, 178, 1, 216, 1, 254, 1, 7, 0, 8, 8, 12, 12, 30, 30, 38, 38, 44, 44, 51, 51, 69, 69, 42, 2, 132, 2, 8, 2, 36, 2, 42, 2, 42, 2, 58, 0, 8, 0, 8, 8, 12, 12, 30, 30, 38, 38, 39, 39, 44, 44, 51, 51, 69, 69, 42, 2, 132, 2, 8, 2, 36, 2, 42, 2, 42, 2, 42, 2, 58, 0, 1, 0, 39, 39, 42, 2, 2, 0, 34, 34, 39, 39, 14, 2, 42, 2, 18, 0, 9, 9, 14, 14, 21, 21, 23, 23, 28, 28, 34, 34, 35, 35, 37, 37, 39, 39, 41, 41, 43, 43, 44, 44, 47, 47, 51, 51, 55, 55, 59, 59, 61, 61, 62, 62, 42, 2, 8, 2, 8, 2, 8, 2, 8, 2, 14, 2, 42, 2, 36, 2, 26, 2, 96, 2, 36, 2, 42, 2, 106, 2, 116, 2, 8, 2, 64, 2, 64, 2, 126, 2, 1, 0, 34, 34, 14, 2, 8, 0, 22, 22, 27, 27, 34, 34, 37, 37, 43, 43, 56, 56, 62, 62, 65, 65, 58, 2, 36, 2, 70, 2, 8, 2, 90, 2, 8, 2, 64, 2, 58, 0, 18, 0, 8, 8, 22, 22, 27, 27, 34, 34, 37, 37, 39, 39, 41, 41, 43, 43, 45, 45, 47, 47, 49, 49, 51, 51, 53, 53, 55, 55, 56, 56, 57, 57, 62, 62, 65, 65, 8, 2, 58, 2, 36, 2, 70, 2, 8, 2, 64, 2, 64, 2, 80, 2, 64, 2, 64, 2, 64, 2, 64, 2, 64, 2, 64, 2, 8, 2, 64, 2, 64, 2, 58, 0, 12, 0, 8, 8, 34, 34, 39, 39, 41, 41, 43, 43, 45, 45, 47, 47, 49, 49, 51, 51, 53, 53, 55, 55, 57, 57, 8, 2, 14, 2, 64, 2, 64, 2, 64, 2, 64, 2, 64, 2, 64, 2, 64, 2, 64, 2, 64, 2, 64, 2, 9, 0, 10, 10, 17, 17, 31, 31, 34, 34, 41, 41, 48, 48, 49, 49, 54, 54, 67, 67, 8, 2, 26, 2, 8, 2, 14, 2, 36, 2, 42, 2, 48, 2, 58, 2, 58, 0, 9, 0, 10, 11, 17, 17, 31, 31, 34, 34, 41, 41, 48, 48, 49, 49, 54, 54, 67, 67, 8, 2, 26, 2, 8, 2, 14, 2, 36, 2, 42, 2, 48, 2, 58, 2, 58, 0, 2, 0, 11, 11, 34, 34, 8, 2, 14, 2, 1, 0, 21, 21, 20, 2, 1, 0, 32, 32, 20, 2, 1, 0, 76, 76, 58, 0, 2, 0, 21, 21, 33, 33, 20, 2, 20, 2, 1, 0, 73, 73, 58, 0, 1, 0, 71, 71, 58, 0, 2, 0, 34, 34, 73, 73, 20, 2, 58, 0, 1, 0, 20, 20, 20, 2, 1, 0, 70, 70, 58, 0, 2, 0, 32, 32, 71, 71, 20, 2, 58, 0, 2, 0, 25, 25, 70, 70, 20, 2, 58, 0, 1, 0, 25, 25, 20, 2, 2, 0, 71, 71, 73, 73, 58, 0, 58, 0, 2, 0, 25, 25, 71, 71, 20, 2, 58, 0, 2, 0, 21, 21, 71, 71, 20, 2, 58, 0, 1, 0, 75, 75, 58, 0, 1, 0, 33, 33, 20, 2, 1, 0, 5, 5, 4, 0];

        static ref RE: Regex<SparseDFA<&'static [u8], u16>> = {
            let forward: SparseDFA<&'static [u8], u16> = unsafe {
                SparseDFA::from_bytes(&FORWARD_BYTES)
            };
            let reverse: SparseDFA<&'static [u8], u16> = unsafe {
                SparseDFA::from_bytes(&REVERSE_BYTES)
            };
            Regex::from_dfas(forward, reverse)
        };
    }
    RE.find_iter(item).collect()
}
