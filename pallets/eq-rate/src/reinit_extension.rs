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

//! A `SignedExtension` module. Adds a reinit request for the signer of each trx

use super::Config;
use crate::Pallet;
use codec::{Decode, Encode};
use core::fmt::Debug;
use eq_primitives::subaccount::{SubAccType, SubaccountsManager};
use frame_support::dispatch::DispatchInfo;
use frame_support::traits::Contains;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::{DispatchInfoOf, Dispatchable, SignedExtension};
use sp_runtime::transaction_validity::TransactionValidityError;
use sp_std::marker::PhantomData;

/// Account reinit request
#[derive(Encode, Decode, Clone, Eq, PartialEq, scale_info::TypeInfo)]
pub struct ReinitAccount<
    T: Config + Send + Sync + scale_info::TypeInfo,
    CallsWithReinit: 'static + Contains<T::RuntimeCall> + Sync + Send + Clone + Eq + scale_info::TypeInfo,
>(PhantomData<(T, CallsWithReinit)>);

impl<
        T: Config + Send + Sync + scale_info::TypeInfo,
        CallsWithReinit: 'static + Contains<T::RuntimeCall> + Sync + Send + Clone + Eq + scale_info::TypeInfo,
    > Debug for ReinitAccount<T, CallsWithReinit>
{
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "ReinitAccount")
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

impl<
        T: Config + Send + Sync + scale_info::TypeInfo,
        CallsWithReinit: 'static + Contains<T::RuntimeCall> + Sync + Send + Clone + Eq + scale_info::TypeInfo,
    > ReinitAccount<T, CallsWithReinit>
{
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<
        T: Config + Send + Sync + scale_info::TypeInfo,
        CallsWithReinit: 'static + Contains<T::RuntimeCall> + Sync + Send + Clone + Eq + scale_info::TypeInfo,
    > SignedExtension for ReinitAccount<T, CallsWithReinit>
where
    T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
{
    type AccountId = T::AccountId;
    type Call = T::RuntimeCall;
    type AdditionalSigned = ();
    type Pre = ();
    const IDENTIFIER: &'static str = "ReinitAccount";

    fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
        Ok(())
    }

    #[allow(unused_must_use)]
    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> Result<(), TransactionValidityError> {
        if <crate::AutoReinitEnabled<T>>::get() && CallsWithReinit::contains(call) {
            if let Some(bails_subacc) =
                T::SubaccountsManager::get_subaccount_id(who, &SubAccType::Bailsman)
            {
                <Pallet<T>>::do_reinit(&bails_subacc);
            };
            if let Some(trader_subacc) =
                T::SubaccountsManager::get_subaccount_id(who, &SubAccType::Trader)
            {
                <Pallet<T>>::do_reinit(&trader_subacc);
            };
            if let Some(borrow_subacc) =
                T::SubaccountsManager::get_subaccount_id(who, &SubAccType::Borrower)
            {
                <Pallet<T>>::do_reinit(&borrow_subacc);
            };
        }

        Ok(())
    }
}
