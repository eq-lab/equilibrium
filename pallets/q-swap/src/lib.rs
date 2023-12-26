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

//! # Equilibrium Q Swap

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(warnings)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

use codec::{Decode, Encode};
use core::ops::{Div, Sub};
use eq_primitives::asset::{Asset, EQ, Q};
use eq_primitives::balance::{BalanceGetter, EqCurrency};
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::vestings::EqVestingSchedule;
use eq_primitives::{SignedBalance, Vesting};
use eq_utils::{balance_from_eq_fixedu128, eq_ensure, eq_fixedu128_from_balance};
use frame_support::pallet_prelude::DispatchResult;
use frame_support::traits::{ExistenceRequirement, Get, IsSubType};
use frame_support::transactional;
use scale_info::TypeInfo;
use sp_runtime::traits::{
    AtLeast32BitUnsigned, CheckedAdd, CheckedDiv, CheckedMul, DispatchInfoOf, SignedExtension, Zero,
};
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction,
};
use sp_runtime::{ArithmeticError, FixedPointOperand, Percent};
use sp_std::convert::{TryFrom, TryInto};
use sp_std::fmt::Debug;
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use eq_primitives::Vesting;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Numerical representation of stored balances
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + FixedPointOperand
            + TryFrom<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>;
        /// Origin for setting configuration
        type SetQSwapConfigurationOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        // Used for managing vestings #1
        type Vesting1: Vesting<Self::AccountId>
            + EqVestingSchedule<Self::Balance, Self::AccountId, Moment = Self::BlockNumber>;
        // Used for managing vestings #2
        type Vesting2: Vesting<Self::AccountId>
            + EqVestingSchedule<Self::Balance, Self::AccountId, Moment = Self::BlockNumber>;
        // Used for managing vestings #3
        type Vesting3: Vesting<Self::AccountId>
            + EqVestingSchedule<Self::Balance, Self::AccountId, Moment = Self::BlockNumber>;
        /// Used for managing balances and currencies
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>
            + BalanceGetter<Self::AccountId, Self::Balance>;
        /// Returns vesting #1 account
        type Vesting1AccountId: Get<Self::AccountId>;
        /// Returns vesting #2 account
        type Vesting2AccountId: Get<Self::AccountId>;
        /// Returns vesting #3 account
        type Vesting3AccountId: Get<Self::AccountId>;
        /// Returns Q holder account
        type QHolderAccountId: Get<Self::AccountId>;
        /// Returns Asset holder account
        type AssetHolderAccountId: Get<Self::AccountId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// Stores Q swap configuration
    #[pallet::storage]
    pub type QSwapConfigurations<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        Asset,
        SwapConfiguration<T::Balance, T::BlockNumber>,
        ValueQuery,
    >;

    /// Max amount of Q to receive by each user.
    #[pallet::storage]
    pub type QReceivingThreshold<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

    /// Stores Q amount transferred to users
    #[pallet::storage]
    pub type QReceivedAmounts<T: Config> =
        StorageMap<_, Identity, T::AccountId, T::Balance, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Transfer event. Included values are:
        /// - from `AccountId`
        /// - requested amount (asset #1)
        /// - requested amount (asset #2)
        /// - Q received amount
        /// - Q vested amount #1
        /// - Q vested amount #2
        /// \[from, amount_1, amount_2, amount_3, amount_4, amount_5 \]
        QSwap(
            T::AccountId,
            T::Balance,
            T::Balance,
            T::Balance,
            T::Balance,
            T::Balance,
        ),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Swaps are disabled
        SwapsAreDisabled,
        /// Configuration is invalid
        InvalidConfiguration,
        /// Available balance is not enough to perform swap
        NotEnoughBalance,
        /// Specified amount is too small to perform swap
        AmountTooSmall,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 4))]
        pub fn set_config(
            origin: OriginFor<T>,
            mb_max_q_amount: Option<T::Balance>,
            mb_q_swap_configurations: Option<
                Vec<(Asset, SwapConfigurationInput<T::Balance, T::BlockNumber>)>,
            >,
        ) -> DispatchResultWithPostInfo {
            T::SetQSwapConfigurationOrigin::ensure_origin(origin)?;

            Self::do_set_config(mb_max_q_amount, mb_q_swap_configurations)?;

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight((T::WeightInfo::swap(), DispatchClass::Normal, Pays::No))]
        #[transactional]
        pub fn swap(
            origin: OriginFor<T>,
            asset: Asset,
            amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let caller = ensure_signed(origin)?;
            let configuration = QSwapConfigurations::<T>::get(asset);
            let max_q_amount = QReceivingThreshold::<T>::get();

            Self::ensure_valid_amount(&configuration, &amount)?;
            Self::ensure_swap_enabled(&configuration)?;

            Self::do_swap(&caller, &asset, &amount, &max_q_amount, &configuration)?;

            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    fn ensure_swap_enabled(
        configuration: &SwapConfiguration<T::Balance, T::BlockNumber>,
    ) -> DispatchResult {
        eq_ensure!(
            configuration.enabled,
            Error::<T>::SwapsAreDisabled,
            target: "q_swap",
            "{}:{}. Q swap is not allowed.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn ensure_valid_amount(
        configuration: &SwapConfiguration<T::Balance, T::BlockNumber>,
        amount: &T::Balance,
    ) -> DispatchResult {
        eq_ensure!(
            amount.ge(&configuration.min_amount),
            Error::<T>::AmountTooSmall,
            target: "q_swap",
            "{}:{}. Specified amount is too small to perform swap.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn ensure_enough_balance(
        balance: &SignedBalance<T::Balance>,
        amount: &T::Balance,
    ) -> DispatchResult {
        let remaining_balance = balance
            .sub_balance(amount)
            .ok_or(ArithmeticError::Underflow)?;

        eq_ensure!(
            remaining_balance.is_positive(),
            Error::<T>::NotEnoughBalance,
            target: "q_swap",
            "{}:{}. Available balance is not enough to perform swap.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn do_set_config(
        mb_max_q_amount: Option<T::Balance>,
        mb_q_swap_configurations: Option<
            Vec<(Asset, SwapConfigurationInput<T::Balance, T::BlockNumber>)>,
        >,
    ) -> DispatchResult {
        let max_q_amount = mb_max_q_amount.unwrap_or(QReceivingThreshold::<T>::get());

        if let Some(q_swap_configurations) = mb_q_swap_configurations {
            for (asset, config) in q_swap_configurations {
                let mut configuration = QSwapConfigurations::<T>::get(asset);
                configuration.set(config);

                eq_ensure!(
                    configuration.is_valid() && !max_q_amount.is_zero(),
                    Error::<T>::InvalidConfiguration,
                    target: "q_swap",
                    "{}:{}. Invalid configuration provided.",
                    file!(),
                    line!()
                );

                QSwapConfigurations::<T>::insert(asset, configuration)
            }
        }

        if let Some(max_q_amount) = mb_max_q_amount {
            QReceivingThreshold::<T>::put(max_q_amount);
        }

        Ok(())
    }

    fn do_swap(
        who: &T::AccountId,
        asset: &Asset,
        amount: &T::Balance,
        max_q_amount: &T::Balance,
        configuration: &SwapConfiguration<T::Balance, T::BlockNumber>,
    ) -> DispatchResult {
        let (
            asset_1_amount,
            asset_2_amount,
            q_instant_swap,
            q_received_after,
            vesting_1_amount,
            vesting_2_amount,
        ) = if !configuration.secondary_asset_q_price.is_zero() {
            Self::get_multi_asset_swap_data(
                who,
                &asset.clone(),
                amount,
                max_q_amount,
                configuration,
            )?
        } else {
            Self::get_single_asset_swap_data(who, asset, amount, max_q_amount, configuration)?
        };

        let q_holder_account_id = T::QHolderAccountId::get();
        let asset_holder_account_id = T::AssetHolderAccountId::get();

        if !q_instant_swap.is_zero() {
            T::EqCurrency::currency_transfer(
                &q_holder_account_id,
                who,
                Q,
                q_instant_swap,
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::QSwap,
                true,
            )?;
        }

        if !asset_1_amount.is_zero() {
            T::EqCurrency::currency_transfer(
                who,
                &asset_holder_account_id,
                *asset,
                asset_1_amount,
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::QSwap,
                true,
            )?;
        }

        if !asset_2_amount.is_zero() {
            T::EqCurrency::currency_transfer(
                who,
                &asset_holder_account_id,
                configuration.secondary_asset,
                asset_2_amount,
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::QSwap,
                true,
            )?;
        }

        if !vesting_1_amount.is_zero() {
            Self::do_vest(
                configuration.main_vesting_number,
                who,
                &q_holder_account_id,
                vesting_1_amount,
                configuration.main_vesting_starting_block,
                configuration.main_vesting_duration_blocks,
            )?;
        }

        if !vesting_2_amount.is_zero() {
            Self::do_vest(
                configuration.secondary_vesting_number,
                who,
                &q_holder_account_id,
                vesting_2_amount,
                configuration.secondary_vesting_starting_block,
                configuration.secondary_vesting_duration_blocks,
            )?;
        }

        QReceivedAmounts::<T>::insert(who, q_received_after);

        Self::deposit_event(Event::QSwap(
            who.clone(),
            asset_1_amount,
            asset_2_amount,
            q_instant_swap,
            vesting_1_amount,
            vesting_2_amount,
        ));

        Ok(())
    }

    fn do_vest(
        vesting_number: u8,
        who: &T::AccountId,
        q_holder_account_id: &T::AccountId,
        amount: T::Balance,
        starting_block: T::BlockNumber,
        duration_blocks: T::Balance,
    ) -> DispatchResult {
        match vesting_number {
            1 => {
                Self::do_upsert_vesting::<T::Vesting1>(
                    who,
                    &q_holder_account_id,
                    &T::Vesting1AccountId::get(),
                    amount,
                    starting_block,
                    duration_blocks,
                )?;
            }
            2 => {
                Self::do_upsert_vesting::<T::Vesting2>(
                    who,
                    &q_holder_account_id,
                    &T::Vesting2AccountId::get(),
                    amount,
                    starting_block,
                    duration_blocks,
                )?;
            }
            3 => {
                Self::do_upsert_vesting::<T::Vesting3>(
                    who,
                    &q_holder_account_id,
                    &T::Vesting3AccountId::get(),
                    amount,
                    starting_block,
                    duration_blocks,
                )?;
            }
            _ => (),
        };

        Ok(())
    }

    fn do_upsert_vesting<
        TVesting: Vesting<T::AccountId>
            + EqVestingSchedule<T::Balance, T::AccountId, Moment = T::BlockNumber>,
    >(
        who: &T::AccountId,
        q_holder_account_id: &T::AccountId,
        q_vesting_account_id: &T::AccountId,
        amount: T::Balance,
        starting_block: T::BlockNumber,
        duration_blocks: T::Balance,
    ) -> DispatchResult {
        T::EqCurrency::currency_transfer(
            &q_holder_account_id,
            &q_vesting_account_id,
            Q,
            amount,
            ExistenceRequirement::AllowDeath,
            eq_primitives::TransferReason::QSwap,
            true,
        )?;

        if TVesting::has_vesting_schedule(who.clone()) {
            TVesting::update_vesting_schedule(who, amount, duration_blocks)?;
        } else {
            let per_block = duration_blocks
                .lt(&amount)
                .then(|| amount.div(duration_blocks))
                .unwrap_or(amount.div(amount));

            TVesting::add_vesting_schedule(who, amount, per_block, starting_block)?;
        }

        Ok(())
    }

    fn get_single_asset_swap_data(
        who: &T::AccountId,
        asset: &Asset,
        amount: &T::Balance,
        max_q_amount: &T::Balance,
        configuration: &SwapConfiguration<T::Balance, T::BlockNumber>,
    ) -> Result<
        (
            T::Balance,
            T::Balance,
            T::Balance,
            T::Balance,
            T::Balance,
            T::Balance,
        ),
        sp_runtime::DispatchError,
    > {
        let balance = T::EqCurrency::get_balance(who, asset);
        Self::ensure_enough_balance(&balance, amount)?;

        // Example #1: 1Q = 1700EQ (502.96 discounted EQ), instant_swap_share = 0.5
        //   swap(1005.92EQ)
        //     vesting #1:
        //       coeff = 502.96EQ / 1700EQ ~ 0.3
        //       q_total_amount = 1005.92EQ / 502.96EQ = 2Q
        //       q_instant_swap_amount = q_total_amount * coeff * instant_swap_share = 0.3Q
        //       q_vesting_amount = q_total_amount * coeff - q_instant_swap_amount = 0.3Q
        //     vesting #2:
        //       q_vesting_amount = q_total_amount - q_instant_swap_amount - q_vesting_amount = 1.4Q

        let main_asset_q_price_fixed = eq_fixedu128_from_balance(configuration.main_asset_q_price);
        let main_asset_q_discounted_price_fixed =
            eq_fixedu128_from_balance(configuration.main_asset_q_discounted_price);
        let amount_fixed = eq_fixedu128_from_balance(*amount);

        let vesting_1_coeff_fixed = main_asset_q_discounted_price_fixed
            .checked_div(&main_asset_q_price_fixed)
            .ok_or(ArithmeticError::DivisionByZero)?;

        let q_total_amount_fixed = amount_fixed
            .checked_div(&main_asset_q_discounted_price_fixed)
            .ok_or(ArithmeticError::DivisionByZero)?;

        let vesting_1_amount_fixed = q_total_amount_fixed
            .checked_mul(&vesting_1_coeff_fixed)
            .ok_or(ArithmeticError::Overflow)?;

        let vesting_1_amount =
            balance_from_eq_fixedu128(vesting_1_amount_fixed).ok_or(ArithmeticError::Overflow)?;
        let q_instant_swap = configuration.instant_swap_share.mul_floor(vesting_1_amount);

        let q_received = QReceivedAmounts::<T>::get(who);
        let q_received_after = q_received
            .checked_add(&q_instant_swap)
            .ok_or(ArithmeticError::Overflow)?;

        let (q_instant_swap, q_received_after) = if q_received_after.le(&max_q_amount) {
            (q_instant_swap, q_received_after)
        } else {
            let q_surplus = q_received_after.sub(*max_q_amount);
            let q_received_after = *max_q_amount;
            let q_instant_swap = q_instant_swap.sub(q_surplus);

            (q_instant_swap, q_received_after)
        };

        let vesting_1_amount = vesting_1_amount.sub(q_instant_swap);

        let q_total_amount: T::Balance =
            balance_from_eq_fixedu128(q_total_amount_fixed).ok_or(ArithmeticError::Overflow)?;
        let vesting_2_amount = q_total_amount.sub(vesting_1_amount).sub(q_instant_swap);

        Ok((
            *amount,
            T::Balance::zero(),
            q_instant_swap,
            q_received_after,
            vesting_1_amount,
            vesting_2_amount,
        ))
    }

    fn get_multi_asset_swap_data(
        who: &T::AccountId,
        asset: &Asset,
        amount: &T::Balance,
        max_q_amount: &T::Balance,
        configuration: &SwapConfiguration<T::Balance, T::BlockNumber>,
    ) -> Result<
        (
            T::Balance,
            T::Balance,
            T::Balance,
            T::Balance,
            T::Balance,
            T::Balance,
        ),
        sp_runtime::DispatchError,
    > {
        let balance = T::EqCurrency::get_balance(who, asset);
        Self::ensure_enough_balance(&balance, amount)?;

        // Example #2: 1Q = 1000EQ (295.86 discounted EQ) + 0.1DOT, instant_swap_share = 0.5
        //   swap(0.15DOT)
        //     vesting #1:
        //       one_q = 295.86EQ / 0.1DOT * 0.1DOT + 295.86EQ = 591.72EQ
        //       coeff = 295.86EQ / 1000EQ ~ 0.3
        //       eq_amount = (0.15DOT / 0.1DOT) * 295.86EQ = 443.79EQ
        //       dot_amount = 0.15DOT
        //       q_total_amount = (eq_amount + dot_amount * (295.86EQ / 0.1DOT)) / one_q = 1.5Q
        //       q_instant_swap_amount = q_total_amount * coeff * instant_swap_share = 0.225Q
        //       q_vesting_amount = q_total_amount * coeff - q_instant_swap = 0.225Q
        //     vesting #2:
        //       q_vesting_amount = q_total_amount - q_instant_swap_amount - q_vesting_amount = 1Q

        let main_asset_q_price_fixed = eq_fixedu128_from_balance(configuration.main_asset_q_price);
        let secondary_asset_q_price_fixed =
            eq_fixedu128_from_balance(configuration.secondary_asset_q_price);
        let secondary_asset_q_discounted_price_fixed =
            eq_fixedu128_from_balance(configuration.secondary_asset_q_discounted_price);
        let amount_fixed = eq_fixedu128_from_balance(*amount);

        let one_q_fixed = secondary_asset_q_discounted_price_fixed
            .checked_mul(&EqFixedU128::from(2u128))
            .ok_or(ArithmeticError::Overflow)?;

        let vesting_1_coeff_fixed = secondary_asset_q_discounted_price_fixed
            .checked_div(&secondary_asset_q_price_fixed)
            .ok_or(ArithmeticError::DivisionByZero)?;

        let eq_amount_fixed = amount_fixed
            .checked_div(&main_asset_q_price_fixed)
            .ok_or(ArithmeticError::DivisionByZero)?
            .checked_mul(&secondary_asset_q_discounted_price_fixed)
            .ok_or(ArithmeticError::Overflow)?;

        let eq_amount: T::Balance =
            balance_from_eq_fixedu128(eq_amount_fixed).ok_or(ArithmeticError::Overflow)?;

        let balance = T::EqCurrency::get_balance(who, &EQ);
        Self::ensure_enough_balance(&balance, &eq_amount)?;

        let eq_to_dot_amount_fixed = amount_fixed
            .checked_mul(&secondary_asset_q_discounted_price_fixed)
            .ok_or(ArithmeticError::Overflow)?
            .checked_div(&main_asset_q_price_fixed)
            .ok_or(ArithmeticError::DivisionByZero)?;

        let q_total_amount_fixed = (eq_amount_fixed
            .checked_add(&eq_to_dot_amount_fixed)
            .ok_or(ArithmeticError::Overflow)?)
        .checked_div(&one_q_fixed)
        .ok_or(ArithmeticError::DivisionByZero)?;

        let vesting_1_amount_fixed = q_total_amount_fixed
            .checked_mul(&vesting_1_coeff_fixed)
            .ok_or(ArithmeticError::Overflow)?;

        let vesting_1_amount =
            balance_from_eq_fixedu128(vesting_1_amount_fixed).ok_or(ArithmeticError::Overflow)?;
        let q_instant_swap = configuration.instant_swap_share.mul_floor(vesting_1_amount);

        let q_received = QReceivedAmounts::<T>::get(who);
        let q_received_after = q_received
            .checked_add(&q_instant_swap)
            .ok_or(ArithmeticError::Overflow)?;

        let (q_instant_swap, q_received_after) = if q_received_after.le(&max_q_amount) {
            (q_instant_swap, q_received_after)
        } else {
            let q_surplus = q_received_after.sub(*max_q_amount);
            let q_received_after = *max_q_amount;
            let q_instant_swap = q_instant_swap.sub(q_surplus);

            (q_instant_swap, q_received_after)
        };

        let vesting_1_amount = vesting_1_amount.sub(q_instant_swap);

        let q_total_amount: T::Balance =
            balance_from_eq_fixedu128(q_total_amount_fixed).ok_or(ArithmeticError::Overflow)?;
        let vesting_2_amount = q_total_amount.sub(vesting_1_amount).sub(q_instant_swap);

        Ok((
            *amount,
            eq_amount,
            q_instant_swap,
            q_received_after,
            vesting_1_amount,
            vesting_2_amount,
        ))
    }
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, scale_info::TypeInfo)]
pub struct CheckQSwap<T: Config + Send + Sync + scale_info::TypeInfo>(PhantomData<T>)
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>;

impl<T: Config + Send + Sync + scale_info::TypeInfo> Debug for CheckQSwap<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "CheckQSwap")
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> Default for CheckQSwap<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> CheckQSwap<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> SignedExtension for CheckQSwap<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    const IDENTIFIER: &'static str = "CheckQSwap";
    type AccountId = T::AccountId;
    type Call = T::RuntimeCall;
    type AdditionalSigned = ();
    type Pre = ();

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        Ok(())
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        self.validate(who, call, info, len)
            .map(|_| Self::Pre::default())
            .map_err(Into::into)
    }

    /// Checks:
    /// - Swap is enabled.
    /// - Available balance is enough to perform swap.
    /// - Swapping amount is greater or equal to the minimum swap amount.
    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> TransactionValidity {
        if let Some(local_call) = call.is_sub_type() {
            if let Call::swap { asset, amount } = local_call {
                let configuration = QSwapConfigurations::<T>::get(asset);

                Pallet::<T>::ensure_swap_enabled(&configuration).map_err(|_| {
                    InvalidTransaction::Custom(ValidityError::SwapsAreDisabled.into())
                })?;
                Pallet::<T>::ensure_valid_amount(&configuration, amount).map_err(|_| {
                    InvalidTransaction::Custom(ValidityError::AmountTooSmall.into())
                })?;

                let balance = T::EqCurrency::get_balance(who, &asset);

                Pallet::<T>::ensure_enough_balance(&balance, amount).map_err(|_| {
                    InvalidTransaction::Custom(ValidityError::NotEnoughBalance.into())
                })?;
            }
        }

        Ok(ValidTransaction::default())
    }
}

/// Claim validation errors
#[repr(u8)]
pub enum ValidityError {
    /// Swaps are disabled
    SwapsAreDisabled = 1,
    /// Configuration is invalid
    InvalidConfiguration = 2,
    /// Available balance is not enough to perform swap
    NotEnoughBalance = 3,
    /// Specified amount is too small to perform swap
    AmountTooSmall = 4,
}

impl From<ValidityError> for u8 {
    fn from(err: ValidityError) -> Self {
        err as u8
    }
}

#[derive(Default, Debug, Decode, Encode, PartialEq, TypeInfo)]
pub struct SwapConfiguration<Balance, BlockNumber> {
    pub enabled: bool,
    pub min_amount: Balance,
    pub main_asset_q_price: Balance,
    pub main_asset_q_discounted_price: Balance,
    pub secondary_asset: Asset,
    pub secondary_asset_q_price: Balance,
    pub secondary_asset_q_discounted_price: Balance,
    pub instant_swap_share: Percent,
    pub main_vesting_number: u8,
    pub secondary_vesting_number: u8,
    pub main_vesting_starting_block: BlockNumber,
    pub main_vesting_duration_blocks: Balance,
    pub secondary_vesting_starting_block: BlockNumber,
    pub secondary_vesting_duration_blocks: Balance,
}

impl<Balance: PartialOrd + Zero, BlockNumber: Zero> SwapConfiguration<Balance, BlockNumber> {
    fn set(&mut self, config: SwapConfigurationInput<Balance, BlockNumber>) {
        if let Some(enabled) = config.mb_enabled {
            self.enabled = enabled;
        }

        if let Some(min_amount) = config.mb_min_amount {
            self.min_amount = min_amount;
        }

        if let Some(main_asset_q_price) = config.mb_main_asset_q_price {
            self.main_asset_q_price = main_asset_q_price;
        }

        if let Some(main_asset_q_discounted_price) = config.mb_main_asset_q_discounted_price {
            self.main_asset_q_discounted_price = main_asset_q_discounted_price;
        }

        if let Some(secondary_asset) = config.mb_secondary_asset {
            self.secondary_asset = secondary_asset;
        }

        if let Some(secondary_asset_q_price) = config.mb_secondary_asset_q_price {
            self.secondary_asset_q_price = secondary_asset_q_price;
        }

        if let Some(secondary_asset_q_discounted_price) =
            config.mb_secondary_asset_q_discounted_price
        {
            self.secondary_asset_q_discounted_price = secondary_asset_q_discounted_price;
        }

        if let Some(instant_swap_share) = config.mb_instant_swap_share {
            self.instant_swap_share = instant_swap_share;
        }

        if let Some(main_vesting_starting_block) = config.mb_main_vesting_starting_block {
            self.main_vesting_starting_block = main_vesting_starting_block;
        }

        if let Some(main_vesting_duration_blocks) = config.mb_main_vesting_duration_blocks {
            self.main_vesting_duration_blocks = main_vesting_duration_blocks;
        }

        if let Some(secondary_vesting_starting_block) = config.mb_secondary_vesting_starting_block {
            self.secondary_vesting_starting_block = secondary_vesting_starting_block;
        }

        if let Some(secondary_vesting_duration_blocks) = config.mb_secondary_vesting_duration_blocks
        {
            self.secondary_vesting_duration_blocks = secondary_vesting_duration_blocks;
        }

        if let Some(main_vesting_number) = config.mb_main_vesting_number {
            self.main_vesting_number = main_vesting_number;
        }

        if let Some(secondary_vesting_number) = config.mb_secondary_vesting_number {
            self.secondary_vesting_number = secondary_vesting_number;
        }
    }

    fn is_valid(&self) -> bool {
        !self.enabled
            || self.min_amount.gt(&Balance::zero())
                && !self.main_asset_q_price.is_zero()
                && !self.main_asset_q_discounted_price.is_zero()
                && !self.instant_swap_share.is_zero()
                && (!self.main_vesting_number.is_zero() || !self.secondary_vesting_number.is_zero())
                && (self.main_vesting_number.is_zero()
                    || !self.main_vesting_starting_block.is_zero()
                        && !self.main_vesting_duration_blocks.is_zero())
                && (self.secondary_vesting_number.is_zero()
                    || !self.secondary_vesting_starting_block.is_zero()
                        && !self.secondary_vesting_duration_blocks.is_zero())
                && self.main_asset_q_discounted_price <= self.main_asset_q_price
                && (self.secondary_asset_q_price.is_zero()
                    || !self.secondary_asset_q_discounted_price.is_zero()
                        && !self.secondary_asset.get_id() > 0
                        && !self.secondary_asset_q_price.is_zero()
                        && self.secondary_asset_q_discounted_price <= self.secondary_asset_q_price
                        && !self.main_asset_q_price.is_zero())
    }
}

#[derive(Clone, Default, Debug, Decode, Encode, PartialEq, TypeInfo)]
pub struct SwapConfigurationInput<Balance, BlockNumber> {
    pub mb_enabled: Option<bool>,
    pub mb_min_amount: Option<Balance>,
    pub mb_main_asset_q_price: Option<Balance>,
    pub mb_main_asset_q_discounted_price: Option<Balance>,
    pub mb_secondary_asset: Option<Asset>,
    pub mb_secondary_asset_q_price: Option<Balance>,
    pub mb_secondary_asset_q_discounted_price: Option<Balance>,
    pub mb_instant_swap_share: Option<Percent>,
    pub mb_main_vesting_number: Option<u8>,
    pub mb_secondary_vesting_number: Option<u8>,
    pub mb_main_vesting_starting_block: Option<BlockNumber>,
    pub mb_main_vesting_duration_blocks: Option<Balance>,
    pub mb_secondary_vesting_starting_block: Option<BlockNumber>,
    pub mb_secondary_vesting_duration_blocks: Option<Balance>,
}
