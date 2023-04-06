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
use eq_primitives::{
    asset::{Asset, AssetGetter, DebtWeightType},
    balance::EqCurrency,
    price::PriceGetter,
    TransferReason,
};
use eq_utils::eq_ensure;
use frame_support::{
    codec::{Decode, Encode},
    dispatch::DispatchResultWithPostInfo,
    traits::{ExistenceRequirement, Get, UnixTime},
};
#[allow(unused_imports)]
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
    traits::{AccountIdConversion, AtLeast32BitUnsigned, MaybeDisplay},
    DispatchResult, FixedI64, FixedPointNumber, ModuleId, Perquintill, RuntimeDebug,
};
use sp_std::fmt::Debug;

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub enum BinaryMode {
    /// Price growths/ain't growth relatively to .0 value
    CallPut(FixedI64),
    /// Price is in/out window
    InOut(FixedI64, FixedI64),
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct BinaryInfo<T: Config> {
    /// When binary was started (seconds since unix epoch)
    pub start_time: u64,
    /// When binary should be ended (seconds since unix epoch)
    pub end_time: u64,
    /// Asset used to vote
    pub proper: Asset,
    /// Minimal amount af asset to vote
    pub minimal_amount: T::Balance,
    /// Tracked asset and its price at the start of binary
    pub target: (Asset, BinaryMode),
    /// Total balance for false/true result
    pub total: (/*false*/ T::Balance, /*true*/ T::Balance),
    /// Total amount of balance, claimed by winners (tracked in case of rounding residues)
    pub claimed: T::Balance,
}

mod mock;
mod tests;
pub mod weights;
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Codec
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + From<u64>
            + Into<u64>;
        type BinaryId: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + MaybeDisplay
            + Ord;
        /// Used to deal with Assets
        type AssetGetter: AssetGetter;
        /// Used to deal with prices
        type PriceGetter: PriceGetter;
        /// Timestamp provider
        type UnixTime: UnixTime;
        /// Used for currency-related operations and calculations
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        /// Participation offset in seconds
        #[pallet::constant]
        type DepositOffset: Get<u64>;
        /// Penalty for quitting option
        #[pallet::constant]
        type Penalty: Get<Perquintill>;
        /// Account for storing reserved balance
        #[pallet::constant]
        type ModuleId: Get<ModuleId>;
        /// Treasury account
        #[pallet::constant]
        type TreasuryModuleId: Get<ModuleId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn binaries)]
    pub type Binaries<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BinaryId, (BinaryInfo<T>, Option<bool>)>;

    #[pallet::storage]
    #[pallet::getter(fn votes)]
    pub type Votes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        (T::BinaryId, bool),
        Blake2_128Concat,
        T::AccountId,
        T::Balance,
        ValueQuery,
    >;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
            Binaries::<T>::translate_values(Self::update_binary);
            10_000
        }
    }

    #[pallet::genesis_config]
    #[cfg_attr(feature = "std", derive(Default))]
    pub struct GenesisConfig {
        pub empty: (),
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};
            EqPalletAccountInitializer::<T>::initialize(
                &T::ModuleId::get().into_account_truncating(),
            );
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New binary option just started
        /// \[binary_id, target_asset, proper_asset\]
        Start(T::BinaryId, Asset, BinaryMode, Asset),
        /// Binary option deleted
        /// \[binary_id\]
        Purge(T::BinaryId),
        /// `who` participats in binary option with `deposit`
        /// \[binary_id, who, deposit\]
        Enter(T::BinaryId, T::AccountId, T::Balance),
        /// `who` quits binary option
        /// \[binary_id, who\]
        Quit(T::BinaryId, T::AccountId),
        /// `who` wins binary option and claims `reward`
        /// \[binary_id, who, reward\]
        Claim(T::BinaryId, T::AccountId, T::Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
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
        // Not enought amount to participate in binary option
        LowDeposit,
        // Time to participate in binary option is over
        ParticipateTimeIsOver,
        // Could not vote for opposite result in the same binary option
        DepositForOppositeResult,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::start())]
        pub fn start(
            origin: OriginFor<T>,
            binary_id: T::BinaryId,
            duration: u64,
            target: Asset,
            target_mode: BinaryMode,
            proper: Asset,
            minimal_amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            Self::do_start(
                binary_id,
                duration,
                target,
                target_mode,
                proper,
                minimal_amount,
            )?;

            Ok(().into())
        }

        #[pallet::weight(T::WeightInfo::purge())]
        pub fn purge(origin: OriginFor<T>, binary_id: T::BinaryId) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            let (binary, result) = Binaries::<T>::get(binary_id).ok_or(Error::<T>::NoBinary)?;
            let result = result.ok_or(Error::<T>::TryPurgeEarlier)?;

            Self::do_purge(binary_id, binary, result)?;

            Ok(().into())
        }

        #[pallet::weight(T::WeightInfo::deposit())]
        pub fn deposit(
            origin: OriginFor<T>,
            binary_id: T::BinaryId,
            expected_result: bool,
            amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Binaries::<T>::try_mutate_exists(binary_id, |binary| {
                let (binary, _) = binary.as_mut().ok_or(Error::<T>::NoBinary)?;

                Self::do_deposit(&who, binary_id, binary, expected_result, amount)
            })?;

            Ok(().into())
        }

        #[pallet::weight(T::WeightInfo::withdraw())]
        pub fn withdraw(
            origin: OriginFor<T>,
            binary_id: T::BinaryId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Binaries::<T>::try_mutate_exists(binary_id, |binary| {
                let (binary, _) = binary.as_mut().ok_or(Error::<T>::NoBinary)?;

                Self::do_withdraw(&who, binary_id, binary)
            })?;

            Ok(().into())
        }

        #[pallet::weight(T::WeightInfo::claim())]
        pub fn claim(origin: OriginFor<T>, binary_id: T::BinaryId) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Binaries::<T>::try_mutate_exists(binary_id, |binary| {
                let (binary, result) = binary.as_mut().ok_or(Error::<T>::NoBinary)?;
                let result = result.ok_or(Error::<T>::TryClaimEarlier)?;

                Self::do_claim(&who, binary_id, binary, result)
            })?;

            Ok(().into())
        }

        #[pallet::weight(T::WeightInfo::claim())]
        pub fn claim_other(
            origin: OriginFor<T>,
            other: T::AccountId,
            binary_id: T::BinaryId,
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            Binaries::<T>::try_mutate_exists(binary_id, |binary| {
                let (binary, result) = binary.as_mut().ok_or(Error::<T>::NoBinary)?;
                let result = result.ok_or(Error::<T>::TryClaimEarlier)?;

                Self::do_claim(&other, binary_id, binary, result)
            })?;

            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    fn update_binary(
        (binary, result): (BinaryInfo<T>, Option<bool>),
    ) -> Option<(BinaryInfo<T>, Option<bool>)> {
        if binary.end_time <= Self::get_timestamp() && result.is_none() {
            let (target_asset, target_mode) = &binary.target;
            let fixed_price = T::PriceGetter::get_price(target_asset).unwrap_or(FixedI64::zero());

            let new_result = match *target_mode {
                BinaryMode::CallPut(price) => fixed_price > price,
                BinaryMode::InOut(price_down, price_up) => {
                    fixed_price > price_down && fixed_price < price_up
                }
            };
            Some((binary, Some(new_result)))
        } else {
            Some((binary, result))
        }
    }

    fn do_start(
        binary_id: T::BinaryId,
        duration: u64,
        target: Asset,
        target_mode: BinaryMode,
        proper: Asset,
        minimal_amount: T::Balance,
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
            !Binaries::<T>::contains_key(&binary_id),
            Error::<T>::AlreadyStarted,
            target: "gens_binary_opt",
            "Option with id {:?} already started",
            binary_id,
        );

        let start_time = Self::get_timestamp();
        let end_time = start_time + duration;

        let binary_params = BinaryInfo {
            start_time,
            end_time,
            proper,
            minimal_amount,
            target: (target, target_mode.clone()),
            total: (0u64.into(), 0u64.into()),
            claimed: 0u64.into(),
        };
        Binaries::<T>::insert(binary_id, (binary_params, Option::<bool>::None));

        Self::deposit_event(Event::Start(binary_id, target, target_mode, proper));
        Ok(().into())
    }

    fn do_purge(binary_id: T::BinaryId, binary: BinaryInfo<T>, result: bool) -> DispatchResult {
        // All winners have to claim their reward, or purge will result with an error
        // When winner call `claim` it will delete bet, so at this point there should be only
        // lose bets
        let winners_count = Votes::<T>::iter_prefix((binary_id, result)).count();
        eq_ensure!(
            winners_count == 0,
            Error::<T>::TryPurgeWithWinners,
            target: "gens_binary_opt",
            "There are at least {} winners, option cannot be purged",
            winners_count,
        );

        // This means that there are no winners and no one is able to claim reward,
        // so remain_balance should be transfered away from binary option;
        // otherwise, the remained balance would have already been distributed among winners
        if binary.claimed.into() == 0 {
            let remain_balance = Votes::<T>::iter_prefix((binary_id, !result))
                .fold(T::Balance::from(0u64), |acc, (_, balance)| acc + balance);

            T::EqCurrency::currency_transfer(
                &T::ModuleId::get().into_account_truncating(),
                &T::TreasuryModuleId::get().into_account_truncating(),
                binary.proper,
                remain_balance,
                ExistenceRequirement::KeepAlive,
                TransferReason::Common,
                true,
            )?;
        }

        Votes::<T>::remove_prefix((binary_id, true));
        Votes::<T>::remove_prefix((binary_id, false));
        Binaries::<T>::remove(binary_id);

        Self::deposit_event(Event::Purge(binary_id));
        Ok(().into())
    }

    fn do_deposit(
        who: &T::AccountId,
        binary_id: T::BinaryId,
        binary: &mut BinaryInfo<T>,
        expected_result: bool,
        amount: T::Balance,
    ) -> DispatchResult {
        let now = Self::get_timestamp();
        eq_ensure!(
            binary.end_time > now + T::DepositOffset::get(),
            Error::<T>::ParticipateTimeIsOver,
            target: "gens_binary_opt",
            "Time for depositing ({}) is over (now: {})",
            binary.end_time - T::DepositOffset::get(),
            now,
        );

        eq_ensure!(
            !Votes::<T>::contains_key((binary_id, !expected_result), who.clone()),
            Error::<T>::DepositForOppositeResult,
            target: "gens_binary_opt",
            "Cannot deposit for opposite result {}",
            !expected_result,
        );

        Votes::<T>::try_mutate((binary_id, expected_result), who.clone(), |balance| {
            let new_balance = *balance + amount;
            eq_ensure!(
                new_balance >= binary.minimal_amount,
                Error::<T>::LowDeposit,
                target: "gens_binary_opt",
                "Deposit is to low ({:?} < {:?}) for participating in option",
                new_balance,
                binary.minimal_amount,
            );

            T::EqCurrency::currency_transfer(
                &who,
                &T::ModuleId::get().into_account_truncating(),
                binary.proper,
                amount,
                ExistenceRequirement::AllowDeath,
                TransferReason::Common,
                true,
            )?;

            if expected_result {
                binary.total.1 = binary.total.1 + amount;
            } else {
                binary.total.0 = binary.total.0 + amount;
            }

            *balance = new_balance;

            Self::deposit_event(Event::Enter(binary_id, who.clone(), amount));
            Ok(().into())
        })
    }

    fn do_withdraw(
        who: &T::AccountId,
        binary_id: T::BinaryId,
        binary: &mut BinaryInfo<T>,
    ) -> DispatchResult {
        let true_balance = Votes::<T>::get((binary_id, true), who.clone());
        let (expected_result, amount) = if true_balance.into() == 0 {
            (false, Votes::<T>::get((binary_id, false), who.clone()))
        } else {
            (true, true_balance)
        };

        eq_ensure!(
            amount.into() != 0,
            Error::<T>::NoDeposit,
            target: "gens_binary_opt",
            "Account {:?} has no deposit to withdraw",
            who
        );

        let now = Self::get_timestamp();
        eq_ensure!(
            binary.end_time > now + T::DepositOffset::get(),
            Error::<T>::ParticipateTimeIsOver,
            target: "gens_binary_opt",
            "Time for withdrawing ({}) is over (now: {})",
            binary.end_time - T::DepositOffset::get(),
            now,
        );

        let penalty = Self::penalty(amount);
        T::EqCurrency::currency_transfer(
            &T::ModuleId::get().into_account_truncating(),
            &who,
            binary.proper,
            amount - penalty,
            ExistenceRequirement::KeepAlive,
            TransferReason::Common,
            true,
        )?;
        T::EqCurrency::currency_transfer(
            &T::ModuleId::get().into_account_truncating(),
            &T::TreasuryModuleId::get().into_account_truncating(),
            binary.proper,
            penalty,
            ExistenceRequirement::KeepAlive,
            TransferReason::Common,
            true,
        )?;

        if expected_result {
            binary.total.1 = binary.total.1 - amount;
        } else {
            binary.total.0 = binary.total.0 - amount;
        }
        Votes::<T>::remove((binary_id, expected_result), who);

        Self::deposit_event(Event::Quit(binary_id, who.clone()));
        Ok(().into())
    }

    fn do_claim(
        who: &T::AccountId,
        binary_id: T::BinaryId,
        binary: &mut BinaryInfo<T>,
        result: bool,
    ) -> DispatchResult {
        // This guarantees that there is at least one bet for `result` and total_`result` greater than 0
        let balance = Votes::<T>::get((binary_id, result), who.clone());

        eq_ensure!(
            balance.into() != 0,
            Error::<T>::NoReward,
            target: "gens_binary_opt",
            "Account {:?} has no reward to claim",
            who
        );

        // The most precise way to calculate reward by the next formula:
        // winner_reward = winner_bet * all_bets / all_winner_bets.
        // It also can lead to minor rounding errors,
        // and reward could differ from expected arithmetic amount.
        let (total_false, total_true) = binary.total;
        let total_win = if result { total_true } else { total_false };
        let mut reward = Self::mul_coefficient(balance, total_false + total_true, total_win)
            .ok_or(ArithmeticError::Overflow)?;

        binary.claimed += reward;
        if let None = Votes::<T>::iter_prefix((binary_id, result)).nth(1) {
            // This is the last winner, he can take residues
            reward += total_false + total_true;
            reward -= binary.claimed;
        }

        T::EqCurrency::currency_transfer(
            &T::ModuleId::get().into_account_truncating(),
            who,
            binary.proper,
            reward,
            ExistenceRequirement::KeepAlive,
            TransferReason::Common,
            true,
        )?;
        Votes::<T>::remove((binary_id, result), who.clone());

        Self::deposit_event(Event::Claim(binary_id, who.clone(), reward));
        Ok(().into())
    }

    fn penalty(amount: T::Balance) -> T::Balance {
        T::Penalty::get() * amount
    }

    fn get_timestamp() -> u64 {
        T::UnixTime::now().as_secs()
    }

    fn mul_coefficient(a: T::Balance, b: T::Balance, c: T::Balance) -> Option<T::Balance> {
        // return a * b / c
        match sp_runtime::helpers_128bit::multiply_by_rational(
            a.into() as u128,
            b.into() as u128,
            c.into() as u128,
        ) {
            Ok(abc) if abc <= u64::MAX as u128 => Some((abc as u64).into()),
            _ => None,
        }
    }
}
