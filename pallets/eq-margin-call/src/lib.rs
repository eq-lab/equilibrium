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

//! # Equilibrium Margin Call pallet.
//!
//! Equilibrium Margin Call pallet is a Substrate module that provides
//! the functionality necessary to process margin calls which may
//! happen in the network due to various reasons.
//!
//! Main module methods are `check_margin` which diagnoses an account margin state and
//! `try_margincall` which is called from an offchain worker of eq-rate (reinit method).
//!
//! The margin state of an account is supposed to have 5 grades:
//! * Good, anything goes,
//! * SubGood, the account is forbidden to borrow
//! * MaintenanceTimerGoing, the margin call per se, a timer is activated that gives 24 h to top up the account to a necessary limit
//! * SubCritical and MaintenanceTimerOver which result in a liquidation of the account

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

use sp_std::prelude::*;
use sp_std::*;

use frame_support::Parameter;
use frame_system as system;
use frame_system::offchain::SendTransactionTypes;

use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    traits::{Get, UnixTime},
};

use core::convert::TryInto;
use eq_primitives::{
    asset::*,
    balance::BalanceGetter,
    balance_number::EqFixedU128,
    price::PriceGetter,
    subaccount::{SubAccType, SubaccountsManager},
    BailsmanManager, BalanceChange, MarginCallManager, MarginState, OrderAggregateBySide,
    OrderAggregates, OrderChange, OrderSide, SignedBalance, ONE_TOKEN,
};
use eq_utils::vec_map::VecMap;
use eq_utils::{
    fixed::{balance_from_eq_fixedu128, eq_fixedu128_from_balance, eq_fixedu128_from_fixedi64},
    multiply_by_rational,
};
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, Bounded, CheckedAdd, MaybeSerializeDeserialize, Member, Zero},
    ArithmeticError, DispatchError, FixedPointNumber, Percent,
};

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod benchmarking;
pub mod weights;
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    //Config
    #[pallet::config]
    pub trait Config: frame_system::Config + SendTransactionTypes<Call<Self>> {
        /// Timestamp provider
        type UnixTime: UnixTime;
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Numerical representation of stored balances
        type Balance: Member
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Parameter
            + Default
            + From<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>
            + Copy
            + Clone;
        /// Gets information about account balances
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;
        /// Receives currency price data from oracle
        type PriceGetter: eq_primitives::price::PriceGetter;
        /// Used to integrate bailsman operations
        type BailsmenManager: BailsmanManager<Self::AccountId, Self::Balance>;
        /// `initial_margin` setting, when the margin is below this value, borrowing is prohibited
        #[pallet::constant]
        type InitialMargin: Get<EqFixedU128>;
        /// `maintenance_margin` setting, when the margin is below this value, a MaintenanceMarginCall event is fired
        #[pallet::constant]
        type MaintenanceMargin: Get<EqFixedU128>;
        /// `critical_margin` setting, when the margin is below this value, a position is liquidated immediately
        #[pallet::constant]
        type CriticalMargin: Get<EqFixedU128>;
        /// `maintenance_period` setting, a time period (in seconds) when the margin account can be topped up to the `initial_margin` setting to avoid a margin call
        #[pallet::constant]
        type MaintenancePeriod: Get<u64>;
        /// Provides aggregates for the margin calculation
        type OrderAggregates: OrderAggregates<Self::AccountId>;
        /// Provides asset_data for the margin calculation
        type AssetGetter: AssetGetter;
        /// Provides subaccount info for MarginCall events
        type SubaccountsManager: SubaccountsManager<Self::AccountId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /* -------------- STORAGE -------------- */
    #[pallet::storage]
    #[pallet::getter(fn maintenance_timers)]
    pub type MaintenanceTimers<T: Config> =
        StorageMap<_, Identity, T::AccountId, Option<u64>, ValueQuery>;

    /* ------------ EVENTS --------------- */
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Event is fired when an account achieves the `maintenance_margin` level.
        /// \[main_acc, maybe(subacc_type,subacc_id), timer\]
        MaintenanceMarginCall(T::AccountId, Option<(SubAccType, T::AccountId)>, u64),
        /// Event is fired when an account is liquidated.
        /// \[main_acc, maybe(subacc_type,subacc_id)\]
        MarginCallExecuted(T::AccountId, Option<(SubAccType, T::AccountId)>),
    }

    /*------------ HOOKS ------------------*/
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Not allowed with zero collateral
        ZeroCollateral,
    }

    /* ------------------ GENESIS ------------------------- */
    #[pallet::genesis_config]
    pub struct GenesisConfig {}
    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {}
        }
    }
    #[cfg(feature = "std")]
    impl GenesisConfig {
        /// Direct implementation of `GenesisBuild::build_storage`.
        /// Kept in order not to break dependency.
        pub fn build_storage<T: Config>(&self) -> Result<sp_runtime::Storage, String> {
            <Self as GenesisBuild<T>>::build_storage(self)
        }

        /// Direct implementation of `GenesisBuild::assimilate_storage`.
        /// Kept in order not to break dependency.
        pub fn assimilate_storage<T: Config>(
            &self,
            storage: &mut sp_runtime::Storage,
        ) -> Result<(), String> {
            <Self as GenesisBuild<T>>::assimilate_storage(self, storage)
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |_config: &GenesisConfig| {};
            extra_genesis_builder(self);
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Tries to margin-call an account from another account signed call.
        #[pallet::weight(T::WeightInfo::try_margincall_external())]
        pub fn try_margincall_external(
            origin: OriginFor<T>,
            who: <T as system::Config>::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;
            log::trace!(target: "eq_margin_call", "Try margin call on account '{:?}' external", who);
            let _ = Self::try_margincall(&who)?;
            Ok(().into())
        }
    }
}

//------------------- MarginCallManager --------------------------------------
impl<T: Config> MarginCallManager<T::AccountId, T::Balance> for Pallet<T> {
    /// Diagnoses the current margin state of an account with changes presumed and returns a `MarginState`
    fn check_margin_with_change(
        who: &T::AccountId,
        balance_changes: &[BalanceChange<T::Balance>],
        order_changes: &[OrderChange],
    ) -> Result<(MarginState, bool), DispatchError> {
        let (margin, is_margin_increased) =
            Self::calculate_portfolio_margin(who, balance_changes, order_changes)?;

        #[cfg(feature = "std")]
        println!("margin = {margin:?}");

        let initial_margin = T::InitialMargin::get();
        let maintenance_margin = T::MaintenanceMargin::get();
        let critical_margin = T::CriticalMargin::get();
        let maintenance_period = T::MaintenancePeriod::get();

        let state = if margin < critical_margin {
            // we're below x < critical_margin (5%), this is a MC
            let no_orders = T::OrderAggregates::get_asset_weights(&who).is_empty();
            if no_orders {
                MarginState::SubCritical
            } else {
                MarginState::MaintenanceIsGoing
            }
        } else if let Some(start) = <MaintenanceTimers<T>>::get(who) {
            // independently extract the timer and make it impact an output state
            if margin < initial_margin {
                let now = T::UnixTime::now().as_secs();
                let no_orders = T::OrderAggregates::get_asset_weights(&who).is_empty();
                if no_orders && now.saturating_sub(start) > maintenance_period {
                    MarginState::MaintenanceTimeOver // this is a MC
                } else {
                    MarginState::MaintenanceIsGoing // else we still have time
                }
            } else {
                MarginState::MaintenanceEnd
            }
        } else if margin < maintenance_margin {
            // critical_margin (5%) <= x < maintenance_margin (10%), we create a maintenance timer
            MarginState::MaintenanceStart
        } else if margin < initial_margin {
            // maintenance_margin (10%) <= x < initial_margin (20%)
            MarginState::SubGood
        } else {
            // x > initial_margin (20%)
            MarginState::Good
        };

        Ok((state, is_margin_increased))
    }

    /// Tries to margin-call an account and returns its margin check result as `MarginState`
    fn try_margincall(who: &T::AccountId) -> Result<MarginState, DispatchError> {
        let mut state = Self::check_margin(who)?;

        match state {
            //Good and SubGood states now never undergo MC
            MarginState::Good | MarginState::SubGood | MarginState::MaintenanceIsGoing => {}

            // 1. Position is good now, delete a maintenance timer if exists
            MarginState::MaintenanceEnd => {
                <MaintenanceTimers<T>>::remove(who);
            }

            //2. Check maintenance_margin condition, i.e. margin < maintenance_margin (10%), start a timer.
            //   If we are in a MaintenanceIsGoing state, leave as is
            MarginState::MaintenanceStart => {
                let now = T::UnixTime::now().as_secs();
                <MaintenanceTimers<T>>::insert(who, Some(now));
                if let Some((owner, subacc_type)) = T::SubaccountsManager::get_owner_id(&who) {
                    // Subaccount
                    Self::deposit_event(Event::<T>::MaintenanceMarginCall(
                        owner,
                        Some((subacc_type, who.clone())),
                        now,
                    ));
                } else {
                    // Main account
                    Self::deposit_event(Event::<T>::MaintenanceMarginCall(who.clone(), None, now));
                }
                state = MarginState::MaintenanceIsGoing;
            }

            //3. check if a timer is over or the margin is subcritical -> do the MC.
            MarginState::MaintenanceTimeOver | MarginState::SubCritical => {
                T::BailsmenManager::receive_position(who, false)?;
                <MaintenanceTimers<T>>::remove(who);
                if let Some((owner, subacc_type)) = T::SubaccountsManager::get_owner_id(&who) {
                    // Subaccount
                    Self::deposit_event(Event::<T>::MarginCallExecuted(
                        owner,
                        Some((subacc_type, who.clone())),
                    ));
                } else {
                    // Main account
                    Self::deposit_event(Event::<T>::MarginCallExecuted(who.clone(), None));
                }
                // don't care about error here
                // MarginState calc getting balances and prices
            }
        }

        Ok(state)
    }

    fn get_critical_margin() -> EqFixedU128 {
        T::CriticalMargin::get()
    }
}

/* ----------------- IMPL PALLET ------------------ */
impl<T: Config> Pallet<T> {
    /// Calculates sell and buy margin and returns min of them
    fn calculate_portfolio_margin_for_balances(
        owner: &T::AccountId,
        balances: &VecMap<Asset, SignedBalance<T::Balance>>,
        order_changes: &[OrderChange],
    ) -> Result<EqFixedU128, DispatchError> {
        let mut order_aggregates = T::OrderAggregates::get_asset_weights(&owner);

        //add order changes to order aggregates
        for change in order_changes {
            let price =
                eq_fixedu128_from_fixedi64(change.price).ok_or(ArithmeticError::Overflow)?;

            order_aggregates
                .entry(change.asset)
                .or_insert(Default::default())
                .add(change.amount, price, change.side)
                .ok_or(ArithmeticError::Overflow)?
        }

        // make set of assets from balances, order aggregates
        let mut assets: Vec<Asset> = balances
            .iter()
            .map(|(a, _)| *a)
            .chain(order_aggregates.iter().map(|(a, _)| *a))
            .collect();
        assets.sort();
        assets.dedup();

        let zero = SignedBalance::zero();

        let mut buy_collateral = zero;
        let mut buy_collateral_eqd = zero;
        let mut buy_debt = zero;

        let mut sell_collateral = zero;
        let mut sell_collateral_eqd = zero;
        let mut sell_debt = zero;

        // Closure accumulate collateral, collateral_eqd and debt parts for next margin calculation
        let calc_margin_parts = |side: OrderSide,
                                 maybe_ord_aggr: Option<&OrderAggregateBySide>,
                                 balance: &SignedBalance<T::Balance>,
                                 price: EqFixedU128,
                                 discount: Percent,
                                 collateral: &mut SignedBalance<T::Balance>,
                                 collateral_eqd: &mut SignedBalance<T::Balance>,
                                 debt: &mut SignedBalance<T::Balance>|
         -> Option<()> {
            let ord_aggr = maybe_ord_aggr.map(|o| o.get_by_side(side));

            let balance_with_aggr_amount = if let Some(ord_aggr) = ord_aggr {
                let amount_by_price_aggr = balance_from_eq_fixedu128(ord_aggr.amount_by_price)?;
                let amount_aggr = balance_from_eq_fixedu128(ord_aggr.amount)?;

                let amount_part: SignedBalance<T::Balance>;
                if side == OrderSide::Sell {
                    *collateral_eqd = collateral_eqd.add_balance(&amount_by_price_aggr)?;
                    amount_part = balance.sub_balance(&amount_aggr)?;
                } else {
                    *collateral_eqd = collateral_eqd.sub_balance(&amount_by_price_aggr)?;
                    amount_part = balance.add_balance(&amount_aggr)?;
                };
                amount_part
            } else {
                *balance
            };
            match balance_with_aggr_amount {
                SignedBalance::Positive(collateral_part) => {
                    let cp_by_price = price.checked_mul_int(collateral_part.into())?;
                    let discounted = discount.mul_floor(cp_by_price);
                    *collateral = collateral.add_balance(&discounted.into())?;
                }
                SignedBalance::Negative(debt_part) => {
                    *debt = debt.sub_balance(&price.checked_mul_int(debt_part.into())?.into())?;
                }
            };

            Some(())
        };

        let calc_margin = |collateral: &mut SignedBalance<T::Balance>,
                           collateral_eqd: SignedBalance<T::Balance>,
                           debt: &mut SignedBalance<T::Balance>|
         -> Result<EqFixedU128, DispatchError> {
            match collateral_eqd {
                SignedBalance::Positive(eqd_balance) => {
                    *collateral = collateral
                        .add_balance(&eqd_balance)
                        .ok_or(ArithmeticError::Overflow)?
                }
                SignedBalance::Negative(eqd_balance) => {
                    *debt = debt
                        .sub_balance(&eqd_balance)
                        .ok_or(ArithmeticError::Overflow)?
                }
            };

            if debt.is_zero() {
                //set margin to max value when zero debt
                return Ok(EqFixedU128::max_value());
            }

            frame_support::ensure!(!collateral.is_zero(), Error::<T>::ZeroCollateral);

            let margin = match *collateral + *debt {
                SignedBalance::Positive(net) => {
                    multiply_by_rational(net, ONE_TOKEN, collateral.abs())
                        .map(eq_fixedu128_from_balance)
                        .ok_or(ArithmeticError::Overflow)?
                }
                SignedBalance::Negative(_) => EqFixedU128::zero(),
            };

            Ok(margin)
        };

        for asset in assets {
            let asset_data = T::AssetGetter::get_asset_data(&asset)?;

            let price = T::PriceGetter::get_price(&asset)?;
            let discount = asset_data.collateral_discount;

            let maybe_order_aggregate = order_aggregates.get(&asset);

            let balance = balances.get(&asset).unwrap_or(&zero);

            if asset == EQD {
                buy_collateral_eqd = buy_collateral_eqd
                    .checked_add(&balance)
                    .ok_or(ArithmeticError::Overflow)?;

                sell_collateral_eqd = sell_collateral_eqd
                    .checked_add(&balance)
                    .ok_or(ArithmeticError::Overflow)?;

                continue;
            }

            calc_margin_parts(
                OrderSide::Sell,
                maybe_order_aggregate,
                balance,
                price,
                discount,
                &mut sell_collateral,
                &mut sell_collateral_eqd,
                &mut sell_debt,
            )
            .ok_or(ArithmeticError::Overflow)?;

            calc_margin_parts(
                OrderSide::Buy,
                maybe_order_aggregate,
                balance,
                price,
                discount,
                &mut buy_collateral,
                &mut buy_collateral_eqd,
                &mut buy_debt,
            )
            .ok_or(ArithmeticError::Overflow)?;
        }

        let sell_margin = calc_margin(&mut sell_collateral, sell_collateral_eqd, &mut sell_debt)?;
        let buy_margin = calc_margin(&mut buy_collateral, buy_collateral_eqd, &mut buy_debt)?;

        let margin = cmp::min(sell_margin, buy_margin);
        Ok(margin)
    }

    /// Calculates the current portfolio margin (simply `margin` herebefore), a real value (inherited from the LTV calculation).
    /// returns current margin + fact that margin was increased
    pub(crate) fn calculate_portfolio_margin(
        who: &T::AccountId,
        balance_changes: &[BalanceChange<T::Balance>],
        order_changes: &[OrderChange],
    ) -> Result<(EqFixedU128, bool), DispatchError> {
        let mut balances = T::BalanceGetter::iterate_account_balances(who);

        // calculate previous margin
        let margin_before = Self::calculate_portfolio_margin_for_balances(who, &balances, &[])?;

        // modify balances with changes
        for balance_change in balance_changes {
            balances
                .entry(balance_change.asset)
                .and_modify(|balance| *balance = balance.clone() + balance_change.change.clone())
                .or_insert(balance_change.change.clone());
        }

        let margin_after =
            Self::calculate_portfolio_margin_for_balances(who, &balances, order_changes)?;
        Ok((margin_after, margin_after > margin_before))
    }
}
