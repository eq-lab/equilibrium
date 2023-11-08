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
use eq_primitives::asset::{Asset, Q};
use eq_primitives::balance::{BalanceGetter, EqCurrency};
use eq_primitives::balance_number::EqFixedU128;
use eq_primitives::vestings::EqVestingSchedule;
use eq_primitives::{SignedBalance, Vesting};
use eq_utils::eq_ensure;
use frame_support::pallet_prelude::DispatchResult;
use frame_support::traits::{ExistenceRequirement, Get, IsSubType};
use frame_support::transactional;
use frame_support::weights::Weight;
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;
use sp_runtime::traits::{
    AtLeast32BitUnsigned, CheckedAdd, DispatchInfoOf, Saturating, SignedExtension, Zero,
};
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction,
};
use sp_runtime::{ArithmeticError, FixedPointNumber, FixedPointOperand, Percent};
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
        // Used for managing vestings
        type Vesting: Vesting<Self::AccountId>
            + EqVestingSchedule<Self::Balance, Self::AccountId, Moment = Self::BlockNumber>;
        /// Used for managing balances and currencies
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>
            + BalanceGetter<Self::AccountId, Self::Balance>;
        /// Returns vesting account
        type VestingAccountId: Get<Self::AccountId>;
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
        /// - requested amount
        /// - Q received amount
        /// - Q vested amount
        /// \[from, amount_1, amount_2, amount_3\]
        QSwap(T::AccountId, T::Balance, T::Balance, T::Balance),
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

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut reads = 0;
            let mut writes = 0;

            for (asset, mut config) in QSwapConfigurations::<T>::iter() {
                if config.enabled && config.vesting_starting_block.le(&now) {
                    config.enabled = false;
                    writes += 1;

                    QSwapConfigurations::<T>::insert(asset, config);
                }
                reads += 1;
            }

            return T::DbWeight::get().reads_writes(reads, writes);
        }
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
        let balance = T::EqCurrency::get_balance(who, asset);
        Self::ensure_enough_balance(&balance, amount)?;

        let q_total_amount = EqFixedU128::from_inner(configuration.q_ratio)
            .checked_mul_int(*amount)
            .ok_or(ArithmeticError::Overflow)?;

        let q_holder_account_id = T::QHolderAccountId::get();
        let asset_holder_account_id = T::AssetHolderAccountId::get();

        let vesting_amount = (!configuration.vesting_share.is_zero())
            .then(|| configuration.vesting_share.mul_floor(q_total_amount))
            .unwrap_or(T::Balance::zero());

        let q_amount = q_total_amount.sub(vesting_amount);

        let q_received = QReceivedAmounts::<T>::get(who);
        let q_received_after = q_received
            .checked_add(&q_amount)
            .ok_or(ArithmeticError::Underflow)?;

        let (vesting_amount, q_amount, q_received_after) = if q_received_after.le(&max_q_amount) {
            (vesting_amount, q_amount, q_received_after)
        } else {
            let q_surplus = q_received_after.sub(*max_q_amount);
            let q_received_after = *max_q_amount;
            let vesting_amount = vesting_amount.saturating_add(q_surplus);
            let q_amount = q_amount.saturating_sub(q_surplus);

            (vesting_amount, q_amount, q_received_after)
        };

        T::EqCurrency::currency_transfer(
            who,
            &asset_holder_account_id,
            *asset,
            *amount,
            ExistenceRequirement::AllowDeath,
            eq_primitives::TransferReason::QSwap,
            true,
        )?;

        if !vesting_amount.is_zero() {
            T::EqCurrency::currency_transfer(
                &q_holder_account_id,
                &T::VestingAccountId::get(),
                Q,
                vesting_amount,
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::QSwap,
                true,
            )?;

            if T::Vesting::has_vesting_schedule(who.clone()) {
                T::Vesting::update_vesting_schedule(
                    who,
                    vesting_amount,
                    configuration.vesting_duration_blocks,
                )?;
            } else {
                let per_block = configuration
                    .vesting_duration_blocks
                    .lt(&vesting_amount)
                    .then(|| vesting_amount.div(configuration.vesting_duration_blocks))
                    .unwrap_or(vesting_amount.div(vesting_amount));

                T::Vesting::add_vesting_schedule(
                    who,
                    vesting_amount,
                    per_block,
                    configuration.vesting_starting_block,
                )?;
            }
        }

        if !q_amount.is_zero() {
            T::EqCurrency::currency_transfer(
                &q_holder_account_id,
                who,
                Q,
                q_amount,
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::QSwap,
                true,
            )?;
        }

        QReceivedAmounts::<T>::insert(who, q_received_after);

        Self::deposit_event(Event::QSwap(who.clone(), *amount, q_amount, vesting_amount));

        Ok(())
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
    pub q_ratio: u128,
    pub vesting_share: Percent,
    pub vesting_starting_block: BlockNumber,
    pub vesting_duration_blocks: Balance,
}

impl<Balance: PartialOrd + Zero, BlockNumber: Zero> SwapConfiguration<Balance, BlockNumber> {
    fn set(&mut self, config: SwapConfigurationInput<Balance, BlockNumber>) {
        if let Some(enabled) = config.mb_enabled {
            self.enabled = enabled;
        }

        if let Some(min_amount) = config.mb_min_amount {
            self.min_amount = min_amount;
        }

        if let Some(q_ratio) = config.mb_q_ratio {
            self.q_ratio = q_ratio;
        }

        if let Some(vesting_share) = config.mb_vesting_share {
            self.vesting_share = vesting_share;
        }

        if let Some(vesting_starting_block) = config.mb_vesting_starting_block {
            self.vesting_starting_block = vesting_starting_block;
        }

        if let Some(vesting_duration_blocks) = config.mb_vesting_duration_blocks {
            self.vesting_duration_blocks = vesting_duration_blocks;
        }
    }

    fn is_valid(&self) -> bool {
        !self.enabled
            || self.min_amount.gt(&Balance::zero())
                && !self.q_ratio.is_zero()
                && !self.vesting_starting_block.is_zero()
                && !self.vesting_duration_blocks.is_zero()
    }
}

#[derive(Clone, Default, Debug, Decode, Encode, PartialEq, TypeInfo)]
pub struct SwapConfigurationInput<Balance, BlockNumber> {
    pub mb_enabled: Option<bool>,
    pub mb_min_amount: Option<Balance>,
    pub mb_q_ratio: Option<u128>,
    pub mb_vesting_share: Option<Percent>,
    pub mb_vesting_starting_block: Option<BlockNumber>,
    pub mb_vesting_duration_blocks: Option<Balance>,
}
