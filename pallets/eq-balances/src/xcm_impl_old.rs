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

use xcm::v3::{send_xcm, MultiAssets};

use super::*;

impl<T: Config> Pallet<T> {
    pub fn reanchor(mut this: MultiLocation, target: &MultiLocation) -> Option<MultiLocation> {
        let inverted_target = T::UniversalLocation::get().invert_target(&target).ok()?;
        this.prepend_with(inverted_target).ok()?;
        this.simplify(target.interior());

        Some(this)
    }

    pub fn xcm_message(
        asset: MultiAsset,
        fees: MultiAsset,
        beneficiary: MultiLocation,
        reserved: bool,
    ) -> Xcm<()> {
        let asset_and_fee: MultiAssets = vec![asset, fees.clone()].into();
        Xcm(vec![
            if reserved {
                ReserveAssetDeposited(asset_and_fee.clone())
            } else {
                WithdrawAsset(asset_and_fee.clone())
            },
            ClearOrigin,
            BuyExecution {
                fees,
                weight_limit: WeightLimit::Unlimited,
            },
            DepositAsset {
                assets: AllCounted(asset_and_fee.len() as u32).into(),
                beneficiary,
            },
        ])
    }

    pub fn get_xcm_transfer_params(
        kind: XcmDestination,
        multi_location: MultiLocation,
    ) -> Result<XcmDestinationResolved, DispatchError> {
        let (destination, asset_location, beneficiary) = match kind {
            XcmDestination::Common(to) => {
                let destination =
                    eq_utils::chain_part(&to).ok_or(Error::<T>::XcmInvalidDestination)?;
                let asset_location = Self::reanchor(multi_location, &destination)
                    .ok_or(Error::<T>::XcmInvalidDestination)?;
                let beneficiary = eq_utils::non_chain_part(&to).into();
                (destination, asset_location, beneficiary)
            }
            XcmDestination::Native(to) => {
                let destination = eq_utils::chain_part(&multi_location)
                    .ok_or(Error::<T>::XcmInvalidDestination)?;
                let asset_location = eq_utils::non_chain_part(&multi_location).into();
                let beneficiary = to.multi_location().into();
                (destination, asset_location, beneficiary)
            }
        };

        Ok(XcmDestinationResolved {
            destination,
            asset_location,
            beneficiary,
        })
    }

    /// All actions for transfer without xcm enabled/disabled checks.
    /// Should be used only after checks or from sudo.
    pub fn do_xcm_transfer_old(
        from: T::AccountId,
        asset: Asset,
        amount: T::Balance,
        kind: XcmDestination,
        deal_with_fee: XcmTransferDealWithFee,
    ) -> DispatchResult {
        let is_native_asset_transfer = asset == T::AssetGetter::get_main_asset();
        if is_native_asset_transfer {
            Self::ensure_xcm_transfer_limit_not_exceeded(&from, amount)?;
        }

        let (multi_location, decimals, self_reserved) = Self::xcm_data(&asset)?;

        let XcmDestinationResolved {
            destination,
            asset_location,
            beneficiary,
        } = Self::get_xcm_transfer_params(kind, multi_location)?;

        let xcm_amount =
            balance_into_xcm(amount.into(), decimals).ok_or(ArithmeticError::Overflow)?;

        let (to_transfer, to_withdraw, xcm) = match destination {
            // hotfix: transfer MATIC, MXETH, MXUSDC, MXWBTC to MOONBEAM
            PARACHAIN_MOONBEAM if asset != GLMR && !self_reserved => {
                let multi_asset = MultiAsset {
                    id: Concrete(asset_location),
                    fun: Fungible(xcm_amount),
                };
                let multi_assets: xcm::v3::MultiAssets =
                    vec![multi_asset.clone(), multi_asset.clone()].into();
                let temp_xcm = Xcm(vec![
                    WithdrawAsset(multi_assets.clone()),
                    ClearOrigin,
                    BuyExecution {
                        fees: multi_asset.clone(),
                        weight_limit: WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: AllCounted(multi_assets.len() as u32).into(),
                        beneficiary: beneficiary.clone(),
                    },
                ]);

                let (xcm_fee_asset, xcm_fee_amount) =
                    T::XcmToFee::convert((asset.clone(), destination.clone(), &temp_xcm))
                        .ok_or(Error::<T>::XcmInvalidDestination)?;
                let (xcm_fee_multilocation, xcm_fee_decimals, xcm_fee_self_reserved) =
                    Self::xcm_data(&xcm_fee_asset)?;
                ensure!(xcm_fee_self_reserved, Error::<T>::XcmWrongFeeAsset);
                let xcm_fee_asset_multilocation =
                    Self::reanchor(xcm_fee_multilocation, &destination)
                        .ok_or(Error::<T>::XcmInvalidDestination)?;
                let xcm_fee_asset_multiasset = MultiAsset {
                    id: Concrete(xcm_fee_asset_multilocation),
                    fun: Fungible(xcm_fee_amount),
                };
                let multi_assets: MultiAssets =
                    vec![xcm_fee_asset_multiasset.clone(), multi_asset].into();
                let xcm = Xcm(vec![
                    WithdrawAsset(multi_assets.clone()),
                    ClearOrigin,
                    BuyExecution {
                        fees: xcm_fee_asset_multiasset,
                        weight_limit: WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: AllCounted(multi_assets.len() as u32).into(),
                        beneficiary: beneficiary.clone(),
                    },
                ]);
                use xcm_executor::traits::Convert as _;
                let their_sovereign = T::LocationToAccountId::convert(destination.clone())
                    .map_err(|_| Error::<T>::XcmInvalidDestination)?;

                // Initialize their_sovereign account as pallet to prevent ED deleting
                let destination_info = frame_system::Pallet::<T>::account(&their_sovereign);

                if destination_info.providers == destination_info.consumers {
                    EqPalletAccountInitializer::<T>::initialize(&their_sovereign);
                }

                let fee_local = balance_from_xcm(xcm_fee_amount, xcm_fee_decimals)
                    .ok_or(ArithmeticError::Overflow)?;
                let to_transfer = vec![
                    (
                        T::TreasuryModuleId::get().into_account_truncating(),
                        xcm_fee_asset,
                        fee_local,
                        TransferReason::XcmPayment,
                    ),
                    (their_sovereign, asset, amount, TransferReason::XcmTransfer),
                ];
                (to_transfer, vec![], xcm)
            }
            _ => {
                let xcm = Self::xcm_message(
                    MultiAsset {
                        id: Concrete(asset_location.clone()),
                        fun: Fungible(xcm_amount),
                    },
                    MultiAsset {
                        id: Concrete(asset_location.clone()),
                        fun: Fungible(0),
                    },
                    beneficiary.clone(),
                    self_reserved,
                );
                let (xcm_fee_asset, xcm_fee_amount) =
                    T::XcmToFee::convert((asset.clone(), destination.clone(), &xcm))
                        .ok_or(Error::<T>::XcmInvalidDestination)?;
                let (xcm_fee_multi_location, xcm_fee_decimals, fee_self_reserved) = if xcm_fee_asset
                    == asset
                {
                    (asset_location.clone(), decimals, self_reserved)
                } else {
                    let (multi_location, decimals, self_reserved) = Self::xcm_data(&xcm_fee_asset)?;
                    (
                        Self::reanchor(multi_location, &destination)
                            .ok_or(Error::<T>::XcmInvalidDestination)?,
                        decimals,
                        self_reserved,
                    )
                };

                let (to_transfer, to_withdraw, xcm) = match deal_with_fee {
                    XcmTransferDealWithFee::SovereignAccWillPay => Self::sovereign_acc_will_pay(
                        destination.clone(),
                        beneficiary.clone(),
                        (asset, xcm_fee_asset),
                        (asset_location, xcm_fee_multi_location),
                        (decimals, xcm_fee_decimals),
                        (self_reserved, fee_self_reserved),
                        amount,
                        xcm_amount,
                        xcm_fee_amount,
                    )?,
                    XcmTransferDealWithFee::AccOnTargetChainWillPay => {
                        frame_support::fail!(Error::<T>::MethodUnimplemented)
                    }
                    XcmTransferDealWithFee::ThisAccWillPay => Self::this_acc_will_pay(
                        destination.clone(),
                        beneficiary.clone(),
                        (asset, xcm_fee_asset),
                        (asset_location, xcm_fee_multi_location),
                        (decimals, xcm_fee_decimals),
                        (self_reserved, fee_self_reserved),
                        amount,
                        xcm_amount,
                        xcm_fee_amount,
                    )?,
                };
                (to_transfer, to_withdraw, xcm)
            }
        };

        // wrap in transaction all methods that could cause side effects
        // rollback on any error, but save send_result to show proper error
        let send_result = frame_support::storage::with_transaction(
            || -> TransactionOutcome<Result<SendResult<_>, DispatchError>> {
                use TransactionOutcome::*;

                for (to, asset, amount, reason) in to_transfer {
                    let res = Self::currency_transfer(
                        &from,
                        &to,
                        asset,
                        amount,
                        ExistenceRequirement::AllowDeath,
                        reason,
                        true,
                    );
                    if let Err(err) = res {
                        return Rollback(Err(err));
                    }
                }

                for (asset, amount, reason) in to_withdraw {
                    let res = Self::withdraw(
                        &from,
                        asset,
                        amount,
                        true,
                        Some(reason),
                        WithdrawReasons::empty(),
                        ExistenceRequirement::AllowDeath,
                    );
                    if let Err(err) = res {
                        return Rollback(Err(err));
                    }
                }

                if is_native_asset_transfer {
                    Self::update_xcm_native_transfers(&from, amount);
                }

                log::trace!(target: "eq_balances", "Sending XcmMessage dest: {:?}, xcm: {:?}", destination, xcm);
                match send_xcm::<T::XcmRouter>(destination.clone(), xcm) {
                    Ok(send_result) => Commit(Ok(SendResult::Ok(send_result))),
                    Err(err) => Rollback(Ok(SendResult::Err(err))),
                }
            },
        )?;

        if let Err(send_error) = send_result {
            log::error!("XcmRouter::SendError {:?}", send_error);
            Self::deposit_event(Event::XcmMessageSendError(send_error));
            frame_support::fail!(Error::<T>::XcmSend)
        } else {
            Self::deposit_event(Event::XcmTransfer(destination, beneficiary));
        }

        Ok(())
    }

    pub fn sovereign_acc_will_pay(
        destination: MultiLocation,
        beneficiary: MultiLocation,
        (tx_asset, fee_asset): (Asset, Asset),
        (tx_multi_location, fee_multi_location): (MultiLocation, MultiLocation),
        (tx_decimals, fee_decimals): (u8, u8),
        (self_reserved, fee_self_reserved): (bool, bool),
        mut amount: T::Balance,
        xcm_amount: XcmBalance,
        xcm_fee_amount: XcmBalance,
    ) -> Result<
        (
            Vec<(T::AccountId, Asset, T::Balance, TransferReason)>, // transfer actions
            Vec<(Asset, T::Balance, WithdrawReason)>,               // withdraw actions
            Xcm<()>,                                                // actual XCM to send
        ),
        DispatchError,
    > {
        // hotfix to avoid unreserved EQ issuance on moonbeam
        if destination == eq_primitives::xcm_origins::dot::PARACHAIN_MOONBEAM
            || tx_asset != fee_asset
        {
            ensure!(!fee_self_reserved, Error::<T>::XcmWrongFeeAsset);
        }

        let xcm_fee_in_tx_asset = if fee_asset == tx_asset {
            xcm_fee_amount
        } else {
            if fee_decimals < tx_decimals {
                let xcm_fee_amount =
                    balance_swap_decimals(xcm_fee_amount, fee_decimals, tx_decimals)
                        .ok_or(ArithmeticError::Overflow)?;

                multiply_by_rational(
                    xcm_fee_amount,
                    T::PriceGetter::get_price::<EqFixedU128>(&fee_asset)?.into_inner(),
                    T::PriceGetter::get_price::<EqFixedU128>(&tx_asset)?.into_inner(),
                )
                .ok_or(ArithmeticError::Overflow)?
            } else {
                let xcm_fee_amount = multiply_by_rational(
                    xcm_fee_amount,
                    T::PriceGetter::get_price::<EqFixedU128>(&fee_asset)?.into_inner(),
                    T::PriceGetter::get_price::<EqFixedU128>(&tx_asset)?.into_inner(),
                )
                .ok_or(ArithmeticError::Overflow)?;

                balance_swap_decimals(xcm_fee_amount, fee_decimals, tx_decimals)
                    .ok_or(ArithmeticError::Overflow)?
            }
        };

        if xcm_fee_in_tx_asset > xcm_amount {
            frame_support::fail!(Error::<T>::XcmNotEnoughToPayFee);
        }

        let xcm = Self::xcm_message(
            MultiAsset {
                id: Concrete(tx_multi_location),
                fun: Fungible(xcm_amount - xcm_fee_in_tx_asset),
            },
            MultiAsset {
                id: Concrete(fee_multi_location),
                fun: Fungible(xcm_fee_amount),
            },
            beneficiary,
            self_reserved,
        );

        let mut to_transfer = Vec::with_capacity(2);
        let mut to_withdraw = Vec::with_capacity(1);

        let destination = if self_reserved {
            use xcm_executor::traits::Convert as _;

            let destination = T::LocationToAccountId::convert(destination)
                .map_err(|_| Error::<T>::XcmInvalidDestination)?;

            // Initialize destination account as pallet to prevent treasury buyout
            let destination_info = frame_system::Pallet::<T>::account(&destination);
            if destination_info.providers == destination_info.consumers {
                EqPalletAccountInitializer::<T>::initialize(&destination);
            }

            Some(destination)
        } else {
            None
        };

        if fee_asset != tx_asset {
            let fee_local = balance_from_xcm(xcm_fee_in_tx_asset, tx_decimals)
                .ok_or(ArithmeticError::Overflow)?;
            amount -= fee_local;

            to_transfer.push((
                // Store fee in treasury only if tx_asset is not self_reserved
                if let Some(ref destination) = destination {
                    destination.clone()
                } else {
                    T::TreasuryModuleId::get().into_account_truncating()
                },
                tx_asset,
                fee_local,
                TransferReason::XcmPayment,
            ));
        }

        if let Some(destination) = destination {
            to_transfer.push((destination, tx_asset, amount, TransferReason::XcmTransfer));
        } else {
            to_withdraw.push((tx_asset, amount, WithdrawReason::XcmTransfer));
        }

        Ok((to_transfer, to_withdraw, xcm))
    }

    pub fn this_acc_will_pay(
        destination: MultiLocation,
        beneficiary: MultiLocation,
        (tx_asset, fee_asset): (Asset, Asset),
        (tx_multi_location, fee_multi_location): (MultiLocation, MultiLocation),
        (_, fee_decimals): (u8, u8),
        (tx_self_reserved, fee_self_reserved): (bool, bool),
        amount: T::Balance,
        xcm_amount: XcmBalance,
        xcm_fee_amount: XcmBalance,
    ) -> Result<
        (
            Vec<(T::AccountId, Asset, T::Balance, TransferReason)>, // transfer actions
            Vec<(Asset, T::Balance, WithdrawReason)>,               // withdraw actions
            Xcm<()>,                                                // actual XCM to send
        ),
        DispatchError,
    > {
        eq_ensure!(
            tx_self_reserved == fee_self_reserved,
            Error::<T>::XcmWrongFeeAsset,
            ""
        );

        let xcm = Self::xcm_message(
            MultiAsset {
                id: Concrete(tx_multi_location),
                fun: Fungible(xcm_amount),
            },
            MultiAsset {
                id: Concrete(fee_multi_location),
                fun: Fungible(xcm_fee_amount),
            },
            beneficiary,
            tx_self_reserved,
        );

        let fee_local =
            balance_from_xcm(xcm_fee_amount, fee_decimals).ok_or(ArithmeticError::Overflow)?;

        let (to_transfer, to_withdraw) = if tx_self_reserved {
            use xcm_executor::traits::Convert as _;

            let destination = T::LocationToAccountId::convert(destination)
                .map_err(|_| Error::<T>::XcmInvalidDestination)?;

            // Initialize destination account as pallet to prevent treasury buyout
            let destination_info = frame_system::Pallet::<T>::account(&destination);
            if destination_info.providers == destination_info.consumers {
                EqPalletAccountInitializer::<T>::initialize(&destination);
            }

            (
                vec![
                    (
                        destination.clone(),
                        tx_asset,
                        amount,
                        TransferReason::XcmTransfer,
                    ),
                    (
                        destination,
                        fee_asset,
                        fee_local,
                        TransferReason::XcmPayment,
                    ),
                ],
                vec![],
            )
        } else {
            (
                vec![],
                vec![
                    (tx_asset, amount, WithdrawReason::XcmTransfer),
                    (fee_asset, fee_local, WithdrawReason::XcmPayment),
                ],
            )
        };

        Ok((to_transfer, to_withdraw, xcm))
    }
}

fn multiply_by_rational(
    a: impl Into<u128>,
    b: impl Into<u128>,
    c: impl Into<u128>,
) -> Option<u128> {
    // return a * b / c
    sp_runtime::helpers_128bit::multiply_by_rational_with_rounding(
        a.into(),
        b.into(),
        c.into(),
        sp_arithmetic::per_things::Rounding::NearestPrefDown,
    )
}
