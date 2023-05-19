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

use codec::Codec;
use eq_primitives::balance::Balance;
use eq_primitives::{
    asset::{Asset, AssetGetter},
    balance::EqCurrency,
    price::PriceGetter,
    TransferReason,
};
use eq_utils::eq_ensure;
use frame_support::{
    codec::{Decode, Encode},
    dispatch::DispatchResultWithPostInfo,
    traits::{ExistenceRequirement, Get, UnixTime},
    PalletId,
};
#[allow(unused_imports)]
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{CheckedAdd, CheckedSub};
use sp_runtime::Permill;
use sp_runtime::{
    traits::{AccountIdConversion, AtLeast32BitUnsigned, Zero},
    ArithmeticError, DispatchResult, FixedI64, RuntimeDebug,
};
use sp_std::convert::TryInto;
use sp_std::fmt::Debug;

#[derive(
    Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, scale_info::TypeInfo, codec::MaxEncodedLen,
)]
pub enum BinaryMode {
    /// Price increased/decreased relatively to .0 value
    CallPut(FixedI64),
    /// Price is in/out window
    InOut(FixedI64, FixedI64),
}

#[derive(
    Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, scale_info::TypeInfo, codec::MaxEncodedLen,
)]
pub struct BinaryInfo {
    /// When binary was started (seconds since unix epoch)
    pub start_time: u64,
    /// When binary should be ended (seconds since unix epoch)
    pub end_time: u64,
    /// Participation offset in seconds
    pub deposit_offset: u64,
    /// Asset used to vote
    pub proper: Asset,
    /// Minimal amount af asset to vote
    pub minimal_amount: Balance,
    /// Tracked asset and its price at the start of binary
    pub target: (Asset, BinaryMode),
    /// Total balance for false/true result
    pub total: (/*false*/ Balance, /*true*/ Balance),
    /// Total amount of balance, claimed by winners (tracked in case of rounding residues)
    pub claimed: Balance,
    /// Fees ratio transferred to treasury during claim calls
    pub fee: Permill,
    /// Penalty for quitting option
    pub penalty: Permill,
}

pub mod benchmarking;
mod mock;
mod origin;
mod tests;
pub mod weights;
pub use weights::WeightInfo;

pub use pallet::*;

type BinaryId = u64;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::origin::EnsureManager;
    use eq_utils::vec_map::VecMap;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::EitherOfDiverse;
    use frame_system::pallet_prelude::*;

    pub type EnsureManagerOrManagementOrigin<T, I> =
        EitherOfDiverse<EnsureManager<T, I>, <T as Config<I>>::ToggleBinaryCreateOrigin>;

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;
        /// Origin for binary creation
        type ToggleBinaryCreateOrigin: EnsureOrigin<Self::Origin>;
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Codec
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + From<Balance>
            + Into<Balance>
            + codec::MaxEncodedLen;
        /// Used to deal with Assets
        type AssetGetter: AssetGetter;
        /// Used to deal with prices
        type PriceGetter: PriceGetter;
        /// Timestamp provider
        type UnixTime: UnixTime;
        /// Used for currency-related operations and calculations
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        #[pallet::constant]
        type UpdateOnceInBlocks: Get<Self::BlockNumber>;
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        /// Treasury account
        #[pallet::constant]
        type TreasuryModuleId: Get<Self::AccountId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::storage]
    #[pallet::getter(fn manager)]
    pub type PalletManager<T: Config<I>, I: 'static = ()> = StorageValue<_, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn binaries)]
    pub type Binaries<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, BinaryId, (BinaryInfo, Option<bool>, u32)>;

    #[pallet::storage]
    #[pallet::getter(fn votes)]
    pub type Votes<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Blake2_128Concat,
        BinaryId,
        VecMap<T::AccountId, (bool, T::Balance)>,
        ValueQuery,
    >;

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            if (n % T::UpdateOnceInBlocks::get()).is_zero() {
                Binaries::<T, I>::translate(Self::update_binary);
                <T as pallet::Config<I>>::WeightInfo::on_initialize()
            } else {
                Weight::from_ref_time(1)
            }
        }

        fn on_runtime_upgrade() -> Weight {
            use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};
            EqPalletAccountInitializer::<T>::initialize(
                &T::PalletId::get().into_account_truncating(),
            );
            Weight::from_ref_time(1)
        }
    }

    #[pallet::genesis_config]
    #[cfg_attr(feature = "std", derive(Default))]
    pub struct GenesisConfig {
        pub empty: (),
    }

    #[pallet::genesis_build]
    impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig {
        fn build(&self) {
            use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};
            EqPalletAccountInitializer::<T>::initialize(
                &T::PalletId::get().into_account_truncating(),
            );
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// New binary option just started
        /// \[binary_id, target_asset, binary_mode, proper_asset\]
        Create(BinaryId, Asset, BinaryMode, Asset),
        /// Binary option deleted
        /// \[binary_id\]
        Purge(BinaryId),
        /// `who` participants in binary option with `deposit`
        /// \[binary_id, who, deposit\]
        Enter(BinaryId, T::AccountId, T::Balance),
        /// `who` quits binary option
        /// \[binary_id, who\]
        Quit(BinaryId, T::AccountId),
        /// `who` wins binary option and claims `reward`
        /// \[binary_id, who, reward\]
        Claim(BinaryId, T::AccountId, T::Balance),
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        // This binary option yet not started
        NoBinary,
        // Could not start another instance of binary option
        AlreadyStarted,
        // Could not claim reward: result not yet acquired
        TryClaimEarlier,
        // User is not a winner, neither a participant
        NoReward,
        // Purge if the time has not yet come
        TryPurgeEarlier,
        // Purge if winners have not claimed their reward
        TryPurgeWithWinners,
        // User doesn't participate in binary option
        NoDeposit,
        // Not enough amount to participate in binary option
        LowDeposit,
        // Time to participate in binary option is over
        ParticipateTimeIsOver,
        // Could not vote for opposite result in the same binary option
        DepositForOppositeResult,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        #[pallet::weight(T::WeightInfo::create())]
        pub fn create(
            origin: OriginFor<T>,
            binary_id: BinaryId,
            duration: u64,
            deposit_offset: u64,
            target: Asset,
            target_mode: BinaryMode,
            proper: Asset,
            minimal_amount: T::Balance,
            fee: Permill,
            penalty: Permill,
        ) -> DispatchResultWithPostInfo {
            T::ToggleBinaryCreateOrigin::ensure_origin(origin)?;

            Self::create_inner(
                binary_id,
                duration,
                deposit_offset,
                target,
                target_mode,
                proper,
                minimal_amount,
                fee,
                penalty,
            )?;

            Ok(Pays::No.into())
        }

        #[pallet::weight(T::WeightInfo::purge())]
        pub fn purge(origin: OriginFor<T>, binary_id: BinaryId) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            Self::purge_inner(binary_id)?;

            Ok(().into())
        }

        #[pallet::weight(T::WeightInfo::deposit())]
        pub fn deposit(
            origin: OriginFor<T>,
            binary_id: BinaryId,
            expected_result: bool,
            amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Binaries::<T, I>::try_mutate_exists(binary_id, |binary| {
                let (binary, _, _) = binary.as_mut().ok_or(Error::<T, I>::NoBinary)?;

                Self::deposit_inner(who, binary_id, binary, expected_result, amount)
            })?;

            Ok(().into())
        }

        #[pallet::weight(T::WeightInfo::withdraw())]
        pub fn withdraw(origin: OriginFor<T>, binary_id: BinaryId) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Binaries::<T, I>::try_mutate_exists(binary_id, |binary| {
                let (binary, _, _) = binary.as_mut().ok_or(Error::<T, I>::NoBinary)?;

                Self::withdraw_inner(who, binary_id, binary)
            })?;

            Ok(().into())
        }

        #[pallet::weight(T::WeightInfo::claim())]
        pub fn claim(origin: OriginFor<T>, binary_id: BinaryId) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Binaries::<T, I>::try_mutate_exists(binary_id, |binary| {
                let (binary, result, winners_left) =
                    binary.as_mut().ok_or(Error::<T, I>::NoBinary)?;
                let result = result.ok_or(Error::<T, I>::TryClaimEarlier)?;

                Self::claim_inner(who, binary_id, binary, result, winners_left)
            })?;

            Ok(().into())
        }

        #[pallet::weight(T::WeightInfo::claim_other())]
        pub fn claim_other(
            origin: OriginFor<T>,
            other: T::AccountId,
            binary_id: BinaryId,
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            Binaries::<T, I>::try_mutate_exists(binary_id, |binary| {
                let (binary, result, winners_left) =
                    binary.as_mut().ok_or(Error::<T, I>::NoBinary)?;
                let result = result.ok_or(Error::<T, I>::TryClaimEarlier)?;

                Self::claim_inner(other, binary_id, binary, result, winners_left)
            })?;

            Ok(().into())
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    fn update_binary(
        binary_id: BinaryId,
        (binary, result, winners_left): (BinaryInfo, Option<bool>, u32),
    ) -> Option<(BinaryInfo, Option<bool>, u32)> {
        let (target_asset, target_mode) = &binary.target;
        let fixed_price: Result<FixedI64, _> = T::PriceGetter::get_price(target_asset);

        if binary.end_time <= Self::get_timestamp() && result.is_none() && fixed_price.is_ok() {
            let fixed_price = fixed_price.unwrap();
            let new_result = match *target_mode {
                BinaryMode::CallPut(price) => fixed_price > price,
                BinaryMode::InOut(price_down, price_up) => {
                    fixed_price > price_down && fixed_price < price_up
                }
            };
            let votes = Votes::<T, I>::get(binary_id);
            let new_winners_left = votes
                .iter()
                .filter(|(_, (prediction, _))| *prediction == new_result)
                .count() as u32;
            Some((binary, Some(new_result), new_winners_left))
        } else {
            Some((binary, result, winners_left))
        }
    }

    fn create_inner(
        binary_id: BinaryId,
        duration: u64,
        deposit_offset: u64,
        target: Asset,
        target_mode: BinaryMode,
        proper: Asset,
        minimal_amount: T::Balance,
        fee: Permill,
        penalty: Permill,
    ) -> DispatchResult {
        T::AssetGetter::get_asset_data(&target).map_err(|err| {
            frame_support::log::error!("Option's target asset {:?} does not exist", target);
            err
        })?;

        T::AssetGetter::get_asset_data(&proper).map_err(|err| {
            frame_support::log::error!("Option's proper asset {:?} does not exist", proper);
            err
        })?;

        eq_ensure!(
            !Binaries::<T, I>::contains_key(&binary_id),
            Error::<T, I>::AlreadyStarted,
            target: "gens_binary_opt",
            "Option with id {:?} already started",
            binary_id,
        );

        let start_time = Self::get_timestamp();
        let end_time = start_time
            .checked_add(duration)
            .ok_or(ArithmeticError::Overflow)?;

        let binary_params = BinaryInfo {
            start_time,
            end_time,
            proper,
            minimal_amount: minimal_amount.into(),
            fee,
            penalty,
            deposit_offset,
            target: (target, target_mode.clone()),
            total: (0u128.into(), 0u128.into()),
            claimed: 0u128.into(),
        };
        Binaries::<T, I>::insert(binary_id, (binary_params, Option::<bool>::None, 0));

        Self::deposit_event(Event::Create(binary_id, target, target_mode, proper));
        Ok(().into())
    }

    fn purge_inner(binary_id: BinaryId) -> DispatchResult {
        let (binary, result, winners_left) =
            Binaries::<T, I>::get(binary_id).ok_or(Error::<T, I>::NoBinary)?;
        result.ok_or(Error::<T, I>::TryPurgeEarlier)?;

        // All winners have to claim their reward, or purge will result with an error
        // When winner call `claim` it will delete bet, so at this point there should be only
        // lose bets
        eq_ensure!(
            winners_left == 0,
            Error::<T, I>::TryPurgeWithWinners,
            target: "gens_binary_opt",
            "There are at least {} winners, option cannot be purged",
            winners_left,
        );

        // This means that there are no winners and no one is able to claim reward,
        // so remain_balance should be transferred away from binary option;
        // otherwise, the remained balance would have already been distributed among winners
        if binary.claimed == 0 {
            let remain_balance = binary
                .total
                .0
                .checked_add(binary.total.1)
                .ok_or(ArithmeticError::Overflow)?;
            T::EqCurrency::currency_transfer(
                &T::PalletId::get().into_account_truncating(),
                &T::TreasuryModuleId::get(),
                binary.proper,
                remain_balance.into(),
                ExistenceRequirement::KeepAlive,
                TransferReason::Common,
                true,
            )?;
        }

        Votes::<T, I>::remove(binary_id);
        Binaries::<T, I>::remove(binary_id);

        Self::deposit_event(Event::Purge(binary_id));
        Ok(().into())
    }

    fn deposit_inner(
        who: T::AccountId,
        binary_id: BinaryId,
        binary: &mut BinaryInfo,
        expected_result: bool,
        amount: T::Balance,
    ) -> DispatchResult {
        let now = Self::get_timestamp();
        eq_ensure!(
            binary.end_time > now.checked_add(binary.deposit_offset).ok_or(ArithmeticError::Overflow)?,
            Error::<T, I>::ParticipateTimeIsOver,
            target: "gens_binary_opt",
            "Time for depositing ({}) is over (now: {})",
            binary.end_time.checked_sub(binary.deposit_offset).ok_or(ArithmeticError::Overflow)?,
            now,
        );

        Votes::<T, I>::try_mutate(binary_id, |votes| {
            match votes.get_mut(&who) {
                Some((prediction, balance)) => {
                    eq_ensure!(
                        *prediction == expected_result,
                        Error::<T, I>::DepositForOppositeResult,
                        target: "gens_binary_opt",
                        "Cannot deposit for opposite result {}",
                        !expected_result,
                    );
                    *balance = balance
                        .checked_add(&amount)
                        .ok_or(ArithmeticError::Overflow)?;
                }
                None => {
                    eq_ensure!(
                        amount >= binary.minimal_amount.into(),
                        Error::<T, I>::LowDeposit,
                        target: "gens_binary_opt",
                        "Deposit is to low ({:?} < {:?}) for participating in option",
                        amount,
                        binary.minimal_amount,
                    );
                    votes.push(who.clone(), (expected_result, amount));
                }
            }

            T::EqCurrency::currency_transfer(
                &who,
                &T::PalletId::get().into_account_truncating(),
                binary.proper,
                amount,
                ExistenceRequirement::AllowDeath,
                TransferReason::Common,
                true,
            )?;

            if expected_result {
                binary.total.1 = binary
                    .total
                    .1
                    .checked_add(amount.into())
                    .ok_or(ArithmeticError::Overflow)?;
            } else {
                binary.total.0 = binary
                    .total
                    .0
                    .checked_add(amount.into())
                    .ok_or(ArithmeticError::Overflow)?;
            }

            Self::deposit_event(Event::Enter(binary_id, who, amount));
            Ok(().into())
        })
    }

    fn withdraw_inner(
        who: T::AccountId,
        binary_id: BinaryId,
        binary: &mut BinaryInfo,
    ) -> DispatchResult {
        let mut votes = Votes::<T, I>::get(binary_id);
        let vote = votes.get(&who);

        eq_ensure!(
            vote.is_some(),
            Error::<T, I>::NoDeposit,
            target: "gens_binary_opt",
            "Account {:?} has no deposit to withdraw",
            who
        );
        let (expected_result, amount) = *vote.unwrap();

        let now = Self::get_timestamp();
        eq_ensure!(
            binary.end_time > now.checked_add(binary.deposit_offset).ok_or(ArithmeticError::Overflow)?,
            Error::<T, I>::ParticipateTimeIsOver,
            target: "gens_binary_opt",
            "Time for withdrawing ({}) is over (now: {})",
            binary.end_time.checked_sub(binary.deposit_offset).ok_or(ArithmeticError::Overflow)?,
            now,
        );

        let penalty = binary.penalty * amount;
        T::EqCurrency::currency_transfer(
            &T::PalletId::get().into_account_truncating(),
            &who,
            binary.proper,
            amount
                .checked_sub(&penalty)
                .ok_or(ArithmeticError::Overflow)?,
            ExistenceRequirement::KeepAlive,
            TransferReason::Common,
            true,
        )?;
        T::EqCurrency::currency_transfer(
            &T::PalletId::get().into_account_truncating(),
            &T::TreasuryModuleId::get(),
            binary.proper,
            penalty,
            ExistenceRequirement::KeepAlive,
            TransferReason::Common,
            true,
        )?;

        if expected_result {
            binary.total.1 = binary
                .total
                .1
                .checked_sub(amount.into())
                .ok_or(ArithmeticError::Overflow)?;
        } else {
            binary.total.0 = binary
                .total
                .0
                .checked_sub(amount.into())
                .ok_or(ArithmeticError::Overflow)?;
        }
        votes.remove(&who);
        Votes::<T, I>::set(binary_id, votes);

        Self::deposit_event(Event::Quit(binary_id, who));
        Ok(().into())
    }

    fn claim_inner(
        who: T::AccountId,
        binary_id: BinaryId,
        binary: &mut BinaryInfo,
        result: bool,
        winners_left: &mut u32,
    ) -> DispatchResult {
        // This guarantees that there is at least one bet for `result` and total_`result` greater than 0
        let mut votes = Votes::<T, I>::get(binary_id);
        let vote = votes.get(&who);

        eq_ensure!(
            vote.is_some() && vote.unwrap().0 == result,
            Error::<T, I>::NoReward,
            target: "gens_binary_opt",
            "Account {:?} has no reward to claim",
            &who
        );

        let (_, balance) = *vote.unwrap();
        // The most precise way to calculate reward by the next formula:
        // winner_reward = winner_bet * all_bets / all_winner_bets.
        // It also can lead to minor rounding errors,
        // and reward could differ from expected arithmetic amount.
        let (total_false, total_true) = binary.total;
        let total_win = if result { total_true } else { total_false };

        *winners_left -= 1;
        let reward = if *winners_left == 0 {
            // This is the last winner, they can take residues
            (total_false
                .checked_add(total_true)
                .ok_or(ArithmeticError::Overflow)?
                .checked_sub(binary.claimed)
                .ok_or(ArithmeticError::Overflow)?)
            .into()
        } else {
            Self::mul_coefficient(
                balance.into(),
                total_false
                    .checked_add(total_true)
                    .ok_or(ArithmeticError::Overflow)?,
                total_win,
            )
            .ok_or(ArithmeticError::Overflow)?
        };

        binary.claimed = binary
            .claimed
            .checked_add(reward.into())
            .ok_or(ArithmeticError::Overflow)?;

        let fees = binary.fee
            * (reward
                .checked_sub(&balance)
                .ok_or(ArithmeticError::Overflow)?);

        T::EqCurrency::currency_transfer(
            &T::PalletId::get().into_account_truncating(),
            &T::TreasuryModuleId::get(),
            binary.proper,
            fees,
            ExistenceRequirement::KeepAlive,
            TransferReason::Common,
            true,
        )?;

        T::EqCurrency::currency_transfer(
            &T::PalletId::get().into_account_truncating(),
            &who,
            binary.proper,
            reward.checked_sub(&fees).ok_or(ArithmeticError::Overflow)?,
            ExistenceRequirement::KeepAlive,
            TransferReason::Common,
            true,
        )?;
        votes.remove(&who);
        Votes::<T, I>::set(binary_id, votes);

        Self::deposit_event(Event::Claim(binary_id, who, reward));
        Ok(().into())
    }

    fn get_timestamp() -> u64 {
        T::UnixTime::now().as_secs()
    }

    fn mul_coefficient(a: u128, b: u128, c: u128) -> Option<T::Balance> {
        // return a * b / c
        sp_runtime::helpers_128bit::multiply_by_rational_with_rounding(
            a,
            b,
            c,
            sp_runtime::Rounding::Down,
        )
        .map(|value| value.into())
    }
}
