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

//! # Equilibrium Aggregates Pallet
//!
//! Equilibrium's Pallet to aggregate currency data for different groups of users

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

mod mock;
mod tests;

use eq_primitives::{
    asset::Asset, balance::BalanceGetter, Aggregates, AggregatesAssetRemover, SignedBalance,
    TotalAggregates, UserGroup,
};
#[allow(unused_imports)]
use frame_support::debug; // This usage is required by a macro
use frame_support::{
    codec::Codec, pallet_prelude::DispatchResult, traits::OnKilledAccount, Parameter,
};
use sp_runtime::{
    traits::{
        AtLeast32BitUnsigned, CheckedAdd, CheckedSub, MaybeSerializeDeserialize, Member, Zero,
    },
    ArithmeticError, DispatchError,
};
use sp_std::fmt::Debug;
use sp_std::iter::Iterator;
use sp_std::prelude::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use eq_primitives::asset::Asset;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Numerical representation of stored balances
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Codec
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            // + From<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>;

        /// Type containing balance checks
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;
    }
    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // Pallet storage - stores user groups
    #[pallet::storage]
    #[pallet::getter(fn account_user_groups)]
    pub type AccountUserGroups<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        UserGroup,
        Blake2_128Concat,
        T::AccountId,
        bool,
        ValueQuery,
    >;

    /// Pallet storage - stores aggregates for each user group
    #[pallet::storage]
    #[pallet::getter(fn total_user_groups)]
    pub type TotalUserGroups<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        UserGroup,
        Blake2_128Concat,
        Asset,
        TotalAggregates<T::Balance>,
        ValueQuery,
    >;
}

impl<T: Config> Pallet<T> {
    fn in_usergroup(account_id: &T::AccountId, user_group: UserGroup) -> bool {
        let acc = <AccountUserGroups<T>>::get(&user_group, &account_id);
        acc
    }

    /// insert `account_id` in `user_group` if `is_in` == `true`,
    /// otherwise, remove
    fn set_usergroup(
        account_id: &T::AccountId,
        user_group: UserGroup,
        is_in: bool,
    ) -> DispatchResult {
        if is_in == <AccountUserGroups<T>>::get(user_group, account_id) {
            return Ok(());
        }
        if is_in {
            <AccountUserGroups<T>>::insert(user_group, account_id, true);
            for (asset, signed_balance) in T::BalanceGetter::iterate_account_balances(account_id) {
                Self::update_group_total(
                    asset,
                    &SignedBalance::zero(),
                    &signed_balance,
                    user_group,
                )?;
            }
        } else {
            <AccountUserGroups<T>>::remove(user_group, account_id);
            for (asset, signed_balance) in T::BalanceGetter::iterate_account_balances(account_id) {
                Self::update_group_total(
                    asset,
                    &signed_balance,
                    &signed_balance.negate(),
                    user_group,
                )?;
            }
        }

        Ok(())
    }

    fn update_group_total(
        asset: Asset,
        prev_balance: &SignedBalance<T::Balance>,
        delta_balance: &SignedBalance<T::Balance>,
        user_group: UserGroup,
    ) -> DispatchResult {
        let result = TotalUserGroups::<T>::mutate(user_group, asset, |total| -> Option<()> {
            match (delta_balance, prev_balance) {
                (SignedBalance::Positive(delta), SignedBalance::Negative(prev)) => {
                    let aggregates_change = prev.min(delta);

                    total.collateral = total
                        .collateral
                        .checked_add(delta)?
                        .checked_sub(aggregates_change)?;
                    total.debt = total.debt.checked_sub(aggregates_change)?;
                }
                (SignedBalance::Positive(delta), SignedBalance::Positive(_prev)) => {
                    total.collateral = total.collateral.checked_add(delta)?;
                }
                (SignedBalance::Negative(delta), SignedBalance::Negative(_prev)) => {
                    total.debt = total.debt.checked_add(delta)?;
                }
                (SignedBalance::Negative(delta), SignedBalance::Positive(prev)) => {
                    let aggregates_change = prev.min(delta);
                    total.collateral = total.collateral.checked_sub(aggregates_change)?;
                    total.debt = total
                        .debt
                        .checked_add(delta)?
                        .checked_sub(aggregates_change)?;
                }
            };

            Some(())
        });

        result.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))
    }

    fn update_total(
        account_id: &T::AccountId,
        asset: Asset,
        prev_balance: &SignedBalance<T::Balance>,
        delta_balance: &SignedBalance<T::Balance>,
    ) -> DispatchResult {
        for group in UserGroup::iterator() {
            if <AccountUserGroups<T>>::get(&group, &account_id) {
                Self::update_group_total(asset, prev_balance, delta_balance, group)?;
            }
        }
        Ok(())
    }

    fn iter_account(user_group: UserGroup) -> Box<dyn Iterator<Item = T::AccountId>> {
        Box::new(<AccountUserGroups<T>>::iter_prefix(user_group).map(|(k, _v)| k))
    }

    fn iter_total(
        user_group: UserGroup,
    ) -> Box<dyn Iterator<Item = (Asset, TotalAggregates<T::Balance>)>> {
        Box::new(<TotalUserGroups<T>>::iter_prefix(user_group))
    }

    fn get_total(user_group: UserGroup, asset: Asset) -> TotalAggregates<T::Balance> {
        <TotalUserGroups<T>>::get(user_group, asset)
    }
}

impl<T: Config> Aggregates<T::AccountId, T::Balance> for Pallet<T> {
    fn in_usergroup(account_id: &T::AccountId, user_group: UserGroup) -> bool {
        Self::in_usergroup(account_id, user_group)
    }
    fn set_usergroup(
        account_id: &T::AccountId,
        user_group: UserGroup,
        is_in: bool,
    ) -> DispatchResult {
        Self::set_usergroup(account_id, user_group, is_in)
    }
    fn update_total(
        account_id: &T::AccountId,
        asset: Asset,
        prev_balance: &SignedBalance<T::Balance>,
        delta_balance: &SignedBalance<T::Balance>,
    ) -> DispatchResult {
        Self::update_total(account_id, asset, prev_balance, delta_balance)
    }
    fn iter_account(user_group: UserGroup) -> Box<dyn Iterator<Item = T::AccountId>> {
        Self::iter_account(user_group)
    }
    fn iter_total(
        user_group: UserGroup,
    ) -> Box<dyn Iterator<Item = (Asset, TotalAggregates<T::Balance>)>> {
        Self::iter_total(user_group)
    }
    fn get_total(user_group: UserGroup, asset: Asset) -> TotalAggregates<T::Balance> {
        Self::get_total(user_group, asset)
    }
}

impl<T: Config> AggregatesAssetRemover for Pallet<T> {
    fn remove_asset(asset: &Asset) {
        <TotalUserGroups<T>>::remove(UserGroup::Bailsmen, asset);
        <TotalUserGroups<T>>::remove(UserGroup::Borrowers, asset);
        <TotalUserGroups<T>>::remove(UserGroup::Balances, asset);
    }
}

impl<T: Config> OnKilledAccount<T::AccountId> for Pallet<T> {
    fn on_killed_account(who: &T::AccountId) {
        for group in UserGroup::iterator() {
            if Self::in_usergroup(&who, group) {
                // Already done in eq_balances::delete_account but left for safety.
                // Do nothing if account already removed from user group.
                let _ = Self::set_usergroup(&who, group, false);
            }
        }
    }
}
