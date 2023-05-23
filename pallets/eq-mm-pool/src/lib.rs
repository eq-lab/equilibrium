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

//! # Equilibrium Market Maker Pools Pallet
//!
//! Substrate module that provides
//! the functionality of Market Maker via `crate_order` and
//! management of Market Makers accounts.
//!
//! Also provides methods `deposit` and `request_withdraw` for lenders
//! to operate with funds on a given MM pool.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

pub mod benchmarking;
pub mod migration;
mod mock;
mod tests;
pub mod weights;

use codec::{Decode, Encode, MaxEncodedLen};
use core::convert::TryInto;
use eq_dex::WeightInfo as _;
use eq_primitives::{
    asset::{Asset, AssetGetter},
    balance::EqCurrency,
    dex::{DeleteOrderReason, OrderManagement},
    subaccount::{SubAccType, SubaccountsManager},
    Aggregates, EqPalletAccountInitializer, OrderId, OrderSide, OrderType,
    PalletAccountInitializer, TransferReason, UserGroup,
};
use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    ensure,
    traits::{ExistenceRequirement, Get as _, UnixTime},
    weights::Weight,
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_io::hashing::blake2_256;
use sp_runtime::{
    traits::AtLeast32BitUnsigned, traits::Zero as _, DispatchError, DispatchResult, FixedI64,
    Perbill,
};
use sp_std::prelude::*;
pub use weights::WeightInfo;

pub use pallet::*;

/// In seconds
pub type Timestamp = u64;
pub type EpochCounter = u64;
pub type Balance = eq_primitives::balance::Balance;

#[derive(
    Encode, Decode, Clone, PartialEq, Eq, Debug, Default, MaxEncodedLen, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EpochInfo {
    // Current epoch number
    pub counter: EpochCounter,
    // Start timestamp of epoch
    pub started_at: Timestamp,
    // Duration of current epoch
    pub duration: Timestamp,
    // Maybe new duration for next epoch
    pub new_duration: Option<Timestamp>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, MaxEncodedLen, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct MmPoolInfo<AccountId: MaxEncodedLen, Balance: MaxEncodedLen> {
    // Account where pallet stores tokens
    pub account_id: AccountId,
    // Min deposit balance
    pub min_amount: Balance,
    // Total amount of deposited funds
    // = total_deposit + total_borrowed
    pub total_staked: Balance,
    // Current balance of pool.account_id
    pub total_deposit: Balance,
    // Borrowed by mms
    pub total_borrowed: Balance,
    // Pending withdrawals of all lenders
    // <= total_deposit (could be broked, see int-tests-js mmBorrowerDebt)
    pub total_pending_withdrawals: PendingWithdrawal<Balance>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, MaxEncodedLen, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LenderInfo<Balance: MaxEncodedLen> {
    // Amount of deposited funds by current lender
    pub deposit: Balance,
    // Pending withdrawals of current lender
    pub pending_withdrawals: PendingWithdrawal<Balance>,
}

pub type MmId = u16;

#[derive(
    Encode, Decode, Clone, PartialEq, Eq, Debug, Default, MaxEncodedLen, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct MmInfo<Balance> {
    // Max allocation of current Mm from a pool
    pub weight: Perbill,
    // Total borrowed by this Mm
    pub borrowed: Balance,
}

#[derive(
    Encode, Decode, Clone, PartialEq, Eq, Debug, Default, MaxEncodedLen, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PendingWithdrawal<Balance: MaxEncodedLen> {
    // last traked epoch number
    pub last_epoch: EpochCounter,
    // amount that is already awailable to withdraw
    pub available: Balance,
    // amount requested to withdraw after 1 epochs
    pub available_next_epoch: Balance,
    // amount requested to withdraw after 2 epochs
    pub requested: Balance,
}

impl<Balance: sp_std::ops::AddAssign + Default + Clone + MaxEncodedLen> PendingWithdrawal<Balance> {
    pub fn default<T: Config>() -> Self {
        Self {
            last_epoch: Pallet::<T>::epoch().counter,
            available: Balance::default(),
            available_next_epoch: Balance::default(),
            requested: Balance::default(),
        }
    }

    /// Advance epoch counter and
    /// swap requested balances
    /// available <- available_next_epoch <- requested
    pub fn advance_epoch(&mut self, new_epoch: EpochCounter) {
        match new_epoch - self.last_epoch {
            0 => {}
            1 => {
                let available_next_epoch = sp_std::mem::take(&mut self.requested);
                self.available +=
                    sp_std::mem::replace(&mut self.available_next_epoch, available_next_epoch);
            }
            _ => {
                self.available += sp_std::mem::take(&mut self.requested);
                self.available += sp_std::mem::take(&mut self.available_next_epoch);
            }
        }

        self.last_epoch = new_epoch;
    }

    /// Total peding balance
    pub fn total(&self) -> Balance {
        let mut total = self.available.clone();
        total += self.available_next_epoch.clone();
        total += self.requested.clone();
        total
    }
}

pub(crate) const MINIMAL_DURATION: u64 = 24 * 60 * 60; // one day

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use codec::Codec;
    use eq_primitives::balance_number::EqFixedU128;
    use eq_primitives::Aggregates;
    use eq_utils::eq_ensure;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    // #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Pallet's AccountId for MM
        #[pallet::constant]
        type ModuleId: Get<frame_support::PalletId>;

        type MarketMakersManagementOrigin: EnsureOrigin<Self::Origin>;
        /// Numerical representation of stored balances
        type Balance: Member
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Codec
            + Copy
            + Parameter
            + Default
            + MaxEncodedLen
            + From<Balance>
            + Into<Balance>;
        /// Used for currency-related operations and calculations
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        /// Weight information for extrinsics of eqDex.
        type DexWeightInfo: eq_dex::WeightInfo;
        /// Used to work with `AccountUserGroups`
        type Aggregates: Aggregates<Self::AccountId, Self::Balance>;
        /// Used to create Borrower subaccount on genesis for MM
        type SubaccountsManager: SubaccountsManager<Self::AccountId>;
        /// Used to operate on Dex
        type OrderManagement: OrderManagement<AccountId = Self::AccountId>;
        /// Used to create pool for Asset
        type AssetGetter: AssetGetter;
        /// Timestamp provider
        type UnixTime: UnixTime;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create pool for a given currency
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_pool())]
        pub fn create_pool(
            origin: OriginFor<T>,
            currency: Asset,
            min_amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            T::MarketMakersManagementOrigin::ensure_origin(origin)?;
            let _ = T::AssetGetter::get_asset_data(&currency)?;

            let mut pools = Pools::<T>::get();
            let new_pos = match Self::search_by_key(&mut pools, &currency) {
                Ok(_) => return Err(Error::<T>::PoolAlreadyExists.into()),
                Err(new_pos) => new_pos,
            };

            let account_id = Self::generate_pool_acc(currency)?;
            EqPalletAccountInitializer::<T>::initialize(&account_id);

            let new_pool = MmPoolInfo {
                account_id: account_id.clone(),
                min_amount,
                total_staked: T::Balance::zero(),
                total_deposit: T::Balance::zero(),
                total_borrowed: T::Balance::zero(),
                total_pending_withdrawals: PendingWithdrawal::default::<T>(),
            };
            pools.insert(new_pos, (currency, new_pool));
            Pools::<T>::put(pools);

            Self::deposit_event(Event::PoolCreated(account_id, currency, min_amount));
            Ok(().into())
        }

        /// Change minimal deposit for pool
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::change_min_amount())]
        pub fn change_min_amount(
            origin: OriginFor<T>,
            currency: Asset,
            min_amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            T::MarketMakersManagementOrigin::ensure_origin(origin)?;

            let mut pools = Pools::<T>::get();
            let current_pool = Self::search_by_key(&mut pools, &currency)
                .map_err(|_| Error::<T>::NoPoolWithCurrency)?;

            current_pool.min_amount = min_amount;

            Pools::<T>::put(pools);

            Self::deposit_event(Event::PoolChanged(currency, min_amount));
            Ok(().into())
        }

        /// Set new duration for epoch
        /// Duration will change at the next epoch
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::set_epoch_duration())]
        pub fn set_epoch_duration(
            origin: OriginFor<T>,
            epoch_duration: Timestamp,
        ) -> DispatchResultWithPostInfo {
            T::MarketMakersManagementOrigin::ensure_origin(origin)?;

            eq_ensure!(
                epoch_duration >= MINIMAL_DURATION,
                Error::<T>::WrongNewDuration,
                "{}:{}. New duration {:?} less than minimal value {:?}.",
                file!(),
                line!(),
                epoch_duration,
                MINIMAL_DURATION
            );

            let mut epoch = Epoch::<T>::get();
            let counter = epoch.counter;
            let duration = epoch.duration;
            epoch.new_duration = Some(epoch_duration);
            Epoch::<T>::put(epoch.clone());

            Self::deposit_event(Event::NewEpochDuration(counter + 1, duration));
            Ok(().into())
        }

        /// <dev>
        /// Set new duration for current epoch and restart timer
        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn force_set_epoch(
            origin: OriginFor<T>,
            epoch_duration: Timestamp,
        ) -> DispatchResultWithPostInfo {
            T::MarketMakersManagementOrigin::ensure_origin(origin)?;

            let mut epoch = Epoch::<T>::get();
            epoch.started_at = Self::get_timestamp();
            epoch.duration = epoch_duration;
            Epoch::<T>::put(epoch.clone());

            Ok(().into())
        }

        /// Add `manager` to managers list of a market maker with a given `mm_id`
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::add_manager())]
        pub fn add_manager(
            origin: OriginFor<T>,
            manager: T::AccountId,
            mm_id: MmId,
        ) -> DispatchResultWithPostInfo {
            T::MarketMakersManagementOrigin::ensure_origin(origin)?;

            ensure!(
                Managers::<T>::get(&manager).is_none(),
                Error::<T>::BorrowerAlreadyExists
            );

            let trading_acc = Self::generate_trade_acc(mm_id, &manager)?;
            Managers::<T>::insert(&manager, (mm_id, &trading_acc));

            Self::deposit_event(Event::BorrowerAdded(manager, mm_id, trading_acc));
            Ok(().into())
        }

        // TODO: more complicated, than just remove entry from storage
        // delete dex orders, return deposits
        // #[pallet::weight(T::WeightInfo::remove_manager())]
        // pub fn remove_manager(
        //     origin: OriginFor<T>,
        //     manager: T::AccountId,
        // ) -> DispatchResultWithPostInfo {
        //     T::MarketMakersManagementOrigin::ensure_origin(origin)?;
        //     Self::deposit_event(Event::BorrowerRemoved(manager));
        //     Ok(().into())
        // }

        /// Allows Mm to borrow (not more than weight) from specified pools
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::set_allocations(weight_per_asset.len() as u32))]
        pub fn set_allocations(
            origin: OriginFor<T>,
            mm_id: MmId,
            weight_per_asset: Vec<(Asset, Perbill)>,
        ) -> DispatchResultWithPostInfo {
            T::MarketMakersManagementOrigin::ensure_origin(origin)?;

            let mut mm = MarketMakers::<T>::get(&mm_id);
            for (currency, weight) in weight_per_asset {
                let _ = T::AssetGetter::get_asset_data(&currency)?;

                match Self::search_by_key(&mut mm, &currency) {
                    Ok(mm_info) => mm_info.weight = weight,
                    Err(pos) => {
                        let new_mm_info = MmInfo {
                            weight,
                            borrowed: T::Balance::zero(),
                        };
                        mm.insert(pos, (currency, new_mm_info));
                    }
                }
            }
            MarketMakers::<T>::insert(&mm_id, mm.clone());

            let mm = mm
                .into_iter()
                .map(|(currency, mm_info)| (currency, mm_info.weight))
                .collect();
            Self::deposit_event(Event::<T>::NewAllocations(mm_id, mm));
            Ok(().into())
        }

        /// Manager's function to borrow funds from pool to use them in DEX
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::borrow())]
        pub fn borrow(
            origin: OriginFor<T>,
            amount: T::Balance,
            currency: Asset,
        ) -> DispatchResultWithPostInfo {
            let manager = ensure_signed(origin)?;
            Self::do_borrow(manager, currency, amount)?;
            Ok(Pays::No.into())
        }

        /// Manager's function to return funds to pool
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::repay())]
        pub fn repay(
            origin: OriginFor<T>,
            amount: T::Balance,
            currency: Asset,
        ) -> DispatchResultWithPostInfo {
            let manager = ensure_signed(origin)?;
            Self::do_repay(manager, currency, amount)?;
            Ok(Pays::No.into())
        }

        /// User's function to deposit funds to pool
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::deposit())]
        pub fn deposit(
            origin: OriginFor<T>,
            amount: T::Balance,
            currency: Asset,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_deposit(who, currency, amount)?;
            Ok(().into())
        }

        /// User's function to request deposited funds back
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::request_withdrawal())]
        pub fn request_withdrawal(
            origin: OriginFor<T>,
            amount: T::Balance,
            currency: Asset,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_request_withdrawal(who, currency, amount)?;
            Ok(().into())
        }

        /// User's function to withdraw funds from pool 2 epochs after request
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::withdraw())]
        pub fn withdraw(origin: OriginFor<T>, currency: Asset) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_withdraw(who, currency)?;
            Ok(().into())
        }

        /// Manager's function to create orders via trading account
        #[pallet::call_index(11)]
        #[pallet::weight(
            <T as pallet::Config>::DexWeightInfo::create_limit_order()
                .max(<T as pallet::Config>::DexWeightInfo::create_market_order())
                .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(2))
        )]
        pub fn create_order(
            origin: OriginFor<T>,
            currency: Asset,
            order_type: OrderType,
            side: OrderSide,
            amount: EqFixedU128,
        ) -> DispatchResultWithPostInfo {
            let manager = ensure_signed(origin)?;

            let (_, trader_acc) =
                Managers::<T>::get(&manager).ok_or(Error::<T>::BorrowerDoesNotExist)?;

            T::OrderManagement::create_order(trader_acc, currency, order_type, side, amount)?;
            Ok(Pays::No.into())
        }

        /// Delete order. This must be called by order owner or root.
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::DexWeightInfo::delete_order_external())]
        pub fn delete_order(
            origin: OriginFor<T>,
            currency: Asset,
            order_id: OrderId,
            price: FixedI64,
        ) -> DispatchResultWithPostInfo {
            let manager = ensure_signed(origin)?;

            let (_, _) = Managers::<T>::get(&manager).ok_or(Error::<T>::BorrowerDoesNotExist)?;

            T::OrderManagement::delete_order(
                &currency,
                order_id,
                price,
                DeleteOrderReason::Cancel,
            )?;
            Ok(Pays::No.into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
            let mut epoch = Epoch::<T>::get();

            let next_epoch_timestamp = epoch.started_at + epoch.duration;
            let global_advance_epoch_weight = if Self::get_timestamp() >= next_epoch_timestamp {
                epoch.started_at = next_epoch_timestamp;
                Self::global_advance_epoch(epoch)
            } else {
                Weight::zero()
            };

            T::DbWeight::get()
                .reads(2)
                .saturating_add(global_advance_epoch_weight)
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New pool is created
        /// [pool_account_id, currency, min_amount]
        PoolCreated(T::AccountId, Asset, T::Balance),
        /// Existing pool is changed
        /// [pool_account_id, currency, min_amount]
        PoolChanged(Asset, T::Balance),
        /// Next epoch's duration
        /// [next_epoch_counter, next_epoch_duration]
        NewEpochDuration(EpochCounter, Timestamp),
        /// New allocations for mm
        /// [mm_id, new_allocations]
        NewAllocations(MmId, Vec<(Asset, Perbill)>),
        /// New manager for market maker is added
        /// [manager_account_id, mm_id, trading_account_id]
        BorrowerAdded(T::AccountId, MmId, T::AccountId),
        /// Borrower for pool is removed
        /// [manager_account_id, currency]
        BorrowerRemoved(T::AccountId, Asset),
        /// New Borrowing from mm pool
        /// [manager, asset, borrowed_amount, mm_id, total_borrowed_amount]
        Borrow(T::AccountId, Asset, T::Balance, MmId, T::Balance),
        /// Borrower return borrowing to mm pool
        /// [manager, asset, returned_amount, mm_id, total_borrowed_amount]
        Repay(T::AccountId, Asset, T::Balance, MmId, T::Balance),
        /// New Deposit to mm pool
        /// [who, asset, deposit_amount, total_deposit_amount]
        Deposit(T::AccountId, Asset, T::Balance, T::Balance),
        /// Requested withdrawal from mm pool
        /// [who, asset, withdrawal_amount]
        WithdrawalRequest(T::AccountId, Asset, T::Balance),
        /// Completed withdrawal from mm pool
        /// [who, asset, withdrawal_amount]
        WithdrawalCompleted(T::AccountId, Asset, T::Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Trying to add new pool, where there is already pool with the same currency
        PoolAlreadyExists,
        /// There is no pool for this currency
        NoPoolWithCurrency,
        /// Trying to add manager account, which already is a manager
        BorrowerAlreadyExists,
        /// Trying to mutate not existing manager
        BorrowerDoesNotExist,
        /// Market maker cannot borrow from specified pool
        NoAllocation,
        /// Trying to borrow more than pool can afford
        NotEnoughToBorrow,
        /// Trying to borrow more than manager is allowed
        Overweight,
        /// Trying to repay with zero borrowed amount
        NoFundsToRepay,
        /// No deposits were made before
        NoDeposit,
        /// Amount is less than min
        AmountLessThanMin,
        /// Trying to withdraw more than existed deposit
        NotEnoughToWithdraw,
        /// Trying to withdraw without premature request
        WithdrawalNotRequested,
        /// Error while creating account, should never occur
        ExternalError,
        /// New duration less than minimal value
        WrongNewDuration,
    }

    #[pallet::storage]
    #[pallet::getter(fn epoch)]
    pub type Epoch<T: Config> = StorageValue<_, EpochInfo, ValueQuery>;

    /// Storage of all mm pools settings
    #[pallet::storage]
    #[pallet::getter(fn pools)]
    #[pallet::unbounded]
    pub type Pools<T: Config> =
        StorageValue<_, Vec<(Asset, MmPoolInfo<T::AccountId, T::Balance>)>, ValueQuery>;

    /// Storage of all mm pools settings
    #[pallet::storage]
    #[pallet::getter(fn deposits)]
    #[pallet::unbounded]
    pub type Deposits<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Vec<(Asset, LenderInfo<T::Balance>)>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn mm)]
    #[pallet::unbounded]
    pub type MarketMakers<T: Config> =
        StorageMap<_, Blake2_128Concat, MmId, Vec<(Asset, MmInfo<T::Balance>)>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn managers)]
    pub type Managers<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (MmId, T::AccountId), OptionQuery>;

    // TODO: remove storage
    #[pallet::storage]
    #[pallet::getter(fn manager)]
    pub type Manager<T: Config> = StorageValue<_, T::AccountId>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub epoch_duration: Timestamp,
        pub _runtime: PhantomData<T>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> sp_std::default::Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                epoch_duration: 604_800, // 1 week
                _runtime: PhantomData,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            let epoch = EpochInfo {
                counter: 0,
                started_at: Pallet::<T>::get_timestamp(),
                duration: self.epoch_duration,
                new_duration: None,
            };
            Epoch::<T>::put(epoch);
        }
    }
}

impl<T: Config> Pallet<T> {
    fn get_timestamp() -> Timestamp {
        T::UnixTime::now().as_secs()
    }

    fn generate_trade_acc(mm_id: MmId, manager: &T::AccountId) -> Result<T::AccountId, Error<T>> {
        let raw = (b"eq/mm-trader__", mm_id, manager).using_encoded(blake2_256);
        T::AccountId::decode(&mut &raw[..]).map_err(|_| Error::<T>::ExternalError)
    }

    /// Generates and returns `AccountId` using poolId and asset
    fn generate_pool_acc(asset: Asset) -> Result<T::AccountId, Error<T>> {
        let raw = (b"eq/mm-pool__", asset).using_encoded(blake2_256);
        T::AccountId::decode(&mut &raw[..]).map_err(|_| Error::<T>::ExternalError)
    }

    pub fn global_advance_epoch(mut epoch: EpochInfo) -> Weight {
        let new_epoch = epoch.counter + 1;
        epoch.counter = new_epoch;
        if let Some(new_epoch_duration) = epoch.new_duration {
            epoch.duration = new_epoch_duration;
            epoch.new_duration = None;
        }
        Epoch::<T>::put(epoch);

        let len = Pools::<T>::mutate(|pools| {
            for (_currency, pool_info) in pools.iter_mut() {
                pool_info.total_pending_withdrawals.advance_epoch(new_epoch);
            }
            pools.len()
        });

        T::WeightInfo::global_advance_epoch(len as u32)
    }

    /// Returns Ok(entry) by currency
    /// If there is no currency, returns Err(pos) where new entry could be inserted
    fn search_by_key<'a, K: Ord, V>(
        vec_map: &'a mut Vec<(K, V)>,
        key: &K,
    ) -> Result<&'a mut V, usize> {
        let found = vec_map.binary_search_by(|(a, _)| a.cmp(key))?;
        Ok(&mut vec_map[found].1)
    }

    pub fn get_subacc_creating(
        pool_account_id: &T::AccountId,
    ) -> Result<T::AccountId, DispatchError> {
        match T::SubaccountsManager::get_subaccount_id(pool_account_id, &SubAccType::Trader) {
            Some(subacc) => Ok(subacc),
            None => {
                return T::SubaccountsManager::create_subaccount_inner(
                    pool_account_id,
                    &SubAccType::Trader,
                )
                .map(|subacc| {
                    T::Aggregates::set_usergroup(&subacc, UserGroup::Borrowers, true)
                        .map(|_| subacc)
                })?;

                // return Ok(subacc);
            }
        }
    }

    fn do_borrow(manager: T::AccountId, currency: Asset, amount: T::Balance) -> DispatchResult {
        let mut pools = Pools::<T>::get();
        let current_pool = Self::search_by_key(&mut pools, &currency)
            .map_err(|_| Error::<T>::NoPoolWithCurrency)?;

        ensure!(
            current_pool.total_deposit
                >= current_pool.total_pending_withdrawals.available
                    + current_pool.total_pending_withdrawals.available_next_epoch
                    + amount,
            Error::<T>::NotEnoughToBorrow
        );

        let (mm_id, trading_acc) =
            Managers::<T>::get(&manager).ok_or(Error::<T>::BorrowerDoesNotExist)?;
        let mut mm = MarketMakers::<T>::get(&mm_id);
        let allocation =
            Self::search_by_key(&mut mm, &currency).map_err(|_| Error::<T>::NoAllocation)?;

        ensure!(
            allocation.borrowed + amount <= allocation.weight * current_pool.total_staked,
            Error::<T>::Overweight
        );

        let trading_subacc = Self::get_subacc_creating(&trading_acc)?;

        allocation.borrowed += amount;
        let borrowed = allocation.borrowed;
        T::EqCurrency::currency_transfer(
            &current_pool.account_id,
            &trading_subacc,
            currency,
            amount,
            ExistenceRequirement::AllowDeath,
            TransferReason::Common,
            true,
        )?;
        MarketMakers::<T>::insert(&mm_id, mm);

        current_pool.total_deposit -= amount;
        current_pool.total_borrowed += amount;
        Pools::<T>::put(pools);

        Self::deposit_event(Event::Borrow(manager, currency, amount, mm_id, borrowed));
        Ok(())
    }

    fn do_repay(manager: T::AccountId, currency: Asset, amount: T::Balance) -> DispatchResult {
        let mut pools = Pools::<T>::get();
        let current_pool = Self::search_by_key(&mut pools, &currency)
            .map_err(|_| Error::<T>::NoPoolWithCurrency)?;

        let (mm_id, trading_acc) =
            Managers::<T>::get(&manager).ok_or(Error::<T>::BorrowerDoesNotExist)?;

        let mut mm = MarketMakers::<T>::get(&mm_id);
        let allocation =
            Self::search_by_key(&mut mm, &currency).map_err(|_| Error::<T>::NoAllocation)?;

        ensure!(allocation.borrowed >= amount, Error::<T>::NoFundsToRepay);

        let trading_subacc = Self::get_subacc_creating(&trading_acc)?;

        allocation.borrowed -= amount;
        let borrowed = allocation.borrowed;
        T::EqCurrency::currency_transfer(
            &trading_subacc,
            &current_pool.account_id,
            currency,
            amount,
            ExistenceRequirement::AllowDeath,
            TransferReason::Common,
            true,
        )?;
        MarketMakers::<T>::insert(&mm_id, mm);

        current_pool.total_deposit += amount;
        current_pool.total_borrowed -= amount;
        Pools::<T>::put(pools);

        Self::deposit_event(Event::Repay(manager, currency, amount, mm_id, borrowed));
        Ok(())
    }

    fn do_deposit(who: T::AccountId, currency: Asset, amount: T::Balance) -> DispatchResult {
        let mut pools = Pools::<T>::get();
        let current_pool = Self::search_by_key(&mut pools, &currency)
            .map_err(|_| Error::<T>::NoPoolWithCurrency)?;

        ensure!(
            amount >= current_pool.min_amount,
            Error::<T>::AmountLessThanMin
        );

        let mut deposits = Deposits::<T>::get(&who);
        let new_amount = match Self::search_by_key(&mut deposits, &currency) {
            Err(new_pos) => {
                let new_lender = LenderInfo {
                    deposit: amount,
                    pending_withdrawals: PendingWithdrawal::default::<T>(),
                };
                deposits.insert(new_pos, (currency, new_lender));

                amount
            }
            Ok(lender) => {
                lender
                    .pending_withdrawals
                    .advance_epoch(Self::epoch().counter);
                lender.deposit += amount;
                lender.deposit
            }
        };

        T::EqCurrency::currency_transfer(
            &who,
            &current_pool.account_id,
            currency,
            amount,
            ExistenceRequirement::AllowDeath,
            TransferReason::Common,
            true,
        )?;
        Deposits::<T>::insert(&who, deposits);

        current_pool.total_deposit += amount;
        current_pool.total_staked += amount;
        Pools::<T>::put(pools);

        Self::deposit_event(Event::Deposit(who, currency, amount, new_amount));
        Ok(())
    }

    fn do_request_withdrawal(
        who: T::AccountId,
        currency: Asset,
        amount: T::Balance,
    ) -> DispatchResult {
        let mut pools = Pools::<T>::get();
        let current_pool = Self::search_by_key(&mut pools, &currency)
            .map_err(|_| Error::<T>::NoPoolWithCurrency)?;

        let mut deposits = Deposits::<T>::get(&who);
        let current_deposit =
            Self::search_by_key(&mut deposits, &currency).map_err(|_| Error::<T>::NoDeposit)?;

        current_deposit
            .pending_withdrawals
            .advance_epoch(Self::epoch().counter);

        ensure!(
            current_deposit.deposit >= current_deposit.pending_withdrawals.total() + amount,
            Error::<T>::NotEnoughToWithdraw
        );

        current_deposit.pending_withdrawals.requested += amount;
        // current_deposit.deposit -= amount;
        Deposits::<T>::insert(&who, deposits);

        current_pool.total_pending_withdrawals.requested += amount;
        // current_pool.total_deposit -= amount;
        Pools::<T>::put(pools);

        Self::deposit_event(Event::WithdrawalRequest(who, currency, amount));
        Ok(())
    }

    fn do_withdraw(who: T::AccountId, currency: Asset) -> DispatchResult {
        let mut pools = Pools::<T>::get();
        let current_pool = Self::search_by_key(&mut pools, &currency)
            .map_err(|_| Error::<T>::NoPoolWithCurrency)?;

        let mut deposits = Deposits::<T>::get(&who);
        let current_deposit =
            Self::search_by_key(&mut deposits, &currency).map_err(|_| Error::<T>::NoDeposit)?;

        current_deposit
            .pending_withdrawals
            .advance_epoch(Self::epoch().counter);

        let amount = current_deposit.pending_withdrawals.available;
        ensure!(!amount.is_zero(), Error::<T>::WithdrawalNotRequested);

        T::EqCurrency::currency_transfer(
            &current_pool.account_id,
            &who,
            currency,
            amount,
            ExistenceRequirement::AllowDeath,
            TransferReason::Common,
            true,
        )?;

        current_deposit.pending_withdrawals.available = T::Balance::zero();
        current_deposit.deposit -= amount;
        Deposits::<T>::insert(&who, deposits);

        current_pool.total_pending_withdrawals.available -= amount;
        current_pool.total_deposit -= amount;
        current_pool.total_staked -= amount;
        Pools::<T>::put(pools);

        Self::deposit_event(Event::WithdrawalCompleted(who, currency, amount));
        Ok(())
    }
}