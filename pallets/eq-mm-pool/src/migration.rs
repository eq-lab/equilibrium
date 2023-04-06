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
use eq_primitives::{Aggregates, UserGroup};

pub mod current {
    use eq_primitives::asset::Asset;

    pub type PoolsStorage<T> = sp_std::vec::Vec<(
        Asset,
        crate::MmPoolInfo<<T as frame_system::Config>::AccountId, <T as crate::Config>::Balance>,
    )>;

    pub type DepositsStorage<T> =
        sp_std::vec::Vec<(Asset, crate::LenderInfo<<T as crate::Config>::Balance>)>;
}

pub mod commit_d2730d3b2d7a2a22c97ba35e7995566a8f1bd115 {
    use codec::{Decode, Encode};
    use eq_primitives::asset::Asset;
    #[cfg(feature = "std")]
    use serde::{Deserialize, Serialize};

    #[derive(Encode, Decode, Clone, PartialEq, Eq, Debug)]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub struct MmPoolInfo<T: crate::Config> {
        // Account where pallet stores tokens
        pub account_id: T::AccountId,
        // Min deposit balance
        pub min_amount: T::Balance,
        // Pool Currency
        pub currency: Asset,
        // Initial total balance, active (not used by mm) balance is balance of AccountId
        pub initial_balance: T::Balance,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Debug, Eq)]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub enum LockPeriod {
        None,
        ThreeMonth,
        SixMonth,
        Year,
    }

    pub type PoolsStorage<T> = sp_std::vec::Vec<MmPoolInfo<T>>;

    pub type DepositsStorage<T> = (
        LockPeriod,
        sp_std::vec::Vec<(Asset, <T as crate::Config>::Balance)>,
    );
}

pub mod previous {
    pub use super::commit_d2730d3b2d7a2a22c97ba35e7995566a8f1bd115::*;
}

pub fn pools_translate<T: Config>(
    pallet_subacc: &T::AccountId,
    old_value: Option<previous::PoolsStorage<T>>,
) -> Option<current::PoolsStorage<T>> {
    Some(
        old_value
            .unwrap_or_default()
            .into_iter()
            .map(|old| {
                let borrowed = T::EqCurrency::total_balance(pallet_subacc, old.currency);

                let new = MmPoolInfo {
                    account_id: old.account_id,
                    min_amount: old.min_amount,
                    total_staked: old.initial_balance,
                    total_deposit: old.initial_balance - borrowed,
                    total_borrowed: borrowed,
                    total_pending_withdrawals: PendingWithdrawal::default::<T>(),
                };
                (old.currency, new)
            })
            .collect(),
    )
}

pub fn deposits_translate<T: Config>(
    old_value: previous::DepositsStorage<T>,
) -> Option<current::DepositsStorage<T>> {
    Some(
        old_value
            .1
            .into_iter()
            .map(|(asset, deposit)| {
                let new = LenderInfo {
                    deposit,
                    pending_withdrawals: PendingWithdrawal::default::<T>(),
                };
                (asset, new)
            })
            .collect(),
    )
}

pub fn migrate_pallet_subaccount_funds<T: Config>(
    pallet_subacc: &T::AccountId,
    pool_accounts: Vec<(Asset, T::AccountId)>,
) -> Option<()> {
    // let pallet_subacc =
    //     T::SubaccountsManager::get_subaccount_id(&pallet_acc, &SubAccType::Borrower)?;

    for (currency, pool_acc) in pool_accounts {
        let pool_subacc = Pallet::<T>::get_subacc_creating(&pool_acc).ok()?;
        let amount = T::EqCurrency::total_balance(pallet_subacc, currency);

        T::EqCurrency::currency_transfer(
            pallet_subacc,
            &pool_subacc,
            currency,
            amount,
            ExistenceRequirement::AllowDeath,
            TransferReason::Common,
            false,
        )
        .ok()?;
    }

    Some(())
}

pub fn set_borrower_user_group<T: Config>() {
    for (_, (_, trader_acc)) in Managers::<T>::iter() {
        if let Some(borrower_acc) =
            T::SubaccountsManager::get_subaccount_id(&trader_acc, &SubAccType::Trader)
        {
            let _ = T::Aggregates::set_usergroup(&borrower_acc, UserGroup::Borrowers, true);
        }
    }
}
