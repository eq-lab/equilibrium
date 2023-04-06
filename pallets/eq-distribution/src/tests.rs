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

#![cfg(test)]

use crate::mock::*;
use crate::Error;
use crate::ExistenceRequirement;
use eq_primitives::asset;
use frame_support::assert_ok;
use frame_support::{assert_err, dispatch::DispatchError};
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::ModuleError;

#[test]
fn transfer_success() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        assert_ok!(ModuleDistribution::transfer(
            frame_system::RawOrigin::Root.into(),
            asset::GENS,
            ACC_ID,
            AMOUNT,
        ));

        let transfer_result = TRANSFER.with(|v| v.borrow().clone());
        assert_eq!(
            transfer_result.unwrap().0,
            (
                asset::GENS,
                DistributionModuleId::get().into_account_truncating(),
                ACC_ID,
                AMOUNT
            )
        );
        assert_eq!(
            transfer_result.unwrap().1 == ExistenceRequirement::AllowDeath,
            true
        );
    });
}

#[test]
fn transfer_should_be_from_root() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        assert_err!(
            ModuleDistribution::transfer(Origin::signed(1), asset::GENS, ACC_ID, 100,),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn transfer_not_allowed() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = false);
        assert_err!(
            ModuleDistribution::transfer(
                frame_system::RawOrigin::Root.into(),
                asset::GENS,
                ACC_ID,
                AMOUNT,
            ),
            DispatchError::Module(ModuleError {
                index: 0,
                error: *b"zero",
                message: Option::None
            })
        );
    });
}

#[test]
fn transfer_non_native_assets() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        for asset in <AssetGetterMock as asset::AssetGetter>::get_assets() {
            assert_ok!(ModuleDistribution::transfer(
                frame_system::RawOrigin::Root.into(),
                asset,
                ACC_ID,
                AMOUNT,
            ));

            let transfer_result = TRANSFER.with(|v| v.borrow().clone());
            assert_eq!(
                transfer_result.unwrap().0,
                (
                    asset,
                    DistributionModuleId::get().into_account_truncating(),
                    ACC_ID,
                    100
                )
            );
            assert_eq!(
                transfer_result.unwrap().1 == ExistenceRequirement::AllowDeath,
                true
            );
        }
    });
}

#[test]
fn transfer_non_existing_assets() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        for asset in [asset::EQD, asset::BNB] {
            assert_err!(
                ModuleDistribution::transfer(
                    frame_system::RawOrigin::Root.into(),
                    asset,
                    ACC_ID,
                    100,
                ),
                DispatchError::Module(ModuleError {
                    index: 0,
                    error: *b"one1",
                    message: Option::None
                })
            );
        }
    });
}

#[test]
fn vested_transfer_success() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        VESTING_EXISTS.with(|v| *v.borrow_mut() = false);
        assert_ok!(ModuleDistribution::vested_transfer(
            frame_system::RawOrigin::Root.into(),
            ACC_ID,
            (AMOUNT, 10, 3)
        ));
        let transfer_result = TRANSFER.with(|v| v.borrow().clone());
        assert_eq!(
            transfer_result.unwrap().0,
            (
                asset::GENS,
                DistributionModuleId::get().into_account_truncating(),
                VestingModuleId::get().into_account_truncating(),
                AMOUNT
            )
        );
        assert_eq!(
            transfer_result.unwrap().1 == ExistenceRequirement::AllowDeath,
            true
        );
        let added_vesting = ADDED_VESTING.with(|v| v.borrow().clone());
        assert_eq!(added_vesting, Option::Some((1, (AMOUNT, 10, 3))));
    });
}

#[test]
fn vested_transfer_should_be_from_root() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        VESTING_EXISTS.with(|v| *v.borrow_mut() = false);
        assert_err!(
            ModuleDistribution::vested_transfer(Origin::signed(1), ACC_ID, (100, 10, 3)),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn vested_transfer_exists() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        VESTING_EXISTS.with(|v| *v.borrow_mut() = true);
        assert_err!(
            ModuleDistribution::vested_transfer(
                frame_system::RawOrigin::Root.into(),
                ACC_ID,
                (AMOUNT, 10, 3)
            ),
            Error::<Test, _>::ExistingVestingSchedule
        );
    });
}

#[test]
fn vested_transfer_per_block_zero() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        VESTING_EXISTS.with(|v| *v.borrow_mut() = false);
        assert_err!(
            ModuleDistribution::vested_transfer(
                frame_system::RawOrigin::Root.into(),
                ACC_ID,
                (AMOUNT, 0, 3)
            ),
            Error::<Test, _>::AmountLow
        );
    });
}

#[test]
fn vested_transfer_not_allowed() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = false);
        VESTING_EXISTS.with(|v| *v.borrow_mut() = false);
        assert_err!(
            ModuleDistribution::vested_transfer(
                frame_system::RawOrigin::Root.into(),
                ACC_ID,
                (AMOUNT, 10, 3)
            ),
            DispatchError::Module(ModuleError {
                index: 0,
                error: *b"zero",
                message: Option::None
            })
        );
    });
}

#[test]
fn transfer_should_be_from_manager_or_root() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        VESTING_EXISTS.with(|v| *v.borrow_mut() = false);

        let acc_id = 1;

        assert_err!(
            ModuleDistribution::transfer(
                frame_system::RawOrigin::Signed(acc_id).into(),
                asset::GENS,
                ACC_ID,
                AMOUNT
            ),
            DispatchError::BadOrigin,
        );
        assert_ok!(ModuleDistribution::transfer(
            pallet_collective::RawOrigin::Member(acc_id).into(),
            asset::GENS,
            ACC_ID,
            AMOUNT
        ));

        let transfer_result = TRANSFER.with(|v| v.borrow().clone());

        assert_eq!(
            transfer_result.unwrap().0,
            (
                asset::GENS,
                DistributionModuleId::get().into_account_truncating(),
                ACC_ID,
                AMOUNT
            )
        );
        assert_eq!(
            transfer_result.unwrap().1 == ExistenceRequirement::AllowDeath,
            true
        );

        assert_ok!(ModuleDistribution::transfer(
            frame_system::RawOrigin::Root.into(),
            asset::GENS,
            ACC_ID,
            AMOUNT
        ));

        let transfer_result = TRANSFER.with(|v| v.borrow().clone());

        assert_eq!(
            transfer_result.unwrap().0,
            (
                asset::GENS,
                DistributionModuleId::get().into_account_truncating(),
                ACC_ID,
                AMOUNT
            )
        );
        assert_eq!(
            transfer_result.unwrap().1 == ExistenceRequirement::AllowDeath,
            true
        );
    })
}

#[test]
fn vested_transfer_should_be_from_manager_or_root() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        VESTING_EXISTS.with(|v| *v.borrow_mut() = false);

        let acc_id = 1;

        assert_err!(
            ModuleDistribution::vested_transfer(
                frame_system::RawOrigin::Signed(acc_id).into(),
                ACC_ID,
                (AMOUNT, 10, 3)
            ),
            DispatchError::BadOrigin,
        );
        assert_ok!(ModuleDistribution::vested_transfer(
            pallet_collective::RawOrigin::Member(acc_id).into(),
            ACC_ID,
            (AMOUNT, 10, 3)
        ));
        let transfer_result = TRANSFER.with(|v| v.borrow().clone());
        assert_eq!(
            transfer_result.unwrap().0,
            (
                asset::GENS,
                DistributionModuleId::get().into_account_truncating(),
                VestingModuleId::get().into_account_truncating(),
                AMOUNT
            )
        );
        assert_eq!(
            transfer_result.unwrap().1 == ExistenceRequirement::AllowDeath,
            true
        );
        let added_vesting = ADDED_VESTING.with(|v| v.borrow().clone());
        assert_eq!(added_vesting, Option::Some((ACC_ID, (AMOUNT, 10, 3))));

        assert_ok!(ModuleDistribution::vested_transfer(
            frame_system::RawOrigin::Root.into(),
            ACC_ID,
            (AMOUNT, 10, 3)
        ));
        let transfer_result = TRANSFER.with(|v| v.borrow().clone());
        assert_eq!(
            transfer_result.unwrap().0,
            (
                asset::GENS,
                DistributionModuleId::get().into_account_truncating(),
                VestingModuleId::get().into_account_truncating(),
                AMOUNT
            )
        );
        assert_eq!(
            transfer_result.unwrap().1 == ExistenceRequirement::AllowDeath,
            true
        );
        let added_vesting = ADDED_VESTING.with(|v| v.borrow().clone());
        assert_eq!(added_vesting, Option::Some((1, (100, 10, 3))));
    });
}

#[test]
fn no_manager_does_not_fail_only_root() {
    new_test_ext().execute_with(|| {
        CAN_TRANSFER.with(|v| *v.borrow_mut() = true);
        VESTING_EXISTS.with(|v| *v.borrow_mut() = false);

        let acc_id = 1;

        assert_err!(
            ModuleDistribution::transfer(
                frame_system::RawOrigin::Signed(acc_id).into(),
                asset::GENS,
                ACC_ID,
                AMOUNT
            ),
            DispatchError::BadOrigin
        );

        assert_err!(
            ModuleDistribution::vested_transfer(
                frame_system::RawOrigin::Signed(acc_id).into(),
                ACC_ID,
                (AMOUNT, 10, 3)
            ),
            DispatchError::BadOrigin
        );

        assert_ok!(ModuleDistribution::vested_transfer(
            frame_system::RawOrigin::Root.into(),
            ACC_ID,
            (AMOUNT, 10, 3)
        ));
    })
}
