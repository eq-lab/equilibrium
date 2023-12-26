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

//! # Equilibrium Vesting Pallet
//!
//! Equilibrium's Vesting Pallet is a custom vesting pallet for Equilibrium
//! substrate.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

pub mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

use codec::{Decode, Encode};
use core::convert::{TryFrom, TryInto};
use eq_primitives::vestings::EqVestingSchedule;
use eq_primitives::{AccountRefCounter, AccountRefCounts, IsTransfersEnabled};
use eq_utils::{eq_ensure, ok_or_error};
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::traits::{Currency, ExistenceRequirement, Get};
use frame_support::PalletId;
use frame_system::{ensure_root, ensure_signed};
use sp_arithmetic::traits::CheckedDiv;
use sp_arithmetic::ArithmeticError;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{
    traits::{
        AtLeast32BitUnsigned, Convert, MaybeSerializeDeserialize, Saturating, StaticLookup, Zero,
    },
    DispatchResult, RuntimeDebug,
};
use sp_std::fmt::Debug;
use sp_std::prelude::*;
pub use weights::WeightInfo;

/// Struct to encode the vesting schedule of an individual account
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct VestingInfo<Balance, BlockNumber> {
    /// Locked amount at genesis
    pub locked: Balance,
    /// Amount that gets unlocked every block after `starting_block`
    pub per_block: Balance,
    /// Starting block for unlocking (vesting)
    pub starting_block: BlockNumber,
}

impl<Balance: AtLeast32BitUnsigned + Copy, BlockNumber: AtLeast32BitUnsigned + Copy>
    VestingInfo<Balance, BlockNumber>
{
    /// Gets amount yet to be vested at block `n`
    pub fn locked_at<BlockNumberToBalance: Convert<BlockNumber, Balance>>(
        &self,
        n: BlockNumber,
    ) -> Balance {
        // Number of blocks that count toward vesting
        // Saturating to 0 when n < starting_block
        let vested_block_count = n.saturating_sub(self.starting_block);
        // Return amount that is still locked in vesting
        let maybe_balance =
            BlockNumberToBalance::convert(vested_block_count).checked_mul(&self.per_block);
        if let Some(balance) = maybe_balance {
            self.locked.saturating_sub(balance)
        } else {
            Zero::zero()
        }
    }

    /// Gets amount available to be vested at block `n`
    pub fn unlocked_at<BlockNumberToBalance: Convert<BlockNumber, Balance>>(
        &self,
        n: BlockNumber,
    ) -> Balance {
        // Number of blocks that count toward vesting
        // Saturating to 0 when n < starting_block
        let vested_block_count = n.saturating_sub(self.starting_block);
        // Return amount that is still locked in vesting
        let maybe_balance =
            BlockNumberToBalance::convert(vested_block_count).checked_mul(&self.per_block);
        if let Some(balance) = maybe_balance {
            balance.min(self.locked)
        } else {
            self.locked
        }
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        /// The minimum amount transferred to call `vested_transfer`
        #[pallet::constant]
        type MinVestedTransfer: Get<Self::Balance>;
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// The overarching event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Numerical representation of stored balances
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + TryFrom<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>;
        /// Standard balances pallet for utility token or adapter
        type Currency: Currency<Self::AccountId, Balance = Self::Balance>;
        /// Convert the block number into a balance
        type BlockNumberToBalance: Convert<Self::BlockNumber, Self::Balance>;
        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
        /// Checks if transaction disabled flag is off
        type IsTransfersEnabled: eq_primitives::IsTransfersEnabled;
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Unlock any vested funds of the sender account.
        ///
        /// The dispatch origin for this call must be _Signed_ and the sender must have funds still
        /// locked under this module.
        ///
        /// Emits either `VestingCompleted` or `VestingUpdated`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::vest_locked().max(T::WeightInfo::vest_unlocked()))]
        pub fn vest(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::update_lock(who)
        }

        /// Unlock any vested funds of a `target` account.
        ///
        /// The dispatch origin for this call must be _Signed_.
        ///
        /// - `target`: The account whose vested funds should be unlocked. Must have funds still
        /// locked under this module.
        ///
        /// Emits either `VestingCompleted` or `VestingUpdated`.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::vest_other_locked().max(T::WeightInfo::vest_other_unlocked()))]
        pub fn vest_other(
            origin: OriginFor<T>,
            target: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;
            Self::update_lock(T::Lookup::lookup(target)?)
        }

        /// Force a vested transfer.
        ///
        /// The dispatch origin for this call must be _Root_.
        ///
        /// - `source`: The account whose funds should be transferred.
        /// - `target`: The account that should be transferred the vested funds.
        /// - `amount`: The amount of funds to transfer and will be vested.
        /// - `schedule`: The vesting schedule attached to the transfer.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::vested_transfer())]
        pub fn force_vested_transfer(
            origin: OriginFor<T>,
            source: <T::Lookup as StaticLookup>::Source,
            target: <T::Lookup as StaticLookup>::Source,
            schedule: VestingInfo<T::Balance, T::BlockNumber>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let is_enabled = T::IsTransfersEnabled::get();
            eq_ensure!(
                is_enabled,
                Error::<T, I>::TransfersAreDisabled,
                target: "eq_vesting",
                "{}:{}. Transfers are not allowed.",
                file!(),
                line!(),
            );

            eq_ensure!(
                schedule.locked >= T::MinVestedTransfer::get(),
                Error::<T, I>::AmountLow,
                target: "eq_vesting",
                "{}:{}. Schedule locked less than MinVestedTransfer. Schedule: {:?}, \
                MinVestedTransfer: {:?}.",
                file!(),
                line!(),
                schedule.locked,
                T::MinVestedTransfer::get()
            );
            eq_ensure!(
                schedule.per_block > T::Balance::zero(),
                Error::<T, I>::AmountLow,
                target: "eq_vesting",
                "{}:{}. Schedule per block equals zero. Schedule: {:?}.",
                file!(),
                line!(),
                schedule.per_block
            );

            let target = T::Lookup::lookup(target)?;
            let source = T::Lookup::lookup(source)?;
            eq_ensure!(
                !Vesting::<T, I>::contains_key(&target),
                Error::<T, I>::ExistingVestingSchedule,
                target: "eq_vesting",
                "{}:{}. An existing vesting schedule already exists for account. Who: {:?}.",
                file!(),
                line!(),
                target
            );

            T::Currency::transfer(
                &source,
                &Self::account_id(),
                schedule.locked,
                ExistenceRequirement::AllowDeath,
            )?;

            Self::add_vesting_schedule(
                &target,
                schedule.locked,
                schedule.per_block,
                schedule.starting_block,
            )
            .expect("user does not have an existing vesting schedule; q.e.d.");

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// The amount vested has been updated. This could indicate more funds are available. The
        /// balance given is the amount which is left unvested (and thus locked)
        /// \[account, unvested\]
        VestingUpdated(T::AccountId, T::Balance),
        /// An `account` has become fully vested. No further vesting can happen
        /// \[account\]
        VestingCompleted(T::AccountId),
        /// New value of AccountsPerBlock set
        /// \[accounts_per_block\]
        NewAccountsPerBlock(u32),
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// The account given is not vesting
        NotVesting,
        /// An existing vesting schedule already exists for this account that cannot be clobbered
        ExistingVestingSchedule,
        /// Amount being transferred is too low to create a vesting schedule
        AmountLow,
        /// Self documented error code
        TransfersAreDisabled,
        /// This method is not allowed in production
        MethodNotAllowed,
    }

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

    /// Pallet storage: information regarding the vesting of a given account
    #[pallet::storage]
    #[pallet::getter(fn vesting)]
    pub type Vesting<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::AccountId, VestingInfo<T::Balance, T::BlockNumber>>;

    /// Pallet storage: information about already vested balances for given account
    #[pallet::storage]
    #[pallet::getter(fn vested)]
    pub type Vested<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
        pub vestings: Vec<(T::AccountId, T::Balance, T::Balance, T::BlockNumber)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
        fn default() -> Self {
            Self {
                vestings: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig<T, I> {
        fn build(&self) {
            use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};

            EqPalletAccountInitializer::<T>::initialize(
                &T::PalletId::get().into_account_truncating(),
            );

            let mut deposit: T::Balance = Zero::zero();

            for (who, locked, per_block, starting_block) in &self.vestings {
                let vesting_schedule = VestingInfo {
                    locked: *locked,
                    per_block: *per_block,
                    starting_block: *starting_block,
                };
                Vesting::<T, I>::insert(who, vesting_schedule);
                AccountRefCounter::<T>::inc_ref(&who);

                let _ = Pallet::<T, I>::update_lock(who.clone());

                deposit = deposit.saturating_add(*locked);
            }

            T::Currency::deposit_creating(&T::PalletId::get().into_account_truncating(), deposit);
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    /// Get eq-vesting pallet account
    pub fn account_id() -> T::AccountId {
        T::PalletId::get().into_account_truncating()
    }
    /// (Re)set or remove the module's currency lock on `who`'s account in accordance with their
    /// current unvested amount.
    fn update_lock(who: T::AccountId) -> DispatchResultWithPostInfo {
        let option_vesting_info = Self::vesting(&who);
        let vesting = ok_or_error!(
            option_vesting_info,
            Error::<T, I>::NotVesting,
            "{}:{}. The account is not vesting. Who: {:?}.",
            file!(),
            line!(),
            who
        )?;
        let now = <frame_system::Pallet<T>>::block_number();
        let unlocked_now = vesting.unlocked_at::<T::BlockNumberToBalance>(now);
        let vested = Self::vested(&who).unwrap_or_else(T::Balance::zero);
        let to_vest = unlocked_now.saturating_sub(vested);

        if to_vest > T::Balance::zero() {
            T::Currency::transfer(
                &Self::account_id(),
                &who,
                to_vest,
                ExistenceRequirement::KeepAlive,
            )?;

            if unlocked_now == vesting.locked {
                Vesting::<T, I>::remove(&who);
                Vested::<T, I>::remove(&who);
                AccountRefCounter::<T>::dec_ref(&who);
                Self::deposit_event(Event::<T, I>::VestingCompleted(who));
            } else {
                Vested::<T, I>::insert(&who, unlocked_now);
                let locked_now = vesting.locked.saturating_sub(unlocked_now);
                Self::deposit_event(Event::<T, I>::VestingUpdated(who, locked_now));
            }
        };
        Ok(().into())
    }
}

impl<T: Config<I>, I: 'static> EqVestingSchedule<T::Balance, T::AccountId> for Pallet<T, I>
where
    T::Balance: MaybeSerializeDeserialize + Debug,
{
    type Moment = T::BlockNumber;

    /// Vesting amount rest if user called vest now.
    fn vesting_balance(who: &T::AccountId) -> Option<T::Balance> {
        if let Some(v) = Self::vesting(who) {
            let now = <frame_system::Pallet<T>>::block_number();
            let locked_now = v.locked_at::<T::BlockNumberToBalance>(now);
            Some(locked_now)
        } else {
            None
        }
    }

    /// Adds a vesting schedule to a given account.
    ///
    /// If there already exists a vesting schedule for the given account, an `Err` is returned
    /// and nothing is updated.
    ///
    /// On success, a linearly reducing amount of funds will be locked. In order to realise any
    /// reduction of the lock over time as it diminishes, the account owner must use `vest` or
    /// `vest_other`.
    ///
    /// Is a no-op if the amount to be vested is zero.
    fn add_vesting_schedule(
        who: &T::AccountId,
        locked: T::Balance,
        per_block: T::Balance,
        starting_block: T::BlockNumber,
    ) -> DispatchResult {
        if locked.is_zero() {
            return Ok(());
        }
        if Vesting::<T, I>::contains_key(who) {
            Err({
                log::error!(
                    "{}:{}. An existing vesting schedule already exists for account. Who: {:?}.",
                    file!(),
                    line!(),
                    who
                );
                Error::<T, I>::ExistingVestingSchedule
            })?
        }
        let vesting_schedule = VestingInfo {
            locked,
            per_block,
            starting_block,
        };
        Vesting::<T, I>::insert(who, vesting_schedule);
        AccountRefCounter::<T>::inc_ref(&who);
        // it can't fail, but even if somehow it did, we don't really care.
        let _ = Self::update_lock(who.clone());
        Ok(())
    }

    /// Updates an existings vesting schedule for a given account.
    fn update_vesting_schedule(
        who: &T::AccountId,
        locked: T::Balance,
        duration_blocks: T::Balance,
    ) -> DispatchResult {
        if locked.is_zero() {
            return Ok(());
        }

        let mut vesting = ok_or_error!(
            Self::vesting(who),
            Error::<T, I>::NotVesting,
            "{}:{}. The account is not vesting. Who: {:?}.",
            file!(),
            line!(),
            who
        )?;

        vesting.locked = vesting.locked.saturating_add(locked);
        vesting.per_block = vesting
            .locked
            .checked_div(&duration_blocks)
            .ok_or(ArithmeticError::DivisionByZero)?;

        Vesting::<T, I>::insert(who, vesting);
        // it can't fail, but even if somehow it did, we don't really care.
        let _ = Self::update_lock(who.clone());

        Ok(())
    }
}

impl<T: Config<I>, I: 'static> eq_primitives::Vesting<T::AccountId> for Pallet<T, I> {
    fn update_vest_lock(who: T::AccountId) -> DispatchResultWithPostInfo {
        Self::update_lock(who)
    }

    fn has_vesting_schedule(who: T::AccountId) -> bool {
        Self::vesting(&who).is_some()
    }
}
