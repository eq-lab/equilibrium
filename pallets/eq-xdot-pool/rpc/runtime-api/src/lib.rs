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

//! Runtime API definition for `eq-xdot-pool` pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Codec;
use sp_runtime::traits::MaybeDisplay;

sp_api::decl_runtime_apis! {
    pub trait EqXdotPoolApi<Balance>
    where
        Balance: Codec + MaybeDisplay
    {
        fn invariant(pool_id: u32) -> Option<u128>;

        fn fy_token_out_for_base_in(pool_id: u32, base_amount: Balance) -> Option<Balance>;

        fn base_out_for_fy_token_in(pool_id: u32, fy_token_amount: Balance) -> Option<Balance>;

        fn fy_token_in_for_base_out(pool_id: u32, base_amount: Balance) -> Option<Balance>;

        fn base_in_for_fy_token_out(pool_id: u32, fy_token_amount: Balance) -> Option<Balance>;

        fn base_out_for_lp_in(pool_id: u32, lp_in: Balance) -> Option<Balance>;

        fn base_and_fy_out_for_lp_in(pool_id: u32, lp_in: Balance) -> Option<(Balance, Balance)>;

        fn max_base_xbase_in_and_out(pool_id: u32) ->Option<(Balance, Balance, Balance, Balance)>;
    }
}
