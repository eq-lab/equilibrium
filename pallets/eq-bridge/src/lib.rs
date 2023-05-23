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

//! # Eq-bridge pallet
//!
//! The eq-bridge pallet is responsible for:
//!
//! 1. Initiating transfer from Equilibrium to another network: `EqBridge::transfer_native`.
//!     A fixed fee is taken in the native tokens for the transfer.
//!     After EqBridge has written off the fee and the amount of the transfer from the user's account,
//!     it calls the `chainbridge::transfer_fungible` method to create a proposal and generate an event for relays to process.
//!
//! 2. Converting asset representation from bridge’s resource_id to Equilibrium’s asset_id
//!
//! 3. Finalizing transfer from source chain: when native asset is transferred, the tokens are transferred from the bridge to the user,
//!     when a physical (bridgeable) asset is transferred, the tokens are minted (deposited) to the user.
//!
//! 4. SUDO management of resource_id <> asset_id mapping.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(warnings)]

pub mod benchmarking;
mod mock;
mod tests;
pub mod weights;

use codec::{Decode, Encode};
use core::convert::TryFrom;
use core::convert::TryInto;
use eq_primitives::asset::{Asset, AssetGetter, AssetType};
use eq_primitives::balance::{EqCurrency, XcmDestination};
use eq_primitives::AccountType;
use frame_support::traits::{Currency, EnsureOrigin, ExistenceRequirement, Get, WithdrawReasons};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure};
use frame_system::ensure_signed;
use sp_arithmetic::traits::SaturatedConversion;
use sp_core::U256;
use sp_std::prelude::*;
pub use weights::WeightInfo;

pub use pallet::*;

const ETHEREUM_ADDRESS_LENGTH: usize = 20;
const SUBSTRATE_ADDRESS_LENGTH: usize = 32;
const SUBSTRATE_PREFIX_LENGTH: usize = 4;
const SUBSTRATE_WITH_PREFIX_ADDRESS_LENGTH: usize =
    SUBSTRATE_ADDRESS_LENGTH + SUBSTRATE_PREFIX_LENGTH;

#[derive(Encode, Decode, Debug, Copy, Clone, PartialEq, Eq, scale_info::TypeInfo)]
pub enum ChainAddressType {
    Ethereum,
    Substrate,
    SubstrateWithPrefix,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn asset_resource)]
    pub type AssetResource<T: Config> =
        StorageMap<_, Blake2_128Concat, Asset, chainbridge::ResourceId>;

    #[pallet::storage]
    #[pallet::getter(fn resources)]
    pub(super) type Resources<T: Config> =
        StorageMap<_, Blake2_128Concat, chainbridge::ResourceId, Asset>;

    #[pallet::storage]
    #[pallet::getter(fn minimum_transfer_amount)]
    pub type MinimumTransferAmount<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        chainbridge::ChainId,
        Blake2_128Concat,
        chainbridge::ResourceId,
        T::Balance,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn enabled_withdrawals)]
    pub type EnabledWithdrawals<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        chainbridge::ResourceId,
        Vec<chainbridge::ChainId>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn get_chain_address_type)]
    pub type ChainAddressTypes<T: Config> =
        StorageMap<_, Blake2_128Concat, chainbridge::ChainId, ChainAddressType, OptionQuery>;

    #[pallet::config]
    pub trait Config: frame_system::Config + chainbridge::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type BridgeManagementOrigin: EnsureOrigin<Self::Origin>;

        /// Specifies the origin check provided by the bridge for calls that can only be called by the bridge pallet
        type BridgeOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;

        /// Integrates balances operations of `eq-balances` pallet
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;

        /// Used to deal with Assets
        type AssetGetter: AssetGetter;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        //
        // Initiation calls. These start a bridge transfer.
        //

        /// Transfers some amount of the native token to some recipient on a (whitelisted) destination chain.
        /// Charges fee and accumulates it on the special account.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn transfer_native(
            origin: OriginFor<T>,
            amount: T::Balance,
            recipient: Vec<u8>,
            dest_id: chainbridge::ChainId,
            resource_id: chainbridge::ResourceId,
        ) -> DispatchResultWithPostInfo {
            let source = ensure_signed(origin)?;

            Self::do_transfer_native(source, amount, recipient, dest_id, resource_id)
        }

        /// Stores an asset on chain under an associated resource ID.
        /// Sudo only.
        ///
        /// # <weight>
        /// - O(1) write
        /// # </weight>
        #[pallet::weight(<T as pallet::Config>::WeightInfo::set_resource())]
        pub fn set_resource(
            origin: OriginFor<T>,
            id: chainbridge::ResourceId,
            asset: Asset,
        ) -> DispatchResultWithPostInfo {
            T::BridgeManagementOrigin::ensure_origin(origin)?;
            Self::register_resource(id, asset)
        }

        /// Enable asset withdrawals to specific chain under an associated resource ID.
        /// Sudo only.
        ///
        /// # <weight>
        /// - O(1) write
        /// # </weight>
        #[pallet::weight(<T as pallet::Config>::WeightInfo::enable_withdrawals())]
        pub fn enable_withdrawals(
            origin: OriginFor<T>,
            resource_id: chainbridge::ResourceId,
            chain_id: chainbridge::ChainId,
        ) -> DispatchResultWithPostInfo {
            T::BridgeManagementOrigin::ensure_origin(origin)?;
            Self::toggle_withdrawals_state(resource_id, chain_id, true)
        }

        /// Disable asset withdrawals to specific chain under an associated resource ID.
        /// Sudo only.
        ///
        /// # <weight>
        /// - O(1) write
        /// # </weight>
        #[pallet::weight(<T as pallet::Config>::WeightInfo::disable_withdrawals())]
        pub fn disable_withdrawals(
            origin: OriginFor<T>,
            resource_id: chainbridge::ResourceId,
            chain_id: chainbridge::ChainId,
        ) -> DispatchResultWithPostInfo {
            T::BridgeManagementOrigin::ensure_origin(origin)?;
            Self::toggle_withdrawals_state(resource_id, chain_id, false)
        }

        /// Stores minimum transfer amount for sending asset to external chain.
        /// Sudo only.
        ///
        /// # <weight>
        /// - O(1) write
        /// # </weight>
        #[pallet::weight(<T as pallet::Config>::WeightInfo::set_minimum_transfer_amount())]
        pub fn set_minimum_transfer_amount(
            origin: OriginFor<T>,
            dest_id: chainbridge::ChainId,
            resource_id: chainbridge::ResourceId,
            minimum_amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            T::BridgeManagementOrigin::ensure_origin(origin)?;
            Self::update_minimum_transfer_amount(dest_id, resource_id, minimum_amount)
        }

        /// Stores chain id relation to chain address type.
        /// Sudo only.
        ///
        /// # <weight>
        /// - O(1) write
        /// # </weight>
        #[pallet::weight(T::DbWeight::get().writes(1).ref_time())]
        pub fn set_chain_address_type(
            origin: OriginFor<T>,
            dest_id: chainbridge::ChainId,
            address_type: Option<ChainAddressType>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Self::update_chain_address_type(dest_id, address_type)
        }

        //
        // Executable calls. These can be triggered by a bridge transfer initiated on another chain
        //

        /// Deposits specified amount of Eq/Gens tokens to the user's account
        // TODO: transfer/transfer_basic depending on the asset: basic/not basic (look in benchmarking)
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer())]
        pub fn transfer(
            origin: OriginFor<T>,
            to: T::AccountId,
            amount: T::Balance,
            resource_id: chainbridge::ResourceId,
        ) -> DispatchResultWithPostInfo {
            let source = T::BridgeOrigin::ensure_origin(origin)?;
            let asset = Self::resources(resource_id).ok_or(Error::<T>::InvalidResourceId)?;

            let is_mintable_asset = Self::is_mintable_asset(&asset)?;
            if is_mintable_asset {
                <T as Config>::EqCurrency::deposit_creating(&to, asset, amount, true, None)?;
                Self::deposit_event(Event::FromBridgeTransfer(to, asset, amount));
            } else {
                T::EqCurrency::currency_transfer(
                    &source,
                    &to,
                    asset,
                    amount,
                    ExistenceRequirement::AllowDeath,
                    eq_primitives::TransferReason::Common,
                    true,
                )?;
            }

            Ok(().into())
        }

        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer())]
        pub fn xcm_transfer(
            origin: OriginFor<T>,
            to: Vec<u8>,
            amount: T::Balance,
            resource_id: chainbridge::ResourceId,
        ) -> DispatchResultWithPostInfo {
            let from = T::BridgeOrigin::ensure_origin(origin)?;
            let asset = Self::resources(resource_id).ok_or(Error::<T>::InvalidResourceId)?;
            let _ = T::AssetGetter::get_asset_data(&asset)?;

            <T as Config>::EqCurrency::deposit_creating(&from, asset, amount, false, None)?;
            match AccountType::try_from(to.clone()) {
                Ok(acc) => {
                    Self::deposit_event(Event::FromBridgeTransferNext(to, asset, amount));
                    <T as Config>::EqCurrency::xcm_transfer(
                        &from,
                        asset,
                        amount,
                        XcmDestination::Native(acc),
                    )?;
                }
                Err(_) => {
                    use xcm::latest::{Junction::*, Junctions::*, NetworkId};

                    let (para_id, account_type): (u32, AccountType) =
                        Decode::decode(&mut &to[..]).map_err(|_| Error::<T>::InvalidAccount)?;
                    let account_type = account_type.multi_location(NetworkId::Any);
                    let location = if para_id == 0 {
                        X1(account_type)
                    } else {
                        X2(Parachain(para_id), account_type)
                    };

                    Self::deposit_event(Event::FromBridgeTransferNext(to, asset, amount));
                    <T as Config>::EqCurrency::xcm_transfer(
                        &from,
                        asset,
                        amount,
                        XcmDestination::Common((1, location).into()),
                    )?;
                }
            }

            Ok(().into())
        }

        /// This can be called by the bridge to demonstrate an arbitrary call from a proposal.
        #[pallet::weight(<T as pallet::Config>::WeightInfo::remark())]
        pub fn remark(origin: OriginFor<T>, hash: T::Hash) -> DispatchResultWithPostInfo {
            T::BridgeOrigin::ensure_origin(origin)?;
            Self::deposit_event(Event::Remark(hash));
            Ok(().into())
        }
    }
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Demonstrate an arbitrary call from a proposal. \[hash\]
        Remark(T::Hash),
        /// Transfers funds from the bridge into the network
        FromBridgeTransfer(T::AccountId, Asset, T::Balance),
        /// Transfers funds from the bridge into the network to transfer next
        FromBridgeTransferNext(Vec<u8>, Asset, T::Balance),
        /// Transfers funds out of the network to the bridge
        ToBridgeTransfer(T::AccountId, Asset, T::Balance),
        /// Allowability withdrawals for the resource id to chain has changed. \[resource_id, chain_id, enabled\]
        WithdrawalsToggled(chainbridge::ResourceId, chainbridge::ChainId, bool),
        /// Minimum transfer amount to out of the network has changed. \[chainId, resourceId, new_minimum_amount\]
        MinimumTransferAmountChanged(chainbridge::ChainId, chainbridge::ResourceId, T::Balance),
        /// ChainAddressType has changed. \[chainId, Option<ChainAddressType>\]
        ChainAddressTypeChanged(chainbridge::ChainId, Option<ChainAddressType>),
    }
    #[pallet::error]
    pub enum Error<T> {
        InvalidTransfer,
        /// Resource id not mapped to `Asset`
        InvalidResourceId,
        /// not allowed to bridge tokens of this type
        InvalidAssetType,
        /// wrong recipient address
        RecipientChainAddressTypeMismatch,
        /// Bridge transfers to this chain are disabled
        DisabledChain,
        /// Interactions with this chain is not permitted
        ChainNotWhitelisted,
        /// Asset withdrawals to this chain are disabled
        DisabledWithdrawals,
        /// Withdrawals to this resource id and chain id have equal allowability
        WithdrawalsAllowabilityEqual,
        /// Transfer amount is lower than a minimum for given `ChainId` and `ResourceId`
        TransferAmountLowerMinimum,
        /// Invalid destination Account for XCM transfer
        InvalidAccount,
        /// Attempt to set ChainAddressType to current value
        ChainAddressTypeEqual,
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub _runtime: PhantomData<T>,
        pub resources: Vec<(chainbridge::ResourceId, Asset)>,
        pub minimum_transfer_amount:
            Vec<(chainbridge::ChainId, chainbridge::ResourceId, T::Balance)>,
        pub enabled_withdrawals: Vec<(chainbridge::ResourceId, Vec<chainbridge::ChainId>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                resources: vec![],
                minimum_transfer_amount: vec![],
                enabled_withdrawals: vec![],
                _runtime: PhantomData,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            // set_resource
            for (resource_id, asset) in self.resources.iter() {
                Pallet::<T>::register_resource(*resource_id, *asset).expect("register");
            }
            // set_minimum_transfer_amount
            for (chain_id, resource_id, asset) in self.minimum_transfer_amount.iter() {
                MinimumTransferAmount::<T>::insert(chain_id, resource_id, asset);
            }
            // enabled_withdrawals
            for (resource_id, chain_ids) in self.enabled_withdrawals.iter() {
                for chain_id in chain_ids {
                    Pallet::<T>::toggle_withdrawals_state(*resource_id, *chain_id, true)
                        .expect("toggle_withdrawals_state failed on build genesis");
                }
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Register an asset for a resource Id, enabling associated transfers.
    fn register_resource(id: chainbridge::ResourceId, asset: Asset) -> DispatchResultWithPostInfo {
        Resources::<T>::insert(id, asset);
        AssetResource::<T>::insert(asset, id);
        Ok(().into())
    }

    fn toggle_withdrawals_state(
        resource_id: chainbridge::ResourceId,
        chain_id: chainbridge::ChainId,
        enabled: bool,
    ) -> DispatchResultWithPostInfo {
        let mut enabled_chains = EnabledWithdrawals::<T>::get(resource_id);
        match enabled_chains.binary_search_by(|x| x.cmp(&chain_id)) {
            Ok(_) if enabled => frame_support::fail!(Error::<T>::WithdrawalsAllowabilityEqual),
            Ok(idx) => {
                enabled_chains.remove(idx);
                ()
            }
            Err(idx) if enabled => enabled_chains.insert(idx, chain_id),
            Err(_) => frame_support::fail!(Error::<T>::WithdrawalsAllowabilityEqual),
        };

        EnabledWithdrawals::<T>::insert(resource_id, enabled_chains);
        Self::deposit_event(Event::WithdrawalsToggled(resource_id, chain_id, enabled));
        Ok(().into())
    }

    fn update_minimum_transfer_amount(
        dest_id: chainbridge::ChainId,
        resource_id: chainbridge::ResourceId,
        minimum_amount: T::Balance,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            <chainbridge::Pallet<T>>::chain_whitelisted(dest_id),
            Error::<T>::ChainNotWhitelisted
        );
        Self::resources(resource_id).ok_or(Error::<T>::InvalidResourceId)?;

        MinimumTransferAmount::<T>::insert(dest_id, resource_id, minimum_amount);

        Self::deposit_event(Event::MinimumTransferAmountChanged(
            dest_id,
            resource_id,
            minimum_amount,
        ));
        Ok(().into())
    }

    fn update_chain_address_type(
        dest_id: chainbridge::ChainId,
        address_type: Option<ChainAddressType>,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            <chainbridge::Pallet<T>>::chain_whitelisted(dest_id),
            Error::<T>::ChainNotWhitelisted
        );
        ensure!(
            ChainAddressTypes::<T>::get(dest_id) != address_type,
            Error::<T>::ChainAddressTypeEqual
        );

        match address_type {
            Some(value) => ChainAddressTypes::<T>::insert(dest_id, value),
            None => ChainAddressTypes::<T>::remove(dest_id),
        }

        Self::deposit_event(Event::ChainAddressTypeChanged(dest_id, address_type));
        Ok(().into())
    }

    fn is_mintable_asset(asset: &Asset) -> Result<bool, sp_runtime::DispatchError> {
        let asset_data = T::AssetGetter::get_asset_data(&asset)?;

        match (asset_data.asset_type, asset_data.id) {
            (AssetType::Physical, _) | (_, eq_primitives::asset::EQD) => Ok(true),
            (AssetType::Native, _) | (AssetType::Synthetic, _) => Ok(false),
            _ => Err(Error::<T>::InvalidAssetType.into()),
        }
    }

    pub fn do_transfer_native(
        source: T::AccountId,
        amount: T::Balance,
        recipient: Vec<u8>,
        dest_id: chainbridge::ChainId,
        resource_id: chainbridge::ResourceId,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            <chainbridge::Pallet<T>>::chain_whitelisted(dest_id),
            Error::<T>::InvalidTransfer
        );
        ensure!(
            <chainbridge::Pallet<T>>::chain_enabled(dest_id),
            Error::<T>::DisabledChain
        );
        ensure!(
            Self::withdrawals_enabled(resource_id, dest_id),
            Error::<T>::DisabledWithdrawals
        );
        ensure!(
            Self::is_address_valid(&recipient, dest_id),
            Error::<T>::RecipientChainAddressTypeMismatch
        );
        ensure!(
            amount >= <MinimumTransferAmount<T>>::get(dest_id, resource_id),
            Error::<T>::TransferAmountLowerMinimum
        );
        let asset = Self::resources(resource_id).ok_or(Error::<T>::InvalidResourceId)?;
        let is_basic_asset = asset == <T as eq_assets::Config>::MainAsset::get();

        let fee = chainbridge::Fees::<T>::get(dest_id);
        let total = if is_basic_asset { fee + amount } else { fee };

        <T as chainbridge::Config>::Currency::ensure_can_withdraw(
            &source,
            total,
            WithdrawReasons::empty(),
            0u32.into(),
        )?;

        // Maybe should use transaction here
        let fee_id = <chainbridge::Pallet<T>>::fee_account_id();
        <T as chainbridge::Config>::Currency::transfer(
            &source,
            &fee_id,
            fee,
            ExistenceRequirement::AllowDeath,
        )?;

        let is_mintable_asset = Self::is_mintable_asset(&asset)?;

        if is_mintable_asset {
            <T as Config>::EqCurrency::withdraw(
                &source,
                asset,
                amount,
                true,
                None,
                WithdrawReasons::empty(),
                ExistenceRequirement::AllowDeath,
            )?;
            Self::deposit_event(Event::ToBridgeTransfer(source, asset, amount));
        } else {
            let bridge_id = <chainbridge::Pallet<T>>::account_id();
            T::EqCurrency::currency_transfer(
                &source,
                &bridge_id,
                asset,
                amount,
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::Common,
                true,
            )?;
        }

        <chainbridge::Pallet<T>>::transfer_fungible(
            dest_id,
            resource_id,
            recipient,
            U256::from(amount.saturated_into::<u128>()),
        )
    }

    /// Asserts if withdrawals to chain are disabled.
    pub fn withdrawals_enabled(
        resource_id: chainbridge::ResourceId,
        chain_id: chainbridge::ChainId,
    ) -> bool {
        let enabled_chains = <EnabledWithdrawals<T>>::get(resource_id);
        match enabled_chains.binary_search_by(|x| x.cmp(&chain_id)) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn is_address_valid(recipient: &Vec<u8>, dest_id: chainbridge::ChainId) -> bool {
        let expected_address_type = ChainAddressTypes::<T>::get(dest_id);
        match expected_address_type {
            Some(ChainAddressType::Ethereum) => recipient.len() == ETHEREUM_ADDRESS_LENGTH,
            Some(ChainAddressType::Substrate) => recipient.len() == SUBSTRATE_ADDRESS_LENGTH,
            Some(ChainAddressType::SubstrateWithPrefix) => {
                recipient.len() == SUBSTRATE_ADDRESS_LENGTH
                    || recipient.len() == SUBSTRATE_WITH_PREFIX_ADDRESS_LENGTH
            }
            None => true,
        }
    }
}

impl<T: Config>
    eq_primitives::chainbridge::Bridge<
        T::AccountId,
        T::Balance,
        chainbridge::ChainId,
        chainbridge::ResourceId,
    > for Pallet<T>
{
    fn transfer_native(
        source: T::AccountId,
        amount: T::Balance,
        recipient: Vec<u8>,
        dest_id: chainbridge::ChainId,
        resource_id: chainbridge::ResourceId,
    ) -> DispatchResultWithPostInfo {
        Self::do_transfer_native(source, amount, recipient, dest_id, resource_id)
    }

    fn get_fee(dest_id: chainbridge::ChainId) -> T::Balance {
        chainbridge::Fees::<T>::get(dest_id)
    }
}

impl<T: Config> eq_primitives::chainbridge::ResourceGetter<chainbridge::ResourceId> for Pallet<T> {
    fn get_resource_by_asset(asset: Asset) -> Option<chainbridge::ResourceId> {
        Pallet::<T>::asset_resource(asset)
    }

    fn get_asset_by_resource(resource_id: chainbridge::ResourceId) -> Option<Asset> {
        Pallet::<T>::resources(resource_id)
    }
}
