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

//! # Equilibrium Crowdloan DOTs Pallet

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(warnings)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use eq_primitives::asset::{Asset, CDOT613, CDOT714, CDOT815, DOT, XDOT, XDOT2, XDOT3};
use eq_primitives::balance::{BalanceGetter, DepositReason, EqCurrency, WithdrawReason};
use eq_primitives::subaccount::{SubAccType, SubaccountsManager};
use eq_primitives::{str_asset, IsTransfersEnabled, LendingPoolManager, SignedBalance};
use eq_utils::eq_ensure;
use frame_support::pallet_prelude::DispatchResult;
use frame_support::traits::{ExistenceRequirement, WithdrawReasons};
use frame_support::transactional;
use sp_runtime::traits::{AtLeast32BitUnsigned, Zero};
use sp_std::convert::{TryFrom, TryInto};
use sp_std::fmt::Debug;
use sp_std::vec::Vec;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Numerical representation of stored balances
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + TryFrom<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>;
        /// Origin for enable and disable transfers
        type ToggleTransferOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Used for balance operations
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        /// Gets users balances
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;
        /// Used for managing subaccounts
        type SubaccountsManager: SubaccountsManager<Self::AccountId>;
        /// Checks if transaction disabled flag is off
        type IsTransfersEnabled: eq_primitives::IsTransfersEnabled;
        /// To swap crowdloan DOTs in the lending pool
        type LendingPoolManager: LendingPoolManager<Self::Balance, Self::AccountId>;
    }

    /// Stores Crowdloan DOTs allowed to swap
    #[pallet::storage]
    pub type AllowedCrowdloanDotsSwap<T: Config> =
        StorageValue<_, Vec<CrowdloanDotAsset>, ValueQuery>;

    #[pallet::error]
    pub enum Error<T> {
        /// Transfers are disabled
        TransfersAreDisabled,
        /// Crowdloan DOT swap is not allowed
        CrowdloanDotSwapNotAllowed,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn allow_crowdloan_swap(
            origin: OriginFor<T>,
            assets: Vec<CrowdloanDotAsset>,
        ) -> DispatchResultWithPostInfo {
            T::ToggleTransferOrigin::ensure_origin(origin)?;

            AllowedCrowdloanDotsSwap::<T>::put(assets);

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 2))]
        #[transactional]
        pub fn swap_crowdloan_dots(
            origin: OriginFor<T>,
            mb_who: Option<T::AccountId>,
            assets: Vec<CrowdloanDotAsset>,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_transfers_enabled(&XDOT, T::Balance::default())?;

            let caller = ensure_signed(origin)?;
            let who = mb_who.unwrap_or(caller);

            Self::do_swap_crowdloan_dot(&who, &assets)?;

            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    fn ensure_transfers_enabled(asset: &Asset, amount: T::Balance) -> DispatchResult {
        let is_enabled = T::IsTransfersEnabled::get();

        eq_ensure!(
            is_enabled,
            Error::<T>::TransfersAreDisabled,
            target: "eq_balances",
            "{}:{}. Transfers is not allowed. amount: {:?}, asset: {:?}.",
            file!(),
            line!(),
            amount,
            str_asset!(asset)
        );

        Ok(())
    }

    fn ensure_crowdloan_dot_swap_allowed(assets: &Vec<CrowdloanDotAsset>) -> DispatchResult {
        let allowed = AllowedCrowdloanDotsSwap::<T>::get();

        eq_ensure!(
            assets.iter().all(|a| allowed.contains(a)),
            Error::<T>::CrowdloanDotSwapNotAllowed,
            target: "eq_crowdloan_dots",
            "{}:{}. Swap is not allowed for the specified asset.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn do_swap_crowdloan_dot(
        who: &T::AccountId,
        swap_assets: &Vec<CrowdloanDotAsset>,
    ) -> DispatchResult {
        Self::ensure_crowdloan_dot_swap_allowed(&swap_assets)?;

        let mut accounts_balances: Vec<_> = SubAccType::iterator()
            .filter_map(|t| T::SubaccountsManager::get_subaccount_id(who, &t))
            .map(|a| (a.clone(), T::BalanceGetter::iterate_account_balances(&a)))
            .collect();

        accounts_balances.push((
            who.clone(),
            T::BalanceGetter::iterate_account_balances(&who),
        ));

        for swap_asset in swap_assets {
            let asset = match swap_asset {
                CrowdloanDotAsset::XDOT => XDOT,
                CrowdloanDotAsset::XDOT2 => XDOT2,
                CrowdloanDotAsset::XDOT3 => XDOT3,
                CrowdloanDotAsset::CDOT613 => CDOT613,
                CrowdloanDotAsset::CDOT714 => CDOT714,
                CrowdloanDotAsset::CDOT815 => CDOT815,
            };

            let (main_account, _) = accounts_balances.last().unwrap();

            let lending_amount = T::LendingPoolManager::remove_deposit(&main_account, &asset)?;
            if !lending_amount.is_zero() {
                T::LendingPoolManager::add_deposit(&main_account, &DOT, &lending_amount)?;
            }

            for (account, balances) in &accounts_balances {
                let signed_balance = balances.get(&asset);

                match signed_balance {
                    Some(SignedBalance::Positive(balance)) => {
                        T::EqCurrency::withdraw(
                            account,
                            asset,
                            *balance,
                            false,
                            Some(WithdrawReason::CrowdloanDotSwap),
                            WithdrawReasons::empty(),
                            ExistenceRequirement::KeepAlive,
                        )?;

                        T::EqCurrency::deposit_creating(
                            account,
                            DOT,
                            *balance,
                            false,
                            Some(DepositReason::CrowdloanDotSwap),
                        )?;
                    }
                    Some(SignedBalance::Negative(balance)) => {
                        T::EqCurrency::deposit_creating(
                            account,
                            asset,
                            *balance,
                            false,
                            Some(DepositReason::CrowdloanDotSwap),
                        )?;

                        T::EqCurrency::withdraw(
                            account,
                            DOT,
                            *balance,
                            false,
                            Some(WithdrawReason::CrowdloanDotSwap),
                            WithdrawReasons::empty(),
                            ExistenceRequirement::KeepAlive,
                        )?;
                    }
                    None => {}
                };
            }
        }

        Ok(())
    }
}

#[derive(Decode, Encode, Clone, Debug, PartialEq, scale_info::TypeInfo)]
pub enum CrowdloanDotAsset {
    XDOT,
    XDOT2,
    XDOT3,
    CDOT613,
    CDOT714,
    CDOT815,
}
