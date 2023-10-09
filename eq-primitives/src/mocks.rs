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

use codec::{Decode, Encode};
use core::{cell::RefCell, marker::PhantomData};
use frame_support::traits::{Get, UnixTime};
use sp_runtime::{
    traits::{AccountIdConversion, One, Zero},
    DispatchError, FixedI64, FixedPointNumber,
};
use sp_std::{convert::TryFrom, fmt::Debug};
use xcm::v3::{
    InteriorMultiLocation, Junction::*, Junctions::X2, MultiAssets, MultiLocation, NetworkId,
    SendError, SendResult, SendXcm, Xcm, XcmHash,
};

use crate::{
    asset::{self, Asset},
    balance::Balance,
    PriceGetter, UpdateTimeManager,
};

pub struct XcmRouterOkMock;

impl SendXcm for XcmRouterOkMock {
    type Ticket = ();
    fn validate(
        _destination: &mut Option<MultiLocation>,
        _message: &mut Option<Xcm<()>>,
    ) -> SendResult<Self::Ticket> {
        Ok(((), MultiAssets::new()))
    }

    fn deliver(_one_ticket: Self::Ticket) -> Result<XcmHash, SendError> {
        Ok([0u8; 32])
    }
}

#[cfg(feature = "std")]
thread_local! {
    pub static XCM_MESSAGES: RefCell<Vec<(MultiLocation, Xcm<()>)>> = RefCell::default();
}

/// `send_xcm` saves sent xcm message and returns `Ok(())`
#[cfg(feature = "std")]
pub struct XcmRouterCachedMessagesMock;

#[cfg(feature = "std")]
impl XcmRouterCachedMessagesMock {
    pub fn get() -> Vec<(MultiLocation, Xcm<()>)> {
        XCM_MESSAGES
            .try_with(|cache| cache.borrow().clone())
            .unwrap_or_default()
    }

    pub fn clear() {
        let _ = XCM_MESSAGES.try_with(|cache| cache.borrow_mut().clear());
    }
}

#[cfg(feature = "std")]
impl SendXcm for XcmRouterCachedMessagesMock {
    type Ticket = ();
    fn validate(
        destination: &mut Option<MultiLocation>,
        message: &mut Option<Xcm<()>>,
    ) -> SendResult<()> {
        XCM_MESSAGES
            .try_with(|cache| {
                cache
                    .borrow_mut()
                    .push((destination.unwrap().into(), message.clone().unwrap()))
            })
            .map_err(|_| SendError::Unroutable)
            .map(|_| ((), MultiAssets::new()))
    }
    fn deliver(_: ()) -> Result<XcmHash, SendError> {
        Ok([0; 32])
    }
}

/// `send_xcm` do nothing returns `Err(SendError::Unroutable)`
pub struct XcmRouterErrMock;

impl SendXcm for XcmRouterErrMock {
    type Ticket = ();
    fn validate(
        _destination: &mut Option<MultiLocation>,
        _message: &mut Option<Xcm<()>>,
    ) -> SendResult<()> {
        Err(SendError::Unroutable)
    }
    fn deliver(_: ()) -> Result<XcmHash, SendError> {
        Err(SendError::Unroutable)
    }
}

/// `convert` returns value from `FEE` generic
pub struct XcmToFeeMock<FEE: Get<crate::XcmBalance>>(PhantomData<FEE>);
impl<'xcm, FEE: Get<Balance>>
    sp_runtime::traits::Convert<
        (Asset, MultiLocation, &'xcm Xcm<()>),
        Option<(Asset, crate::XcmBalance)>,
    > for XcmToFeeMock<FEE>
{
    fn convert(
        (asset, _, _): (Asset, MultiLocation, &'xcm Xcm<()>),
    ) -> Option<(Asset, crate::XcmBalance)> {
        Some((asset, FEE::get()))
    }
}

pub struct ZeroFee;

impl<B: Zero> Get<B> for ZeroFee {
    fn get() -> B {
        B::zero()
    }
}

/// `XcmToFee` with zero fee
pub type XcmToFeeZeroMock = XcmToFeeMock<ZeroFee>;

pub struct UniversalLocationMock;
impl Get<InteriorMultiLocation> for UniversalLocationMock {
    fn get() -> InteriorMultiLocation {
        X2(
            GlobalConsensus(NetworkId::Polkadot),
            Parachain(ParachainId::get().into()),
        )
    }
}

pub struct PriceGetterMock;
impl PriceGetter for PriceGetterMock {
    fn get_price<FixedNumber>(asset: &Asset) -> Result<FixedNumber, sp_runtime::DispatchError>
    where
        FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>,
    {
        if asset == &asset::EQD {
            Ok(FixedNumber::one())
        } else {
            Err(DispatchError::Other("No price"))
        }
    }
}

pub struct UpdateTimeManagerEmptyMock<AccountId>(PhantomData<AccountId>);
impl<AccountId> UpdateTimeManager<AccountId> for UpdateTimeManagerEmptyMock<AccountId> {
    fn set_last_update(_: &AccountId) {}

    #[cfg(not(feature = "production"))]
    fn set_last_update_timestamp(_: &AccountId, _: u64) {}

    fn remove_last_update(_: &AccountId) {}
}

pub struct TimeZeroDurationMock;
impl UnixTime for TimeZeroDurationMock {
    fn now() -> core::time::Duration {
        core::time::Duration::new(0, 0)
    }
}

const VESTING_MODULE_ID: frame_support::PalletId = frame_support::PalletId(*b"eq/vestn");

pub struct VestingAccountMock<AccountId>(PhantomData<AccountId>);
impl<AccountId: Encode + Decode> Get<AccountId> for VestingAccountMock<AccountId> {
    fn get() -> AccountId {
        VESTING_MODULE_ID.into_account_truncating()
    }
}

frame_support::parameter_types! {
    pub ParachainId: polkadot_parachain_primitives::primitives::Id = 2011u32.into();
}
