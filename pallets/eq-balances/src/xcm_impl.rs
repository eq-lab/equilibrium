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

use super::*;
use codec::Encode;
use eq_primitives::{
    asset::{EQ, EQD},
    xcm_origins::dot::PARACHAIN_STATEMINT,
};
use eq_xcm::ParaId;
use polkadot_parachain::primitives::Sibling;
use sp_runtime::TransactionOutcome::*;
use xcm::v3::{send_xcm, Junction, Junctions::Here, WildFungibility};

impl<T: Config> Pallet<T> {
    pub fn do_xcm_transfer(
        from: T::AccountId,
        transfer: (Asset, T::Balance),
        fee: (Asset, T::Balance),
        to: XcmDestination,
    ) -> DispatchResult {
        let (asset, amount) = transfer;
        let is_native_asset_transfer = asset == T::AssetGetter::get_main_asset();
        if is_native_asset_transfer {
            Self::ensure_xcm_transfer_limit_not_exceeded(&from, amount)?;
        }
        let (fee_asset, fee_amount) = fee;
        // asset_native_location - asset's multilocation from our pov
        let (asset_native_location, decimals, self_reserved) = Self::xcm_data(&asset)?;
        let XcmDestinationResolved {
            destination,
            asset_location,
            beneficiary,
        } = Self::get_destination(to, asset_native_location)?;
        // fee asset's location from their pov
        let (fee_location, fee_decimals, fee_self_reserved) = if fee_asset == asset {
            (asset_location.clone(), decimals, self_reserved)
        } else {
            let (location, decimals, self_reserved) = Self::xcm_data(&fee_asset)?;
            (
                Self::reanchor(location, &destination).ok_or(Error::<T>::XcmInvalidDestination)?,
                decimals,
                self_reserved,
            )
        };
        let xcm_amount =
            balance_into_xcm(amount.into(), decimals).ok_or(ArithmeticError::Overflow)?;
        let xcm_fee_amount =
            balance_into_xcm(fee_amount.into(), fee_decimals).ok_or(ArithmeticError::Overflow)?;
        let fee_multi_asset = MultiAsset {
            id: Concrete(fee_location.clone()),
            fun: Fungible(xcm_fee_amount),
        };
        // Common parameter for ReserveAssetDeposited and Withdraw.
        // Join to single multi_asset if asset == fee_asset.
        let multi_assets = if asset == fee_asset {
            let xcm_amount_and_fee = xcm_amount
                .checked_add(xcm_fee_amount)
                .ok_or(ArithmeticError::Overflow)?;
            vec![MultiAsset {
                id: Concrete(asset_location.clone()),
                fun: Fungible(xcm_amount_and_fee),
            }]
            .into()
        } else {
            vec![
                MultiAsset {
                    id: Concrete(asset_location.clone()),
                    fun: Fungible(xcm_amount),
                },
                fee_multi_asset.clone(),
            ]
            .into()
        };

        use xcm_executor::traits::Convert as _;
        let their_sovereign = T::LocationToAccountId::convert(destination.clone())
            .map_err(|_| Error::<T>::XcmInvalidDestination)?;

        // wrap in transaction all methods that could cause side effects
        // rollback on any error, but save send_result to show proper error
        let send_result = frame_support::storage::with_transaction(
            || -> TransactionOutcome<Result<SendResult<_>, DispatchError>> {
                // Initialize their_sovereign account as pallet to prevent ED deleting
                let their_sovereign_info = frame_system::Pallet::<T>::account(&their_sovereign);

                if their_sovereign_info.providers == their_sovereign_info.consumers {
                    EqPalletAccountInitializer::<T>::initialize(&their_sovereign);
                }
                // The instruction that move tokens in holding on destination chain to deposit it to user in DepositAsset.
                let (transfer_instruction, local_transfers_result) =
                    match (self_reserved, fee_self_reserved) {
                        // ReserveAssetDeposited with 1 or 2 tokens.
                        // Both should be transferred to their sovereign locally.
                        (true, true) => {
                            let local_transfers_result = Self::currency_transfer(
                                &from,
                                &their_sovereign,
                                asset,
                                amount,
                                ExistenceRequirement::AllowDeath,
                                TransferReason::XcmTransfer,
                                true,
                            )
                            .and(Self::currency_transfer(
                                &from,
                                &their_sovereign,
                                fee_asset,
                                fee_amount,
                                ExistenceRequirement::AllowDeath,
                                TransferReason::XcmPayment,
                                true,
                            ));
                            (ReserveAssetDeposited(multi_assets), local_transfers_result)
                        }
                        // Withdraw with 1 or 2 tokens
                        // Both should be withdrawn from user locally.
                        (false, false) => {
                            let local_transfers_result = Self::withdraw(
                                &from,
                                asset,
                                amount,
                                true,
                                Some(WithdrawReason::XcmTransfer),
                                WithdrawReasons::empty(),
                                ExistenceRequirement::AllowDeath,
                            )
                            .and(Self::withdraw(
                                &from,
                                fee_asset,
                                fee_amount,
                                true,
                                Some(WithdrawReason::XcmPayment),
                                WithdrawReasons::empty(),
                                ExistenceRequirement::AllowDeath,
                            ));
                            (WithdrawAsset(multi_assets), local_transfers_result)
                        }
                        // Withdraw with 2 tokens.
                        // We need to have our fee asset on sovereign account on destination chain.
                        // Transferable should be withdrawn and fee should be transfered to our treasury.
                        (false, true) => {
                            let treasury_acc = T::TreasuryModuleId::get().into_account_truncating();
                            let local_transfers_result = Self::currency_transfer(
                                &from,
                                &treasury_acc,
                                fee_asset,
                                fee_amount,
                                ExistenceRequirement::AllowDeath,
                                TransferReason::XcmPayment,
                                true,
                            )
                            .and(Self::withdraw(
                                &from,
                                asset,
                                amount,
                                true,
                                Some(WithdrawReason::XcmPayment),
                                WithdrawReasons::empty(),
                                ExistenceRequirement::AllowDeath,
                            ));

                            (WithdrawAsset(multi_assets), local_transfers_result)
                        }
                        // Transfering EQ & EQD to Statemint
                        (true, false)
                            if [EQ, EQD].contains(&asset) && destination == PARACHAIN_STATEMINT =>
                        {
                            let treasury_acc = T::TreasuryModuleId::get().into_account_truncating();
                            let local_transfers_result = Self::withdraw(
                                &from,
                                asset,
                                amount,
                                true,
                                Some(WithdrawReason::XcmTransfer),
                                WithdrawReasons::empty(),
                                ExistenceRequirement::AllowDeath,
                            )
                            .and(Self::currency_transfer(
                                &from,
                                &treasury_acc,
                                fee_asset,
                                fee_amount,
                                ExistenceRequirement::AllowDeath,
                                TransferReason::XcmPayment,
                                true,
                            ));
                            (
                                WithdrawAsset(fee_multi_asset.clone().into()),
                                local_transfers_result,
                            )
                        }
                        // In this branch we should send [ReserveAssetDeposited, Withdraw, ClearOrigin, BuyExecution, ...]
                        // but it will fail in barrier on destination
                        (true, false) => return Rollback(Err(Error::<T>::XcmWrongFeeAsset.into())),
                    };

                if let Err(err) = local_transfers_result {
                    return Rollback(Err(err));
                }

                let mut xcm = if [EQ, EQD].contains(&asset) && destination == PARACHAIN_STATEMINT {
                    let mut xcm = Xcm::<()>(vec![
                        transfer_instruction,
                        BuyExecution {
                            fees: fee_multi_asset,
                            weight_limit: WeightLimit::Unlimited,
                        },
                    ]);
                    let equilibrium_souvereign: T::AccountId =
                        Sibling::from(T::ParachainId::get()).into_account_truncating();
                    let encoded = Encode::encode(&equilibrium_souvereign);
                    let (prefix, _) = beneficiary.clone().split_last_interior();
                    let acc = encoded.try_into();
                    if let Err(_) = acc {
                        return Rollback(Err(Error::<T>::XcmInvalidDestination.into()));
                    }
                    let equilibrium_souvereign_multilocation =
                        prefix.pushed_with_interior(Junction::AccountId32 {
                            network: None,
                            id: acc.unwrap(),
                        });
                    if let Err(_) = equilibrium_souvereign_multilocation {
                        return Rollback(Err(Error::<T>::XcmInvalidDestination.into()));
                    }

                    xcm.0.push(ReceiveTeleportedAsset(
                        MultiAsset {
                            id: Concrete(asset_location.clone()),
                            fun: Fungible(xcm_amount),
                        }
                        .into(),
                    ));
                    xcm.0.push(DepositAsset {
                        assets: (AllOfCounted {
                            id: Concrete(fee_location),
                            fun: WildFungibility::Fungible,
                            count: 2,
                        })
                        .into(),
                        beneficiary: equilibrium_souvereign_multilocation.unwrap(),
                    });
                    xcm
                } else {
                    let mut xcm = Xcm::<()>(vec![
                        transfer_instruction,
                        ClearOrigin,
                        BuyExecution {
                            fees: fee_multi_asset,
                            weight_limit: WeightLimit::Unlimited,
                        },
                    ]);

                    if (self_reserved, fee_self_reserved) == (false, true) {
                        // Moonbeam case: pay EQ to withdraw mxUSDC and return remains EQ to our souvereign
                        // May be change later to send [ReserveAssetDeposited, BuyExecution, Withdraw, ClearOrigin]
                        let equilibrium_souvereign: T::AccountId =
                            if destination.clone() == MultiLocation::new(1, Here) {
                                ParaId::from(T::ParachainId::get()).into_account_truncating()
                            } else {
                                Sibling::from(T::ParachainId::get()).into_account_truncating()
                            };

                        // depence on beneficiary type take 32 or 20 bytes from our souvereign account
                        let encoded = Encode::encode(&equilibrium_souvereign);
                        let equilibrium_souvereign_multilocation = match beneficiary
                            .clone()
                            .split_last_interior()
                        {
                            (prefix, Some(Junction::AccountId32 { .. })) => {
                                let acc = encoded.try_into();
                                if let Err(_) = acc {
                                    return Rollback(Err(Error::<T>::XcmInvalidDestination.into()));
                                }
                                prefix.pushed_with_interior(Junction::AccountId32 {
                                    network: None,
                                    id: acc.unwrap(),
                                })
                            }
                            (prefix, Some(Junction::AccountKey20 { .. })) => {
                                let acc = encoded.try_into();
                                if let Err(_) = acc {
                                    return Rollback(Err(Error::<T>::XcmInvalidDestination.into()));
                                }
                                prefix.pushed_with_interior(Junction::AccountKey20 {
                                    network: None,
                                    key: acc.unwrap(),
                                })
                            }
                            _ => return Rollback(Err(Error::<T>::XcmInvalidDestination.into())),
                        };

                        if let Err(_) = equilibrium_souvereign_multilocation {
                            return Rollback(Err(Error::<T>::XcmInvalidDestination.into()));
                        }

                        xcm.0.push(DepositAsset {
                            assets: (AllOfCounted {
                                id: Concrete(fee_location),
                                fun: WildFungibility::Fungible,
                                count: 2,
                            })
                            .into(),
                            beneficiary: equilibrium_souvereign_multilocation.unwrap(),
                        });
                    }

                    xcm
                };

                xcm.0.push(DepositAsset {
                    assets: AllCounted(2).into(),
                    beneficiary: beneficiary.clone(),
                });

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
            frame_support::fail!(Error::<T>::XcmSend);
        } else {
            Self::deposit_event(Event::XcmTransfer(destination, beneficiary));
        }

        Ok(())
    }

    pub fn get_destination(
        dest: XcmDestination,
        asset_native_location: MultiLocation,
    ) -> Result<XcmDestinationResolved, DispatchError> {
        let (destination, asset_location, beneficiary) = match dest {
            XcmDestination::Common(to) => {
                let destination =
                    eq_utils::chain_part(&to).ok_or(Error::<T>::XcmInvalidDestination)?;
                let asset_location = Self::reanchor(asset_native_location, &destination)
                    .ok_or(Error::<T>::XcmInvalidDestination)?;
                let beneficiary = eq_utils::non_chain_part(&to).into();
                (destination, asset_location, beneficiary)
            }
            XcmDestination::Native(to) => {
                let destination = eq_utils::chain_part(&asset_native_location)
                    .ok_or(Error::<T>::XcmInvalidDestination)?;
                let asset_location = eq_utils::non_chain_part(&asset_native_location).into();
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
}
