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

//! # Equilibrium Treasury Pallet
//!
//! Equilibrium's Treasury Pallet is a Substrate module which:
//! 1. Manages and stores user transaction fees.
//! 2. Charges treasury fee i.e. a small fee on an active debt.
//! 3. Provides a conversion of non-basic assets to the basic asset in case when an account must pay some fee
//! yet lacks sufficient funds in the basic asset (Treasury buyout).

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

pub mod benchmarking;
mod mock;
mod tests;
pub mod weights;
pub use weights::WeightInfo;

use codec::{Codec, Decode, Encode, MaxEncodedLen};
use core::convert::{TryFrom, TryInto};
use eq_balances::NegativeImbalance;
use eq_primitives::{
    asset::{Asset, AssetGetter},
    balance::{BalanceGetter, EqCurrency},
    balance_number::EqFixedU128,
    EqBuyout, PriceGetter, SignedBalance,
};
#[allow(unused_imports)]
use eq_primitives::{AccountRefCounter, AccountRefCounts};
use eq_utils::{eq_ensure, multiply_by_rational};
use frame_support::traits::IsSubType;
use frame_support::{
    dispatch::DispatchResult,
    ensure, fail,
    traits::{ExistenceRequirement, Get, OnUnbalanced, UnixTime},
    PalletId, Parameter,
};
use frame_support::{pallet_prelude::DispatchResultWithPostInfo, traits::Imbalance};
use frame_system as system;
use sp_arithmetic::{FixedPointNumber, FixedPointOperand};
use sp_runtime::{
    traits::{AccountIdConversion, AtLeast32BitUnsigned, MaybeSerializeDeserialize, Member, Zero},
    traits::{DispatchInfoOf, One, SignedExtension},
    transaction_validity::{
        InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction,
    },
    DispatchError,
};
use sp_runtime::{ArithmeticError, Permill};
use sp_std::{collections::btree_map::BTreeMap, fmt::Debug, marker::PhantomData, vec::Vec};
use system::ensure_signed;

pub use pallet::*;

const BUYOUT_LIMIT_PERIOD_IN_SEC: u64 = 86400; // 1 day

/// Type of amount
#[derive(
    Copy, Clone, Debug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum Amount<Balance> {
    /// Amount of native asset user get for buyout
    Buyout(Balance),
    /// Amount of exchange asset user give for buyout
    Exchange(Balance),
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_authorship::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Pallets AccountId for balances
        type PalletId: Get<PalletId>;
        /// Numerical representation of stored balances
        type Balance: Member
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Codec
            + Copy
            + Parameter
            + Default
            + TryFrom<eq_primitives::balance::Balance>
            + Into<eq_primitives::balance::Balance>
            + MaxEncodedLen
            + FixedPointOperand;
        /// Gets users balances to calculate the fees and check margin call conditions
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;
        /// Used for currency-related operations and calculations
        type EqCurrency: EqCurrency<Self::AccountId, Self::Balance>;
        /// Gets currency prices from oracle
        type PriceGetter: PriceGetter;
        /// Timestamp provider
        type UnixTime: UnixTime;
        /// Fee from collateral buyouts (any currency that is not basic asset)
        #[pallet::constant]
        type BuyFee: Get<Permill>;
        /// Fee from the basic asset buyouts
        #[pallet::constant]
        type SellFee: Get<Permill>;
        /// Used to deal with Assets
        type AssetGetter: AssetGetter;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        /// Min amount of native token to buyout
        #[pallet::constant]
        type MinAmountToBuyout: Get<Self::Balance>;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Let user to exchange existed asset to native asset by oracle price plus fee
        /// Parameters:
        /// `asset` - asset to exchange
        /// `amount` - amount of native asset user will get after buyout
        ///            or amount of exchange asset user will give for buyout
        #[pallet::call_index(0)]
        #[pallet::weight((T::WeightInfo::buyout(), Pays::No))]
        pub fn buyout(
            origin: OriginFor<T>,
            asset: Asset,
            amount: Amount<T::Balance>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_buyout(who, asset, amount)?;
            Ok(().into())
        }

        /// Set/unset buyout limit
        /// Parameters:
        /// `limit` - max value of native token user could get with help of buyout for a period(day), None - to disable buyout limits
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update_buyout_limit())]
        pub fn update_buyout_limit(
            origin: OriginFor<T>,
            limit: Option<T::Balance>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            match limit {
                Some(limit) => BuyoutLimit::<T>::put(limit),
                None => BuyoutLimit::<T>::kill(),
            }

            Ok(().into())
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Attempt to exchange native token to native token
        WrongAssetToBuyout,
        /// Daily buyout limit exceeded
        BuyoutLimitExceeded,
        /// One of transacted currencies is missing price information
        /// or the price is outdated
        NoPrice,
        /// The treasury balance is too low for an operation
        InsufficientTreasuryBalance,
        /// The account balance is too low for an operation
        InsufficientAccountBalance,
    }

    /// Stores limit amount user could by for a period.
    /// When `None` - buyouts not limited
    #[pallet::storage]
    pub type BuyoutLimit<T: Config> = StorageValue<_, T::Balance, OptionQuery>;

    /// Stores amount of buyouts (amount, timestamp of last buyout)
    #[pallet::storage]
    pub type Buyouts<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::Balance, u64), ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Buyout event
        Buyout {
            who: T::AccountId,
            buyout_amount: T::Balance,
            asset: Asset,
            exchange_amount: T::Balance,
        },
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub empty: PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};
            let extra_genesis_builder: fn(&Self) = |_: &GenesisConfig<T>| {
                EqPalletAccountInitializer::<T>::initialize(&Pallet::<T>::account_id());
            };
            extra_genesis_builder(self);
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Returns the module account id
    pub fn account_id() -> T::AccountId {
        T::PalletId::get().into_account_truncating()
    }

    fn ensure_buyout_limit_not_exceeded(
        account_id: &T::AccountId,
        buyout_amount: T::Balance,
    ) -> DispatchResult {
        if let Some(buyout_limit) = BuyoutLimit::<T>::get() {
            let now = T::UnixTime::now().as_secs();
            let current_period = (now / BUYOUT_LIMIT_PERIOD_IN_SEC) * BUYOUT_LIMIT_PERIOD_IN_SEC;
            let (mut buyouts, last_buyout) = Buyouts::<T>::get(account_id);

            if !buyouts.is_zero() && last_buyout < current_period {
                buyouts = Default::default();
                Buyouts::<T>::insert(account_id, (buyouts, now));
            };

            ensure!(
                buyouts + buyout_amount <= buyout_limit,
                Error::<T>::BuyoutLimitExceeded
            );
        }

        Ok(())
    }

    fn update_buyouts(account_id: &T::AccountId, buyout_amount: T::Balance) {
        if BuyoutLimit::<T>::get().is_some() {
            Buyouts::<T>::mutate(account_id, |(prev_buyouts, last)| {
                *prev_buyouts = *prev_buyouts + buyout_amount;
                *last = T::UnixTime::now().as_secs();
            });
        }
    }

    fn calc_amount_to_exchange(
        asset: Asset,
        buyout_amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        let basic_asset = T::AssetGetter::get_main_asset();
        eq_ensure!(
            asset != basic_asset,
            Error::<T>::WrongAssetToBuyout,
            "{}:{}. Exchange same assets forbidden",
            file!(),
            line!(),
        );

        let basic_asset_price_with_fee = {
            let basic_asset_price: EqFixedU128 = T::PriceGetter::get_price(&basic_asset)?;
            basic_asset_price * (EqFixedU128::from(T::SellFee::get()) + EqFixedU128::one())
        };
        let exchange_asset_price: EqFixedU128 = T::PriceGetter::get_price(&asset)?;

        let exchange_amount = multiply_by_rational(
            buyout_amount,
            basic_asset_price_with_fee.into_inner(),
            exchange_asset_price.into_inner(),
        )
        .map(|n| n.try_into().ok())
        .flatten()
        .ok_or(ArithmeticError::Overflow.into());

        exchange_amount
    }

    fn calc_buyout_amount(
        asset: Asset,
        exchange_amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        let basic_asset = T::AssetGetter::get_main_asset();
        eq_ensure!(
            asset != basic_asset,
            Error::<T>::WrongAssetToBuyout,
            "{}:{}. Exchange same assets forbidden",
            file!(),
            line!(),
        );

        let basic_asset_price_with_fee = {
            let basic_asset_price: EqFixedU128 = T::PriceGetter::get_price(&basic_asset)?;
            (basic_asset_price * (EqFixedU128::from(T::SellFee::get()) + EqFixedU128::one()))
                .into_inner()
        };
        let exchange_asset_price = T::PriceGetter::get_price::<EqFixedU128>(&asset)?.into_inner();

        let buyout_amount = multiply_by_rational(
            exchange_amount,
            exchange_asset_price,
            basic_asset_price_with_fee,
        )
        .map(|b| b.try_into().ok())
        .flatten()
        .ok_or(ArithmeticError::Overflow.into());

        buyout_amount
    }

    fn split_to_buyout_and_exchange(
        asset: Asset,
        amount: Amount<T::Balance>,
    ) -> Result<(T::Balance, T::Balance), DispatchError> {
        match amount {
            Amount::Buyout(buyout_amount) => {
                let exchange_amount = Self::calc_amount_to_exchange(asset, buyout_amount)?;
                Ok((buyout_amount, exchange_amount))
            }
            Amount::Exchange(exchange_amount) => {
                let buyout_amount = Self::calc_buyout_amount(asset, exchange_amount)?;
                Ok((buyout_amount, exchange_amount))
            }
        }
    }

    fn do_buyout(who: T::AccountId, asset: Asset, amount: Amount<T::Balance>) -> DispatchResult {
        let (buyout_amount, exchange_amount) = Self::split_to_buyout_and_exchange(asset, amount)?;
        Self::ensure_buyout_limit_not_exceeded(&who, buyout_amount)?;
        let basic_asset = T::AssetGetter::get_main_asset();
        let self_account_id = Self::account_id();

        T::EqCurrency::exchange(
            (&who, &self_account_id),
            (&asset, &basic_asset),
            (exchange_amount, buyout_amount),
        )
        .map_err(|(error, maybe_acc)| match maybe_acc {
            Some(acc) => {
                if acc == self_account_id {
                    Error::<T>::InsufficientTreasuryBalance.into()
                } else if acc == who {
                    Error::<T>::InsufficientAccountBalance.into()
                } else {
                    error
                }
            }
            _ => error,
        })?;

        Self::update_buyouts(&who, buyout_amount);
        Self::deposit_event(Event::<T>::Buyout {
            who,
            buyout_amount,
            asset,
            exchange_amount,
        });

        Ok(())
    }

    /// Gets priority value for a currency. Priority value determines which currency
    /// will be used first to withdraw fees when account has insufficient basic_asset
    fn get_currency_priority(asset: Asset) -> u64 {
        let asset_data = T::AssetGetter::get_asset_data(&asset).expect("Unknown asset");
        asset_data.buyout_priority
    }
}

/// Manager for treasury `basic_asset` exchanging transactions
impl<T: Config> EqBuyout<T::AccountId, T::Balance> for Pallet<T> {
    /// Buyout `amount` of `basic_asset` from the Treasury. Account `who` pays for it with its
    /// funds accordingly to buyout/exchange priority (see `get_currency_priority`)
    fn eq_buyout(who: &T::AccountId, amount: T::Balance) -> DispatchResult {
        let basic_asset = T::AssetGetter::get_main_asset();
        let self_account_id = Self::account_id();
        let mut account_balances: Vec<_> = T::BalanceGetter::iterate_account_balances(who).into();

        let mut prices = account_balances
            .iter()
            .map(|(c, _)| {
                let price = T::PriceGetter::get_price(c).map_err(|_| {
                    log::error!("{}:{}.", file!(), line!());
                    Error::<T>::NoPrice
                })?;
                Ok((c.clone(), price))
            })
            .collect::<Result<BTreeMap<Asset, EqFixedU128>, DispatchError>>()?;

        let basic_token_price = T::PriceGetter::get_price(&basic_asset).map_err(|_| {
            log::error!("{}:{}.", file!(), line!());
            Error::<T>::NoPrice
        })?;
        prices.insert(basic_asset, basic_token_price);

        // check basic asset module balance and issue if not enough
        let self_eq_balance = T::BalanceGetter::get_balance(&self_account_id, &basic_asset);
        frame_support::ensure!(
            self_eq_balance >= SignedBalance::Positive(amount),
            Error::<T>::InsufficientTreasuryBalance
        );

        // sorting account balances according to buyout priority
        account_balances.sort_by(|a, b| {
            Self::get_currency_priority(a.0).cmp(&Self::get_currency_priority(b.0))
        });

        let basic_token_price = *prices.get(&basic_asset).unwrap();
        let basic_token_price_with_fee = (basic_token_price
            * (EqFixedU128::from(T::SellFee::get()) + EqFixedU128::one()))
        .into_inner();

        let mut amount_left = amount.into();

        for (asset, signed_balance) in account_balances {
            //skip basic asset token
            if asset == basic_asset {
                continue;
            }

            match signed_balance {
                SignedBalance::Positive(balance) => {
                    let currency_price = prices.get(&asset).unwrap().into_inner();
                    let balance_in_eq =
                        multiply_by_rational(balance, currency_price, basic_token_price_with_fee)
                            .ok_or(ArithmeticError::Overflow)?;

                    // we are just moving assets, so no aggregates were changed
                    // we are using "unsafe" changes because it can result in a margin call
                    if balance_in_eq < amount_left {
                        T::EqCurrency::currency_transfer(
                            who,
                            &self_account_id,
                            asset,
                            balance,
                            ExistenceRequirement::KeepAlive,
                            eq_primitives::TransferReason::TreasuryEqBuyout,
                            false,
                        )
                        .expect("currency_transfer failure");

                        amount_left = amount_left - balance_in_eq;
                    } else {
                        let balance_to_change = multiply_by_rational(
                            amount_left,
                            basic_token_price_with_fee,
                            currency_price,
                        )
                        .map(|b| b.try_into().ok())
                        .flatten()
                        .ok_or(ArithmeticError::Overflow)?;

                        T::EqCurrency::currency_transfer(
                            who,
                            &self_account_id,
                            asset,
                            balance_to_change,
                            ExistenceRequirement::KeepAlive,
                            eq_primitives::TransferReason::TreasuryEqBuyout,
                            false,
                        )
                        .expect("currency_transfer failure");

                        break;
                    }
                }
                _ => {
                    // nothing to do with debt
                }
            }
        }

        T::EqCurrency::currency_transfer(
            &self_account_id,
            who,
            basic_asset,
            amount,
            ExistenceRequirement::KeepAlive,
            eq_primitives::TransferReason::TreasuryEqBuyout,
            false,
        )
        .expect("currency transfer failure");

        Ok(())
    }

    /// Determine if the amount in asset is enough for a buyout
    fn is_enough(
        asset: Asset,
        amount: T::Balance,
        amount_buyout: T::Balance,
    ) -> Result<bool, DispatchError> {
        let basic_asset = T::AssetGetter::get_main_asset();
        let basic_token_price =
            T::PriceGetter::get_price::<EqFixedU128>(&basic_asset).map_err(|_| {
                log::error!("{}:{}.", file!(), line!());
                Error::<T>::NoPrice
            })?;

        let basic_price_with_fee = (basic_token_price
            * (EqFixedU128::from(T::SellFee::get()) + EqFixedU128::one()))
        .into_inner();

        let currency_price = T::PriceGetter::get_price::<EqFixedU128>(&asset)
            .map_err(|_| {
                log::error!("{}:{}.", file!(), line!());
                Error::<T>::NoPrice
            })?
            .into_inner();

        let balance_in_eq: T::Balance =
            multiply_by_rational(amount, currency_price, basic_price_with_fee)
                .map(|b| b.try_into().ok())
                .flatten()
                .ok_or(ArithmeticError::Overflow)?;

        Ok(balance_in_eq >= amount_buyout)
    }
}

/// Buyout validity errors
#[repr(u8)]
pub enum ValidityError {
    /// Account balance is too low to make buyout
    NotEnoughToBuyout = 0,
    /// Math error
    Math = 1,
    /// Buyout limit exceeded
    BuyoutLimitExceeded = 2,
    /// Amount to buyout less than min amount
    LessThanMinBuyoutAmount = 3,
}

impl From<ValidityError> for u8 {
    fn from(err: ValidityError) -> Self {
        err as u8
    }
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, scale_info::TypeInfo)]
pub struct CheckBuyout<T: Config + Send + Sync + scale_info::TypeInfo>(PhantomData<T>)
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>;

impl<T: Config + Send + Sync + scale_info::TypeInfo> Debug for CheckBuyout<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "CheckBuyout")
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> Default for CheckBuyout<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> CheckBuyout<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> SignedExtension for CheckBuyout<T>
where
    <T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
    const IDENTIFIER: &'static str = "CheckBuyout";
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
    /// - buyout_amount is greater or equal `MinAmountToBuyout`
    /// - `who` has enough to make buyout
    /// - buyout limit not exceeded for `who`
    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> TransactionValidity {
        if let Some(local_call) = call.is_sub_type() {
            if let Call::buyout { asset, amount } = local_call {
                let (buyout_amount, exchange_amount) =
                    Pallet::<T>::split_to_buyout_and_exchange(*asset, *amount)
                        .map_err(|_| InvalidTransaction::Custom(ValidityError::Math.into()))?;

                ensure!(
                    buyout_amount >= T::MinAmountToBuyout::get(),
                    InvalidTransaction::Custom(ValidityError::LessThanMinBuyoutAmount.into())
                );

                match T::BalanceGetter::get_balance(who, asset) {
                    SignedBalance::Positive(balance) => {
                        ensure!(
                            balance >= exchange_amount,
                            InvalidTransaction::Custom(ValidityError::NotEnoughToBuyout.into())
                        )
                    }
                    _ => fail!(InvalidTransaction::Custom(
                        ValidityError::NotEnoughToBuyout.into(),
                    )),
                }

                Pallet::<T>::ensure_buyout_limit_not_exceeded(who, buyout_amount).map_err(
                    |_| InvalidTransaction::Custom(ValidityError::BuyoutLimitExceeded.into()),
                )?;
            }
        }

        Ok(ValidTransaction::default())
    }
}

impl<T: Config> OnUnbalanced<NegativeImbalance<T::Balance>> for Pallet<T> {
    fn on_nonzero_unbalanced(amount: NegativeImbalance<T::Balance>) {
        let _ = T::EqCurrency::deposit_creating(
            &Pallet::<T>::account_id(),
            T::AssetGetter::get_main_asset(),
            amount.peek(),
            false,
            None,
        );
    }
}
