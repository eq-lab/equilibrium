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

use core::marker::PhantomData;
use eq_primitives::asset::AssetXcmGetter;
use frame_support::traits::{Contains, ProcessMessageError};
use xcm::v3::{
    Instruction, Instruction::*, Junction::*, Junctions::*, MultiLocation, Weight, WeightLimit::*,
};
use xcm_executor::traits::ShouldExecute;

pub struct AllowReserveAssetDepositedFrom<EqAssets, AllowedOrigins>(
    PhantomData<(EqAssets, AllowedOrigins)>,
);
impl<EqAssets: AssetXcmGetter, AllowedOrigins: Contains<MultiLocation>> ShouldExecute
    for AllowReserveAssetDepositedFrom<EqAssets, AllowedOrigins>
{
    fn should_execute<RuntimeCall>(
        origin: &MultiLocation,
        instructions: &mut [Instruction<RuntimeCall>],
        max_weight: Weight,
        _weight_credit: &mut Weight,
    ) -> Result<(), ProcessMessageError> {
        if AllowedOrigins::contains(origin) {
            match instructions {
                // We expect withdraw asset only for native asset with MultiLocation { 1, Here }
                [WithdrawAsset(multi_assets), ClearOrigin, BuyExecution {
                    ref mut weight_limit,
                    ..
                }, DepositAsset { .. }] => {
                    let self_reserved_assets = EqAssets::get_self_reserved_xcm_assets();
                    if multi_assets
                        .inner()
                        .iter()
                        .all(|a| self_reserved_assets.contains(&a.id))
                    {
                        *weight_limit = Limited(max_weight);
                        Ok(())
                    } else {
                        Err(ProcessMessageError::Unsupported)
                    }
                }
                // End we expect reserve asset deposited for other assets
                [ReserveAssetDeposited(multi_assets), ClearOrigin, BuyExecution {
                    ref mut weight_limit,
                    ..
                }, DepositAsset { .. }] => {
                    let other_reserved_assets = EqAssets::get_other_reserved_xcm_assets();
                    if multi_assets
                        .inner()
                        .iter()
                        .all(|a| other_reserved_assets.contains(&a.id))
                    {
                        *weight_limit = Limited(max_weight);
                        Ok(())
                    } else {
                        Err(ProcessMessageError::Unsupported)
                    }
                }
                _ => Err(ProcessMessageError::Unsupported),
            }
        } else {
            Err(ProcessMessageError::Unsupported)
        }
    }
}

pub struct AllowReserveTransferAssetsFromAccountId;

impl ShouldExecute for AllowReserveTransferAssetsFromAccountId {
    fn should_execute<RuntimeCall>(
        origin: &MultiLocation,
        instructions: &mut [Instruction<RuntimeCall>],
        _max_weight: Weight,
        _weight_credit: &mut Weight,
    ) -> Result<(), ProcessMessageError> {
        if eq_utils::chain_part(origin).is_none()
            && matches!(eq_utils::non_chain_part(origin), X1(AccountId32 { .. }))
        {
            match instructions {
                [TransferReserveAsset { .. }] => Ok(()),
                _ => Err(ProcessMessageError::Unsupported),
            }
        } else {
            Err(ProcessMessageError::Unsupported)
        }
    }
}
