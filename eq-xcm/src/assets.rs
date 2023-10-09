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

use eq_primitives::{
    asset::{self, Asset},
    balance::{Balance as B, DepositReason, EqCurrency, WithdrawReason},
    chainbridge,
    price::PriceGetter,
    XcmMode,
};
use eq_utils::{multiply_by_rational, XcmBalance};
use frame_support::{
    pallet_prelude::*,
    traits::{ExistenceRequirement, WithdrawReasons},
    weights::{WeightToFee, WeightToFeePolynomial},
};
use sp_runtime::{
    traits::{AccountIdConversion as _, Convert, One},
    FixedI64,
};
#[cfg(not(feature = "std"))]
use sp_std::vec::Vec;
use sp_std::{convert::TryFrom, marker::PhantomData};
use xcm::v3::{
    AssetId, Error as XcmError, Fungibility, MultiAsset, MultiLocation, Weight as XcmWeight,
    XcmContext,
};
use xcm_executor::{
    traits::{TransactAsset, WeightTrader},
    Assets,
};

type XcmResult<T> = Result<T, XcmError>;

pub const GENSHIRO_CHAIN_ID: chainbridge::ChainId = 1;

pub struct EqCurrencyAdapter<
    AccountId,
    Balance,
    EqCurrency,
    EqMatches,
    EqBridge,
    AccountIdConverter,
    CheckedAccount,
>(
    PhantomData<(
        AccountId,
        Balance,
        EqCurrency,
        EqMatches,
        EqBridge,
        AccountIdConverter,
        CheckedAccount,
    )>,
);

impl<
        AccountId: Clone + Encode + Decode, // can't get away without it since Currency is generic over it.
        Balance: TryFrom<B>
            + Member
            + sp_runtime::traits::AtLeast32BitUnsigned
            + codec::FullCodec
            + Copy
            + MaybeSerializeDeserialize
            + sp_std::fmt::Debug
            + Default,
        EqCurrency: eq_primitives::balance::EqCurrency<AccountId, Balance> + Get<Option<XcmMode>>,
        EqMatches: EqMatchesFungible<Asset, Balance>,
        EqBridge: chainbridge::Bridge<AccountId, Balance, chainbridge::ChainId, chainbridge::ResourceId>
            + chainbridge::ResourceGetter<chainbridge::ResourceId>,
        AccountIdConverter: xcm_executor::traits::ConvertLocation<AccountId>,
        CheckedAccount: Get<Option<AccountId>>,
    > TransactAsset
    for EqCurrencyAdapter<
        AccountId,
        Balance,
        EqCurrency,
        EqMatches,
        EqBridge,
        AccountIdConverter,
        CheckedAccount,
    >
{
    fn can_check_in(
        _origin: &MultiLocation,
        _what: &MultiAsset,
        _context: &XcmContext,
    ) -> XcmResult<()> {
        // All checks done in IsTeleporter
        Ok(())
    }

    fn check_in(_origin: &MultiLocation, _what: &MultiAsset, _context: &XcmContext) {}

    fn can_check_out(
        _dest: &MultiLocation,
        _what: &MultiAsset,
        _context: &XcmContext,
    ) -> Result<(), XcmError> {
        Err(XcmError::Unimplemented)
    }

    fn check_out(_dest: &MultiLocation, _what: &MultiAsset, _context: &XcmContext) {}

    fn deposit_asset(
        what: &MultiAsset,
        who: &MultiLocation,
        _context: &XcmContext,
    ) -> XcmResult<()> {
        log::trace!(target: "xcm::eq_currency_adapter", "deposit_asset {:?} to {:?}", what, who);
        let (asset, amount) = EqMatches::matches_fungible(&what).ok_or(XcmError::AssetNotFound)?;
        let who = AccountIdConverter::convert_location(who)
            .ok_or(XcmError::FailedToTransactAsset("AccountIdConversionFailed"))?;

        match <EqCurrency as Get<Option<XcmMode>>>::get() {
            None | Some(XcmMode::Xcm(_)) => {
                log::trace!(target: "xcm::eq_currency_adapter", "deposit_creating {:?}", amount);
                EqCurrency::deposit_creating(
                    &who,
                    asset,
                    amount,
                    true,
                    Some(DepositReason::XcmTransfer),
                )
                .map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;
            }
            Some(XcmMode::Bridge(_)) => {
                let bridge = chainbridge::MODULE_ID.into_account_truncating();

                log::trace!(target: "xcm::eq_currency_adapter", "deposit_creating {:?}", amount);
                EqCurrency::deposit_creating(&bridge, asset, amount, true, None)
                    .map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

                log::trace!(target: "xcm::eq_currency_adapter", "transfer_to_standalone {:?}", amount);
                let resource_id = EqBridge::get_resource_by_asset(asset).ok_or(
                    XcmError::FailedToTransactAsset("EqBridge::ResourceNotFound"),
                )?;
                EqBridge::transfer_native(
                    bridge,
                    amount.into(),
                    who.using_encoded(|a| a.into()),
                    GENSHIRO_CHAIN_ID,
                    resource_id,
                )
                .map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;
            }
        };

        Ok(())
    }

    fn withdraw_asset(
        what: &MultiAsset,
        who: &MultiLocation,
        _maybe_context: Option<&XcmContext>,
    ) -> XcmResult<Assets> {
        log::trace!(target: "xcm::eq_currency_adapter", "withdraw_asset {:?} from {:?}", what, who);
        let (asset, amount) = EqMatches::matches_fungible(what).ok_or(XcmError::AssetNotFound)?;
        let who = AccountIdConverter::convert_location(who)
            .ok_or(XcmError::FailedToTransactAsset("AccountIdConversionFailed"))?;

        log::trace!(target: "xcm::eq_currency_adapter", "withdraw {:?}", amount);
        EqCurrency::withdraw(
            &who,
            asset,
            amount,
            true,
            Some(WithdrawReason::XcmReserve),
            WithdrawReasons::empty(),
            ExistenceRequirement::AllowDeath,
        )
        .map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

        Ok(what.clone().into())
    }

    fn transfer_asset(
        what: &MultiAsset,
        from: &MultiLocation,
        to: &MultiLocation,
        context: &XcmContext,
    ) -> XcmResult<Assets> {
        log::trace!(target: "xcm::eq_currency_adapter", "beam_asset {:?} from {:?} to {:?}", what, from, to);
        match <EqCurrency as Get<Option<XcmMode>>>::get() {
            None | Some(XcmMode::Xcm(_)) => {
                let (asset, amount) =
                    EqMatches::matches_fungible(what).ok_or(XcmError::AssetNotFound)?;
                let from = AccountIdConverter::convert_location(from)
                    .ok_or(XcmError::FailedToTransactAsset("AccountIdConversionFailed"))?;
                let to = AccountIdConverter::convert_location(to)
                    .ok_or(XcmError::FailedToTransactAsset("AccountIdConversionFailed"))?;

                log::trace!(target: "xcm::eq_currency_adapter", "currency_transfer {:?}", amount);
                EqCurrency::currency_transfer(
                    &from,
                    &to,
                    asset,
                    amount,
                    ExistenceRequirement::AllowDeath,
                    eq_primitives::TransferReason::Common,
                    true,
                )
                .map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

                Ok(what.clone().into())
            }
            Some(XcmMode::Bridge(_)) => {
                let assets = Self::withdraw_asset(what, from, Some(context))?;
                Self::deposit_asset(what, to, context)?;
                Ok(assets)
            }
        }
    }
}

pub trait EqMatchesFungible<Asset, Balance> {
    fn matches_fungible(asset: &MultiAsset) -> Option<(Asset, Balance)>;
}

impl<AssetGetter: asset::AssetGetter, Balance: TryFrom<B>> EqMatchesFungible<Asset, Balance>
    for AssetGetter
{
    fn matches_fungible(asset: &MultiAsset) -> Option<(Asset, Balance)> {
        match *asset {
            MultiAsset {
                id: AssetId::Concrete(ref multi_location),
                fun: Fungibility::Fungible(amount),
            } => {
                let asset = AssetGetter::get_assets_data_with_usd()
                    .into_iter()
                    .find_map(|asset| match asset.get_xcm_data() {
                        Some((ref asset_multi_location, decimals, _))
                            if asset_multi_location == multi_location =>
                        {
                            Some((asset.id, decimals))
                        }
                        _ => None,
                    })?;
                let balance = eq_utils::balance_from_xcm(amount, asset.1)?;
                Some((asset.0, balance))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EqTraderAssetInfo {
    asset: Asset,
    multi_location: MultiLocation,
    decimals: u8,
    weight: XcmWeight,
    amount: B,
}

pub enum EqTraderError {
    Overflow,
    NoFee,
}

pub struct EqTrader<
    // AccountId type
    AccountId,
    // Balance type
    Balance: TryFrom<B>
        + Member
        + sp_runtime::traits::AtLeast32BitUnsigned
        + codec::FullCodec
        + Copy
        + MaybeSerializeDeserialize
        + sp_std::fmt::Debug
        + Default,
    // Used to get all assets and filter ones that are used in xcm transactions
    AssetGet: asset::AssetGetter,
    // Used to transfer residues to treasury, if there are unspent tokens
    EqBalances: EqCurrency<AccountId, Balance>,
    // USed to calculate fee from usd amount,
    EqPrices: PriceGetter,
    // EqTreasury pallet account id
    TreasuryAccount: Get<AccountId>,
    // Rational number, which is an amount of usd (in u128) to pay for one weight unit
    UsdXcmWeightToFee: WeightToFeePolynomial<Balance = eq_utils::XcmBalance>,
    // Fallback for asset without price
    FallbackXcmWeightToFee: Convert<(Asset, XcmWeight), Option<eq_utils::XcmBalance>>,
> {
    assets: Vec<EqTraderAssetInfo>,
    phantom_data: PhantomData<(
        AccountId,
        Balance,
        AssetGet,
        EqBalances,
        EqPrices,
        TreasuryAccount,
        UsdXcmWeightToFee,
        FallbackXcmWeightToFee,
    )>,
}

impl<
        AccountId,
        Balance: TryFrom<B>
            + Member
            + sp_runtime::traits::AtLeast32BitUnsigned
            + codec::FullCodec
            + Copy
            + MaybeSerializeDeserialize
            + sp_std::fmt::Debug
            + Default,
        AssetGet: asset::AssetGetter,
        EqBalances: EqCurrency<AccountId, Balance>,
        EqPrices: PriceGetter,
        TreasuryAccount: Get<AccountId>,
        UsdXcmWeightToFee: WeightToFeePolynomial<Balance = eq_utils::XcmBalance>,
        FallbackXcmWeightToFee: Convert<(Asset, XcmWeight), Option<eq_utils::XcmBalance>>,
    >
    EqTrader<
        AccountId,
        Balance,
        AssetGet,
        EqBalances,
        EqPrices,
        TreasuryAccount,
        UsdXcmWeightToFee,
        FallbackXcmWeightToFee,
    >
{
    pub fn weight_to_fee(
        asset: &EqTraderAssetInfo,
        weight: &XcmWeight,
    ) -> Result<XcmBalance, EqTraderError> {
        if let Ok(price) = EqPrices::get_price::<FixedI64>(&asset.asset) {
            let fee_in_usd = eq_utils::balance_into_xcm(
                UsdXcmWeightToFee::weight_to_fee(&weight),
                asset.decimals,
            )
            .ok_or(EqTraderError::Overflow)?;

            let fee = multiply_by_rational(
                fee_in_usd,
                FixedI64::one().into_inner() as u128, // USD price = 1.0
                price.into_inner() as u128,
            )
            .ok_or(EqTraderError::Overflow)?;

            Ok(fee)
        } else if let Some(fee) = FallbackXcmWeightToFee::convert((asset.asset, *weight)) {
            Ok(fee)
        } else {
            Err(EqTraderError::NoFee)
        }
    }

    pub fn inner(&self) -> Vec<EqTraderAssetInfo> {
        self.assets.clone()
    }
}

impl<
        AccountId,
        Balance: TryFrom<B>
            + Member
            + sp_runtime::traits::AtLeast32BitUnsigned
            + codec::FullCodec
            + Copy
            + MaybeSerializeDeserialize
            + sp_std::fmt::Debug
            + Default,
        AssetGet: asset::AssetGetter,
        EqBalances: EqCurrency<AccountId, Balance>,
        EqPrices: PriceGetter,
        TreasuryAccount: Get<AccountId>,
        UsdXcmWeightToFee: WeightToFeePolynomial<Balance = eq_utils::XcmBalance>,
        FallbackXcmWeightToFee: Convert<(Asset, XcmWeight), Option<eq_utils::XcmBalance>>,
    > WeightTrader
    for EqTrader<
        AccountId,
        Balance,
        AssetGet,
        EqBalances,
        EqPrices,
        TreasuryAccount,
        UsdXcmWeightToFee,
        FallbackXcmWeightToFee,
    >
{
    fn new() -> Self {
        let mut assets = AssetGet::get_assets_data_with_usd()
            .into_iter()
            .filter_map(|asset| match asset.get_xcm_data() {
                Some((multi_location, decimals, _)) => Some(EqTraderAssetInfo {
                    asset: asset.id,
                    multi_location,
                    decimals,
                    weight: XcmWeight::zero(),
                    amount: 0,
                }),
                None => None,
            })
            .collect::<Vec<_>>();

        assets.sort_by(|a0, a1| a0.multi_location.cmp(&a1.multi_location));
        Self {
            assets,
            phantom_data: PhantomData,
        }
    }

    fn buy_weight(
        &mut self,
        mut weight: XcmWeight,
        mut payment: Assets,
		_context: &XcmContext,
    ) -> Result<Assets, XcmError> {
        for (id, payment_amount) in payment.fungible.iter_mut() {
            match id {
                AssetId::Concrete(multi_location) => {
                    if let Ok(asset_idx) = self
                        .assets
                        .binary_search_by_key(&multi_location, |asset| &asset.multi_location)
                    {
                        let asset = &mut self.assets[asset_idx];

                        let amount = match Self::weight_to_fee(asset, &weight) {
                            Ok(amount) => amount,
                            Err(EqTraderError::Overflow) => return Err(XcmError::Overflow),
                            Err(EqTraderError::NoFee) => continue,
                        };

                        if let Some(unspent) = payment_amount.checked_sub(amount) {
                            *payment_amount = unspent;
                            asset.weight = asset.weight.saturating_add(weight);
                            asset.amount = asset.amount.saturating_add(amount);
                            weight = XcmWeight::zero();
                            break;
                        }
                    } else {
                        return Err(XcmError::AssetNotFound);
                    }
                }
                _ => continue,
            };
        }
        if weight.any_gt(XcmWeight::zero()) {
            Err(XcmError::TooExpensive)
        } else {
            payment.fungible.retain(|_, amount| amount != &0);
            Ok(payment)
        }
    }

    fn refund_weight(&mut self, weight: XcmWeight, _context: &XcmContext) -> Option<MultiAsset> {
        for asset in self.assets.iter_mut() {
            if asset.weight.all_lt(weight) {
                continue;
            }

            let id = AssetId::Concrete(asset.multi_location.clone());
            let amount = match Self::weight_to_fee(asset, &weight) {
                Ok(amount) => amount,
                Err(_) => continue,
            };

            if asset.amount > 0 {
                asset.weight = asset.weight.saturating_sub(weight);
                asset.amount = asset.amount.saturating_sub(amount);
                return Some((id, amount).into());
            }
        }

        None
    }
}

impl<
        AccountId,
        Balance: TryFrom<B>
            + Member
            + sp_runtime::traits::AtLeast32BitUnsigned
            + codec::FullCodec
            + Copy
            + MaybeSerializeDeserialize
            + sp_std::fmt::Debug
            + Default,
        AssetGet: asset::AssetGetter,
        EqBalances: EqCurrency<AccountId, Balance>,
        EqPrices: PriceGetter,
        TreasuryAccount: Get<AccountId>,
        UsdXcmWeightToFee: WeightToFeePolynomial<Balance = eq_utils::XcmBalance>,
        FallbackXcmWeightToFee: Convert<(Asset, XcmWeight), Option<eq_utils::XcmBalance>>,
    > Drop
    for EqTrader<
        AccountId,
        Balance,
        AssetGet,
        EqBalances,
        EqPrices,
        TreasuryAccount,
        UsdXcmWeightToFee,
        FallbackXcmWeightToFee,
    >
{
    fn drop(&mut self) {
        for asset in self.assets.iter() {
            if asset.amount == 0 {
                continue;
            }

            let decimals = asset.decimals;
            let amount = match eq_utils::balance_from_xcm(asset.amount, decimals) {
                Some(amount) => amount,
                _ => {
                    log::error!(
                        "{}:{}. Could not convert XcmBalance({:?}) to local balance for AssetId {:?}",
                        file!(),
                        line!(),
                        asset.amount,
                        asset.asset,
                    );
                    continue;
                }
            };

            let _ = EqBalances::deposit_creating(
                &TreasuryAccount::get(),
                asset.asset,
                amount,
                true,
                Some(DepositReason::XcmPayment),
            );
        }
    }
}
