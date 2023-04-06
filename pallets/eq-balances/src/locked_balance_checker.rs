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

use frame_support::traits::WithdrawReasons;

use crate::*;

pub struct CheckLocked<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> BalanceChecker<T::Balance, T::AccountId, Pallet<T>, T::SubaccountsManager>
    for CheckLocked<T>
{
    fn need_to_check_impl(
        _who: &T::AccountId,
        _changes: &Vec<(Asset, SignedBalance<T::Balance>)>,
    ) -> bool {
        true
    }

    fn can_change_balance_impl(
        who: &T::AccountId,
        changes: &Vec<(Asset, SignedBalance<T::Balance>)>,
        withdraw_reasons: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        let native_asset = T::AssetGetter::get_main_asset();
        for (asset, change) in changes.into_iter() {
            if asset == &native_asset {
                if withdraw_reasons
                    .unwrap_or(WithdrawReasons::empty())
                    .intersects(
                        WithdrawReasons::TRANSACTION_PAYMENT
                            | WithdrawReasons::FEE
                            | WithdrawReasons::TIP,
                    )
                {
                    return Ok(());
                } else if T::SubaccountsManager::is_master(who) {
                    let balance = Pallet::<T>::get_balance(who, asset);
                    let locked = Pallet::<T>::get_locked(who);
                    if !locked.is_zero() {
                        if matches!(change, SignedBalance::Negative(_))
                            && balance + *change < SignedBalance::Positive(locked)
                        {
                            frame_support::fail!(Error::<T>::Locked);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
