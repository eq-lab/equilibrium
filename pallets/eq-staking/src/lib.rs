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

//! # Equilibrium Staking Pallet

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(warnings)]
pub mod benchmarking;
#[cfg(test)]
mod mock;
mod origin;
#[cfg(test)]
mod tests;
pub mod weights;

use crate::weights::WeightInfo;
use codec::{Decode, Encode, MaxEncodedLen};
use eq_primitives::{
    asset,
    balance::{BalanceGetter, EqCurrency, LockGetter},
    SignedBalance, TransferReason,
};
use frame_support::{
    pallet_prelude::DispatchResult,
    storage::bounded_btree_set::BoundedBTreeSet,
    traits::{EitherOfDiverse, ExistenceRequirement, LockIdentifier, UnixTime},
};
use sp_runtime::traits::{
    AtLeast32BitUnsigned, CheckedAdd, MaybeSerializeDeserialize, Member, Saturating, Zero,
};
use sp_std::{
    convert::{TryFrom, TryInto},
    vec::Vec,
};

const STAKING_ID: LockIdentifier = *b"staking ";

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::{ArithmeticError, DispatchError};

    pub type EnsureManagerOrManagementOrigin<T> =
        EitherOfDiverse<origin::EnsureManager<T>, <T as Config>::RewardManagementOrigin>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Numerical representation of stored balances
        type Balance: Member
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Parameter
            + Default
            + TryFrom<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>
            + Copy
            + MaxEncodedLen;
        /// Used for balance operations
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        /// Gets users balances
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;
        /// Used to get users locks
        type LockGetter: LockGetter<Self::AccountId, Self::Balance>;
        /// Timestamp provider
        type UnixTime: UnixTime;
        /// Max number of stakes for single account
        #[pallet::constant]
        type MaxStakesCount: Get<u32>;
        /// Origin to set manager and pay rewards
        type RewardManagementOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Account with liquidity to pay rewards
        type LiquidityAccount: Get<Self::AccountId>;
        /// Account with liquidity to pay custom rewards
        type LiquidityAccountCustom: Get<Self::AccountId>;
        #[pallet::constant]
        type RewardsLockPeriod: Get<StakePeriod>;
        type WeightInfo: WeightInfo;
        #[pallet::constant]
        type MaxRewardExternalIdsCount: Get<u32>;
    }

    #[pallet::storage]
    pub type PalletManager<T: Config> = StorageValue<_, T::AccountId>;

    #[pallet::storage]
    pub type Stakes<T: Config> = StorageMap<
        _,
        Identity,
        T::AccountId,
        BoundedVec<Stake<T::Balance>, T::MaxStakesCount>,
        ValueQuery,
    >;

    #[pallet::storage]
    pub type Rewards<T: Config> =
        StorageMap<_, Identity, T::AccountId, Stake<T::Balance>, OptionQuery>;

    #[pallet::storage]
    pub type RewardExternalIds<T: Config> =
        StorageValue<_, BoundedBTreeSet<u64, T::MaxRewardExternalIdsCount>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Staked {
            who: T::AccountId,
            amount: T::Balance,
            period: StakePeriod,
        },
        /// \[accounts\]
        Distributed(u32),
        Rewarded {
            who: T::AccountId,
            amount: T::Balance,
            external_id: u64,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The account reached stakes max number
        MaxStakesNumberReached,
        /// No funds to stake
        InsufficientFunds,
        /// No stake with this arguments
        StakeNotFound,
        /// The funds blocking period has not ended yet
        LockPeriodNotEnded,
        /// Some error occurs in custom_reward
        CustomReward(u8),
        /// Error while adding reward external ID
        UnableToAddRewardExternalId,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Stake the minimum value of `amount` and current free EQ balance for `period` if `MaxStakesCount` not reached
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::stake())]
        pub fn stake(
            origin: OriginFor<T>,
            amount: T::Balance,
            period: StakePeriod,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::do_stake(who, amount, period, true)
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::reward())]
        pub fn reward(
            origin: OriginFor<T>,
            who: T::AccountId,
            amount: T::Balance,
            external_id: u64,
        ) -> DispatchResultWithPostInfo {
            <EnsureManagerOrManagementOrigin<T>>::ensure_origin(origin)?;

            if !RewardExternalIds::<T>::get().contains(&external_id) {
                let _ = RewardExternalIds::<T>::mutate(|external_ids| -> DispatchResult {
                    if external_ids.len()
                        == usize::try_from(T::MaxRewardExternalIdsCount::get())
                            .map_err(|_| DispatchError::Arithmetic(ArithmeticError::Overflow))?
                    {
                        let first = external_ids
                            .iter()
                            .next()
                            .copied()
                            .ok_or(Error::<T>::UnableToAddRewardExternalId)?;
                        external_ids.remove(&first);
                    }
                    external_ids
                        .try_insert(external_id)
                        .map_err(|_| Error::<T>::UnableToAddRewardExternalId)?;
                    Ok(())
                })?;

                let now = T::UnixTime::now().as_secs();
                let _ = Rewards::<T>::mutate(who.clone(), |maybe_stake| -> DispatchResult {
                    match maybe_stake {
                        Some(stake) if now >= stake.start + stake.period.as_secs() => {
                            // unstake and new stake
                            let _ = Self::unlock_stake(who.clone(), *stake)?;
                            *maybe_stake = Some(Stake {
                                start: now,
                                amount,
                                period: T::RewardsLockPeriod::get(),
                            });
                        }
                        Some(stake) => {
                            let new_stake_amount = stake
                                .amount
                                .checked_add(&amount)
                                .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;
                            (*stake).amount = new_stake_amount;
                        }
                        None => {
                            *maybe_stake = Some(Stake {
                                start: now,
                                amount,
                                period: T::RewardsLockPeriod::get(),
                            });
                        }
                    };

                    let _ = T::EqCurrency::currency_transfer(
                        &T::LiquidityAccount::get(),
                        &who,
                        asset::EQ,
                        amount,
                        ExistenceRequirement::AllowDeath,
                        TransferReason::Common,
                        true,
                    )?;
                    let new_stake_lock = T::LockGetter::get_lock(who.clone(), STAKING_ID)
                        .checked_add(&amount)
                        .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;
                    T::EqCurrency::extend_lock(STAKING_ID, &who, new_stake_lock);

                    Ok(())
                })?;

                Self::deposit_event(Event::Rewarded {
                    who,
                    amount,
                    external_id,
                });
            }

            Ok(Pays::No.into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().writes(1).ref_time())]
        pub fn add_manager(
            origin: OriginFor<T>,
            manager: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            T::RewardManagementOrigin::ensure_origin(origin)?;

            PalletManager::<T>::put(manager);

            Ok(Pays::No.into())
        }

        /// Unlock stake if mb_stake_index is some or unlock rewards otherwise.
        /// Checks is lock period ended and throw error if not so.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::unlock_stake().max(T::WeightInfo::unlock_reward()))]
        pub fn unlock(origin: OriginFor<T>, mb_stake_index: Option<u32>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            if let Some(stake_index) = mb_stake_index {
                Stakes::<T>::mutate(who.clone(), |stakes| -> DispatchResult {
                    match stakes.get(stake_index as usize) {
                        Some(stake) => {
                            let _ = Self::unlock_stake(who, *stake)?;
                            stakes.remove(stake_index as usize);

                            Ok(())
                        }
                        None => Err(Error::<T>::StakeNotFound.into()),
                    }
                })
            } else {
                Rewards::<T>::mutate(who.clone(), |mb_stake| match mb_stake {
                    Some(stake) => {
                        let _ = Self::unlock_stake(who, *stake)?;
                        *mb_stake = None;

                        Ok(())
                    }
                    None => Err(Error::<T>::StakeNotFound.into()),
                })
            }
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::stake() * rewards.len() as u64)]
        pub fn custom_reward(
            origin: OriginFor<T>,
            rewards: Vec<(T::AccountId, StakePeriod, T::Balance)>,
        ) -> DispatchResultWithPostInfo {
            T::RewardManagementOrigin::ensure_origin(origin)?;

            for i in 0..rewards.len() {
                let (who, period, amount) = rewards[i].clone();
                let _ = T::EqCurrency::currency_transfer(
                    &T::LiquidityAccountCustom::get(),
                    &who,
                    asset::EQ,
                    amount,
                    ExistenceRequirement::AllowDeath,
                    TransferReason::Common,
                    true,
                )
                .map_err(|_| {
                    Self::deposit_event(Event::Distributed(i as u32));
                    Error::<T>::CustomReward(CustomRewardError::Transfer as u8)
                })?;
                let _ = Self::do_stake(who, amount, period, false).map_err(|_| {
                    Self::deposit_event(Event::Distributed(i as u32));
                    Error::<T>::CustomReward(CustomRewardError::Lock as u8)
                })?;
            }

            Self::deposit_event(Event::Distributed(rewards.len() as u32));

            Ok(Pays::No.into())
        }
    }
}

impl<T: Config> Pallet<T> {
    fn unlock_stake(who: T::AccountId, stake: Stake<T::Balance>) -> DispatchResult {
        let Stake {
            start,
            period,
            amount,
        } = stake;
        let now = T::UnixTime::now().as_secs();

        frame_support::ensure!(
            now >= start + period.as_secs(),
            Error::<T>::LockPeriodNotEnded
        );

        let mut staking_lock = T::LockGetter::get_lock(who.clone(), STAKING_ID);
        staking_lock = staking_lock.saturating_sub(amount);
        T::EqCurrency::set_lock(STAKING_ID, &who, staking_lock);
        Ok(())
    }

    fn do_stake(
        who: T::AccountId,
        amount: T::Balance,
        period: StakePeriod,
        event: bool,
    ) -> DispatchResult {
        if let SignedBalance::Positive(current_balance) =
            T::BalanceGetter::get_balance(&who, &asset::EQ)
        {
            let stake_locked = T::LockGetter::get_lock(who.clone(), STAKING_ID);
            let amount = current_balance.saturating_sub(stake_locked).min(amount);

            frame_support::ensure!(!amount.is_zero(), Error::<T>::InsufficientFunds);

            let start = T::UnixTime::now().as_secs();
            let _ = Stakes::<T>::mutate(who.clone(), |stakes| -> DispatchResult {
                stakes
                    .try_push(Stake {
                        amount,
                        start,
                        period,
                    })
                    .map_err(|_| Error::<T>::MaxStakesNumberReached.into())
            })?;

            T::EqCurrency::extend_lock(STAKING_ID, &who, stake_locked + amount);
            if event {
                Self::deposit_event(Event::Staked {
                    who,
                    amount,
                    period,
                });
            }
        }

        Ok(())
    }
}

/// Possible lock periods in months
#[derive(
    Copy, Debug, Decode, Encode, Clone, Eq, PartialEq, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum StakePeriod {
    One,
    Two,
    Three,
    Six,
    Twelve,
    Sixteen,
    Eighteen,
    TwentyFour,
}
const MONTH_IN_SECS: u64 = if cfg!(feature = "production") {
    30 * 24 * 60 * 60
} else {
    10 * 60
};

impl StakePeriod {
    fn as_secs(&self) -> u64 {
        match self {
            Self::One => MONTH_IN_SECS,
            Self::Two => 2 * MONTH_IN_SECS,
            Self::Three => 3 * MONTH_IN_SECS,
            Self::Six => 6 * MONTH_IN_SECS,
            Self::Twelve => 12 * MONTH_IN_SECS,
            Self::Sixteen => 16 * MONTH_IN_SECS,
            Self::Eighteen => 18 * MONTH_IN_SECS,
            Self::TwentyFour => 24 * MONTH_IN_SECS,
        }
    }
}

#[derive(
    Copy, Debug, Decode, Encode, Clone, Eq, PartialEq, scale_info::TypeInfo, MaxEncodedLen,
)]
pub struct Stake<Balance> {
    period: StakePeriod,
    start: u64,
    amount: Balance,
}

#[derive(Encode, Decode, scale_info::TypeInfo)]
#[repr(u8)]
pub enum CustomRewardError {
    Transfer = 0,
    Lock = 1,
}
