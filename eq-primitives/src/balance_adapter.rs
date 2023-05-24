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

//! Module adapting `eq_primitives::balance::EqCurrency;` to work as `BasicCurrency` in
//! Substrate runtime

use codec::{FullCodec, MaxEncodedLen};
use frame_support::traits::{
    BalanceStatus, Currency, Get, LockableCurrency, ReservableCurrency, SignedImbalance,
};
use sp_arithmetic::FixedPointOperand;
use sp_runtime::traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, Member, Zero};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::fmt::Debug;
use sp_std::marker;

use crate::balance::EqCurrency;
pub use crate::imbalances::{NegativeImbalance, PositiveImbalance};
pub use crate::signed_balance::{SignedBalance, SignedBalance::*};

/// Adapts [`EqCurrency`](../trait.EqCurrency.html) to work as `BasicCurrency`
/// in Substrate runtime
pub struct BalanceAdapter<Balance, MultiCurrency, CurrencyGetter>(
    marker::PhantomData<(Balance, MultiCurrency, CurrencyGetter)>,
);

impl<AccountId, Balance, MultiCurrency, CurrencyGetter> Currency<AccountId>
    for BalanceAdapter<Balance, MultiCurrency, CurrencyGetter>
where
    Balance: Member
        + AtLeast32BitUnsigned
        + FullCodec
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + Default
        + MaxEncodedLen
        + scale_info::TypeInfo
        + FixedPointOperand,
    MultiCurrency: EqCurrency<AccountId, Balance>,
    CurrencyGetter: Get<crate::asset::Asset>,
{
    type Balance = Balance;
    type PositiveImbalance = PositiveImbalance<Balance>;
    type NegativeImbalance = NegativeImbalance<Balance>;

    fn total_balance(who: &AccountId) -> Self::Balance {
        MultiCurrency::total_balance(who, CurrencyGetter::get())
    }

    fn can_slash(_who: &AccountId, _value: Self::Balance) -> bool {
        unimplemented!("fn can_slash")
    }

    fn total_issuance() -> Self::Balance {
        MultiCurrency::currency_total_issuance(CurrencyGetter::get())
    }

    fn minimum_balance() -> Self::Balance {
        MultiCurrency::minimum_balance_value()
    }

    fn burn(_amount: Self::Balance) -> Self::PositiveImbalance {
        unimplemented!("fn burn");
    }

    fn issue(amount: Self::Balance) -> Self::NegativeImbalance {
        // used in XcmConfig -> Trader=UsingComponents<..., OnUnbalanced>
        Self::NegativeImbalance::new(amount)
    }

    fn free_balance(who: &AccountId) -> Self::Balance {
        MultiCurrency::free_balance(who, CurrencyGetter::get())
    }

    fn ensure_can_withdraw(
        who: &AccountId,
        amount: Self::Balance,
        reasons: frame_support::traits::WithdrawReasons,
        new_balance: Self::Balance,
    ) -> sp_runtime::DispatchResult {
        MultiCurrency::ensure_can_withdraw(who, CurrencyGetter::get(), amount, reasons, new_balance)
    }

    fn transfer(
        source: &AccountId,
        dest: &AccountId,
        value: Self::Balance,
        existence_requirement: frame_support::traits::ExistenceRequirement,
    ) -> sp_runtime::DispatchResult {
        MultiCurrency::currency_transfer(
            source,
            dest,
            CurrencyGetter::get(),
            value,
            existence_requirement,
            crate::TransferReason::Common,
            true,
        )
    }

    fn slash(_who: &AccountId, _value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
        unimplemented!("fn slash")
    }

    fn deposit_into_existing(
        who: &AccountId,
        value: Self::Balance,
    ) -> Result<Self::PositiveImbalance, sp_runtime::DispatchError> {
        MultiCurrency::deposit_into_existing(who, CurrencyGetter::get(), value, None)
            .expect("deposit_into_existing failed");
        Ok(PositiveImbalance::new(value))
    }

    fn deposit_creating(who: &AccountId, value: Self::Balance) -> Self::PositiveImbalance {
        let result = MultiCurrency::deposit_creating(who, CurrencyGetter::get(), value, true, None);
        if result.is_err() {
            log::error!(
                "{}:{}. Error while deposit_creating, value: {:?}",
                file!(),
                line!(),
                value
            );
            PositiveImbalance::new(Zero::zero())
        } else {
            PositiveImbalance::new(value)
        }
    }

    fn withdraw(
        who: &AccountId,
        value: Self::Balance,
        reasons: frame_support::traits::WithdrawReasons,
        liveness: frame_support::traits::ExistenceRequirement,
    ) -> Result<Self::NegativeImbalance, sp_runtime::DispatchError> {
        MultiCurrency::withdraw(
            who,
            CurrencyGetter::get(),
            value,
            true,
            None,
            reasons,
            liveness,
        )
        .map(|_| NegativeImbalance::new(value))
    }

    fn make_free_balance_be(
        _who: &AccountId,
        _balance: Self::Balance,
    ) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
        #[cfg(any(feature = "runtime-benchmarks", feature = "std"))]
        {
            MultiCurrency::make_free_balance_be(
                _who,
                CurrencyGetter::get(),
                SignedBalance::Positive(_balance),
            );
            frame_support::traits::SignedImbalance::Positive(Self::PositiveImbalance::new(
                Zero::zero(),
            ))
        }

        #[cfg(not(any(feature = "runtime-benchmarks", feature = "std")))]
        {
            unimplemented!()
        }
    }
}

impl<AccountId, Balance, MultiCurrency, CurrencyGetter> ReservableCurrency<AccountId>
    for BalanceAdapter<Balance, MultiCurrency, CurrencyGetter>
where
    Balance: Member
        + AtLeast32BitUnsigned
        + FullCodec
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + Default
        + MaxEncodedLen
        + scale_info::TypeInfo
        + FixedPointOperand,
    MultiCurrency: EqCurrency<AccountId, Balance>,
    CurrencyGetter: Get<crate::asset::Asset>,
{
    /// Check if `who` can reserve `value` from their free balance.
    ///
    /// Always `true` if value to be reserved is zero.
    fn can_reserve(who: &AccountId, value: Balance) -> bool {
        if value.is_zero() {
            return true;
        }
        MultiCurrency::total_balance(who, CurrencyGetter::get()) > value
    }

    fn reserved_balance(who: &AccountId) -> Self::Balance {
        let asset = CurrencyGetter::get();
        MultiCurrency::reserved_balance(who, asset)
    }

    /// Move `value` from the free balance from `who` to their reserved balance.
    ///
    /// Is a no-op if value to be reserved is zero.
    fn reserve(who: &AccountId, value: Self::Balance) -> DispatchResult {
        let asset = CurrencyGetter::get();
        MultiCurrency::reserve(who, asset, value)
    }

    /// Unreserve some funds, returning any amount that was unable to be unreserved.
    ///
    /// Is a no-op if the value to be unreserved is zero or the account does not exist.
    fn unreserve(who: &AccountId, value: Self::Balance) -> Self::Balance {
        let asset = CurrencyGetter::get();
        MultiCurrency::unreserve(who, asset, value)
    }

    /// Slash from reserved balance, returning the negative imbalance created,
    /// and any amount that was unable to be slashed.
    ///
    /// Is a no-op if the value to be slashed is zero or the account does not exist.
    fn slash_reserved(
        who: &AccountId,
        value: Self::Balance,
    ) -> (Self::NegativeImbalance, Self::Balance) {
        let asset = CurrencyGetter::get();
        MultiCurrency::slash_reserved(who, asset, value)
    }

    /// Move the reserved balance of one account into the balance of another, according to `status`.
    ///
    /// Is a no-op if:
    /// - the value to be moved is zero; or
    /// - the `slashed` id equal to `beneficiary` and the `status` is `Reserved`.
    fn repatriate_reserved(
        slashed: &AccountId,
        beneficiary: &AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> Result<Self::Balance, DispatchError> {
        let asset = CurrencyGetter::get();
        MultiCurrency::repatriate_reserved(slashed, beneficiary, asset, value, status)
    }
}

impl<AccountId, Balance, MultiCurrency, CurrencyGetter> LockableCurrency<AccountId>
    for BalanceAdapter<Balance, MultiCurrency, CurrencyGetter>
where
    Balance: Member
        + AtLeast32BitUnsigned
        + FullCodec
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + Default
        + MaxEncodedLen
        + scale_info::TypeInfo
        + FixedPointOperand,
    MultiCurrency: EqCurrency<AccountId, Balance>,
    CurrencyGetter: Get<crate::asset::Asset>,
{
    type Moment = MultiCurrency::Moment;

    type MaxLocks = MultiCurrency::MaxLocks;

    fn set_lock(
        id: frame_support::traits::LockIdentifier,
        who: &AccountId,
        amount: Self::Balance,
        _reasons: frame_support::traits::WithdrawReasons,
    ) {
        MultiCurrency::set_lock(id, who, amount)
    }

    fn extend_lock(
        id: frame_support::traits::LockIdentifier,
        who: &AccountId,
        amount: Self::Balance,
        _reasons: frame_support::traits::WithdrawReasons,
    ) {
        MultiCurrency::extend_lock(id, who, amount)
    }

    fn remove_lock(id: frame_support::traits::LockIdentifier, who: &AccountId) {
        MultiCurrency::remove_lock(id, who)
    }
}
