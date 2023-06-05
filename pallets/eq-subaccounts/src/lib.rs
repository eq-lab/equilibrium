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

//! # Equilibrium Subaccounts Pallet
//!
//! All users in the system have one master account and 3 sub-accounts for different roles:
//! borrower / bailsman / lender.
//! From the blockchain perspective these are all full-fledged accounts with account Id
//! and there is a map of master accounts to sub-accounts in the storage.
//! Sub-accounts are created when users transfer assets from master accounts to sub-accounts.
//! By default there are no sub-accounts in the system, in order to create one,
//! one must select the sub-account type and make a transfer to it.
//! If there is no such sub-account it will be created.

//! Sub-accounts from technical perspective are full-fledged substrate accounts. There is a map master account -> sub-accounts.
//! When a sub-account is created this map is updated.
//! Cross-chain deposits / withdrawals work only to/from master accounts:
//! users deposit bitcoin through the bridge, and then transfers it from the master account to the corresponding sub-account.
//! (In testnet these are just issue/burn actions which modify balances and hold no economic value).
//! In the aggregates pallet, we respectively count aggregates for each role - borrower / bailsman / lender
//! as well as total aggregates (how many of each asset total in the system, how much is that in usd terms e.g. dollar value).

//! Balances:
//! Master account supports only positive balances (can’t go negative).
//! Borrower accounts may go negative e.g. borrow.
//! Lender accounts support only positive balances (can’t go negative).
//! Bailsmen accounts may go negative in balance when they receive liquidated debt (negative balance) from borrowers.

//! The very first deposit of funds to the master account automatically converts some amount of deposited funds
//! (depositEQ setting in the balance pallet the actual value is set in the runtime lib.rs) so that users can pay fees
//! and transact in the system.
//! Bailsman sub-account gets counted in bailsman aggregates if the sub-account’s balance becomes higher than a certain amount
//! (MinimalCollateral setting in the bailsman pallet). If the balance as a result of the transfer has become less than this amount,
//! the system removes the sub-account from aggregates.
//! Bailsman can not unregister (stop being a bailsman) while he has debt (negative balance).
//! ExistentialDeposit: Works collectively for the master account + borrower / bailsman / lender sub-accounts.
//! E.g. if total balance across all accounts is less than existential deposit, all of them are deleted collectively.
//! There is an offchain-worker which does this deletion automatically.
//! When accounts are deleted, all leftover balances are transferred to the account of bailsman pallet.

#![cfg_attr(not(feature = "std"), no_std)]
// #![deny(warnings)]

pub mod benchmarking;
mod mock;
mod tests;
pub mod weights;

use codec::Codec;
use core::convert::TryInto;
use eq_primitives::{
    asset::Asset,
    balance::{BalanceChecker, BalanceGetter, EqCurrency},
    str_asset,
    subaccount::{SubAccType, SubaccountsManager},
    Aggregates, BailsmanManager, IsTransfersEnabled, SignedBalance, TransferReason,
    UpdateTimeManager, UserGroup,
};
use eq_utils::{eq_ensure, ok_or_error};
use eq_whitelists::CheckWhitelisted;
use frame_support::{
    codec::{Decode, Encode},
    traits::{ExistenceRequirement, WithdrawReasons},
    weights::Weight,
    Parameter,
};
use frame_system::ensure_signed;
use sp_io::hashing::blake2_256;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, Member},
    DispatchError, DispatchResult,
};
use sp_std::{fmt::Debug, prelude::*};
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    use eq_primitives::asset::{Asset, AssetGetter};
    use eq_primitives::PriceGetter;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Numerical representation of stored balances
        type Balance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Codec
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + From<u128>
            + Into<u128>;
        /// Used to deal with Assets
        type AssetGetter: AssetGetter;
        /// Gets users balances
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;
        /// Used to work with `TotalAggregates` storing aggregated collateral and
        /// debt for user groups
        type Aggregates: Aggregates<Self::AccountId, Self::Balance>;
        /// Used for currency-related operations and calculations
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        /// Bailsman pallet integration for operations with bailsman subaccount
        type BailsmenManager: eq_primitives::BailsmanManager<Self::AccountId, Self::Balance>;
        /// Gets currency prices from oracle
        type PriceGetter: PriceGetter;
        /// Used for managing last update time in Equilibrium Rate pallet
        type UpdateTimeManager: eq_primitives::UpdateTimeManager<Self::AccountId>;
        /// Whitelist checking
        type Whitelist: eq_whitelists::CheckWhitelisted<Self::AccountId>;
        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
        /// Checks if transaction disabled flag is off
        type IsTransfersEnabled: eq_primitives::IsTransfersEnabled;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Transfers `value` amount of `currency` from main account to `subacc_type` subaccount
        #[pallet::call_index(0)]
        #[pallet::weight((Pallet::<T>::transfer_max_weight(subacc_type, true), DispatchClass::Normal))]
        pub fn transfer_to_subaccount(
            origin: OriginFor<T>,
            subacc_type: SubAccType,
            asset: Asset,
            value: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::ensure_transfers_enabled()?;

            let mut subaccount_created = false;
            let subacc_id = match <Subaccount<T>>::get(&who, &subacc_type) {
                Some(account_id) => account_id,
                None => {
                    subaccount_created = true;
                    let account_id = Self::create_subaccount_inner(&who, &subacc_type)?;
                    if subacc_type != SubAccType::Bailsman {
                        // bailsmen usergroup will be set after transfer if value is enough to become bailsman
                        Self::try_set_usergroup(&account_id, &subacc_type).err();
                    }
                    Self::deposit_event(Event::SubaccountCreated(
                        who.clone(),
                        account_id.clone(),
                        subacc_type,
                    ));
                    account_id
                }
            };

            let redistribute_amount =
                if T::Aggregates::in_usergroup(&subacc_id, UserGroup::Bailsmen) {
                    // if subaccount is bailsman we need to reinit or fail on transfer.
                    T::BailsmenManager::redistribute(&subacc_id)?
                } else {
                    0
                };

            T::EqCurrency::currency_transfer(
                &who,
                &subacc_id,
                asset,
                value,
                ExistenceRequirement::AllowDeath,
                TransferReason::Subaccount,
                true
            ).map_err(|err| {
                log::error!(
                    "{}:{}. Error transferring to subaccount. Who: {:?}, amount: {:?}, currency: {:?}, subaccount type: {:?}, subaccount id {:?}",
                    file!(),
                    line!(),
                    who,
                    value,
                    str_asset!(asset),
                    subacc_type,
                    subacc_id
                );
                err
            })?;

            if subacc_type == SubAccType::Bailsman
                && !T::Aggregates::in_usergroup(&subacc_id, UserGroup::Bailsmen)
            {
                Self::try_set_usergroup(&subacc_id, &subacc_type).map_or((), |_| {
                    Self::deposit_event(Event::RegisterBailsman(who, subacc_id))
                })
            }

            Ok(Some(Self::transfer_post_weight(
                subacc_type,
                redistribute_amount,
                subaccount_created,
            ))
            .into())
        }

        /// Transfers `amount` of `currency` from subaccount to main account. If `subacc_type`
        /// is `Bailsman` and it's total collateral value becomes less than minimal bailsman
        /// collateral value - subaccount will be unregistered as bailsman.
        #[pallet::call_index(1)]
        #[pallet::weight((Pallet::<T>::transfer_max_weight(subacc_type, false), DispatchClass::Normal))]
        pub fn transfer_from_subaccount(
            origin: OriginFor<T>,
            subacc_type: SubAccType,
            asset: Asset,
            amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::ensure_transfers_enabled()?;

            let redistribute_amount =
                Self::do_transfer_from_subaccount(&who, subacc_type, &who, asset, amount)?;

            Ok(Some(Self::transfer_post_weight(
                subacc_type,
                redistribute_amount,
                false,
            ))
            .into())
        }

        /// Transfers `amount` of `currency` from subaccount to 'destination' account. If `subacc_type`
        /// is `Bailsman` and it's total collateral value becomes less than minimal bailsman
        /// collateral value - subaccount will be unregistered as bailsman.
        /// Destination should not be subaccount.
        #[pallet::call_index(2)]
        #[pallet::weight((Pallet::<T>::transfer_max_weight(subacc_type, false), DispatchClass::Normal))]
        pub fn transfer(
            origin: OriginFor<T>,
            subacc_type: SubAccType,
            destination: T::AccountId,
            asset: Asset,
            amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::ensure_transfers_enabled()?;
            Self::ensure_is_master_acc(&destination)?;

            let redistribute_amount =
                Self::do_transfer_from_subaccount(&who, subacc_type, &destination, asset, amount)?;

            Ok(Some(Self::transfer_post_weight(
                subacc_type,
                redistribute_amount,
                false,
            ))
            .into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New subaccount created
        /// - first element is subaccount owner's `AccountId`
        /// - second element is `AccountId` of created subaccount
        /// - last element is a type of created subaccount
        /// \[owner, subaccount, type\]
        SubaccountCreated(T::AccountId, T::AccountId, SubAccType),
        /// Register bailsman subaccount as bailsman
        /// - first element is subaccount owner's `AccountId`
        /// - second element is subaccount of type Bailsman
        /// \[owner, subaccount\]
        RegisterBailsman(T::AccountId, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Cannot create a subaccount: user already has subaccount of
        /// this type
        AlreadyHasSubaccount,
        /// Cannot delete subaccount or transfer from it: no subaccount of this type
        NoSubaccountOfThisType,
        /// Cannot create a subaccount: account in whitelist
        AccountInWhiteList,
        /// Transfers are disabled
        TransfersAreDisabled,
        /// Debt not allowed to be creating in this operation
        Debt,
        /// Entropy is not allow to generate subaccount. Try one more time.
        EntropyError,
        /// Account is not a master account. Transfers to external subaccounts prohibited.
        AccountIsNotMaster,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    /// Pallet storage - double map storing subaccounts as `AccountId` where
    /// user's main `AccountId` and `SubAccType` used as keys
    #[pallet::storage]
    #[pallet::getter(fn subaccount)]
    pub type Subaccount<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        SubAccType,
        T::AccountId,
    >;

    /// Pallet storage - a map storing a tuple  (`AccountId`, `SubAccType`)
    /// for each existing subaccount. First element in stored tuple is
    /// `AccountId` of main user account, owning the subaccount and second
    /// is `SubAccType` of key subaccount
    #[pallet::storage]
    #[pallet::getter(fn owner_account)]
    pub type OwnerAccount<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::AccountId, SubAccType)>;

    /// Vec<(Master account, SubAccType, Subaccount, Vec<(amount, asset)>)>
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub balances: Vec<(T::AccountId, SubAccType, T::AccountId, Vec<(i128, u64)>)>,
        pub timestamp: u64,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                balances: Default::default(),
                timestamp: 0,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            let extra_genesis_builder: fn(&Self) = |config: &GenesisConfig<T>| {
                for &(ref master_acc, subacc_type, ref subaccount, ref sub_acc_balance) in
                    config.balances.iter()
                {
                    let mut is_register = false;
                    if <Subaccount<T>>::get(&master_acc, &subacc_type) == None {
                        <OwnerAccount<T>>::insert(&subaccount, (&master_acc, &subacc_type));
                        <Subaccount<T>>::insert(&master_acc, &subacc_type, &subaccount);
                        if subacc_type != SubAccType::Bailsman {
                            <Pallet<T>>::try_set_usergroup(&subaccount, &subacc_type).expect(
                                "eq-subaccounts. try_set_usergroup failed on build genesis",
                            );
                            is_register = true;
                        }
                    }

                    for &(amount, asset) in sub_acc_balance.iter() {
                        let asset_typed = Asset::new(asset)
                            .expect("eq-subaccounts. Asset::new failed on build genesis");

                        if !T::AssetGetter::exists(asset_typed) {
                            panic!("eq-subaccounts. Add balance for not existing asset");
                        }

                        if amount >= 0 {
                            T::EqCurrency::deposit_creating(
                                subaccount,
                                asset_typed,
                                (amount as u128).into(),
                                false,
                                None,
                            )
                            .expect("eq-subaccounts. deposit_creating failed");
                            // deposit_into_existing()
                        } else {
                            T::EqCurrency::withdraw(
                                subaccount,
                                asset_typed,
                                (amount.abs() as u128).into(),
                                false,
                                None,
                                WithdrawReasons::empty(),
                                ExistenceRequirement::KeepAlive,
                            )
                            .expect("eq-subaccounts. withdraw failed");
                        }
                    }
                    if subacc_type == SubAccType::Bailsman && !is_register {
                        <Pallet<T>>::try_set_usergroup(&subaccount, &subacc_type)
                            .expect("eq-subaccounts. try_set_usergroup failed on build genesis");
                    }
                    #[cfg(not(feature = "production"))]
                    if subacc_type == SubAccType::Bailsman
                        || subacc_type == SubAccType::Trader
                        || subacc_type == SubAccType::Borrower
                    {
                        let now = config.timestamp;
                        T::UpdateTimeManager::set_last_update_timestamp(&subaccount, now);
                    }
                    frame_system::Pallet::<T>::inc_providers(master_acc);
                }
            };

            if !cfg!(feature = "production") {
                extra_genesis_builder(self);
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    fn do_transfer_from_subaccount(
        who: &T::AccountId,
        subacc_type: SubAccType,
        destination: &T::AccountId,
        asset: Asset,
        amount: T::Balance,
    ) -> Result<u32, DispatchError> {
        let subaccount = Self::try_get_subaccount(&who, &subacc_type)?;

        let is_active_bailsman = subacc_type == SubAccType::Bailsman
            && T::Aggregates::in_usergroup(&subaccount, UserGroup::Bailsmen);

        let mut redistribute_amount = 0;
        let should_unreg = if is_active_bailsman {
            redistribute_amount = T::BailsmenManager::redistribute(&subaccount)?;
            let should_unreg = T::BailsmenManager::should_unreg_bailsman(
                &subaccount,
                &vec![(asset, SignedBalance::Negative(amount))],
                None,
            )
            .map_err(|err| {
                log::error!(
                    "{}:{}. Error during transfer {:?} {:?} from bailsman subaccount. Couldn't \
                            make checks for unreg bailsman. Bailsman {:?}, main account: {:?}",
                    file!(),
                    line!(),
                    str_asset!(asset),
                    amount,
                    subaccount,
                    who
                );
                err
            })?;

            should_unreg
        } else {
            false
        };

        // Transfer will fail if bailsman has debt,
        // because change is negative
        // only positive changes are allowed
        T::EqCurrency::currency_transfer(
            &subaccount,
            &destination,
            asset,
            amount,
            ExistenceRequirement::AllowDeath,
            TransferReason::Subaccount,
            true,
        )
        .map_err(|err| {
            log::error!(
                "{}:{}. Error during transfer from {:?} subaccount. Couldn't transfer \
                    {:?} from {:?} to account: {:?}",
                file!(),
                line!(),
                subacc_type,
                str_asset!(asset),
                subaccount,
                destination
            );
            err
        })?;

        if is_active_bailsman && should_unreg {
            // Transfer already checks for debt
            T::BailsmenManager::unregister_bailsman(&subaccount).map_err(|err| {
                log::error!(
                    "{}:{}. Error during transfer from bailsman subaccount. Couldn't \
                        unregister bailsman {:?}, main account: {:?}",
                    file!(),
                    line!(),
                    subaccount,
                    who
                );
                err
            })?;
        }

        Ok(redistribute_amount)
    }

    fn ensure_transfers_enabled() -> DispatchResult {
        let is_enabled = T::IsTransfersEnabled::get();
        eq_ensure!(
            is_enabled,
            Error::<T>::TransfersAreDisabled,
            target: "eq_subaccounts",
            "{}:{}. Transfers is not allowed.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn ensure_is_master_acc(who: &T::AccountId) -> DispatchResult {
        eq_ensure!(
            Self::is_master(who),
            Error::<T>::AccountIsNotMaster,
            target: "eq_subaccounts",
            "{}:{}. Destination account should be master account. AccountId {:?}",
            file!(),
            line!(),
            who
        );

        Ok(())
    }

    fn try_get_subaccount(
        who: &T::AccountId,
        subacc_type: &SubAccType,
    ) -> Result<T::AccountId, Error<T>> {
        let subaccount = <Subaccount<T>>::get(who, subacc_type);
        ok_or_error!(
            subaccount,
            Error::<T>::NoSubaccountOfThisType,
            "{}:{}. Cannot transfer from subaccount: subaccount does not \
                exist. Who: {:?}, subaccount type: {:?}",
            file!(),
            line!(),
            who,
            subacc_type
        )
    }

    /// Generates and returns `AccountId` using current block height and extrinsic
    /// index for entropy
    fn generate_account_id(
        who: &T::AccountId,
        subacc_type: &SubAccType,
    ) -> Result<T::AccountId, sp_runtime::DispatchError> {
        let height = frame_system::Pallet::<T>::block_number();
        let ext_index = frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default();

        let entropy =
            (b"eq/subaccounts__", who, height, ext_index, subacc_type, 0).using_encoded(blake2_256);
        T::AccountId::decode(&mut &entropy[..]).map_err(|_| Error::<T>::EntropyError.into())
    }

    /// Adds subaccount to corresponding UserGroup according to `subacc_type`
    fn try_set_usergroup(
        subacc_id: &T::AccountId,
        subacc_type: &SubAccType,
    ) -> Result<(), sp_runtime::DispatchError> {
        match subacc_type {
            SubAccType::Trader | SubAccType::Borrower => {
                T::Aggregates::set_usergroup(subacc_id, UserGroup::Borrowers, true)?;
            }
            SubAccType::Bailsman => {
                T::BailsmenManager::register_bailsman(&subacc_id)?;
            }
        };
        Ok(())
    }

    fn transfer_max_weight(subacc_type: &SubAccType, maybe_creating: bool) -> Weight {
        match subacc_type {
            SubAccType::Bailsman => {
                if maybe_creating {
                    T::WeightInfo::transfer_to_bailsman_and_redistribute(
                        T::BailsmenManager::distribution_queue_len() as u32,
                    )
                    .max(T::WeightInfo::transfer_to_bailsman_register())
                } else {
                    T::WeightInfo::transfer_to_bailsman_and_redistribute(
                        T::BailsmenManager::distribution_queue_len(),
                    )
                }
            }
            _ => {
                if maybe_creating {
                    T::WeightInfo::transfer_to_subaccount()
                        .max(T::WeightInfo::transfer_to_borrower_register())
                } else {
                    T::WeightInfo::transfer_to_subaccount()
                }
            }
        }
    }

    fn transfer_post_weight(
        subacc_type: SubAccType,
        redistribute_amount: u32,
        subaccount_created: bool,
    ) -> Weight {
        match subacc_type {
            SubAccType::Bailsman => {
                if subaccount_created {
                    T::WeightInfo::transfer_to_bailsman_register()
                } else {
                    T::WeightInfo::transfer_to_bailsman_and_redistribute(redistribute_amount)
                }
            }
            _ => {
                if subaccount_created {
                    T::WeightInfo::transfer_to_borrower_register()
                } else {
                    T::WeightInfo::transfer_to_subaccount()
                }
            }
        }
    }
}

impl<T: Config> SubaccountsManager<T::AccountId> for Pallet<T> {
    fn create_subaccount_inner(
        who: &T::AccountId,
        subacc_type: &SubAccType,
    ) -> Result<T::AccountId, sp_runtime::DispatchError> {
        eq_ensure!(
            !T::Whitelist::in_whitelist(&who),
            Error::<T>::AccountInWhiteList,
            target: "eq_subaccounts",
            "{}:{}. Account is in whitelist. Who: {:?}, \
            subaccount type: {:?}",
            file!(),
            line!(),
            who,
            subacc_type
        );

        eq_ensure!(
            !Self::has_subaccount(&who, &subacc_type),
            Error::<T>::AlreadyHasSubaccount,
            target: "eq_subaccounts",
            "{}:{}. Account already has subaccount of this type. Who: {:?}, \
            subaccount type: {:?}",
            file!(),
            line!(),
            who,
            subacc_type
        );

        let subaccount = Self::generate_account_id(&who, &subacc_type)?;
        T::UpdateTimeManager::set_last_update(&subaccount);
        <OwnerAccount<T>>::insert(&subaccount, (&who, &subacc_type));
        <Subaccount<T>>::insert(&who, &subacc_type, &subaccount);
        // increment subaccount providers here, so it will no happen in currency_transfer
        frame_system::Pallet::<T>::inc_providers(&subaccount.clone().into());
        frame_system::Pallet::<T>::inc_providers(&who.clone().into());

        Ok(subaccount)
    }

    fn delete_subaccount_inner(
        who: &T::AccountId,
        subacc_type: &SubAccType,
    ) -> Result<T::AccountId, sp_runtime::DispatchError> {
        let subaccount = <Subaccount<T>>::get(&who, &subacc_type);
        let subaccount = ok_or_error!(
            subaccount,
            Error::<T>::NoSubaccountOfThisType,
            "{}:{}. Account does not have subaccount of this type. Who: {:?}, \
            subaccount type: {:?}",
            file!(),
            line!(),
            who,
            subacc_type
        )?;

        // Removing deleted subaccount from corresponding aggregates
        match subacc_type {
            SubAccType::Trader | SubAccType::Borrower => {
                T::Aggregates::set_usergroup(&subaccount, UserGroup::Borrowers, false)?
            }
            SubAccType::Bailsman => {
                if T::Aggregates::in_usergroup(&subaccount, UserGroup::Bailsmen) {
                    // unregister also delete from usergroup
                    T::BailsmenManager::unregister_bailsman(&subaccount).map_err(|err| {
                        log::error!(
                            "{}:{}. Error during unregister bailsman subaccount. Couldn't \
                            unregister bailsman {:?}, main account: {:?}",
                            file!(),
                            line!(),
                            subaccount,
                            who
                        );
                        err
                    })?;
                }
            }
        };

        T::UpdateTimeManager::remove_last_update(&subaccount);
        <OwnerAccount<T>>::remove(&subaccount);
        <Subaccount<T>>::remove(&who, &subacc_type);
        frame_system::Pallet::<T>::dec_providers(&subaccount)?;
        frame_system::Pallet::<T>::dec_providers(&who)?;

        Ok(subaccount)
    }

    fn has_subaccount(who: &T::AccountId, subacc_type: &SubAccType) -> bool {
        <Subaccount<T>>::get(&who, subacc_type).is_some()
    }

    fn get_subaccount_id(who: &T::AccountId, subacc_type: &SubAccType) -> Option<T::AccountId> {
        <Subaccount<T>>::get(&who, &subacc_type)
    }

    fn is_subaccount(who: &T::AccountId, subaccount_id: &T::AccountId) -> bool {
        if let Some((main_acc, _)) = Self::owner_account(subaccount_id) {
            return main_acc == *who;
        }

        false
    }

    fn get_owner_id(subaccount: &T::AccountId) -> Option<(T::AccountId, SubAccType)> {
        <OwnerAccount<T>>::get(&subaccount)
    }

    fn get_subaccounts_amount(who: &T::AccountId) -> usize {
        <Subaccount<T>>::iter_prefix(&who).count()
    }
}

impl<T: Config> BalanceChecker<T::Balance, T::AccountId, T::BalanceGetter, Pallet<T>>
    for Pallet<T>
{
    fn can_change_balance_impl(
        who: &T::AccountId,
        changes: &Vec<(Asset, SignedBalance<T::Balance>)>,
        _withdraw_reasons: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        if T::Aggregates::in_usergroup(who, UserGroup::Borrowers) {
            return Ok(());
        }

        for (asset, change) in changes.iter() {
            let new_balance = T::BalanceGetter::get_balance(who, &asset) + change.clone();
            if let SignedBalance::Negative(_) = &new_balance {
                return Err(Error::<T>::Debt.into());
            }
        }

        Ok(())
    }
}
