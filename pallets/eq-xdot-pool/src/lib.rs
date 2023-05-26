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

//! # Eq-xdot pallet

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(warnings)]

mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub mod yield_math;

use crate::traits::{AssetChecker, Assets, OnPoolInitialized};
use crate::yield_math::{ConstType, YieldMathTrait};
use codec::{Decode, Encode, FullCodec};
use eq_primitives::{
    xdot_pool::{PoolId, XBasePrice, XdotPoolInfoTrait},
    PalletAccountInitializer,
};
use frame_support::{
    dispatch::DispatchResult, dispatch::DispatchResultWithPostInfo, ensure, traits::UnixTime,
    transactional,
};
use frame_system::ensure_signed;
use sp_io::hashing::blake2_256;
use sp_runtime::{
    traits::{CheckedAdd, CheckedSub, Convert, Zero},
    DispatchError,
};
use sp_std::{
    convert::TryInto,
    fmt::Debug,
    ops::{AddAssign, BitOrAssign, ShlAssign},
    prelude::*,
};
use substrate_fixed::{
    traits::{Fixed, FixedSigned, ToFixed},
    transcendental::pow,
};
pub use weights::WeightInfo;

pub mod benchmarking;

pub use pallet::*;

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, Debug, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct XdotPoolInfo<T: pallet::Config> {
    /// LP asset
    pub pool_asset: T::AssetId,
    /// Pool asset balance in pool
    pub lp_total_supply: T::Balance,
    /// Account with pool balances
    pub account: T::AccountId,
    /// Pool's base asset
    pub base_asset: T::AssetId,
    /// Pool's x-base asset
    pub xbase_asset: T::AssetId,
    /// Fee coefficient for selling base to the pool
    pub g1: T::XdotNumber,
    /// Fee coefficient for selling fy to the poll
    pub g2: T::XdotNumber,
    /// Pool's maturity timestamp in seconds
    /// After this date xbase tokens can be redeemable for base tokens
    pub maturity: u64,
    /// 1 / maturity period
    pub ts: T::XdotNumber,
}

impl<T: pallet::Config> XdotPoolInfoTrait<T::AssetId, T::Balance> for XdotPoolInfo<T> {
    fn base_asset(&self) -> T::AssetId {
        self.base_asset
    }

    fn xbase_asset(&self) -> T::AssetId {
        self.xbase_asset
    }

    fn base_balance(&self) -> T::Balance {
        T::Assets::balance(self.base_asset, &self.account)
    }

    fn xbase_balance(&self) -> T::Balance {
        T::Assets::balance(self.xbase_asset, &self.account)
    }

    fn virtual_xbase_balance(&self) -> Option<T::Balance> {
        let xbase_balance = self.xbase_balance();
        self.lp_total_supply.checked_add(&xbase_balance)
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use yield_math::YieldMathTrait;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// Current number of pools (also ID for the next created pool)
    #[pallet::storage]
    #[pallet::getter(fn pool_count)]
    pub type PoolCount<T: Config> = StorageValue<_, PoolId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pools)]
    pub type Pools<T: Config> = StorageMap<_, Blake2_128Concat, PoolId, XdotPoolInfo<T>>;

    #[pallet::storage]
    #[pallet::getter(fn initializer)]
    pub type Initializer<T: Config> = StorageMap<_, Blake2_128Concat, PoolId, T::AccountId>;

    #[pallet::config]
    pub trait Config: frame_system::Config + eq_rate::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type PoolsManagementOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Primitive integer type that [`XdotNumber`](#associatedtype.XdotNumber) based on.
        type FixedNumberBits: Copy + ToFixed + AddAssign + BitOrAssign + ShlAssign;
        /// Fixed point data type with a required precision that used for all calculations.
        type XdotNumber: Clone
            + Copy
            + FullCodec
            + FixedSigned<Bits = Self::FixedNumberBits>
            + PartialOrd<ConstType>
            + From<ConstType>
            + scale_info::TypeInfo;

        type NumberConvert: Convert<u64, Self::XdotNumber>;

        type BalanceConvert: Convert<Self::Balance, Self::XdotNumber>
            + Convert<Self::XdotNumber, Option<Self::Balance>>;

        /// External implementation for required operations with assets
        type Assets: traits::Assets<Self::AssetId, Self::Balance, Self::AccountId>;

        /// The asset ID type
        type AssetId: Parameter + Ord + Copy + Default + scale_info::TypeInfo;

        /// Yield math and calculations
        type YieldMath: YieldMathTrait<Self::XdotNumber>;

        /// Type using for prices
        type PriceNumber: sp_std::ops::Mul;
        type PriceConvert: Convert<Self::XdotNumber, Self::PriceNumber>;

        /// Type for convert types from Number to u128 with 18 decimals representation
        type FixedNumberConvert: Convert<Self::XdotNumber, Option<u128>>;

        /// Operations that must be performed on pool initialization
        type OnPoolInitialized: OnPoolInitialized;

        type AssetChecker: AssetChecker<Self::AssetId>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Creates pool
        /// - maturity: unix timestamp when maturity is coming
        /// - ts_period: period in secs for ts coeff

        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn create_pool(
            origin: OriginFor<T>,
            initializer: T::AccountId,
            base_asset: T::AssetId,
            xbase_asset: T::AssetId,
            sell_base_fee_coeff: T::XdotNumber,
            sell_xbase_fee_coeff: T::XdotNumber,
            maturity: u64,
            ts_period: T::XdotNumber,
        ) -> DispatchResultWithPostInfo {
            T::PoolsManagementOrigin::ensure_origin(origin)?;

            let ts = Self::calc_ts(ts_period)?;

            Self::do_create_pool(
                base_asset,
                xbase_asset,
                sell_base_fee_coeff,
                sell_xbase_fee_coeff,
                maturity,
                ts,
                initializer,
            )?;

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn initialize(
            origin: OriginFor<T>,
            pool_id: PoolId,
            base_amount: T::Balance,
            xbase_amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_initialize(who, pool_id, base_amount, xbase_amount)?;

            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn change_initializer(
            origin: OriginFor<T>,
            pool_id: PoolId,
            account: Option<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            T::PoolsManagementOrigin::ensure_origin(origin)?;
            Self::get_pool(pool_id)?;

            match account {
                Some(a) => Initializer::<T>::insert(pool_id, a),
                None => Initializer::<T>::remove(pool_id),
            };

            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn remove_pool(origin: OriginFor<T>, pool_id: PoolId) -> DispatchResultWithPostInfo {
            T::PoolsManagementOrigin::ensure_origin(origin)?;

            let _ = Self::get_pool(pool_id)?;

            <Pools<T>>::remove(pool_id);

            Ok(().into())
        }

        /// Mint liquidity tokens, with an optional internal trade to buy xbase tokens beforehand.
        /// The amount of liquidity tokens is calculated from the amount of xbase tokens to buy from the pool,
        /// plus the `xbase_in`. A proportional amount of base tokens need to be sent.
        /// It fails if amount of base tokens for trade less than `base_in`
        #[pallet::call_index(4)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn mint(
            origin: OriginFor<T>,
            pool_id: PoolId,
            min_ratio: (u32, u32),
            max_ratio: (u32, u32),
            base_in: T::Balance,
            xbase_in: T::Balance,
            xbase_to_buy: T::Balance,
        ) -> DispatchResultWithPostInfo {
            // if supply == 0 ensure_root or owner?
            let who = ensure_signed(origin)?;

            Self::ensure_pool_inited(pool_id)?;

            Self::do_mint(
                who,
                pool_id,
                base_in,
                xbase_in,
                xbase_to_buy,
                min_ratio,
                max_ratio,
            )
        }

        /// Burn liquidity tokens in exchange for base and fyToken or base only with `trade_to_base=true`
        #[pallet::call_index(5)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn burn(
            origin: OriginFor<T>,
            pool_id: PoolId,
            min_ratio: (u32, u32),
            max_ratio: (u32, u32),
            tokens_burned: T::Balance,
            trade_to_base: bool,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::ensure_pool_inited(pool_id)?;

            Self::do_burn(
                &who,
                pool_id,
                tokens_burned,
                trade_to_base,
                min_ratio,
                max_ratio,
            )
        }

        /// Sell base for xbase token.
        /// min -  minimm accepted amount of xbase token
        /// Returns amount of xbase token that will be transfered on caller account
        #[pallet::call_index(6)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn sell_base(
            origin: OriginFor<T>,
            pool_id: PoolId,
            base_to_sell: T::Balance,
            min: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::ensure_pool_inited(pool_id)?;

            Self::do_sell_base(&who, pool_id, base_to_sell, min)
        }

        /// Buy base for xbase token
        /// buy_base_amount - amount of base being bought that will be deposited to caller
        /// max - maximum amount of xbase token that will be paid for the trade
        #[pallet::call_index(7)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn buy_base(
            origin: OriginFor<T>,
            pool_id: PoolId,
            base_to_buy: T::Balance,
            xbase_to_sell: T::Balance,
            max: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::ensure_pool_inited(pool_id)?;

            Self::do_buy_base(&who, pool_id, base_to_buy, xbase_to_sell, max)
        }

        /// Sell xbase token for base
        /// xbase_to_sell - amount of xbase token to sell for base
        /// min - minimum accepted amount of base
        #[pallet::call_index(8)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn sell_xbase(
            origin: OriginFor<T>,
            pool_id: PoolId,
            xbase_to_sell: T::Balance,
            min: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::ensure_pool_inited(pool_id)?;

            Self::do_sell_xbase(&who, pool_id, xbase_to_sell, min)
        }
        /// Buy xbase for base
        /// xbase_to_buy - amount of xbase being bought that will be transfered to caller
        /// max - maximum amount of base token that will be paid for the trade
        #[pallet::call_index(9)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn buy_xbase(
            origin: OriginFor<T>,
            pool_id: PoolId,
            base_to_sell: T::Balance,
            xbase_to_buy: T::Balance,
            max: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::ensure_pool_inited(pool_id)?;

            Self::do_buy_xbase(&who, pool_id, xbase_to_buy, base_to_sell, max)
        }

        #[pallet::call_index(10)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::transfer_native())]
        pub fn optimal_mint(
            origin: OriginFor<T>,
            pool_id: PoolId,
            base_in: T::Balance,
            xbase_in: T::Balance,
            lp_to_mint: T::Balance,
            base_in_ratio_corridor: Option<(u64, u64)>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::ensure_pool_inited(pool_id)?;

            Self::do_optimal_mint(
                who,
                pool_id,
                base_in,
                xbase_in,
                lp_to_mint,
                base_in_ratio_corridor,
            )
        }
    }
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Pool with specified id `PoolId` was created successfully.
        ///
        /// Included values are:
        /// - pool identifier `PoolId`
        /// - LP asset
        /// - Base asset
        /// - XBase asset
        /// - g1 coeff
        /// - g2 coeff
        /// - ts coeff
        /// \[pool_id, base_asset, xbase_asset, g1, g2, ts\]
        PoolCreated(
            PoolId,
            T::AssetId,
            T::AssetId,
            T::AssetId,
            T::XdotNumber,
            T::XdotNumber,
            T::XdotNumber,
        ),
        /// LP tokens was minted, base and xbase was transfered into pool
        ///
        /// Included values are:
        /// - account identifier `T::AccountId`
        /// - pool indentifier `PoolId`
        /// - base asset amount `T::Balance`
        /// - xbase asset amount `T::Balance`
        /// - lp tokens amount `T::Balance`
        ///
        /// \[who, pool_id, base_in, xbase_in, tokens_minted\]
        Minted(T::AccountId, PoolId, T::Balance, T::Balance, T::Balance),

        /// LP tokens was burned, base and xbase was transfered from pool
        ///
        /// Included values are:
        /// - account identifier `T::AccountId`
        /// - pool indentifier `PoolId`
        /// - base asset amount `T::Balance`
        /// - xbase asset amount `T::Balance`
        /// - lp tokens amount `T::Balance`
        ///
        /// \[who, pool_id, base_out, xbase_out, tokens_burned\]
        Burned(T::AccountId, PoolId, T::Balance, T::Balance, T::Balance),

        /// Base was sold into pool in exchange for xbase.
        ///
        /// Included values are:
        /// - account identifier `T::AccountId`
        /// - pool indentifier `PoolId`
        /// - base asset amount `T::Balance`
        /// - xbase asset amount `T::Balance`
        SaleBase(T::AccountId, PoolId, T::Balance, T::Balance),

        /// Base was bought from pool in exchange for xbase
        ///
        /// Included values are:
        /// - account identifier `T::AccountId`
        /// - pool indentifier `PoolId`
        /// - base asset amount `T::Balance`
        /// - xbase asset amount `T::Balance`
        BuyBase(T::AccountId, PoolId, T::Balance, T::Balance),

        /// Xbase was sold into pool in exchange for base
        ///
        /// Included values are:
        /// - account identifier `T::AccountId`
        /// - pool indentifier `PoolId`
        /// - base asset amount `T::Balance`
        /// - xbase asset amount `T::Balance`
        SellXBase(T::AccountId, PoolId, T::Balance, T::Balance),

        /// Xbase was bought from pool in exchange for base
        ///
        /// Included values are:
        /// - account identifier `T::AccountId`
        /// - pool indentifier `PoolId`
        /// - base asset amount `T::Balance`
        /// - xbase asset amount `T::Balance`
        BuyXBase(T::AccountId, PoolId, T::Balance, T::Balance),
    }
    #[pallet::error]
    pub enum Error<T> {
        BalanceConvertOverflow,
        ExternalAssetCheckFailed,
        InconsistentStorage,
        NeedToInitialize,
        NoNeedToInitialize,
        LPAssetNotCreated,
        MaturityInThePast,
        MaturityTooFarFromNow,
        NotAuthorized,
        PoolNotFound,
        MathError,
        CalcVirtualXbaseOverflow,
        CalcRatioMathError,
        WrongMinRatio,
        WrongMaxRatio,
        XbaseBalanceTooLow,
        TooMuchXbaseOut,
        TooLowXbaseIn,
        TooLowBaseIn,
        TooMuchBaseIn,
        PoolCountOverflow,
        SellBaseTooLowForMin,
        SellXBaseTooLowForMin,
        TsPeriodTooLarge,
        BuyBaseTooMuchForMax,
        BuyXbaseTooMuchForMax,
        YieldMathFyInForBaseOut,
        YieldMathBaseOutForFyIn,
        YieldMathBaseInForFyOut,
        YieldMathFyOutForBaseIn,
        YieldMathInvariant,
        ExternalError,
        MethodNotAllowed,
    }
}

impl<T: Config> Pallet<T> {
    #[transactional]
    fn do_initialize(
        who: T::AccountId,
        pool_id: PoolId,
        base_in: T::Balance,
        xbase_in: T::Balance,
    ) -> Result<(), DispatchError> {
        let authorized = Self::initializer(pool_id).ok_or(Error::<T>::NoNeedToInitialize)?;
        ensure!(who == authorized, Error::<T>::NotAuthorized);
        let pool = Self::get_pool(pool_id)?;

        if pool.base_balance() == T::Balance::zero() {
            Self::do_mint(
                who.clone(),
                pool_id,
                base_in,
                T::Balance::zero(),
                T::Balance::zero(),
                (0, 1),
                (u32::MAX, 1),
            )
            .map_err(|err| err.error)?;
        }

        if pool.xbase_balance() == T::Balance::zero() {
            T::Assets::transfer(pool.xbase_asset, &who, &pool.account, xbase_in)?;
        }

        if pool.base_balance() != T::Balance::zero() && pool.xbase_balance() != T::Balance::zero() {
            T::OnPoolInitialized::on_initalize(pool_id)?;

            Initializer::<T>::remove(pool_id);
        }

        Ok(())
    }

    /// Returns how much base would be required to buy `xbase_out`.
    fn buy_xbase_preview(
        maturity: u64,
        base_balance: T::Balance,
        virtual_xbase_balance: T::Balance,
        ts: T::XdotNumber,
        g1: T::XdotNumber,
        xbase_out: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        let time_till_maturity = Self::time_till_maturity(maturity)?; // This can't be called after maturity
        let base_in = T::YieldMath::base_in_for_fy_token_out(
            T::BalanceConvert::convert(base_balance),
            T::BalanceConvert::convert(virtual_xbase_balance),
            T::BalanceConvert::convert(xbase_out),
            T::NumberConvert::convert(time_till_maturity),
            ts,
            g1,
        )
        .map(T::BalanceConvert::convert)
        .map_err(|_| Error::<T>::YieldMathBaseInForFyOut)? // maybe todo: event with orirginal error
        .ok_or(Error::<T>::BalanceConvertOverflow)?;

        let new_virtual_xbase_balance = virtual_xbase_balance
            .checked_sub(&xbase_out)
            .ok_or(Error::<T>::MathError)?;
        let new_base_balance = base_balance
            .checked_add(&base_in)
            .ok_or(Error::<T>::MathError)?;

        ensure!(
            new_virtual_xbase_balance >= new_base_balance,
            Error::<T>::XbaseBalanceTooLow
        );

        Ok(base_in)
    }

    /// Mint liquidity tokens.
    /// If `xbase_to_buy` is not zero then an optional internal trade to buy fyToken happens beforehand.
    fn do_mint(
        who: T::AccountId,
        pool_id: PoolId,
        base_in: T::Balance,
        xbase_in: T::Balance,
        xbase_to_buy: T::Balance,
        min_ratio: (u32, u32),
        max_ratio: (u32, u32),
    ) -> DispatchResultWithPostInfo {
        Pools::<T>::try_mutate_exists(pool_id, |maybe_pool_info| -> DispatchResult {
            if let Some(pool) = maybe_pool_info {
                let min_ratio = Self::calc_ratio(min_ratio.0 as u64, min_ratio.1 as u64)?;
                let max_ratio = Self::calc_ratio(max_ratio.0 as u64, max_ratio.1 as u64)?;

                let real_xbase_balance_num = T::BalanceConvert::convert(pool.xbase_balance());
                let base_balance = pool.base_balance();
                let base_balance_num = T::BalanceConvert::convert(base_balance);
                // Check the burn wasn't sandwiched
                // (why burn? because it's original yield comment)

                Self::check_ratio(
                    real_xbase_balance_num,
                    base_balance_num,
                    min_ratio,
                    max_ratio,
                )?;

                let lp_supply_num = T::BalanceConvert::convert(pool.lp_total_supply);
                let base_in_num = T::BalanceConvert::convert(base_in);
                let xbase_in_num = T::BalanceConvert::convert(xbase_in);
                let xbase_to_buy_num = T::BalanceConvert::convert(xbase_to_buy);

                let tokens_minted_num;
                let actual_base_in_num;
                let actual_xbase_in;
                let zero = T::NumberConvert::convert(0);

                // Calculate token amounts
                if lp_supply_num == zero {
                    // Initialize at 1 pool token minted per base token supplied
                    tokens_minted_num = base_in_num;
                    actual_base_in_num = base_in_num;
                    actual_xbase_in = T::Balance::zero();
                } else if xbase_in == T::Balance::zero() && xbase_to_buy == T::Balance::zero() {
                    return Err(Error::<T>::TooLowXbaseIn.into());
                } else if real_xbase_balance_num == zero {
                    // Edge case, no fyToken in the Pool after initialization
                    actual_base_in_num = base_in_num;
                    tokens_minted_num = lp_supply_num
                        .checked_mul(base_in_num)
                        .ok_or(Error::<T>::MathError)?
                        .checked_div(base_balance_num)
                        .ok_or(Error::<T>::MathError)?;
                    actual_xbase_in = T::Balance::zero();
                } else {
                    // There is an optional virtual trade before the mint
                    let base_to_sell = if xbase_to_buy_num > zero {
                        let virtual_xbase_balance = pool
                            .virtual_xbase_balance()
                            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;
                        Self::buy_xbase_preview(
                            pool.maturity,
                            base_balance,
                            virtual_xbase_balance,
                            pool.ts,
                            pool.g1,
                            xbase_to_buy,
                        )?
                    } else {
                        T::Balance::zero()
                    };

                    let base_to_sell_num = T::BalanceConvert::convert(base_to_sell);

                    // We use all the available fyTokens, plus a virtual trade if it happened, surplus is in base tokens
                    // tokensMinted = (supply * (fyTokenToBuy + fyTokenIn)) / (_realFYTokenCached - fyTokenToBuy);
                    let xbase_in_total = xbase_to_buy_num
                        .checked_add(xbase_in_num)
                        .ok_or(Error::<T>::MathError)?;

                    tokens_minted_num = lp_supply_num
                        .checked_mul(xbase_in_total)
                        .ok_or(Error::<T>::MathError)?
                        .checked_div(
                            real_xbase_balance_num
                                .checked_sub(xbase_to_buy_num)
                                .ok_or(Error::<T>::MathError)?,
                        )
                        .ok_or(Error::<T>::MathError)?;

                    // baseIn = base_to_sell + ((_baseCached + base_to_sell) * tokensMinted) / supply;
                    actual_base_in_num = base_to_sell_num
                        .checked_add(
                            base_balance_num
                                .checked_add(base_to_sell_num)
                                .ok_or(Error::<T>::TooMuchBaseIn)?
                                .checked_mul(tokens_minted_num)
                                .ok_or(Error::<T>::MathError)?
                                .checked_div(lp_supply_num)
                                .ok_or(Error::<T>::MathError)?,
                        )
                        .ok_or(Error::<T>::TooMuchXbaseOut)?;
                    ensure!(
                        base_in
                            >= T::BalanceConvert::convert(actual_base_in_num)
                                .ok_or(Error::<T>::BalanceConvertOverflow)?,
                        Error::<T>::TooLowBaseIn
                    );
                    actual_xbase_in = xbase_in;
                }
                // transfer actual_base_in to pallet account
                let actual_base_in = T::BalanceConvert::convert(actual_base_in_num)
                    .ok_or(Error::<T>::BalanceConvertOverflow)?;

                let tokens_minted = T::BalanceConvert::convert(tokens_minted_num)
                    .ok_or(Error::<T>::BalanceConvertOverflow)?;

                Self::mint_internal(&who, pool, actual_base_in, actual_xbase_in, tokens_minted)?;

                Self::deposit_event(Event::Minted(
                    who,
                    pool_id,
                    actual_base_in,
                    actual_xbase_in,
                    tokens_minted,
                ));

                Ok(())
            } else {
                Err(Error::<T>::PoolNotFound.into())
            }
        })?;

        Ok(().into())
    }

    fn do_optimal_mint(
        who: T::AccountId,
        pool_id: PoolId,
        base_in: T::Balance,
        xbase_in: T::Balance,
        lp_to_mint: T::Balance,
        base_in_ratio_corridor: Option<(u64, u64)>,
    ) -> DispatchResultWithPostInfo {
        Pools::<T>::try_mutate_exists(pool_id, |maybe_pool_info| -> DispatchResult {
            if let Some(pool) = maybe_pool_info {
                let base_balance = pool.xbase_balance();

                let base_balance_num = T::BalanceConvert::convert(base_balance);
                let xbase_balance_num = T::BalanceConvert::convert(pool.xbase_balance());
                let lp_supply_num = T::BalanceConvert::convert(pool.lp_total_supply);
                let lp_to_mint_num = T::BalanceConvert::convert(lp_to_mint);
                let xbase_in_num = T::BalanceConvert::convert(xbase_in);

                let xbase_to_buy_num = lp_to_mint_num
                    .checked_mul(xbase_balance_num)
                    .ok_or(Error::<T>::MathError)?
                    .checked_sub(
                        lp_supply_num
                            .checked_mul(xbase_in_num)
                            .ok_or(Error::<T>::MathError)?,
                    )
                    .ok_or(Error::<T>::MathError)?
                    .checked_div(
                        lp_to_mint_num
                            .checked_add(lp_supply_num)
                            .ok_or(Error::<T>::MathError)?,
                    )
                    .ok_or(Error::<T>::MathError)?;

                let virtual_xbase_balance = pool
                    .virtual_xbase_balance()
                    .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;
                let xbase_to_buy = T::BalanceConvert::convert(xbase_to_buy_num)
                    .ok_or(Error::<T>::BalanceConvertOverflow)?;
                let base_to_sell = Self::buy_xbase_preview(
                    pool.maturity,
                    base_balance,
                    virtual_xbase_balance,
                    pool.ts,
                    pool.g1,
                    xbase_to_buy,
                )?;

                let base_to_sell_num = T::BalanceConvert::convert(base_to_sell);

                let actual_base_in_num = base_to_sell_num
                    .checked_add(
                        base_balance_num
                            .checked_add(base_to_sell_num)
                            .ok_or(Error::<T>::TooMuchBaseIn)?
                            .checked_mul(lp_to_mint_num)
                            .ok_or(Error::<T>::MathError)?
                            .checked_div(lp_supply_num)
                            .ok_or(Error::<T>::MathError)?,
                    )
                    .ok_or(Error::<T>::TooMuchXbaseOut)?;

                if base_in_ratio_corridor.is_some() {
                    let (nominator, denominator) = base_in_ratio_corridor.unwrap();
                    let base_in_ratio_corridor = Self::calc_ratio(nominator, denominator)?;
                    let one = T::NumberConvert::convert(1);
                    let base_in_num = T::BalanceConvert::convert(base_in);
                    let actual_ratio = if base_in_ratio_corridor > one {
                        sp_std::cmp::max(base_in_num, actual_base_in_num)
                            .checked_div(sp_std::cmp::min(base_in_num, actual_base_in_num))
                            .ok_or(Error::<T>::MathError)?
                    } else {
                        sp_std::cmp::min(base_in_num, actual_base_in_num)
                            .checked_div(sp_std::cmp::max(base_in_num, actual_base_in_num))
                            .ok_or(Error::<T>::MathError)?
                    };

                    ensure!(
                        actual_ratio <= base_in_ratio_corridor,
                        Error::<T>::TooLowBaseIn
                    );
                }

                let actual_base_in = T::BalanceConvert::convert(actual_base_in_num)
                    .ok_or(Error::<T>::BalanceConvertOverflow)?;
                let total_xbase_in = xbase_in + xbase_to_buy;

                Self::mint_internal(&who, pool, actual_base_in, total_xbase_in, lp_to_mint)?;

                Self::deposit_event(Event::Minted(
                    who,
                    pool_id,
                    actual_base_in,
                    total_xbase_in,
                    lp_to_mint,
                ));

                Ok(())
            } else {
                Err(Error::<T>::PoolNotFound.into())
            }
        })?;

        Ok(().into())
    }

    #[transactional]
    fn mint_internal(
        who: &T::AccountId,
        pool: &mut XdotPoolInfo<T>,
        base_in: T::Balance,
        xbase_in: T::Balance,
        lp_mint: T::Balance,
    ) -> Result<(), DispatchError> {
        T::Assets::transfer(pool.base_asset, who, &pool.account, base_in)?;

        T::Assets::transfer(pool.xbase_asset, who, &pool.account, xbase_in)?;

        T::Assets::mint(pool.pool_asset, &who, lp_mint)?;

        let new_lp_total_supply = pool
            .lp_total_supply
            .checked_add(&lp_mint)
            .ok_or(Error::<T>::MathError)?;

        pool.lp_total_supply = new_lp_total_supply;

        Ok(())
    }

    fn do_burn(
        who: &T::AccountId,
        pool_id: PoolId,
        tokens_burned: T::Balance,
        trade_to_base: bool,
        min_ratio: (u32, u32),
        max_ratio: (u32, u32),
    ) -> DispatchResultWithPostInfo {
        Pools::<T>::try_mutate_exists(pool_id, |maybe_pool_info| -> DispatchResult {
            if let Some(pool) = maybe_pool_info {
                let min_ratio = Self::calc_ratio(min_ratio.0 as u64, min_ratio.1 as u64)?;
                let max_ratio = Self::calc_ratio(max_ratio.0 as u64, max_ratio.1 as u64)?;

                let real_xbase_balance_num = T::BalanceConvert::convert(pool.xbase_balance());
                let base_balance_num = T::BalanceConvert::convert(pool.base_balance());

                // Check the burn wasn't sandwiched
                Self::check_ratio(
                    real_xbase_balance_num,
                    base_balance_num,
                    min_ratio,
                    max_ratio,
                )?;

                let lp_supply_num = T::BalanceConvert::convert(pool.lp_total_supply);

                let tokens_burned_num = T::BalanceConvert::convert(tokens_burned);

                let pool_params = if trade_to_base {
                    Some((pool.maturity, pool.ts, pool.g2))
                } else {
                    None
                };

                let (base_out, xbase_out) = Self::burn_calculations(
                    base_balance_num,
                    real_xbase_balance_num,
                    lp_supply_num,
                    tokens_burned_num,
                    pool_params,
                )?;

                Self::burn_internal(who, pool, base_out, xbase_out, tokens_burned)?;

                Self::deposit_event(Event::Burned(
                    who.clone(),
                    pool_id,
                    base_out,
                    xbase_out,
                    tokens_burned,
                ));

                Ok(())
            } else {
                Err(Error::<T>::PoolNotFound.into())
            }
        })?;

        Ok(().into())
    }

    fn burn_calculations(
        base_balance_num: T::XdotNumber,
        real_xbase_balance_num: T::XdotNumber,
        lp_supply_num: T::XdotNumber,
        tokens_burned_num: T::XdotNumber,
        trade_to_base: Option<(u64, T::XdotNumber, T::XdotNumber)>,
    ) -> Result<(T::Balance, T::Balance), DispatchError> {
        // Calculate trade
        let virtual_xbase_balance_num = real_xbase_balance_num
            .checked_add(lp_supply_num)
            .ok_or(Error::<T>::MathError)?;
        let mut base_token_out_num = tokens_burned_num
            .checked_mul(base_balance_num)
            .ok_or(Error::<T>::MathError)?
            .checked_div(lp_supply_num)
            .ok_or(Error::<T>::MathError)?;
        let mut xbase_token_out_num = tokens_burned_num
            .checked_mul(real_xbase_balance_num)
            .ok_or(Error::<T>::MathError)?
            .checked_div(lp_supply_num)
            .ok_or(Error::<T>::MathError)?;
        let zero = T::NumberConvert::convert(0);
        if let Some((maturity, ts, g2)) = trade_to_base {
            let time_till_maturity = Self::time_till_maturity(maturity)?; // This can't be called after maturity
            let base_minus_virtual_burn = base_balance_num
                .checked_sub(base_token_out_num)
                .ok_or(Error::<T>::MathError)?;
            let xbase_minus_burn = virtual_xbase_balance_num
                .checked_sub(xbase_token_out_num)
                .ok_or(Error::<T>::MathError)?;
            let base_out_for_fy_token_in = T::YieldMath::base_out_for_fy_token_in(
                base_minus_virtual_burn, // Cache, minus virtual burn
                xbase_minus_burn,        // Cache, minus virtual burn
                xbase_token_out_num,     // Sell the virtual fyToken obtained
                T::NumberConvert::convert(time_till_maturity),
                ts,
                g2,
                false,
            )
            .map_err(|_| Error::<T>::YieldMathBaseOutForFyIn)?;
            base_token_out_num = base_token_out_num
                .checked_add(base_out_for_fy_token_in)
                .ok_or(Error::<T>::MathError)?;
            xbase_token_out_num = zero;
        }
        let base_out = T::BalanceConvert::convert(base_token_out_num)
            .ok_or(Error::<T>::BalanceConvertOverflow)?;
        let xbase_out = T::BalanceConvert::convert(xbase_token_out_num)
            .ok_or(Error::<T>::BalanceConvertOverflow)?;

        Ok((base_out, xbase_out))
    }

    #[transactional]
    fn burn_internal(
        who: &T::AccountId,
        pool: &mut XdotPoolInfo<T>,
        base_out: T::Balance,
        xbase_out: T::Balance,
        tokens_burned: T::Balance,
    ) -> Result<(), DispatchError> {
        T::Assets::burn(pool.pool_asset, who, tokens_burned)?;

        T::Assets::transfer(pool.base_asset, &pool.account, who, base_out)?;

        T::Assets::transfer(pool.xbase_asset, &pool.account, who, xbase_out)?;

        let new_lp_token_supply = pool
            .lp_total_supply
            .checked_sub(&tokens_burned)
            .ok_or(Error::<T>::MathError)?;

        pool.lp_total_supply = new_lp_token_supply;

        Ok(())
    }

    fn do_sell_base(
        who: &T::AccountId,
        pool_id: PoolId,
        sell_base_amount: T::Balance,
        min: T::Balance,
    ) -> DispatchResultWithPostInfo {
        let pool = Self::get_pool(pool_id)?;
        let virtual_xbase_balance = pool
            .virtual_xbase_balance()
            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;
        let xbase_out = Self::sell_base_preview(
            pool.maturity,
            pool.base_balance(),
            virtual_xbase_balance,
            pool.ts,
            pool.g1,
            sell_base_amount,
        )?;

        // Slippage check
        ensure!(xbase_out >= min, Error::<T>::SellBaseTooLowForMin);

        Self::base_in_xbase_out(pool, who, sell_base_amount, xbase_out)?;

        Self::deposit_event(Event::SaleBase(
            who.clone(),
            pool_id,
            sell_base_amount,
            xbase_out,
        ));

        Ok(().into())
    }

    /// Returns how much xbase token would be obtained by selling `sell_base_amount` base
    fn sell_base_preview(
        maturity: u64,
        base_balance: T::Balance,
        virtual_xbase_balance: T::Balance,
        ts: T::XdotNumber,
        g1: T::XdotNumber,
        sell_base_amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        let time_till_maturity = Self::time_till_maturity(maturity)?;
        let xbase_out = T::YieldMath::fy_token_out_for_base_in(
            T::BalanceConvert::convert(base_balance),
            T::BalanceConvert::convert(virtual_xbase_balance),
            T::BalanceConvert::convert(sell_base_amount),
            T::NumberConvert::convert(time_till_maturity),
            ts,
            g1,
        )
        .map(T::BalanceConvert::convert)
        .map_err(|_| Error::<T>::YieldMathFyOutForBaseIn)?
        .ok_or(Error::<T>::BalanceConvertOverflow)?;

        let new_virtual_xbase_balance = virtual_xbase_balance
            .checked_sub(&xbase_out)
            .ok_or(Error::<T>::MathError)?;

        let new_base_balance = base_balance
            .checked_add(&sell_base_amount)
            .ok_or(Error::<T>::MathError)?;

        ensure!(
            new_virtual_xbase_balance >= new_base_balance,
            Error::<T>::XbaseBalanceTooLow
        );

        Ok(xbase_out)
    }

    #[transactional]
    fn base_in_xbase_out(
        pool: XdotPoolInfo<T>,
        who: &T::AccountId,
        base_in: T::Balance,
        xbase_out: T::Balance,
    ) -> Result<(), DispatchError> {
        T::Assets::transfer(pool.base_asset, who, &pool.account, base_in)?;
        T::Assets::transfer(pool.xbase_asset, &pool.account, who, xbase_out)?;
        Ok(())
    }

    fn do_buy_base(
        who: &T::AccountId,
        pool_id: PoolId,
        buy_base_amount: T::Balance,
        xbase_to_sell: T::Balance,
        max: T::Balance,
    ) -> DispatchResultWithPostInfo {
        let pool = Self::get_pool(pool_id)?;
        let time_till_maturity = Self::time_till_maturity(pool.maturity)?;
        let virtual_xbase_balance = pool
            .virtual_xbase_balance()
            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;

        let xbase_in = T::YieldMath::fy_token_in_for_base_out(
            T::BalanceConvert::convert(pool.base_balance()),
            T::BalanceConvert::convert(virtual_xbase_balance),
            T::BalanceConvert::convert(buy_base_amount),
            T::NumberConvert::convert(time_till_maturity),
            pool.ts,
            pool.g2,
        )
        .map(T::BalanceConvert::convert)
        .map_err(|_| Error::<T>::YieldMathFyInForBaseOut)?
        .ok_or(Error::<T>::BalanceConvertOverflow)?;

        ensure!(xbase_to_sell >= xbase_in, Error::<T>::TooLowXbaseIn);
        // Slippage check
        ensure!(xbase_in <= max, Error::<T>::BuyBaseTooMuchForMax);

        Self::base_out_xbase_in(pool, who, buy_base_amount, xbase_in)?;

        Self::deposit_event(Event::BuyBase(
            who.clone(),
            pool_id,
            buy_base_amount,
            xbase_in,
        ));

        Ok(().into())
    }

    #[transactional]
    fn base_out_xbase_in(
        pool: XdotPoolInfo<T>,
        who: &T::AccountId,
        base_out: T::Balance,
        xbase_in: T::Balance,
    ) -> Result<(), DispatchError> {
        T::Assets::transfer(pool.xbase_asset, who, &pool.account, xbase_in)?;
        T::Assets::transfer(pool.base_asset, &pool.account, who, base_out)?;
        Ok(())
    }

    fn do_sell_xbase(
        who: &T::AccountId,
        pool_id: PoolId,
        xbase_to_sell: T::Balance,
        min: T::Balance,
    ) -> DispatchResultWithPostInfo {
        let pool = Self::get_pool(pool_id)?;
        let time_till_maturity = Self::time_till_maturity(pool.maturity)?;
        let virtual_xbase_balance = pool
            .virtual_xbase_balance()
            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;

        let base_out = T::YieldMath::base_out_for_fy_token_in(
            T::BalanceConvert::convert(pool.base_balance()),
            T::BalanceConvert::convert(virtual_xbase_balance),
            T::BalanceConvert::convert(xbase_to_sell),
            T::NumberConvert::convert(time_till_maturity),
            pool.ts,
            pool.g2,
            false,
        )
        .map(T::BalanceConvert::convert)
        .map_err(|_| Error::<T>::YieldMathBaseOutForFyIn)? // maybe todo: event with original error
        .ok_or(Error::<T>::BalanceConvertOverflow)?;
        // Slippage check
        ensure!(base_out >= min, Error::<T>::SellXBaseTooLowForMin);

        Self::base_out_xbase_in(pool, who, base_out, xbase_to_sell)?;

        Self::deposit_event(Event::SellXBase(
            who.clone(),
            pool_id,
            base_out,
            xbase_to_sell,
        ));

        Ok(().into())
    }

    fn do_buy_xbase(
        who: &T::AccountId,
        pool_id: PoolId,
        xbase_to_buy: T::Balance,
        base_to_sell: T::Balance,
        max: T::Balance,
    ) -> DispatchResultWithPostInfo {
        let pool = Self::get_pool(pool_id)?;
        let virtual_xbase_balance = pool
            .virtual_xbase_balance()
            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;
        let base_balance = pool.base_balance();
        let base_in = Self::buy_xbase_preview(
            pool.maturity,
            base_balance,
            virtual_xbase_balance,
            pool.ts,
            pool.g1,
            xbase_to_buy,
        )?;

        ensure!(base_to_sell >= base_in, Error::<T>::TooLowBaseIn);

        // // Slippage check
        ensure!(base_in <= max, Error::<T>::BuyXbaseTooMuchForMax);

        Self::base_in_xbase_out(pool, who, base_in, xbase_to_buy)?;

        Self::deposit_event(Event::BuyXBase(who.clone(), pool_id, base_in, xbase_to_buy));

        Ok(().into())
    }

    fn get_pool(pool_id: PoolId) -> Result<XdotPoolInfo<T>, DispatchError> {
        Self::pools(pool_id).ok_or(Error::<T>::PoolNotFound.into())
    }

    fn check_ratio(
        real_xbase_balance: T::XdotNumber,
        base_balance: T::XdotNumber,
        min_ratio: T::XdotNumber,
        max_ratio: T::XdotNumber,
    ) -> Result<(), DispatchError> {
        let zero = T::NumberConvert::convert(0);
        if real_xbase_balance != zero {
            let base_real_fy_ratio = base_balance
                .checked_div(real_xbase_balance)
                .ok_or(Error::<T>::MathError)?;
            ensure!(base_real_fy_ratio >= min_ratio, Error::<T>::WrongMinRatio);
            ensure!(base_real_fy_ratio <= max_ratio, Error::<T>::WrongMaxRatio);
        }
        Ok(())
    }

    fn calc_ratio(nominator: u64, denominator: u64) -> Result<T::XdotNumber, DispatchError> {
        T::NumberConvert::convert(nominator)
            .checked_div(T::NumberConvert::convert(denominator))
            .ok_or(Error::<T>::CalcRatioMathError.into())
    }

    pub fn time_till_maturity(maturity: u64) -> Result<u64, DispatchError> {
        let now = <eq_rate::Pallet<T>>::now().as_secs();
        if now > maturity {
            return Ok(0u64);
        }
        maturity
            .checked_sub(now)
            .ok_or(Error::<T>::MathError.into())
    }

    fn calc_ts(ts_period: T::XdotNumber) -> sp_std::result::Result<T::XdotNumber, DispatchError> {
        let one = T::NumberConvert::convert(1);
        one.checked_div(ts_period)
            .ok_or(Error::<T>::TsPeriodTooLarge.into())
    }

    fn do_create_pool(
        base_asset: T::AssetId,
        xbase_asset: T::AssetId,
        g1: T::XdotNumber,
        g2: T::XdotNumber,
        maturity: u64,
        ts: T::XdotNumber,
        initializer: T::AccountId,
    ) -> DispatchResultWithPostInfo {
        PoolCount::<T>::try_mutate(
            |pool_count| -> sp_std::result::Result<PoolId, DispatchError> {
                let pool_id = *pool_count;

                Pools::<T>::try_mutate_exists(pool_id, |maybe_pool_info| -> DispatchResult {
                    // We expect that XdotPoolInfos have sequential keys.
                    // No XdotPoolInfo can have key greater or equal to PoolCount
                    ensure!(maybe_pool_info.is_none(), Error::<T>::InconsistentStorage);
                    T::AssetChecker::check(base_asset, xbase_asset)?;

                    let pool_asset = T::Assets::create_lp_asset(pool_id)
                        .map_err(|_| Error::<T>::LPAssetNotCreated)?;
                    let zero_balance = T::Balance::zero();
                    let account = Self::generate_account_id(pool_id, pool_asset)?;
                    *maybe_pool_info = Some(XdotPoolInfo {
                        pool_asset,
                        base_asset,
                        xbase_asset,
                        g1,
                        g2,
                        maturity,
                        ts,
                        account: account.clone(),
                        lp_total_supply: zero_balance,
                    });

                    // We expect that pool's account behave like pallet account
                    // in terms of ref counters
                    eq_primitives::EqPalletAccountInitializer::<T>::initialize(&account);

                    Initializer::<T>::insert(pool_id, initializer);

                    Self::deposit_event(Event::PoolCreated(
                        pool_id,
                        pool_asset,
                        base_asset,
                        xbase_asset,
                        g1,
                        g2,
                        ts,
                    ));

                    Ok(())
                })?;

                *pool_count = pool_id
                    .checked_add(1)
                    .ok_or(Error::<T>::PoolCountOverflow)?;

                Ok(pool_id)
            },
        )?;

        Ok(().into())
    }

    /// Generates and returns `AccountId` using poolId and asset
    fn generate_account_id(pool_id: PoolId, asset: T::AssetId) -> Result<T::AccountId, Error<T>> {
        let raw = (b"eq/xdot-pool__", pool_id, asset).using_encoded(blake2_256);
        T::AccountId::decode(&mut &raw[..]).map_err(|_| Error::<T>::ExternalError)
    }

    pub fn invariant(pool_id: PoolId) -> Result<u128, DispatchError> {
        let pool = Self::get_pool(pool_id)?;
        let time_till_maturity = Self::time_till_maturity(pool.maturity)?;
        let virtual_xbase_balance = pool
            .virtual_xbase_balance()
            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;

        T::YieldMath::invariant(
            T::BalanceConvert::convert(pool.base_balance()),
            T::BalanceConvert::convert(virtual_xbase_balance),
            T::BalanceConvert::convert(pool.lp_total_supply),
            T::NumberConvert::convert(time_till_maturity),
            pool.ts,
            pool.g2,
            false,
        )
        .map(T::FixedNumberConvert::convert)
        .map_err(|_| Error::<T>::YieldMathInvariant)?
        .ok_or(Error::<T>::MathError.into())
    }

    pub fn fy_token_out_for_base_in(
        pool_id: PoolId,
        base_amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        let pool = Self::get_pool(pool_id)?;
        let time_till_maturity = Self::time_till_maturity(pool.maturity)?;
        let virtual_xbase_balance = pool
            .virtual_xbase_balance()
            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;
        T::YieldMath::fy_token_out_for_base_in(
            T::BalanceConvert::convert(pool.base_balance()),
            T::BalanceConvert::convert(virtual_xbase_balance),
            T::BalanceConvert::convert(base_amount),
            T::NumberConvert::convert(time_till_maturity),
            pool.ts,
            pool.g1,
        )
        .map(|a| T::BalanceConvert::convert(a))
        .map_err(|_| Error::<T>::YieldMathFyOutForBaseIn)?
        .ok_or(Error::<T>::BalanceConvertOverflow.into())
    }

    pub fn base_out_for_fy_token_in(
        pool_id: PoolId,
        fy_token_amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        let pool = Self::get_pool(pool_id)?;
        let time_till_maturity = Self::time_till_maturity(pool.maturity)?;
        let virtual_xbase_balance = pool
            .virtual_xbase_balance()
            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;
        T::YieldMath::base_out_for_fy_token_in(
            T::BalanceConvert::convert(pool.base_balance()),
            T::BalanceConvert::convert(virtual_xbase_balance),
            T::BalanceConvert::convert(fy_token_amount),
            T::NumberConvert::convert(time_till_maturity),
            pool.ts,
            pool.g2,
            false,
        )
        .map(|a| T::BalanceConvert::convert(a))
        .map_err(|_| Error::<T>::YieldMathBaseOutForFyIn)?
        .ok_or(Error::<T>::BalanceConvertOverflow.into())
    }

    pub fn fy_token_in_for_base_out(
        pool_id: PoolId,
        base_amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        let pool = Self::get_pool(pool_id)?;
        let time_till_maturity = Self::time_till_maturity(pool.maturity)?;
        let virtual_xbase_balance = pool
            .virtual_xbase_balance()
            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;

        T::YieldMath::fy_token_in_for_base_out(
            T::BalanceConvert::convert(pool.base_balance()),
            T::BalanceConvert::convert(virtual_xbase_balance),
            T::BalanceConvert::convert(base_amount),
            T::NumberConvert::convert(time_till_maturity),
            pool.ts,
            pool.g2,
        )
        .map(|a| T::BalanceConvert::convert(a))
        .map_err(|_| Error::<T>::YieldMathFyInForBaseOut)?
        .ok_or(Error::<T>::BalanceConvertOverflow.into())
    }

    pub fn base_in_for_fy_token_out(
        pool_id: PoolId,
        fy_token_amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        let pool = Self::get_pool(pool_id)?;
        let time_till_maturity = Self::time_till_maturity(pool.maturity)?;
        let virtual_xbase_balance = pool
            .virtual_xbase_balance()
            .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?;

        T::YieldMath::base_in_for_fy_token_out(
            T::BalanceConvert::convert(pool.base_balance()),
            T::BalanceConvert::convert(virtual_xbase_balance),
            T::BalanceConvert::convert(fy_token_amount),
            T::NumberConvert::convert(time_till_maturity),
            pool.ts,
            pool.g1,
        )
        .map(|a| T::BalanceConvert::convert(a))
        .map_err(|_| Error::<T>::YieldMathBaseInForFyOut)?
        .ok_or(Error::<T>::BalanceConvertOverflow.into())
    }

    fn calc_xbase_virtual_price(
        pool_info: &XdotPoolInfo<T>,
        custom_time_till_maturity: Option<u64>,
    ) -> Result<T::XdotNumber, DispatchError> {
        let time_till_maturity =
            custom_time_till_maturity.unwrap_or(Self::time_till_maturity(pool_info.maturity)?);
        let virtual_xbase_balance = T::BalanceConvert::convert(
            pool_info
                .virtual_xbase_balance()
                .ok_or(Error::<T>::CalcVirtualXbaseOverflow)?,
        );
        let base_balance = T::BalanceConvert::convert(pool_info.base_balance());

        let alpha = T::NumberConvert::convert(time_till_maturity)
            .checked_mul(pool_info.ts)
            .ok_or(Error::<T>::MathError)?;
        let ratio = base_balance
            .checked_div(virtual_xbase_balance)
            .ok_or(Error::<T>::MathError)?;
        let virtual_price = pow(ratio, alpha).map_err(|_| Error::<T>::MathError)?;

        Ok(virtual_price)
    }

    pub fn base_out_for_lp_in(
        pool_id: eq_primitives::xdot_pool::PoolId,
        lp_in: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        Self::ensure_pool_inited(pool_id)?;
        let pool = Self::get_pool(pool_id)?;
        let base_balance_num = T::BalanceConvert::convert(pool.base_balance());
        let real_xbase_balance_num = T::BalanceConvert::convert(pool.xbase_balance());
        let lp_supply_num = T::BalanceConvert::convert(pool.lp_total_supply);
        let tokens_burned_num = T::BalanceConvert::convert(lp_in);
        let maturity = pool.maturity;
        let ts = pool.ts;
        let g2 = pool.g2;
        Self::burn_calculations(
            base_balance_num,
            real_xbase_balance_num,
            lp_supply_num,
            tokens_burned_num,
            Some((maturity, ts, g2)),
        )
        .map(|(base_out, _)| base_out)
    }

    pub fn base_and_fy_out_for_lp_in(
        pool_id: eq_primitives::xdot_pool::PoolId,
        lp_in: T::Balance,
    ) -> Result<(T::Balance, T::Balance), DispatchError> {
        Self::ensure_pool_inited(pool_id)?;
        let pool = Self::get_pool(pool_id)?;
        let base_balance_num = T::BalanceConvert::convert(pool.base_balance());
        let real_xbase_balance_num = T::BalanceConvert::convert(pool.xbase_balance());
        let lp_supply_num = T::BalanceConvert::convert(pool.lp_total_supply);
        let tokens_burned_num = T::BalanceConvert::convert(lp_in);
        Self::burn_calculations(
            base_balance_num,
            real_xbase_balance_num,
            lp_supply_num,
            tokens_burned_num,
            None,
        )
    }

    fn ensure_pool_inited(pool_id: PoolId) -> Result<(), DispatchError> {
        match Self::initializer(pool_id) {
            Some(_) => Err(Error::<T>::NeedToInitialize.into()),
            None => Ok(()),
        }
    }

    pub fn max_base_xbase_in_and_out(
        pool_id: PoolId,
    ) -> Result<(T::Balance, T::Balance, T::Balance, T::Balance), DispatchError> {
        let pool = Self::get_pool(pool_id)?;
        let base_balance_num = T::BalanceConvert::convert(pool.base_balance());
        let xbase_balance_num = T::BalanceConvert::convert(pool.xbase_balance());
        let time_till_maturity =
            T::NumberConvert::convert(Self::time_till_maturity(pool.maturity)?);
        let max_xbase_out = T::YieldMath::max_fy_token_out(
            base_balance_num,
            xbase_balance_num,
            time_till_maturity,
            pool.ts,
            pool.g1,
        )
        .map_err(|_| Error::<T>::YieldMathBaseInForFyOut)?;
        let max_xbase_in = T::YieldMath::max_fy_token_in(
            base_balance_num,
            xbase_balance_num,
            time_till_maturity,
            pool.ts,
            pool.g2,
        )
        .map_err(|_| Error::<T>::YieldMathBaseInForFyOut)?;
        let zero = T::NumberConvert::convert(0);
        let max_base_in = if max_xbase_out > zero {
            T::YieldMath::base_in_for_fy_token_out(
                base_balance_num,
                xbase_balance_num,
                max_xbase_out,
                time_till_maturity,
                pool.ts,
                pool.g1,
            )
            .map_err(|_| Error::<T>::YieldMathBaseInForFyOut)?
        } else {
            zero
        };

        let max_base_out = T::YieldMath::base_out_for_fy_token_in(
            base_balance_num,
            xbase_balance_num,
            max_base_in,
            time_till_maturity,
            pool.ts,
            pool.g2,
            false,
        )
        .map_err(|_| Error::<T>::YieldMathBaseOutForFyIn)?;

        Ok((
            T::BalanceConvert::convert(max_base_in).ok_or(Error::<T>::BalanceConvertOverflow)?,
            T::BalanceConvert::convert(max_base_out).ok_or(Error::<T>::BalanceConvertOverflow)?,
            T::BalanceConvert::convert(max_xbase_in).ok_or(Error::<T>::BalanceConvertOverflow)?,
            T::BalanceConvert::convert(max_xbase_out).ok_or(Error::<T>::BalanceConvertOverflow)?,
        ))
    }
}

impl<T: Config> XBasePrice<T::AssetId, T::Balance, T::PriceNumber> for Pallet<T> {
    type XdotPoolInfo = XdotPoolInfo<T>;

    fn get_xbase_virtual_price(
        pool_info: &XdotPoolInfo<T>,
        custom_time_till_maturity: Option<u64>,
    ) -> Result<T::PriceNumber, DispatchError> {
        let virtual_price = Self::calc_xbase_virtual_price(pool_info, custom_time_till_maturity)?;
        Ok(T::PriceConvert::convert(virtual_price))
    }

    fn get_lp_virtual_price(
        pool_info: &XdotPoolInfo<T>,
        custom_time_till_maturity: Option<u64>,
    ) -> Result<T::PriceNumber, DispatchError> {
        let time_till_maturity =
            custom_time_till_maturity.unwrap_or(Self::time_till_maturity(pool_info.maturity)?);
        let base_balance = T::BalanceConvert::convert(pool_info.base_balance());
        let xbase_balance = T::BalanceConvert::convert(pool_info.xbase_balance());
        let lp_supply = T::BalanceConvert::convert(pool_info.lp_total_supply);
        let xbase_price = Self::calc_xbase_virtual_price(pool_info, Some(time_till_maturity))?;

        let virtual_price = base_balance
            .checked_add(
                xbase_balance
                    .checked_mul(xbase_price)
                    .ok_or(Error::<T>::MathError)?,
            )
            .ok_or(Error::<T>::MathError)?
            .checked_div(lp_supply)
            .ok_or(Error::<T>::MathError)?;

        Ok(T::PriceConvert::convert(virtual_price))
    }

    fn get_pool(pool_id: PoolId) -> Result<XdotPoolInfo<T>, DispatchError> {
        Self::get_pool(pool_id)
    }
}

pub mod traits {
    use super::*;
    pub trait Assets<AssetId, Balance, AccountId> {
        /// Creates new lp asset
        fn create_lp_asset(pool_id: PoolId) -> Result<AssetId, DispatchError>;
        /// Mint tokens for the specified asset
        fn mint(asset: AssetId, dest: &AccountId, amount: Balance) -> DispatchResult;
        /// Burn tokens for the specified asset
        fn burn(asset: AssetId, dest: &AccountId, amount: Balance) -> DispatchResult;
        /// Transfer tokens for the specified asset
        fn transfer(
            asset: AssetId,
            source: &AccountId,
            dest: &AccountId,
            amount: Balance,
        ) -> DispatchResult;
        /// Checks the balance for the specified asset
        fn balance(asset: AssetId, who: &AccountId) -> Balance;
        /// Returns total issuance of the specified asset
        fn total_issuance(asset: AssetId) -> Balance;
    }

    pub trait OnPoolInitialized {
        fn on_initalize(pool_id: PoolId) -> Result<(), DispatchError>;
    }

    impl OnPoolInitialized for () {
        fn on_initalize(_pool_id: PoolId) -> Result<(), DispatchError> {
            Ok(())
        }
    }

    pub trait AssetChecker<AssetId> {
        fn check(base_asset: AssetId, xbase_asset: AssetId) -> Result<(), DispatchError>;
    }

    impl<AssetId> AssetChecker<AssetId> for () {
        fn check(_base_asset: AssetId, _xbase_asset: AssetId) -> Result<(), DispatchError> {
            Ok(())
        }
    }
}
