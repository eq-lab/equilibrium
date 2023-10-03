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

//! # Equilibrium Balances Pallet
//!
//! This module stores information about account balances in different assets.

//!  (standard polkadot balances module works with only one asset).
//! Users may go negative in their balances e.g. borrow something (we keep track of signed balance) given LTV requirements
//! (There is a criticalLTV setting inside Bailsman pallet which has default value of 105%).

//! When users perform transfer the balances pallet communicates to sub-account and bailsmen pallets
//! which perform checks that are described further below.

//! Subaccounts pallet: checks that only borrower sub accounts can go negative in assets value due to transfers.
//! Bailsman pallet: checks that the margin is good or it will be better with the change

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(warnings)]

use codec::Codec;
pub use eq_primitives::imbalances::{NegativeImbalance, PositiveImbalance};
use eq_primitives::{
    asset::{Asset, AssetGetter, DOT, GLMR, XDOT, XDOT2, XDOT3},
    balance::{
        AccountData, BalanceChecker, BalanceGetter, BalanceRemover, DebtCollateralDiscounted,
        DepositReason, EqCurrency, LockGetter, WithdrawReason, XcmDestination,
        XcmTransferDealWithFee,
    },
    balance_number::EqFixedU128,
    signed_balance::{SignedBalance, SignedBalance::*},
    str_asset,
    subaccount::{SubAccType, SubaccountsManager},
    xcm_origins::dot::PARACHAIN_MOONBEAM,
    AccountRefCounter, AccountRefCounts, AccountType, Aggregates, BailsmanManager,
    EqPalletAccountInitializer, OrderAggregates, PalletAccountInitializer, PriceGetter,
    TransferReason, UpdateTimeManager, UserGroup, XcmMode,
};
use eq_utils::{
    balance_from_xcm, balance_into_xcm, balance_swap_decimals, eq_ensure, vec_map::VecMap,
    XcmBalance,
};
use frame_support::{
    codec::{Decode, Encode},
    dispatch::DispatchError,
    ensure,
    storage::PrefixIterator,
    traits::{
        BalanceStatus, ExistenceRequirement, Get, Imbalance, LockIdentifier, StoredMap, UnixTime,
        WithdrawReasons,
    },
    transactional, PalletId,
};
pub use pallet::*;
#[allow(unused_imports)]
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
    traits::{AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, Convert, Saturating, Zero},
    ArithmeticError, DispatchResult, FixedPointNumber, TransactionOutcome,
};
use sp_std::{
    collections::btree_map::BTreeMap,
    convert::{TryFrom, TryInto},
    fmt::Debug,
    prelude::*,
};
pub use weights::WeightInfo;
use xcm::v3::{
    AssetId::Concrete, Fungibility::Fungible, Instruction::*, InteriorMultiLocation, MultiAsset,
    MultiLocation, SendResult, SendXcm, WeightLimit, WildMultiAsset::*, Xcm,
};

pub mod benchmarking;
pub mod locked_balance_checker;
mod mock;
mod tests;
pub mod weights;
mod xcm_impl;
mod xcm_impl_old;

const XCM_LIMIT_PERIOD_IN_SEC: u64 = 86400; // 1 day

frame_support::parameter_types! {
    pub const MaxLocks: u32 = 10;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use eq_primitives::balance::DepositReason;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::FixedPointOperand;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Origin for enable and disable transfers
        type ToggleTransferOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Origin to force xcm transfers
        type ForceXcmTransferOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Numerical representation of stored balances
        type Balance: Parameter
            + FixedPointOperand
            + Member
            + AtLeast32BitUnsigned
            + Codec
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + TryFrom<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>
            + FixedPointOperand;
        /// Minimum account balance in usd. Accounts with deposit less than
        /// Min(`ExistentialDeposit`,`ExistentialDepositBasic`) must be killed
        #[pallet::constant]
        type ExistentialDeposit: Get<Self::Balance>;
        /// Minimum account balance in basic currency. Accounts with deposit less than
        /// Min(`ExistentialDeposit`,`ExistentialDepositBasic`) must be killed
        #[pallet::constant]
        type ExistentialDepositBasic: Get<Self::Balance>;
        /// Strategy for checking that transfer can be made
        type BalanceChecker: BalanceChecker<
            Self::Balance,
            Self::AccountId,
            Pallet<Self>,
            Self::SubaccountsManager,
        >;
        /// Gets currency prices from oracle
        type PriceGetter: PriceGetter;
        /// Used to work with `TotalAggregates` storing aggregated collateral and debt
        type Aggregates: Aggregates<Self::AccountId, Self::Balance>;
        /// Treasury module's account
        #[pallet::constant]
        type TreasuryModuleId: Get<PalletId>;
        /// Bailsman module's account
        #[pallet::constant]
        type BailsmanModuleId: Get<PalletId>;
        /// Used for managing subaccounts
        type SubaccountsManager: SubaccountsManager<Self::AccountId>;
        /// Used for managing bailsmen
        type BailsmenManager: BailsmanManager<Self::AccountId, Self::Balance>;
        /// Used for managing last update time in Equilibrium Rate pallet
        type UpdateTimeManager: eq_primitives::UpdateTimeManager<Self::AccountId>;
        /// Used to deal with Assets
        type AssetGetter: AssetGetter;
        /// Used for sending XCM
        type XcmRouter: SendXcm;
        // Used to compute XCM execution fee on destination chain
        type XcmToFee: for<'xcm> Convert<
            (Asset, MultiLocation, &'xcm Xcm<()>),
            Option<(Asset, XcmBalance)>,
        >;
        /// Used to convert MultiLocation to AccountId for reserving assets
        type LocationToAccountId: xcm_executor::traits::Convert<MultiLocation, Self::AccountId>;
        /// Used to reanchoring asset with Ancestry
        type UniversalLocation: Get<InteriorMultiLocation>;
        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
        /// Account for storing reserved balance
        #[pallet::constant]
        type ModuleId: Get<PalletId>;
        /// Used to check if account has orders
        type OrderAggregates: OrderAggregates<Self::AccountId>;
        /// The means of storing the balances of an account.
        type AccountStore: StoredMap<Self::AccountId, AccountData<Self::Balance>>;
        /// Parachain Id provider
        type ParachainId: Get<eq_xcm::ParaId>;
        /// Timestamp provider
        type UnixTime: UnixTime;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Transfers `value` amount of `Asset` from trx sender to account id `to`
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::transfer())]
        pub fn transfer(
            origin: OriginFor<T>,
            asset: Asset,
            to: <T as frame_system::Config>::AccountId,
            value: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let from = ensure_signed(origin)?;

            Self::ensure_transfers_enabled(&asset, value)?;

            ensure!(
                Self::is_not_subaccount(&to),
                Error::<T>::TransferToSubaccount
            );

            Self::currency_transfer(
                &from,
                &to,
                asset,
                value,
                ExistenceRequirement::KeepAlive,
                TransferReason::Common,
                true,
            )?;
            Ok(().into())
        }

        /// Adds currency to account balance (sudo only). Used to deposit currency
        /// into system. Disabled in production.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::transfer())]
        pub fn deposit(
            origin: OriginFor<T>,
            asset: Asset,
            to: <T as frame_system::Config>::AccountId,
            value: T::Balance,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            Self::ensure_transfers_enabled(&asset, value)?;

            Self::deposit_creating(&to, asset, value, true, Some(DepositReason::Extrinsic))?;

            Ok(().into())
        }

        /// Burns currency (sudo only). Used to withdraw currency from the system.
        /// Disabled in production.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::transfer())]
        pub fn burn(
            origin: OriginFor<T>,
            asset: Asset,
            from: <T as frame_system::Config>::AccountId,
            value: T::Balance,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            Self::ensure_transfers_enabled(&asset, value)?;

            Self::withdraw(
                &from,
                asset,
                value,
                true,
                Some(WithdrawReason::Extrinsic),
                WithdrawReasons::empty(),
                ExistenceRequirement::AllowDeath,
            )?;
            Ok(().into())
        }

        /// Enable transfers between accounts
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::enable_transfers())]
        pub fn enable_transfers(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            T::ToggleTransferOrigin::ensure_origin(origin)?;

            <IsTransfersEnabled<T>>::put(true);
            Ok(().into())
        }

        /// Disable transfers between accounts
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::disable_transfers())]
        pub fn disable_transfers(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            T::ToggleTransferOrigin::ensure_origin(origin)?;

            <IsTransfersEnabled<T>>::put(false);
            Ok(().into())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::disable_transfers())]
        pub fn xcm_toggle(
            origin: OriginFor<T>,
            mode: Option<XcmMode>,
        ) -> DispatchResultWithPostInfo {
            T::ToggleTransferOrigin::ensure_origin(origin)?;
            match mode {
                Some(enabled) => <IsXcmTransfersEnabled<T>>::put(enabled),
                None => <IsXcmTransfersEnabled<T>>::kill(),
            };

            Ok(().into())
        }

        /// Send asset to parachain or relay chain, where given asset is native for destination
        ///
        /// `asset` - asset to transfer;
        /// `amount` - amount to transfer;
        /// `to` - recipient account on target chain.
        /// Will be deprecated, use `transfer_xcm_native` instead.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::xcm_transfer_native())]
        pub fn xcm_transfer_native(
            origin: OriginFor<T>,
            asset: Asset,
            amount: T::Balance,
            to: AccountType,
            fee_payer: XcmTransferDealWithFee,
        ) -> DispatchResultWithPostInfo {
            Self::can_send_xcm_for_users(&asset, &amount)?;

            let from = ensure_signed(origin)?;

            Self::do_xcm_transfer_old(from, asset, amount, XcmDestination::Native(to), fee_payer)?;

            Ok(().into())
        }

        /// Send any asset with multilocation to another parachain or relay chain via XCM
        ///
        /// `asset` - asset to transfer;
        /// `amount` - amount to transfer;
        /// `to` - recipient location from current chain.
        /// Will be deprecated, use `transfer_xcm` instead.
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::xcm_transfer())]
        pub fn xcm_transfer(
            origin: OriginFor<T>,
            asset: Asset,
            amount: T::Balance,
            to: MultiLocation,
            fee_payer: XcmTransferDealWithFee,
        ) -> DispatchResultWithPostInfo {
            Self::can_send_xcm_for_users(&asset, &amount)?;

            let from = ensure_signed(origin)?;

            Self::do_xcm_transfer_old(from, asset, amount, XcmDestination::Common(to), fee_payer)?;

            Ok(().into())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::xcm_transfer_native())]
        pub fn force_xcm_transfer_native(
            origin: OriginFor<T>,
            asset: Asset,
            amount: T::Balance,
            from: T::AccountId,
            to: AccountType,
        ) -> DispatchResultWithPostInfo {
            T::ForceXcmTransferOrigin::ensure_origin(origin)?;

            Self::do_xcm_transfer_old(
                from,
                asset,
                amount,
                XcmDestination::Native(to),
                XcmTransferDealWithFee::SovereignAccWillPay,
            )?;

            Ok(().into())
        }

        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::xcm_transfer())]
        pub fn force_xcm_transfer(
            origin: OriginFor<T>,
            asset: Asset,
            amount: T::Balance,
            from: T::AccountId,
            to: MultiLocation,
        ) -> DispatchResultWithPostInfo {
            T::ForceXcmTransferOrigin::ensure_origin(origin)?;

            Self::do_xcm_transfer_old(
                from,
                asset,
                amount,
                XcmDestination::Common(to),
                XcmTransferDealWithFee::SovereignAccWillPay,
            )?;

            Ok(().into())
        }

        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::xcm_transfer())]
        pub fn transfer_xcm(
            origin: OriginFor<T>,
            transfer: (Asset, T::Balance),
            fee: (Asset, T::Balance),
            to: MultiLocation,
        ) -> DispatchResultWithPostInfo {
            let (asset, amount) = transfer;
            Self::can_send_xcm_for_users(&asset, &amount)?;

            let from = ensure_signed(origin)?;

            Self::do_xcm_transfer(from, transfer, fee, XcmDestination::Common(to))?;

            Ok(().into())
        }

        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::xcm_transfer_native())]
        pub fn transfer_xcm_native(
            origin: OriginFor<T>,
            transfer: (Asset, T::Balance),
            fee: (Asset, T::Balance),
            to: AccountType,
        ) -> DispatchResultWithPostInfo {
            let (asset, amount) = transfer;
            Self::can_send_xcm_for_users(&asset, &amount)?;

            let from = ensure_signed(origin)?;

            Self::do_xcm_transfer(from, transfer, fee, XcmDestination::Native(to))?;

            Ok(().into())
        }

        /// Allow for `accounts` to make limited xcm native transfers
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::allow_xcm_transfers_native_for(accounts.len() as u32))]
        pub fn allow_xcm_transfers_native_for(
            origin: OriginFor<T>,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            T::ToggleTransferOrigin::ensure_origin(origin)?;

            let now = T::UnixTime::now().as_secs();
            for account_id in accounts {
                if !XcmNativeTransfers::<T>::contains_key(&account_id) {
                    XcmNativeTransfers::<T>::insert(account_id, (T::Balance::zero(), now));
                }
            }

            Ok(().into())
        }

        /// Remove accounts from whitelist of xcm native transfers
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::forbid_xcm_transfers_native_for(accounts.len() as u32))]
        pub fn forbid_xcm_transfers_native_for(
            origin: OriginFor<T>,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            T::ToggleTransferOrigin::ensure_origin(origin)?;

            for account_id in accounts {
                XcmNativeTransfers::<T>::remove(account_id);
            }

            Ok(().into())
        }

        /// Update XCM transfer limit or remove it in case of limit = `None`
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::update_xcm_transfer_native_limit())]
        pub fn update_xcm_transfer_native_limit(
            origin: OriginFor<T>,
            limit: Option<T::Balance>,
        ) -> DispatchResultWithPostInfo {
            T::ToggleTransferOrigin::ensure_origin(origin)?;
            match limit {
                Some(limit) => DailyXcmLimit::<T>::put(limit),
                None => DailyXcmLimit::<T>::kill(),
            }

            Ok(().into())
        }

        #[pallet::call_index(15)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn allow_xdots_swap(
            origin: OriginFor<T>,
            xdot_assets: Vec<XDotAsset>,
        ) -> DispatchResultWithPostInfo {
            T::ToggleTransferOrigin::ensure_origin(origin)?;

            AllowedXdotsSwap::<T>::put(xdot_assets);

            Ok(().into())
        }

        #[pallet::call_index(16)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 2))]
        #[transactional]
        pub fn swap_xdot(
            origin: OriginFor<T>,
            xdot_assets: Vec<XDotAsset>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::ensure_xdot_swap_allowed(&xdot_assets)?;

            let balances = Self::iterate_account_balances(&who);

            for xdot_asset in xdot_assets {
                let asset = match xdot_asset {
                    XDotAsset::XDOT => XDOT,
                    XDotAsset::XDOT2 => XDOT2,
                    XDotAsset::XDOT3 => XDOT3,
                };

                let balance = balances
                    .get(&asset)
                    .filter(|b| b.is_positive())
                    .map(|sb| sb.abs());

                match balance {
                    Some(balance) => {
                        Self::ensure_transfers_enabled(&asset, balance)?;

                        Self::withdraw(
                            &who,
                            asset,
                            balance,
                            false,
                            Some(WithdrawReason::XDotSwap),
                            WithdrawReasons::empty(),
                            ExistenceRequirement::KeepAlive,
                        )?;

                        Self::deposit_creating(
                            &who,
                            DOT,
                            balance,
                            false,
                            Some(DepositReason::XDotSwap),
                        )?;
                    }
                    None => {}
                }
            }

            Ok(().into())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Transfer event. Included values are:
        /// - from `AccountId`
        /// - to `AccountId`
        /// - transfer `Asset`
        /// - transferred amount
        /// - transfer reason
        /// \[from, to, asset, amount, reason\]
        Transfer(
            T::AccountId,
            T::AccountId,
            Asset,
            T::Balance,
            TransferReason,
        ),
        /// Delete account event. \[who\]
        DeleteAccount(T::AccountId),
        /// Exchange event. Included values are:
        /// - accounts that perform exchange
        /// - exchanged assets
        /// - exchanged amounts
        /// \[account_1, asset_1, amount_1, account_2, asset_2, amount_2\]
        Exchange(
            T::AccountId,
            Asset,
            T::Balance,
            T::AccountId,
            Asset,
            T::Balance,
        ),
        /// Deposit event. Included values are:
        /// - to `AccountId`
        /// - transfer `Asset`
        /// - transferred amount
        /// - reason to deposit
        /// \[to, asset, amount, reason\]
        Deposit(T::AccountId, Asset, T::Balance, DepositReason),
        /// Withdraw event. Included values are:
        /// - from `AccountId`
        /// - transfer `Asset`
        /// - transferred amount
        /// \[from, asset, amount\]
        Withdraw(T::AccountId, Asset, T::Balance, WithdrawReason),
        /// XCM message sent
        /// - dest `MultiLocation`
        /// - beneficiary `MultiLocation`
        /// \[dest, beneficiary\]
        XcmTransfer(MultiLocation, MultiLocation),
        /// XCM sending error
        /// - send_error `xcm::latest::SendError`
        /// \[send_error\]
        XcmMessageSendError(xcm::latest::SendError),
        MigrationComplete,
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Beneficiary account must pre-exist
        DeadAccount,
        /// Transfer checks failed
        NotAllowedToChangeBalance,
        /// Transfers are disabled
        TransfersAreDisabled,
        /// This method is not allowed in production
        MethodNotAllowed,
        /// Unimplemented method
        MethodUnimplemented,
        /// Not allowed to delete account
        NotAllowedToDeleteAccount,
        /// Deposit is not enough to buyout Eq from Treasury
        NotEnoughToBuyoutEq,
        /// Deposit is not enough to keep account alive
        NotEnoughToKeepAlive,
        /// Exchange asset to itself
        ExchangeSameAsset,
        /// Direct transfer to subaccount not allowed from EqBalances
        TransferToSubaccount,
        /// Not enough to pay fee on destination chain
        XcmNotEnoughToPayFee,
        /// Error with xcm sending
        XcmSend,
        /// XCM withdraw disabled
        XcmDisabled,
        /// Destination is not valid multilocation for transfer
        XcmInvalidDestination,
        /// Asset is not suitable for XCM transfer
        XcmUnknownAsset,
        /// Fee asset should be the same for self reserved assets
        XcmWrongFeeAsset,
        /// no migration needed
        NoMigration,
        /// Is thrown in case of removing non-zero balance
        NonZeroBalance,
        /// XCM native transfers not allowed for current account
        XcmTransfersNotAllowedForAccount,
        /// Daily XCM transfers limit exceeded
        XcmTransfersLimitExceeded,
        /// Balance is less than locked amount
        Locked,
        /// XDOT swap is not allowed
        XDotSwapNotAllowed,
    }

    /// Reserved balances
    #[pallet::storage]
    pub type Reserved<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        Asset,
        T::Balance,
        ValueQuery,
    >;

    // Accounts' locks
    #[pallet::storage]
    pub type Locked<T: Config> =
        StorageMap<_, Identity, T::AccountId, VecMap<LockIdentifier, T::Balance>, ValueQuery>;

    /// Flag for enable/disable transfers
    #[pallet::storage]
    pub type IsTransfersEnabled<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Stores current xcm executing mode.
    /// `None` means for receiving like in `Xcm` mode and no sending.
    #[pallet::storage]
    pub type IsXcmTransfersEnabled<T: Config> = StorageValue<_, XcmMode, OptionQuery>;

    /// Stores timestamp of next xcm limit period beginning
    #[pallet::storage]
    pub type NextXcmLimitPeriod<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Stores amount of native asset XCM transfers and timestamp of last transfer
    #[pallet::storage]
    pub type XcmNativeTransfers<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::Balance, u64), OptionQuery>;

    /// Stores limit value
    #[pallet::storage]
    pub type DailyXcmLimit<T: Config> = StorageValue<_, T::Balance, OptionQuery>;

    /// Stores limit value
    #[pallet::storage]
    pub type AllowedXdotsSwap<T: Config> = StorageValue<_, Vec<XDotAsset>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub balances: Vec<(T::AccountId, Vec<(T::Balance, u64)>)>,
        pub is_transfers_enabled: bool,
        pub is_xcm_enabled: Option<XcmMode>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                balances: Default::default(),
                is_transfers_enabled: Default::default(),
                is_xcm_enabled: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            let config = self;
            for (ref who, balance) in config.balances.iter() {
                // println!("eq-balances build: who {:?}", who);
                for (free, asset) in balance.iter() {
                    // println!("eq-balances build:   asset: {:?}, free: {:?}", asset, free);
                    let asset_typed =
                        Asset::new(*asset).expect("Asset::new failed on build genesis");

                    if !T::AssetGetter::exists(asset_typed) {
                        panic!("Add balance for not existing asset");
                    }
                    <Pallet<T>>::deposit_creating(who, asset_typed, *free, true, None)
                        .expect("deposit_creating failed");
                }
            }

            EqPalletAccountInitializer::<T>::initialize(
                &T::ModuleId::get().into_account_truncating(),
            );

            <IsTransfersEnabled<T>>::put(config.is_transfers_enabled);
            if let Some(mode) = config.is_xcm_enabled {
                <IsXcmTransfersEnabled<T>>::put(mode);
            }
        }
    }
}

impl<T: Config> BalanceGetter<T::AccountId, T::Balance> for Pallet<T> {
    type Iterator = PrefixIterator<(Asset, SignedBalance<T::Balance>)>;
    type PriceGetter = T::PriceGetter;

    fn get_balance(who: &T::AccountId, asset: &Asset) -> SignedBalance<T::Balance> {
        T::AccountStore::get(who).get(asset)
    }

    fn iterate_balances() -> BTreeMap<T::AccountId, Vec<(Asset, SignedBalance<T::Balance>)>> {
        frame_system::Account::<T>::iter_keys()
            .map(|who| {
                let balances = Self::iterate_account_balances(&who).into();
                (who, balances)
            })
            .collect()
    }

    fn iterate_account_balances(who: &T::AccountId) -> VecMap<Asset, SignedBalance<T::Balance>> {
        match T::AccountStore::get(who) {
            AccountData::V0 { balance, lock: _ } => balance,
        }
    }

    fn get_debt_and_collateral(
        who: &T::AccountId,
    ) -> Result<DebtCollateralDiscounted<T::Balance>, DispatchError> {
        let mut debt = T::Balance::zero();
        let mut collateral = T::Balance::zero();
        let mut discounted_collateral = T::Balance::zero();

        for (asset, account_balance) in Self::iterate_account_balances(who) {
            let price = T::PriceGetter::get_price::<EqFixedU128>(&asset)?;
            let discount = T::AssetGetter::collateral_discount(&asset);

            let abs_value = price
                .checked_mul_int(account_balance.abs().into())
                .map(|b| b.try_into().ok())
                .flatten()
                .ok_or(ArithmeticError::Overflow)?;

            match account_balance {
                Negative(_) => {
                    debt = debt
                        .checked_add(&abs_value)
                        .ok_or(ArithmeticError::Overflow)?;
                }
                Positive(_) => {
                    collateral = collateral
                        .checked_add(&abs_value)
                        .ok_or(ArithmeticError::Overflow)?;
                    let discounted_value = discount
                        .checked_mul_int(abs_value.into())
                        .map(|b| b.try_into().ok())
                        .flatten()
                        .ok_or(ArithmeticError::Overflow)?;
                    discounted_collateral = discounted_collateral
                        .checked_add(&discounted_value)
                        .ok_or(ArithmeticError::Overflow)?;
                }
            };
        }

        Ok(DebtCollateralDiscounted {
            debt,
            collateral,
            discounted_collateral,
        })
    }
}

impl<T: Config> BalanceRemover<T::AccountId> for Pallet<T> {
    fn remove_asset(who: T::AccountId, asset_to_remove: &Asset) -> Result<(), DispatchError> {
        T::AccountStore::mutate(&who, |balances| -> DispatchResult {
            let asset_balance = balances
                .iter()
                .find_map(|(asset, balance)| {
                    if *asset == *asset_to_remove {
                        Some(balance.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            if !asset_balance.is_zero() {
                return Err(Error::<T>::NonZeroBalance.into());
            }

            balances.retain(|&asset, _| asset != *asset_to_remove);

            Ok(())
        })?
    }
}

impl<T: Config> EqCurrency<T::AccountId, T::Balance> for Pallet<T> {
    type Moment = T::BlockNumber;
    type MaxLocks = MaxLocks;
    fn total_balance(who: &T::AccountId, asset: Asset) -> T::Balance {
        let balance = Self::get_balance(&who, &asset);
        match balance {
            SignedBalance::Positive(balance) => balance,
            SignedBalance::Negative(_) => T::Balance::zero(),
        }
    }

    fn debt(who: &T::AccountId, asset: Asset) -> T::Balance {
        let balance = Self::get_balance(&who, &asset);
        match balance {
            SignedBalance::Negative(balance) => balance,
            SignedBalance::Positive(_) => T::Balance::zero(),
        }
    }

    fn currency_total_issuance(asset: Asset) -> T::Balance {
        T::Aggregates::get_total(UserGroup::Balances, asset).collateral
    }

    fn minimum_balance_value() -> T::Balance {
        Self::get_min_existential_deposit()
    }

    fn free_balance(who: &T::AccountId, asset: Asset) -> T::Balance {
        Self::total_balance(who, asset)
    }

    fn ensure_can_withdraw(
        who: &T::AccountId,
        asset: Asset,
        amount: T::Balance,
        withdraw_reasons: WithdrawReasons,
        _new_balance: T::Balance,
    ) -> DispatchResult {
        T::BalanceChecker::can_change_balance(
            &who,
            &vec![(asset, SignedBalance::Negative(amount))],
            Some(withdraw_reasons),
        )
        .map_err(|e| {
            log::error!(
                "{}:{}. Cannot change balance. Who: {:?}, amount: {:?}, currency: {:?}. {:?}",
                file!(),
                line!(),
                who,
                amount,
                str_asset!(asset),
                e
            );
            e
        })
    }

    fn currency_transfer(
        transactor: &T::AccountId,
        dest: &T::AccountId,
        asset: Asset,
        value: T::Balance,
        existence_requirement: ExistenceRequirement,
        transfer_reason: TransferReason,
        ensure_can_change: bool,
    ) -> DispatchResult {
        if value.is_zero() || transactor == dest {
            return Ok(());
        }

        Self::ensure_asset_exists(asset)?;

        // providers count is 0 for new account,
        // account may be empty (without balances)
        // but other pallets depends on this account, so we can't delete it

        let providers = frame_system::Pallet::<T>::providers(dest);

        if providers == 0 && existence_requirement == ExistenceRequirement::KeepAlive {
            let price = T::PriceGetter::get_price::<EqFixedU128>(&asset)?;
            let amount_in_usd = price
                .checked_mul_int(value)
                .ok_or(ArithmeticError::Overflow)?;
            let minimum_balance_value = Self::minimum_balance_value();

            eq_ensure!(amount_in_usd >= minimum_balance_value,
                Error::<T>::NotEnoughToKeepAlive,
                target: "eq_balances",
                "{}:{}. {:?} {:?} Not enough to keep account alive after first deposit. Who: {:?}, transactor {:?}.",
                file!(),
                line!(),
                asset,
                value,
                dest,
                transactor
            );
        } // AllowDeath, for new account will be removed by offchain worker

        if providers == 0 {
            frame_system::Pallet::<T>::inc_providers(dest);
        }

        T::AccountStore::mutate(transactor, |from_account| -> DispatchResult {
            T::AccountStore::mutate(dest, |to_account| -> DispatchResult {
                if ensure_can_change {
                    T::BalanceChecker::can_change_balance(
                        &transactor,
                        &vec![(asset, SignedBalance::Negative(value))],
                        None,
                    )?;

                    T::BalanceChecker::can_change_balance(
                        &dest,
                        &vec![(asset, SignedBalance::Positive(value))],
                        None,
                    )?;
                }

                let from_balance = from_account.entry(asset).or_default();
                let to_balance = to_account.entry(asset).or_default();

                let new_from_balance = from_balance
                    .sub_balance(&value)
                    .ok_or::<DispatchError>(ArithmeticError::Overflow.into())?;
                let new_to_balance = to_balance
                    .add_balance(&value)
                    .ok_or::<DispatchError>(ArithmeticError::Overflow.into())?;

                T::Aggregates::update_total(
                    &transactor,
                    asset,
                    from_balance,
                    &SignedBalance::Negative(value),
                )?;
                T::Aggregates::set_usergroup(&dest, UserGroup::Balances, true)?;
                T::Aggregates::update_total(
                    &dest,
                    asset,
                    to_balance,
                    &SignedBalance::Positive(value),
                )?;

                *from_balance = new_from_balance;
                *to_balance = new_to_balance;

                Ok(())
            })??;

            Self::deposit_event(Event::Transfer(
                transactor.clone(),
                dest.clone(),
                asset,
                value,
                transfer_reason,
            ));

            Ok(())
        })??;

        Ok(())
    }

    fn deposit_into_existing(
        who: &T::AccountId,
        asset: Asset,
        value: T::Balance,
        event: Option<DepositReason>,
    ) -> Result<(), DispatchError> {
        if value.is_zero() {
            return Ok(());
        }

        Self::ensure_asset_exists(asset)?;

        // providers count is 0 for new account,
        // account may be empty (without balances)
        // but other pallets depends on this account, so we can't delete it
        eq_ensure!(
            frame_system::Pallet::<T>::providers(who) != 0,
            Error::<T>::DeadAccount,
            target: "eq_balances",
            "{}:{}. Cannot deposit into new account. Who: {:?}.",
            file!(),
            line!(),
            who
        );

        T::AccountStore::mutate(who, |balances| -> DispatchResult {
            T::BalanceChecker::can_change_balance(
                &who,
                &vec![(asset, SignedBalance::Positive(value))],
                None,
            )
            .map_err(|error| {
                log::error!(
                    "{}:{}. Cannot change balance. Who: {:?}, amount: {:?}, currency: {:?}. {:?}",
                    file!(),
                    line!(),
                    who,
                    value,
                    str_asset!(asset),
                    error
                );
                error
            })?;

            let balance = balances.entry(asset).or_default();
            let new_balance = balance
                .add_balance(&value)
                .ok_or(ArithmeticError::Overflow)?;

            T::Aggregates::update_total(&who, asset, balance, &SignedBalance::Positive(value))?;
            *balance = new_balance;

            if let Some(deposit_reason) = event {
                Self::deposit_event(Event::Deposit(who.clone(), asset, value, deposit_reason))
            }
            Ok(())
        })??;

        Ok(())
    }

    fn deposit_creating(
        who: &T::AccountId,
        asset: Asset,
        value: T::Balance,
        ensure_can_change: bool,
        event: Option<DepositReason>,
    ) -> Result<(), sp_runtime::DispatchError> {
        if value.is_zero() {
            return Ok(());
        }

        Self::ensure_asset_exists(asset)?;

        let providers = frame_system::Pallet::<T>::providers(who);
        if providers == 0 {
            frame_system::Pallet::<T>::inc_providers(who);
        }

        T::AccountStore::mutate(who, |balances| -> DispatchResult {
            if !ensure_can_change
                || T::BalanceChecker::can_change_balance(
                    &who,
                    &vec![(asset, SignedBalance::Positive(value))],
                    None,
                )
                .map_or_else(|_| false, |_| true)
            {
                let balance = balances.entry(asset).or_default();
                let new_balance = balance
                    .add_balance(&value)
                    .ok_or(ArithmeticError::Overflow)?;

                T::Aggregates::set_usergroup(&who, UserGroup::Balances, true)?;
                T::Aggregates::update_total(&who, asset, balance, &SignedBalance::Positive(value))?;
                *balance = new_balance;
                Ok(())
            } else {
                log::trace!(target: "eq_balances",
                        "deposit_creating error who:{:?}, currency:{:?}, value:{:?}, ensure_can_change:{:?}",
                        *who, str_asset!(asset), value, ensure_can_change);
                Ok(())
            }
        })??;

        if let Some(deposit_reason) = event {
            Self::deposit_event(Event::Deposit(who.clone(), asset, value, deposit_reason))
        }

        Ok(())
    }

    fn withdraw(
        who: &T::AccountId,
        asset: Asset,
        value: T::Balance,
        ensure_can_change: bool,
        event: Option<WithdrawReason>,
        withdraw_reasons: WithdrawReasons,
        _liveness: ExistenceRequirement,
    ) -> Result<(), sp_runtime::DispatchError> {
        if value.is_zero() {
            return Ok(());
        }

        Self::ensure_asset_exists(asset)?;

        T::AccountStore::mutate(who, |balances| -> DispatchResult {
            let balance = balances.entry(asset).or_default();
            let new_balance = balance
                .sub_balance(&value)
                .ok_or(ArithmeticError::Overflow)?;
            // if new balance is not negative then heave checks not needed
            if matches!(new_balance, SignedBalance::Negative(_)) && ensure_can_change {
                T::BalanceChecker::can_change_balance(
                    &who,
                    &vec![(asset,SignedBalance::Negative(value))],
                    Some(withdraw_reasons),
                )
                .map_err(|error| {
                    log::error!(
                        "{}:{}. Cannot change balance. Who: {:?}, amount: {:?}, currency: {:?}. {:?}",
                        file!(),
                        line!(),
                        who,
                        value,
                        str_asset!(asset),
                        error
                    );
                    error
                })?;
            }

            T::Aggregates::update_total(&who, asset, balance, &SignedBalance::Negative(value))?;
            *balance = new_balance;
            Ok(())
        })??;

        if let Some(withdraw_reason) = event {
            Self::deposit_event(Event::Withdraw(who.clone(), asset, value, withdraw_reason));
        }

        Ok(())
    }

    // Only for tests. #[cfg(test)] is not visible in other pallets
    #[cfg(any(feature = "runtime-benchmarks", feature = "std"))]
    fn make_free_balance_be(who: &T::AccountId, asset: Asset, value: SignedBalance<T::Balance>) {
        let delta = value - Self::get_balance(who, &asset);
        match delta {
            Positive(d) => {
                Self::deposit_creating(who, asset, d, false, None)
                    .expect("deposit_creating failed");
            }
            Negative(d) => {
                let providers = frame_system::Pallet::<T>::providers(who);
                if providers == 0 {
                    frame_system::Pallet::<T>::inc_providers(who);
                }
                Self::withdraw(
                    who,
                    asset,
                    d,
                    false,
                    None,
                    WithdrawReasons::empty(),
                    ExistenceRequirement::AllowDeath,
                )
                .expect("withdraw failed");
            }
        }
    }

    fn can_be_deleted(who: &T::AccountId) -> Result<bool, sp_runtime::DispatchError> {
        if T::SubaccountsManager::get_owner_id(who).is_some() {
            return Ok(false);
        }

        if !AccountRefCounter::<T>::can_be_deleted(
            &who,
            T::SubaccountsManager::get_subaccounts_amount(who),
        ) {
            return Ok(false);
        }

        let has_orders = |subacc_type| {
            T::SubaccountsManager::get_subaccount_id(who, &subacc_type)
                .map(|subaccount_id| {
                    T::OrderAggregates::get_asset_weights(&subaccount_id).len() != 0
                })
                .unwrap_or(false)
        };

        if has_orders(SubAccType::Trader) || has_orders(SubAccType::Borrower) {
            return Ok(false);
        }

        let DebtCollateralDiscounted {
            mut debt,
            mut collateral,
            discounted_collateral: _,
        } = Self::get_debt_and_collateral(who)?;
        for ref subacc_type in SubAccType::iterator() {
            if let Some(subacc_id) = T::SubaccountsManager::get_subaccount_id(who, subacc_type) {
                let DebtCollateralDiscounted {
                    debt: sub_debt,
                    collateral: sub_collat,
                    discounted_collateral: _,
                } = Self::get_debt_and_collateral(&subacc_id)?;
                collateral = collateral + sub_collat;
                debt = debt + sub_debt;
            }
        }

        let minimum_balance_value = Self::minimum_balance_value();
        if collateral.saturating_sub(debt) >= minimum_balance_value {
            return Ok(false);
        }

        Ok(true)
    }

    fn delete_account(who: &T::AccountId) -> Result<(), sp_runtime::DispatchError> {
        let can_be_deleted = Self::can_be_deleted(who)?;

        eq_ensure!(
            can_be_deleted,
            Error::<T>::NotAllowedToDeleteAccount,
            target: "eq_balances",
            "{}:{}. Can not delete account. Who: {:?}",
            file!(),
            line!(),
            who
        );

        log::trace!(target: "eq_balances", "Delete {:?}\n", who.clone());

        for subacc_type in SubAccType::iterator() {
            if let Some(subacc_id) = T::SubaccountsManager::get_subaccount_id(who, &subacc_type) {
                Self::unreserve_all(&subacc_id);
                // don't care about error here
                // because delete is calling after checking balances and getting prices
                let _ = T::BailsmenManager::receive_position(&subacc_id, true);
                T::Aggregates::set_usergroup(&subacc_id, UserGroup::Balances, false)
                    .expect("set_usergroup failure");
                // Subaccount deleting also deletes from corresponding usergroup.
                T::SubaccountsManager::delete_subaccount_inner(who, &subacc_type)
                    .expect("delete_subaccount_inner failure");
                Locked::<T>::remove(subacc_id);
            }
        }

        // main account
        Self::unreserve_all(who);
        T::Aggregates::set_usergroup(who, UserGroup::Balances, false)?;
        // don't care about error here for the same reason
        let _ = T::BailsmenManager::receive_position(who, true);
        T::UpdateTimeManager::remove_last_update(who);

        // dec_providers also remove account
        // we checked that it's last provider
        frame_system::Pallet::<T>::dec_providers(who).expect("Unexpected dec_providers error");

        Locked::<T>::remove(who);

        Self::deposit_event(Event::DeleteAccount(who.clone()));

        Ok(())
    }

    fn exchange(
        accounts: (&T::AccountId, &T::AccountId),
        assets: (&Asset, &Asset),
        values: (T::Balance, T::Balance),
    ) -> Result<(), (DispatchError, Option<T::AccountId>)> {
        if assets.0 == assets.1 {
            frame_support::log::error!(
                "{}:{}. Exchange same assets. Who: {:?}, amounts: {:?}, asset {:?}.",
                file!(),
                line!(),
                accounts,
                values,
                str_asset!(assets.0)
            );
            return Err((Error::<T>::ExchangeSameAsset.into(), None));
        }

        if values.0.is_zero() && values.1.is_zero() || accounts.0 == accounts.1 {
            return Ok(());
        }

        let mut err_acc: Option<T::AccountId> = None;

        T::AccountStore::mutate(accounts.0, |balances1| -> DispatchResult {
            T::AccountStore::mutate(accounts.1, |balances2| -> DispatchResult {
                T::BalanceChecker::can_change_balance(
                    &accounts.0,
                    &vec![
                        (*assets.0, SignedBalance::Negative(values.0)),
                        (*assets.1, SignedBalance::Positive(values.1)),
                    ],
                    None,
                )
                .map_err(|err| {
                    err_acc = Some(accounts.0.clone());
                    err
                })?;
                T::BalanceChecker::can_change_balance(
                    &accounts.1,
                    &vec![
                        (*assets.0, SignedBalance::Positive(values.0)),
                        (*assets.1, SignedBalance::Negative(values.1)),
                    ],
                    None,
                )
                .map_err(|err| {
                    err_acc = Some(accounts.1.clone());
                    err
                })?;

                {
                    let account1_balance1 = balances1.entry(*assets.0).or_default();
                    let new_account1_balance1 =
                        account1_balance1.sub_balance(&values.0).ok_or_else(|| {
                            err_acc = Some(accounts.0.clone());
                            ArithmeticError::Overflow
                        })?;
                    T::Aggregates::update_total(
                        &accounts.0,
                        *assets.0,
                        account1_balance1,
                        &SignedBalance::Negative(values.0),
                    )?;
                    *account1_balance1 = new_account1_balance1;
                }

                {
                    let account2_balance1 = balances2.entry(*assets.0).or_default();
                    let new_account2_balance1 =
                        account2_balance1.add_balance(&values.0).ok_or_else(|| {
                            err_acc = Some(accounts.1.clone());
                            ArithmeticError::Overflow
                        })?;
                    T::Aggregates::update_total(
                        &accounts.1,
                        *assets.0,
                        account2_balance1,
                        &SignedBalance::Positive(values.0),
                    )?;
                    *account2_balance1 = new_account2_balance1;
                }

                let account1_balance2 = balances1.entry(*assets.1).or_default();

                let new_account1_balance2 =
                    account1_balance2.add_balance(&values.1).ok_or_else(|| {
                        err_acc = Some(accounts.0.clone());
                        ArithmeticError::Overflow
                    })?;

                T::Aggregates::update_total(
                    &accounts.0,
                    *assets.1,
                    account1_balance2,
                    &SignedBalance::Positive(values.1),
                )?;

                *account1_balance2 = new_account1_balance2;

                let account2_balance2 = balances2.entry(*assets.1).or_default();

                let new_account2_balance2 =
                    account2_balance2.sub_balance(&values.1).ok_or_else(|| {
                        err_acc = Some(accounts.1.clone());
                        ArithmeticError::Overflow
                    })?;

                T::Aggregates::update_total(
                    &accounts.1,
                    *assets.1,
                    account2_balance2,
                    &SignedBalance::Negative(values.1),
                )?;

                *account2_balance2 = new_account2_balance2;

                Ok(())
            })??;

            Self::deposit_event(Event::Exchange(
                accounts.0.clone(),
                *assets.0,
                values.0,
                accounts.1.clone(),
                *assets.1,
                values.1,
            ));

            Ok(())
        })
        .map_err(|err| err.into())
        .and_then(|res| res)
        .map_err(|err| (err.into(), err_acc))?;

        Ok(())
    }

    fn reserved_balance(who: &T::AccountId, asset: Asset) -> T::Balance {
        Reserved::<T>::get(who, asset)
    }

    fn reserve(who: &T::AccountId, asset: Asset, amount: T::Balance) -> DispatchResult {
        Reserved::<T>::try_mutate_exists(who, asset, |maybe_reserved| {
            let reserve_account_id = T::ModuleId::get().into_account_truncating();
            Self::currency_transfer(
                &who,
                &reserve_account_id,
                asset,
                amount,
                ExistenceRequirement::KeepAlive,
                TransferReason::Reserve,
                true,
            )?;

            match maybe_reserved.as_mut() {
                Some(reserved) => {
                    *reserved = reserved
                        .checked_add(&amount)
                        .ok_or(ArithmeticError::Overflow)?;
                }
                None => {
                    *maybe_reserved = Some(amount);
                }
            }

            Ok(())
        })
    }

    fn unreserve(who: &T::AccountId, asset: Asset, amount: T::Balance) -> T::Balance {
        Reserved::<T>::mutate_exists(who, &asset, |maybe_reserved| {
            match maybe_reserved.as_mut() {
                None => T::Balance::zero(),
                Some(reserved) => {
                    let reserve_account_id = T::ModuleId::get().into_account_truncating();

                    let amount_to_unreserve = (*reserved).min(amount);

                    let unreserved = match Self::currency_transfer(
                        &reserve_account_id,
                        &who,
                        asset,
                        amount_to_unreserve,
                        ExistenceRequirement::AllowDeath,
                        TransferReason::Unreserve,
                        true,
                    ) {
                        Ok(_) => amount_to_unreserve,
                        Err(_) => T::Balance::zero(),
                    };

                    *reserved = *reserved - unreserved;

                    if *reserved == T::Balance::zero() {
                        *maybe_reserved = None;
                    }

                    unreserved
                }
            }
        })
    }

    fn slash_reserved(
        who: &T::AccountId,
        asset: Asset,
        value: T::Balance,
    ) -> (NegativeImbalance<T::Balance>, T::Balance) {
        let reserve_account_id = T::ModuleId::get().into_account_truncating();
        let reserved = Reserved::<T>::get(who, asset);
        let to_slash = reserved.min(value);

        match Self::withdraw(
            &reserve_account_id,
            asset,
            to_slash,
            false,
            None,
            WithdrawReasons::RESERVE,
            ExistenceRequirement::AllowDeath,
        ) {
            Ok(_) => {
                let new_reserved = reserved - to_slash;
                if new_reserved.is_zero() {
                    Reserved::<T>::remove(who, asset);
                } else {
                    Reserved::<T>::insert(who, asset, new_reserved);
                }
                (NegativeImbalance::new(to_slash), new_reserved)
            }
            Err(e) => {
                log::error!("Slash reserved failed: {:?}", e);
                (NegativeImbalance::zero(), reserved)
            }
        }
    }

    fn repatriate_reserved(
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        asset: Asset,
        value: T::Balance,
        status: BalanceStatus,
    ) -> Result<T::Balance, DispatchError> {
        let reserve_account_id = T::ModuleId::get().into_account_truncating();
        let reserved = Reserved::<T>::get(slashed, asset);
        let to_slash = reserved.min(value);

        if reserved.is_zero() {
            return Ok(T::Balance::zero());
        }

        match status {
            BalanceStatus::Free => {
                Self::currency_transfer(
                    &reserve_account_id,
                    beneficiary,
                    asset,
                    to_slash,
                    ExistenceRequirement::KeepAlive,
                    TransferReason::Reserve,
                    true,
                )?;
            }
            BalanceStatus::Reserved => {
                Reserved::<T>::mutate(beneficiary, asset, |old| {
                    *old = old.checked_add(&to_slash)?;
                    Some(())
                })
                .ok_or(ArithmeticError::Overflow)?;
            }
        }

        let new_reserved = reserved - to_slash;
        if new_reserved.is_zero() {
            Reserved::<T>::remove(slashed, asset);
        } else {
            Reserved::<T>::insert(slashed, asset, new_reserved);
        }
        Ok(new_reserved)
    }

    fn xcm_transfer(
        from: &T::AccountId,
        asset: Asset,
        amount: T::Balance,
        kind: XcmDestination,
    ) -> DispatchResult {
        Self::can_send_xcm(&asset, &amount)?;

        Self::do_xcm_transfer_old(
            from.clone(),
            asset,
            amount,
            kind,
            XcmTransferDealWithFee::SovereignAccWillPay,
        )?;

        Ok(().into())
    }

    fn set_lock(id: LockIdentifier, who: &T::AccountId, amount: T::Balance) {
        let new_locked: T::Balance = Locked::<T>::mutate(who, |map| {
            if !amount.is_zero() {
                *map.entry(id).or_default() = amount;
            } else {
                map.remove(&id);
            }
            map.values().cloned().max().unwrap_or_default()
        });

        let _ = T::AccountStore::mutate(who, |balances| match balances {
            AccountData::V0 {
                balance: _,
                ref mut lock,
            } => {
                *lock = new_locked;
            }
        });
    }

    fn extend_lock(id: LockIdentifier, who: &T::AccountId, amount: T::Balance) {
        if !amount.is_zero() {
            Locked::<T>::mutate(who, |map| {
                let lock = map.entry(id).or_default();
                if *lock < amount {
                    *lock = amount;
                }
            });

            let _ = T::AccountStore::mutate(who, |balances| match balances {
                AccountData::V0 {
                    balance: _,
                    ref mut lock,
                } => {
                    if *lock < amount {
                        *lock = amount;
                    }
                }
            });
        }
    }

    fn remove_lock(id: LockIdentifier, who: &T::AccountId) {
        let new_locked: T::Balance = Locked::<T>::mutate(who, |map| {
            map.remove(&id);
            map.values().cloned().max().unwrap_or_default()
        });

        let _ = T::AccountStore::mutate(who, |balances| match balances {
            AccountData::V0 {
                balance: _,
                ref mut lock,
            } => {
                *lock = new_locked;
            }
        });
    }
}

impl<T: Config> eq_primitives::IsTransfersEnabled for Pallet<T> {
    fn get() -> bool {
        <IsTransfersEnabled<T>>::get()
    }
}

impl<T: Config> Get<Option<XcmMode>> for Pallet<T> {
    fn get() -> Option<XcmMode> {
        IsXcmTransfersEnabled::<T>::get()
    }
}

impl<T: Config> Pallet<T> {
    fn ensure_transfers_enabled(asset: &Asset, amount: T::Balance) -> DispatchResult {
        let is_enabled = <Self as eq_primitives::IsTransfersEnabled>::get();
        eq_ensure!(
            is_enabled,
            Error::<T>::TransfersAreDisabled,
            target: "eq_balances",
            "{}:{}. Transfers is not allowed. amount: {:?}, asset: {:?}.",
            file!(),
            line!(),
            amount,
            str_asset!(asset)
        );

        Ok(())
    }

    fn can_send_xcm(asset: &Asset, amount: &T::Balance) -> DispatchResult {
        eq_ensure!(
            IsXcmTransfersEnabled::<T>::get().map(|v| match v {
                XcmMode::Xcm(enabled) => enabled,
                XcmMode::Bridge(enabled) => enabled,
            }).unwrap_or(false),
            Error::<T>::XcmDisabled,
            target: "eq_balances",
            "{}:{}. XCM is not allowed. amount: {:?}, asset: {:?}.",
            file!(),
            line!(),
            amount,
            str_asset!(asset),
        );

        Ok(())
    }

    fn can_send_xcm_for_users(asset: &Asset, amount: &T::Balance) -> DispatchResult {
        eq_ensure!(
            IsXcmTransfersEnabled::<T>::get() == Some(XcmMode::Xcm(true)),
            Error::<T>::XcmDisabled,
            target: "eq_balances",
            "{}:{}. XCM is not allowed. amount: {:?}, asset: {:?}.",
            file!(),
            line!(),
            amount,
            str_asset!(asset),
        );

        Ok(())
    }

    fn ensure_asset_exists(asset: Asset) -> DispatchResult {
        match T::AssetGetter::get_asset_data(&asset) {
            Ok(_) => Ok(()),
            Err(err) => {
                log::error!(
                    "{}:{}. Deposit/transfer for unknown currency is not allowed. Asset: {:?}.",
                    file!(),
                    line!(),
                    str_asset!(asset)
                );
                Err(err)
            }
        }
    }

    fn ensure_xdot_swap_allowed(xdot_assets: &Vec<XDotAsset>) -> DispatchResult {
        let allowed = AllowedXdotsSwap::<T>::get();

        eq_ensure!(
            xdot_assets.iter().all(|a| allowed.contains(a)),
            Error::<T>::XDotSwapNotAllowed,
            target: "eq_balances",
            "{}:{}. Swap is not allowed for the specified XDOT asset.",
            file!(),
            line!(),
        );

        Ok(())
    }

    fn unreserve_all(who: &T::AccountId) {
        Reserved::<T>::iter_prefix(&who).for_each(|(asset, reserved)| {
            Self::unreserve(who, asset, reserved);
        });
    }

    fn is_not_subaccount(who: &T::AccountId) -> bool {
        T::SubaccountsManager::get_owner_id(who).is_none()
    }

    fn xcm_data(asset: &Asset) -> Result<(MultiLocation, u8, bool), DispatchError> {
        Ok(T::AssetGetter::get_asset_data(asset)?
            .get_xcm_data()
            .ok_or(Error::<T>::XcmUnknownAsset)?)
    }

    fn ensure_xcm_transfer_limit_not_exceeded(
        account_id: &T::AccountId,
        amount: T::Balance,
    ) -> DispatchResult {
        if let Some(transfer_limit) = DailyXcmLimit::<T>::get() {
            let now = T::UnixTime::now().as_secs();
            let current_period = (now / XCM_LIMIT_PERIOD_IN_SEC) * XCM_LIMIT_PERIOD_IN_SEC;
            let (mut transferred, last_transfer) = XcmNativeTransfers::<T>::get(account_id)
                .ok_or(Error::<T>::XcmTransfersNotAllowedForAccount)?;

            if last_transfer < current_period {
                transferred = Default::default();
                XcmNativeTransfers::<T>::insert(account_id, (transferred, now));
            };

            ensure!(
                transferred + amount <= transfer_limit,
                Error::<T>::XcmTransfersLimitExceeded
            );
        }

        Ok(())
    }

    fn update_xcm_native_transfers(account_id: &T::AccountId, amount: T::Balance) {
        if DailyXcmLimit::<T>::get().is_some() {
            XcmNativeTransfers::<T>::mutate_exists(
                account_id,
                |maybe_transfer| match maybe_transfer {
                    Some((current_amount, last_transfer)) => {
                        *current_amount = *current_amount + amount;
                        *last_transfer = T::UnixTime::now().as_secs();
                    }
                    None => {}
                },
            );
        }
    }

    fn get_locked(who: &T::AccountId) -> T::Balance {
        match T::AccountStore::get(who) {
            AccountData::V0 { balance: _, lock } => lock,
        }
    }

    /// Get minimum (T::ExistentialDeposit, T::ExistentialDepositBasic) worth in USD.
    fn get_min_existential_deposit() -> T::Balance {
        let existential_deposit_usd = T::ExistentialDeposit::get();

        match T::PriceGetter::get_price::<EqFixedU128>(&T::AssetGetter::get_main_asset()) {
            Ok(basic_asset_price) => {
                match basic_asset_price.checked_mul_int(T::ExistentialDepositBasic::get()) {
                    Some(existential_deposit_basic_in_usd) => {
                        existential_deposit_usd.min(existential_deposit_basic_in_usd)
                    }
                    _ => existential_deposit_usd,
                }
            }
            _ => existential_deposit_usd,
        }
    }
}

impl<T: Config> LockGetter<T::AccountId, T::Balance> for Pallet<T> {
    fn get_lock(who: T::AccountId, id: LockIdentifier) -> T::Balance {
        *Locked::<T>::get(who)
            .get(&id)
            .unwrap_or(&T::Balance::zero())
    }
}

pub struct XcmDestinationResolved {
    destination: MultiLocation,
    asset_location: MultiLocation,
    beneficiary: MultiLocation,
}

#[derive(Decode, Encode, Clone, Debug, PartialEq, scale_info::TypeInfo)]
pub enum XDotAsset {
    XDOT,
    XDOT2,
    XDOT3,
}
