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

use crate::vec_map::{SortedVec, VecMap};
use crate::{asset::Asset, balance_number::EqFixedU128, signed_balance::EqMember, SignedBalance};
use codec::{Decode, Encode};
use sp_runtime::DispatchError;

pub type DistributionId = u32;

#[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
pub struct Distribution<Balance: EqMember> {
    /// Bailsmen total usd
    pub total_usd: Balance,
    /// Bailsmen amount to reinit
    pub remaining_bailsmen: u32,
    /// Balances to distribute
    pub distribution_balances: SortedVec<(Asset, SignedBalance<Balance>)>,
    /// Prices
    pub prices: SortedVec<(Asset, EqFixedU128)>,
}

impl<Balance: EqMember> Distribution<Balance> {
    pub fn get_asset_price(&self, asset: &Asset) -> Option<EqFixedU128> {
        let price_index = self.prices.binary_search_by(|(a, _)| a.cmp(&asset)).ok()?;
        let (_, price) = self.prices[price_index];
        Some(price)
    }
}

pub struct AccountDistribution<Balance: EqMember> {
    /// Transfers for account received from distribution queue
    pub transfers: VecMap<Asset, SignedBalance<Balance>>,
    /// Distribution id for account before redistribution
    pub last_distribution_id: DistributionId,
    /// Current top distribution id
    pub current_distribution_id: DistributionId,
    /// Changed queue after account distribution
    pub new_queue: VecMap<DistributionId, Distribution<Balance>>,
}

/// Exportable bailsman pallet implementation for integration in other pallets
pub trait BailsmanManager<AccountId, Balance>
where
    Balance: EqMember,
{
    /// Inner implementation of bailsman registration
    fn register_bailsman(who: &AccountId) -> Result<(), sp_runtime::DispatchError>;

    /// Inner implementation of bailsman unregister
    fn unregister_bailsman(who: &AccountId) -> Result<(), sp_runtime::DispatchError>;

    /// Receives account `who` balances in case of margin call. Account balances
    /// are sent to Bailsman Pallet balance
    fn receive_position(who: &AccountId, is_deleting_position: bool) -> Result<(), DispatchError>;

    /// Apply all distributions for bailsman account `who`
    fn redistribute(who: &AccountId) -> Result<u32, sp_runtime::DispatchError>;

    /// Get account distribution from
    fn get_account_distribution(
        who: &AccountId,
    ) -> Result<AccountDistribution<Balance>, sp_runtime::DispatchError>;

    /// Checks if bailsman should be unregistered after decreasing the `currency` balance by `amount`.
    /// Don't checks bailsman debt.
    /// Returns boolean that mean should bails be unregistered or not
    fn should_unreg_bailsman(
        who: &AccountId,
        amounts: &[(Asset, SignedBalance<Balance>)],
        debt_and_discounted_collateral: Option<(Balance, Balance)>,
    ) -> Result<bool, sp_runtime::DispatchError>;

    /// Total count of bailsmen
    fn bailsmen_count() -> u32;

    /// Length of distribution queue
    fn distribution_queue_len() -> u32;
}
