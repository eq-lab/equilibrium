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

use eq_primitives::balance::EqCurrency;
use eq_primitives::{
    asset,
    offchain_batcher::{OffchainErr, OffchainResult},
    TransferReason, Vesting,
};
use eq_utils::{eq_ensure, ok_or_error};
use frame_support::codec::{Decode, Encode};
use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    traits::{ExistenceRequirement, Get, UnixTime},
};
use frame_system::offchain::SubmitTransaction;
use sp_application_crypto::RuntimeAppPublic;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{DispatchError, RuntimeDebug};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod benchmarking;
pub mod weights;
pub use weights::WeightInfo;

pub type AuthIndex = u32;
/// Request data for offchain signing
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct OperationRequest<AccountId, BlockNumber>
where
    AccountId: PartialEq + Eq + Decode + Encode,
    BlockNumber: Decode + Encode,
{
    /// Block number at the time heartbeat is created
    pub account: AccountId,
    /// An index of the authority on the list of validators
    pub authority_index: AuthIndex,
    /// The length of session validator set
    pub validators_len: u32,
    /// Number of a block
    pub block_num: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
    use core::convert::TryInto;
    use eq_primitives::{offchain_batcher::ValidatorOffchainBatcher, Vesting};
    use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*, PalletId};
    use frame_system::{offchain::SendTransactionTypes, pallet_prelude::*};
    use sp_application_crypto::RuntimeAppPublic;

    use crate::{OperationRequest, WeightInfo};

    const DB_PREFIX: &[u8] = b"eq-lockdrop/";

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config:
        frame_system::Config + SendTransactionTypes<Call<Self>> + eq_rate::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Pallet's AccountId for balances
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        /// Minimum amount to lock
        #[pallet::constant]
        type MinLockAmount: Get<Self::Balance>;
        /// Period of lock program in seconds
        #[pallet::constant]
        type LockPeriod: Get<u64>;
        /// Used to update accounts locks
        type Vesting: Vesting<Self::AccountId>;
        /// Used to execute batch operations for every `AuthorityId` key in keys storage
        type ValidatorOffchainBatcher: ValidatorOffchainBatcher<
            Self::AuthorityId,
            Self::BlockNumber,
            Self::AccountId,
        >;
        /// Used for calculation unsigned transaction priority
        #[pallet::constant]
        type LockDropUnsignedPriority: Get<TransactionPriority>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// This is the max amount of unlocks an offchain worker can make
    #[pallet::storage]
    #[pallet::getter(fn offchain_unlocks)]
    pub type MaxOffchainUnlocks<T: Config> = StorageValue<_, u64>;

    /// Pallet storage - start of lock program.
    /// Value is UnixTime timestamp in seconds
    #[pallet::storage]
    #[pallet::getter(fn lock_start)]
    pub type LockStart<T: Config> = StorageValue<_, u64>;

    /// Pallet storage - accounts locks
    #[pallet::storage]
    #[pallet::getter(fn locks)]
    pub type Locks<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

    #[pallet::type_value]
    pub fn DefaultForAutoUnlockEnabled() -> bool {
        true
    }

    /// Stores flag for on/off setting for offchain worker (unlocks)
    #[pallet::storage]
    #[pallet::getter(fn auto_unlock_enabled)]
    pub type AutoUnlockEnabled<T: Config> =
        StorageValue<_, bool, ValueQuery, DefaultForAutoUnlockEnabled>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// User `who` locks `amount` of Eq
        /// \[who, amount\]
        Lock(T::AccountId, T::Balance),
        /// User `who` unlocks `amount` of Eq
        /// \[who, amount\]
        Unlock(T::AccountId, T::Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Not allowed to make lock not in the period of the lock program
        OutOfLockPeriod,
        /// Not allowed to make unlock in the period of the lock program
        LockPeriodNotOver,
        /// Lock start is already initialized
        LockStartNotEmpty,
        /// Not allowed to make multiple locks if account has vesting schedule
        MultipleTransferWithVesting,
        /// Lock amount is lower than minimum allowed
        LockAmountLow,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Runs every block import
        fn offchain_worker(block: T::BlockNumber) {
            // SMAR-593::5
            if !Self::is_lock_over() {
                return;
            }

            let worker_enabled = Self::auto_unlock_enabled();

            if !worker_enabled {
                log::trace!(target: "eq_lockdrop", "Lockdrop offchain worker(block {:?}) is disabled", block);
                return;
            }

            // Only send messages if we are a potential validator.
            if sp_io::offchain::is_validator() && worker_enabled {
                let lock_res = eq_utils::offchain::accure_lock(DB_PREFIX, || {
                    #[allow(unused_must_use)]
                    {
                        T::ValidatorOffchainBatcher::execute_batch(
                            block,
                            Self::unlocks_for_single_auth,
                            "eq-lockdrop",
                        );
                    }
                });

                match lock_res {
                    eq_utils::offchain::LockedExecResult::Executed => {
                        log::trace!(target: "eq_lockdrop", "lockdrop offchain_worker:executed");
                    }
                    eq_utils::offchain::LockedExecResult::Locked => {
                        log::trace!(target: "eq_lockdrop", "lockdrop offchain_worker:locked");
                    }
                }
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Lock `amount` of Eq for lock
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::lock())]
        pub fn lock(origin: OriginFor<T>, amount: T::Balance) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::do_lock(who, amount)?;

            Ok(().into())
        }

        /// Unlock all account's locked Eq
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::unlock_external())]
        pub fn unlock_external(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let unlocked = Self::do_unlock(&who)?;

            Self::deposit_event(Event::Unlock(who, unlocked));

            Ok(().into())
        }

        /// Unlock all account's locked Eq
        /// The dispatch origin for this call must be _None_ (unsigned transaction).
        #[pallet::call_index(2)]
        #[pallet::weight((<T as Config>::WeightInfo::unlock() + <T as Config>::WeightInfo::validate_unsigned(),
            DispatchClass::Operational))]
        pub fn unlock(
            origin: OriginFor<T>,
            request: OperationRequest<T::AccountId, T::BlockNumber>,
            // since signature verification is done in `validate_unsigned`
            // we can skip doing it here again.
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            log::trace!(
                target: "eq_lockdrop",
                "Unlock for account '{:?}' by offchain worker",
                request.account,
            );

            let unlocked = Self::do_unlock(&request.account.clone())?;

            Self::deposit_event(Event::Unlock(request.account, unlocked));

            Ok(().into())
        }

        /// Set `Lock
        /// Start` in `timestamp`
        /// - timestamp: UnixTime timestamp in seconds
        /// WARNING! Check twice before using it!
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::set_lock_start())]
        pub fn set_lock_start(origin: OriginFor<T>, timestamp: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            Self::do_set_lock_start(timestamp)?;

            Ok(().into())
        }

        /// Clear `LockStart` value
        /// WARNING! Check twice before using it!
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::clear_lock_start())]
        pub fn clear_lock_start(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            Self::do_clear_lock_start();

            Ok(().into())
        }

        /// Enables or disables offchain worker. `true` to enable offchain worker
        /// operations, `false` to disable.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::set_auto_unlock())]
        pub fn set_auto_unlock(origin: OriginFor<T>, enabled: bool) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            <AutoUnlockEnabled<T>>::put(enabled);

            Ok(().into())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            const INVALID_VALIDATORS_LEN: u8 = 10;
            match call {
                Call::unlock { request, signature } => {
                    // verify that the incoming (unverified) pubkey is actually an authority id
                    let keys = eq_rate::Keys::<T>::get();
                    if keys.len() as u32 != request.validators_len {
                        return InvalidTransaction::Custom(INVALID_VALIDATORS_LEN).into();
                    }

                    let authority_id = match keys.get(request.authority_index as usize) {
                        Some(id) => id,
                        None => return InvalidTransaction::BadProof.into(),
                    };

                    // check signature (this is expensive so we do it last).
                    let signature_valid = request
                        .using_encoded(|encoded_req| authority_id.verify(&encoded_req, &signature));
                    if !signature_valid {
                        return InvalidTransaction::BadProof.into();
                    }

                    let priority = T::LockDropUnsignedPriority::get();

                    ValidTransaction::with_tag_prefix("Lkdrp")
                        .priority(priority)
                        .and_provides(request.account.clone())
                        .longevity(64)
                        .propagate(true)
                        .build()
                }
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub locks: Vec<(T::AccountId, T::Balance)>,
        pub lock_start: u64,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                locks: Default::default(),
                lock_start: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            use eq_primitives::balance::EqCurrency;
            use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};
            use sp_runtime::traits::Zero;

            EqPalletAccountInitializer::<T>::initialize(&Pallet::<T>::get_account_id());
            let mut total = T::Balance::zero();
            for (who, lock) in &self.locks {
                <Locks<T>>::mutate(who, |amount| {
                    *amount = *amount + *lock;
                });

                total = total + *lock;
            }

            T::EqCurrency::deposit_creating(
                &Pallet::<T>::get_account_id(),
                eq_primitives::asset::EQ,
                total,
                false,
                None,
            )
            .expect("Deposit must not fail");

            <LockStart<T>>::put(self.lock_start);
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Inner function that returns Lockdrop pallet's account id
    fn get_account_id() -> T::AccountId {
        T::PalletId::get().into_account_truncating()
    }

    /// Inner function that calculates the end of lock program
    fn lock_end() -> Option<u64> {
        if let Some(lock_start) = Self::lock_start() {
            Some(lock_start + T::LockPeriod::get())
        } else {
            None
        }
    }

    /// Inner function that checks that lock is active for now
    fn is_lock_period() -> bool {
        let lock_start: Option<u64> = Self::lock_start();

        // SMAR-593::1
        match lock_start {
            None => return true,
            _ => {}
        }

        let now = <eq_rate::Pallet<T>>::now().as_secs();

        match lock_start.map(|start| now <= start) {
            Some(is_period) => is_period,
            None => false,
        }
    }

    fn is_lock_over() -> bool {
        let lock_end = Self::lock_end();
        let now = <eq_rate::Pallet<T>>::now().as_secs();

        match lock_end {
            Some(end) => now >= end,
            None => false,
        }
    }

    /// Inner function that checks that LockStart is initialized
    fn lock_start_inited() -> bool {
        Self::lock_start().is_some()
    }

    fn has_lock(who: &T::AccountId) -> bool {
        Self::locks(&who) != T::Balance::default()
    }

    fn has_vesting(who: T::AccountId) -> bool {
        T::Vesting::has_vesting_schedule(who)
    }

    /// Inner function that does transfer and locks user's Eq
    fn do_lock(who: T::AccountId, amount_to_lock: T::Balance) -> DispatchResultWithPostInfo {
        eq_ensure!(
            Self::is_lock_period(),
            Error::<T>::OutOfLockPeriod,
            target: "eq_lockdrop",
            "{}:{}. Not allowed to lock. Who: {:?}, amount: {:?}, start: {:?}, end: {:?}",
            file!(),
            line!(),
            who,
            amount_to_lock,
            Self::lock_start(),
            Self::lock_end(),
        );

        eq_ensure!(
            !Self::has_vesting(who.clone()) || !Self::has_lock(&who),
            Error::<T>::MultipleTransferWithVesting,
            target: "eq_lockdrop",
            "{}:{}. Attempted to create more than one lock with vesting. Who: {:?}",
            file!(),
            line!(),
            who,
        );

        let min_lock_amount = T::MinLockAmount::get();
        eq_ensure!(
            amount_to_lock >= min_lock_amount,
            Error::<T>::LockAmountLow,
            target: "eq_lockdrop",
            "{}:{}. Attempted to lock less than minimum allowed: {:?}. Who: {:?}",
            file!(),
            line!(),
            amount_to_lock,
            who
        );

        if Self::has_vesting(who.clone()) {
            T::Vesting::update_vest_lock(who.clone())?;
        }

        T::EqCurrency::currency_transfer(
            &who,
            &Self::get_account_id(),
            asset::EQ,
            amount_to_lock,
            ExistenceRequirement::AllowDeath,
            TransferReason::Lock,
            true,
        )?;

        <Locks<T>>::mutate(&who, |amount| {
            *amount = *amount + amount_to_lock;
        });

        Self::deposit_event(Event::Lock(who, amount_to_lock));

        Ok(().into())
    }

    fn do_unlock(who: &T::AccountId) -> sp_std::result::Result<T::Balance, DispatchError> {
        eq_ensure!(
            Self::is_lock_over(),
            Error::<T>::LockPeriodNotOver,
            target: "eq_lockdrop",
            "{}:{}. Not allowed to unlock. Who: {:?}",
            file!(),
            line!(),
            who,
        );

        // get amount
        let locked = Self::locks(who);

        // transfer
        T::EqCurrency::currency_transfer(
            &Self::get_account_id(),
            &who,
            asset::EQ,
            locked,
            ExistenceRequirement::AllowDeath,
            TransferReason::Unlock,
            true,
        )?;

        <Locks<T>>::remove(who);

        Ok(locked)
    }

    /// Inner function that sets storage field LockStart
    /// - timestamp: unix time in seconds
    /// WARNING! Check twice before using it!
    fn do_set_lock_start(timestamp: u64) -> DispatchResultWithPostInfo {
        eq_ensure!(
            !Self::lock_start_inited(),
            Error::<T>::LockStartNotEmpty,
            target: "eq_lockdrop",
            "{}:{}. Not allowed to set lock start because it's inited. timestamp: {:?}.",
            file!(),
            line!(),
            timestamp
        );

        <LockStart<T>>::put(timestamp);

        Ok(().into())
    }

    /// Inner function that clears storage field LockStart
    /// WARNING! Check twice before using it!
    fn do_clear_lock_start() {
        <LockStart<T>>::kill();
    }

    fn unlocks_for_single_auth(
        authority_index: u32,
        key: T::AuthorityId,
        block_number: T::BlockNumber,
        validators_len: u32,
    ) -> OffchainResult<()> {
        if Self::is_lock_over() {
            let offchain_unlocks = Self::offchain_unlocks();

            for (_, (who, _)) in <Locks<T>>::iter()
                .enumerate()
                .filter(|(index, _)| (*index as u32) % validators_len == authority_index)
                .take(offchain_unlocks.unwrap_or_else(|| u64::MAX) as usize)
            {
                let unlock_data = OperationRequest::<T::AccountId, T::BlockNumber> {
                    account: who.clone(),
                    authority_index,
                    validators_len,
                    block_num: block_number,
                };

                let option_signature = key.sign(&unlock_data.encode());
                let signature = ok_or_error!(
                    option_signature,
                    OffchainErr::FailedSigning,
                    "{}:{}. Couldn't sign. Key: {:?}, request account: {:?}, authority_index: {:?}, \
                    validators_len: {:?}, block_num:{:?}.",
                    file!(),
                    line!(),
                    key,
                    &unlock_data.account,
                    &unlock_data.authority_index,
                    &unlock_data.validators_len,
                    &unlock_data.block_num
                )?;
                let acc = unlock_data.account.clone();
                let index = unlock_data.authority_index.clone();
                let len = unlock_data.validators_len.clone();
                let block = unlock_data.block_num.clone();
                let sign = signature.clone();
                let call = Call::unlock {
                    request: unlock_data,
                    signature,
                };

                SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(
                    |_| {
                        log::error!(
                            "{}:{}. Submit unlock error. Signature: {:?}, request account: {:?}, \
                            authority_index: {:?}, validators_len: {:?}, block_num:{:?}.",
                            file!(),
                            line!(),
                            sign,
                            acc,
                            index,
                            len,
                            block
                        );
                        OffchainErr::SubmitTransaction
                    },
                )?;
            }
        }

        Ok(())
    }
}