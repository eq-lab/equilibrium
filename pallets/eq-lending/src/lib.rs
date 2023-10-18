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

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

use codec::{Codec, Decode, Encode, MaxEncodedLen};
use eq_primitives::{
    asset,
    asset::{Asset, AssetGetter, AssetType},
    balance::{BalanceChecker, BalanceGetter, DepositReason, EqCurrency, WithdrawReason},
    balance_number::EqFixedU128,
    subaccount::SubaccountsManager,
    Aggregates, BailsmanManager, PriceGetter, SignedBalance, UserGroup,
};
#[allow(unused_imports)]
use frame_support::debug;
use frame_support::{
    ensure,
    traits::{ExistenceRequirement, Get, UnixTime, WithdrawReasons},
    PalletId,
};
use sp_arithmetic::{traits::CheckedSub, ArithmeticError};
use sp_runtime::{
    traits::{AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, Zero},
    DispatchError, DispatchResult, FixedPointNumber, FixedPointOperand,
};
use sp_std::{convert::TryInto, vec::Vec};

pub use pallet::*;

pub mod benchmarking;
#[cfg(test)]
mod mock;
pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo, MaxEncodedLen)]
pub struct LenderData<Balance> {
    /// deposit for current lender
    pub value: Balance,
    /// last reward for current lender
    pub last_reward: EqFixedU128,
}

impl<Balance: Default> LenderData<Balance> {
    fn default_per_asset<T: Config>(asset: Asset) -> Self {
        Self {
            value: Balance::default(),
            last_reward: <CumulatedReward<T>>::get(asset),
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Numerical representation of stored balances
        type Balance: Member
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Codec
            + Copy
            + Parameter
            + Default
            // + From<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>
            + MaxEncodedLen
            + FixedPointOperand;
        /// Gets users balances to calculate fees and check margin calls
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;
        /// Used to work with `TotalAggregates` storing aggregated collateral and
        /// debt for user groups
        type Aggregates: Aggregates<Self::AccountId, Self::Balance>;
        /// Used to deal with Assets
        type AssetGetter: AssetGetter;
        /// To calculate total usd colateral of pool
        type PriceGetter: PriceGetter;
        /// Interface for working with subaccounts
        type SubaccountsManager: SubaccountsManager<Self::AccountId>;
        /// Lending pool ModuleId
        #[pallet::constant]
        type ModuleId: Get<PalletId>;
        /// For deposits, withdrawal and payouts
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        /// Bailsman pallet integration for operations with bailsman subaccount
        type BailsmanManager: BailsmanManager<Self::AccountId, Self::Balance>;
        /// Timestamp provider
        type UnixTime: UnixTime;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Timestamp of switching from bailsman pool to lending pool
    #[pallet::storage]
    #[pallet::getter(fn only_bailsman_till)]
    pub type OnlyBailsmanTill<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Lenders deposits
    #[pallet::storage]
    #[pallet::getter(fn lender)]
    pub type Lenders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        Asset,
        LenderData<T::Balance>,
        OptionQuery,
    >;

    /// Total lending amount per asset
    #[pallet::storage]
    #[pallet::getter(fn aggregates)]
    pub type LendersAggregates<T: Config> =
        StorageMap<_, Blake2_128Concat, Asset, T::Balance, ValueQuery>;

    /// Table with accumulated rewards per asset
    /// cumulated_reward[i+i] > cumulated_reward[i] is guaranteed
    #[pallet::storage]
    #[pallet::getter(fn rewards)]
    pub type CumulatedReward<T: Config> =
        StorageMap<_, Blake2_128Concat, Asset, EqFixedU128, ValueQuery>;

    #[pallet::error]
    pub enum Error<T> {
        /// Only physical asset types allowed to deposit/withdraw in lending pool
        WrongAssetType,
        /// Not allowed because of debt weight
        DebtExceedLiquidity,
        /// Overflow
        Overflow,
        /// User do not deposit to lending pool
        NotALender,
        /// Try to withdraw more than deposited
        NotEnoughToWithdraw,
        /// Try to add reward to pool without lenders
        NoLendersToClaim,
        /// Bailsman can not be unregistered because of debt weight
        BailsmanCantBeUnregistered,
        /// Bailsman can't generate debt
        BailsmanCantGenerateDebt,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Deposit {
            who: T::AccountId,
            asset: Asset,
            value: T::Balance,
        },
        Withdraw {
            who: T::AccountId,
            asset: Asset,
            value: T::Balance,
        },
        Payout {
            who: T::AccountId,
            asset: Asset,
            payout: T::Balance,
        },
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::deposit())]
        pub fn deposit(
            origin: OriginFor<T>,
            asset: Asset,
            value: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_deposit(&who, asset, value)?;
            Self::deposit_event(Event::<T>::Deposit { who, asset, value });
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::withdraw())]
        pub fn withdraw(
            origin: OriginFor<T>,
            asset: Asset,
            value: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_withdraw(&who, asset, value)?;
            Self::deposit_event(Event::<T>::Withdraw { who, asset, value });
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::payout())]
        pub fn payout(
            origin: OriginFor<T>,
            asset: Asset,
            who: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_signed(origin)?;
            let mut lender = <Lenders<T>>::get(&who, asset).ok_or(Error::<T>::NotALender)?;
            Self::try_payout(&who, &mut lender, asset)?;
            <Lenders<T>>::insert(&who, asset, lender);
            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub lender_balances: Vec<(T::AccountId, Vec<(T::Balance, Asset)>)>,
        pub only_bailsmen_till: Option<u64>, //seconds
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                lender_balances: Default::default(),
                only_bailsmen_till: None,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};
            EqPalletAccountInitializer::<T>::initialize(
                &T::ModuleId::get().into_account_truncating(),
            );

            if let Some(only_bailsmen_till) = self.only_bailsmen_till {
                OnlyBailsmanTill::<T>::put(only_bailsmen_till);
            }

            for (account_id, balances) in self.lender_balances.iter() {
                for (value, asset) in balances {
                    match Pallet::<T>::do_deposit(account_id, *asset, *value) {
                        Err(e) => panic!("eqLending genesis build failed with error {:?}.", e),
                        _ => {}
                    };
                }
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    fn do_deposit(who: &T::AccountId, asset: Asset, value: T::Balance) -> DispatchResult {
        let asset_data = T::AssetGetter::get_asset_data(&asset)?;
        ensure!(
            asset_data.asset_type == AssetType::Physical,
            Error::<T>::WrongAssetType
        );

        let mut lender = <Lenders<T>>::get(who, asset)
            .unwrap_or_else(|| LenderData::default_per_asset::<T>(asset));
        Self::try_payout(who, &mut lender, asset)?;

        lender.value = lender
            .value
            .checked_add(&value)
            .ok_or(Error::<T>::Overflow)?;
        let lenders_aggregate = <LendersAggregates<T>>::get(asset)
            .checked_add(&value)
            .ok_or(Error::<T>::Overflow)?;

        T::EqCurrency::currency_transfer(
            who,
            &T::ModuleId::get().into_account_truncating(),
            asset,
            value,
            frame_support::traits::ExistenceRequirement::KeepAlive,
            eq_primitives::TransferReason::Common,
            true,
        )?;

        <Lenders<T>>::insert(who, asset, lender);
        <LendersAggregates<T>>::insert(asset, lenders_aggregate);

        Ok(())
    }

    fn do_withdraw(who: &T::AccountId, asset: Asset, value: T::Balance) -> DispatchResult {
        let mut lender = <Lenders<T>>::get(who, asset).ok_or(Error::<T>::NotALender)?;
        Self::try_payout(who, &mut lender, asset)?;

        ensure!(lender.value >= value, Error::<T>::NotEnoughToWithdraw);
        lender.value = lender.value - value;
        let lenders_aggregate = <LendersAggregates<T>>::get(asset) - value;

        T::EqCurrency::currency_transfer(
            &T::ModuleId::get().into_account_truncating(),
            who,
            asset,
            value,
            frame_support::traits::ExistenceRequirement::KeepAlive,
            eq_primitives::TransferReason::Common,
            true,
        )?;

        if lender.value.is_zero() {
            <Lenders<T>>::remove(who, asset);
        } else {
            <Lenders<T>>::insert(who, asset, lender);
        }
        <LendersAggregates<T>>::insert(asset, lenders_aggregate);

        Ok(())
    }

    fn do_remove_deposit(who: &T::AccountId, asset: &Asset) -> Result<T::Balance, DispatchError> {
        let lender = Lenders::<T>::get(who, asset);

        if let Some(mut lender) = lender {
            Self::try_payout(who, &mut lender, *asset)?;

            let lenders_aggregate = LendersAggregates::<T>::get(asset)
                .checked_sub(&lender.value)
                .ok_or(ArithmeticError::Underflow)?;

            Lenders::<T>::remove(who, asset);
            LendersAggregates::<T>::insert(asset, lenders_aggregate);

            T::EqCurrency::withdraw(
                &T::ModuleId::get().into_account_truncating(),
                *asset,
                lender.value,
                false,
                Some(WithdrawReason::AssetRemoval),
                WithdrawReasons::empty(),
                ExistenceRequirement::KeepAlive,
            )?;

            Ok(lender.value)
        } else {
            Ok(T::Balance::zero())
        }
    }

    fn do_add_deposit(who: &T::AccountId, asset: &Asset, amount: &T::Balance) -> DispatchResult {
        let asset_data = T::AssetGetter::get_asset_data(asset)?;

        ensure!(
            asset_data.asset_type == AssetType::Physical,
            Error::<T>::WrongAssetType
        );

        let mut lender = Lenders::<T>::get(who, asset)
            .unwrap_or_else(|| LenderData::default_per_asset::<T>(*asset));

        Self::try_payout(who, &mut lender, *asset)?;

        lender.value = lender
            .value
            .checked_add(amount)
            .ok_or(ArithmeticError::Overflow)?;

        let lenders_aggregate = LendersAggregates::<T>::get(asset)
            .checked_add(amount)
            .ok_or(Error::<T>::Overflow)?;

        T::EqCurrency::deposit_creating(
            &T::ModuleId::get().into_account_truncating(),
            *asset,
            *amount,
            false,
            Some(DepositReason::AssetRemoval),
        )?;

        Lenders::<T>::insert(who, asset, lender);
        LendersAggregates::<T>::insert(asset, lenders_aggregate);

        Ok(())
    }

    fn do_add_reward(asset: Asset, reward: T::Balance) -> DispatchResult {
        if reward.is_zero() {
            return Ok(());
        }

        let total_lendable = <LendersAggregates<T>>::get(asset);
        ensure!(
            total_lendable > T::Balance::zero(),
            Error::<T>::NoLendersToClaim
        );

        let diff_reward = EqFixedU128::checked_from_rational(reward, total_lendable)
            .ok_or(Error::<T>::Overflow)?;

        <CumulatedReward<T>>::try_mutate(asset, |cumulated| -> DispatchResult {
            *cumulated = cumulated
                .checked_add(&diff_reward)
                .ok_or(Error::<T>::Overflow)?;
            Ok(())
        })?;

        Ok(())
    }

    fn try_payout(
        who: &T::AccountId,
        lender: &mut LenderData<T::Balance>,
        asset: Asset,
    ) -> Result<(), DispatchError> {
        if let Some(payout) = Self::calc_payout(lender, asset) {
            T::EqCurrency::currency_transfer(
                &T::ModuleId::get().into_account_truncating(),
                who,
                T::AssetGetter::get_main_asset(),
                payout,
                frame_support::traits::ExistenceRequirement::KeepAlive,
                eq_primitives::TransferReason::Common,
                true,
            )?;

            Self::deposit_event(Event::<T>::Payout {
                who: who.clone(),
                asset,
                payout,
            });
        }

        Ok(())
    }

    /// Calculate proper amount of reward that could be obtained by lender
    /// Each lender
    fn calc_payout(lender: &mut LenderData<T::Balance>, asset: Asset) -> Option<T::Balance> {
        let curr_reward = <CumulatedReward<T>>::get(asset);
        let payout = (curr_reward - lender.last_reward).saturating_mul_int(lender.value);

        lender.last_reward = curr_reward;

        (!payout.is_zero()).then(|| payout)
    }

    pub fn get_total_collat(asset: Asset) -> T::Balance {
        T::Aggregates::get_total(UserGroup::Balances, asset).collateral
    }

    pub fn get_total_debt(asset: Asset) -> T::Balance {
        T::Aggregates::get_total(UserGroup::Balances, asset).debt
            - T::Aggregates::get_total(UserGroup::Bailsmen, asset).debt
    }

    /// Returns total lendable (lenders_lendable, bails_lendable)
    /// lender_part == 0 in bailsman_only_period
    pub fn get_lendable_parts(asset: Asset) -> (T::Balance, T::Balance) {
        let bails_lendable = T::Aggregates::get_total(UserGroup::Bailsmen, asset).collateral;
        if Self::is_only_bailsmen_period() {
            (T::Balance::zero(), bails_lendable)
        } else {
            (<LendersAggregates<T>>::get(asset), bails_lendable)
        }
    }

    fn is_only_bailsmen_period() -> bool {
        T::UnixTime::now().as_secs() < OnlyBailsmanTill::<T>::get()
    }

    fn check_bails_pool_after_unreg(who: &T::AccountId) -> DispatchResult {
        T::BalanceGetter::iterate_account_balances(who)
            .into_iter()
            // exclude EQD from check
            .filter(|(asset, _)| *asset != asset::EQD)
            .try_for_each(|(asset, balance)| {
                let total_debt = Self::get_total_debt(asset);
                let (lenders_lendable, bails_lendable) = Self::get_lendable_parts(asset);

                let acc_collat = match balance {
                    SignedBalance::Positive(amount) => amount,
                    SignedBalance::Negative(_) => T::Balance::zero(),
                };

                let asset_data = T::AssetGetter::get_asset_data(&asset)?;
                let total_liquidity = asset_data
                    .debt_weight
                    .mul_floor(bails_lendable - acc_collat)
                    + asset_data.lending_debt_weight.mul_floor(lenders_lendable);
                ensure!(
                    total_debt <= total_liquidity,
                    Error::<T>::BailsmanCantBeUnregistered
                );

                Ok(())
            })
    }
}

impl<T: Config> BalanceChecker<T::Balance, T::AccountId, T::BalanceGetter, T::SubaccountsManager>
    for Pallet<T>
{
    fn need_to_check_impl(
        who: &T::AccountId,
        _changes: &Vec<(Asset, SignedBalance<T::Balance>)>,
    ) -> bool {
        let is_lender = who == &T::ModuleId::get().into_account_truncating();
        let is_bailsman = T::Aggregates::in_usergroup(who, UserGroup::Bailsmen);

        is_lender || is_bailsman
    }

    fn can_change_balance_impl(
        who: &T::AccountId,
        initial_changes: &Vec<(Asset, SignedBalance<T::Balance>)>,
        _withdraw_reasons: Option<WithdrawReasons>,
    ) -> DispatchResult {
        // All lenders are on lending pool account
        let is_lender = who == &T::ModuleId::get().into_account_truncating();
        let is_bailsman = T::Aggregates::in_usergroup(who, UserGroup::Bailsmen);

        // For bailsman check that
        // - if bailsmen should be unregistered check that after subtracting his balances from bails pool
        //   total_bailsmen_balance (asset_id) < total_asset_debt (asset_id) for all assets
        // initial_change has negative balance: it's checked in top level can_change_balance for Tuple
        if is_bailsman {
            let should_unreg =
                T::BailsmanManager::should_unreg_bailsman(&who, &initial_changes, None).map_err(
                    |err| {
                        log::error!(
                            "{}:{}. Error during can_change_balance in eq_lending. Couldn't \
                                make checks for unreg bailsman. Bailsman {:?}",
                            file!(),
                            line!(),
                            who
                        );
                        err
                    },
                )?;
            if should_unreg {
                Self::check_bails_pool_after_unreg(who)?;
            }
        }

        for (asset, change) in initial_changes.iter() {
            let asset_data = T::AssetGetter::get_asset_data(&asset)?;

            match (change, asset_data.asset_type) {
                // we allow to generate debt in Synthetic asset with no weight limit
                (_, AssetType::Synthetic) if !is_bailsman => {}
                (_, AssetType::Native) if is_lender => {}
                // if debt increases check that total_debt + debt_change <= max_asset_debt for asset
                (SignedBalance::Negative(value), asset_type) => {
                    let collat_dec = match T::BalanceGetter::get_balance(who, &asset) {
                        SignedBalance::Positive(prev) => prev.min(*value),
                        SignedBalance::Negative(_) => T::Balance::zero(),
                    };

                    let debt_inc = *value - collat_dec;

                    if debt_inc.is_zero() && !is_bailsman && !is_lender {
                        continue;
                    }

                    // Bails debt can be created only from margin calls. Total debt doesn't increase in this case.
                    if is_bailsman {
                        ensure!(debt_inc.is_zero(), Error::<T>::BailsmanCantGenerateDebt);

                        if asset_type == AssetType::Synthetic {
                            continue;
                        }
                    }

                    let total_collat = Self::get_total_collat(*asset);
                    let total_debt = Self::get_total_debt(*asset);
                    let new_total_debt = total_debt + debt_inc;
                    let (mut lenders_lendable, mut bails_lendable) =
                        Self::get_lendable_parts(*asset);

                    let max_debt_liquidity = if Self::is_only_bailsmen_period() {
                        if is_bailsman {
                            bails_lendable -= collat_dec;
                        }

                        asset_data.debt_weight.mul_floor(bails_lendable)
                    } else {
                        if is_lender {
                            lenders_lendable -= collat_dec;
                        }

                        asset_data.lending_debt_weight * lenders_lendable
                    };

                    ensure!(
                        new_total_debt <= max_debt_liquidity
                            && total_debt + new_total_debt <= total_collat,
                        Error::<T>::DebtExceedLiquidity
                    );
                }
                _ => {}
            }
        }

        Ok(())
    }
}

impl<T: Config> eq_primitives::LendingPoolManager<T::Balance, T::AccountId> for Pallet<T> {
    fn add_reward(asset: Asset, reward: T::Balance) -> DispatchResult {
        Self::do_add_reward(asset, reward)
    }

    fn add_deposit(account: &T::AccountId, asset: &Asset, amount: &T::Balance) -> DispatchResult {
        Self::do_add_deposit(account, asset, amount)
    }

    fn remove_deposit(account: &T::AccountId, asset: &Asset) -> Result<T::Balance, DispatchError> {
        Self::do_remove_deposit(account, asset)
    }
}

impl<T: Config> eq_primitives::LendingAssetRemoval<T::AccountId> for Pallet<T> {
    fn remove_from_aggregates_and_rewards(asset: &Asset) {
        LendersAggregates::<T>::remove(asset);
        CumulatedReward::<T>::remove(asset);
    }

    fn remove_from_lenders(asset: &Asset, account: &T::AccountId) {
        Lenders::<T>::remove(account, asset);
    }
}
