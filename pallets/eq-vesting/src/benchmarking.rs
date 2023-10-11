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

//! Vesting pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use eq_primitives::asset;
use frame_benchmarking::{account, benchmarks_instance_pallet, whitelisted_caller};
use frame_system::{Pallet as System, RawOrigin};
use sp_arithmetic::FixedI64;
use sp_runtime::traits::One;
use sp_runtime::traits::UniqueSaturatedInto;

use crate::Pallet as Vesting;

const SEED: u32 = 0;

type BalanceOf<T, I> = <<T as crate::Config<I>>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

fn add_vesting_schedule<I: 'static, T: Config<I>>(who: &T::AccountId) -> Result<(), &'static str> {
    let locked = 100u32;
    let per_block = 10u32;
    let starting_block = 1u32;

    System::<T>::set_block_number(0u32.into());

    // Add schedule to avoid `NotVesting` error.
    Vesting::<T, I>::add_vesting_schedule(
        &who,
        locked.into(),
        per_block.into(),
        starting_block.into(),
    )?;
    Ok(())
}

pub struct Pallet<T: Config<I>, I: 'static>(crate::Pallet<T, I>);

pub trait Config<I: 'static = ()>:
    eq_assets::Config
    + eq_balances::Config
    + eq_oracle::Config
    + eq_whitelists::Config
    + crate::Config<I>
{
}

benchmarks_instance_pallet! {
    vest_locked {
        let caller = account("caller", 0, SEED);
        add_vesting_schedule::<I, T>(&caller)?;
        // At block zero, everything is vested.
        System::<T>::set_block_number(BlockNumberFor::<T>::zero());
        assert_eq!(
            Vesting::<T, I>::vesting_balance(&caller),
            Some(100u32.into()),
            "Vesting schedule not added",
        );
    }: vest(RawOrigin::Signed(caller.clone()))
    verify {
        // Nothing happened since everything is still vested.
        assert_eq!(
            Vesting::<T, I>::vesting_balance(&caller),
            Some(100u32.into()),
            "Vesting schedule was removed",
        );
    }

    vest_unlocked {
        let caller = account("caller", 0, SEED);
        T::Currency::make_free_balance_be(&<T as pallet::Config<I>>::PalletId::get().into_account_truncating(), (1_000_000u32).into());
        add_vesting_schedule::<I, T>(&caller)?;
        // At block 20, everything is unvested.
        System::<T>::set_block_number(20u32.into());
        assert_eq!(
            Vesting::<T, I>::vesting_balance(&caller),
            Some(BalanceOf::<T,I>::zero()),
            "Vesting schedule still active",
        );
    }: vest(RawOrigin::Signed(caller.clone()))
    verify {
        // Vesting schedule is removed!
        assert_eq!(
            Vesting::<T, I>::vesting_balance(&caller),
            None,
            "Vesting schedule was not removed",
        );
    }

    vest_other_locked {
        let other: T::AccountId = account("other", 0, SEED);
        let other_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(other.clone());
        T::Currency::make_free_balance_be(&<T as pallet::Config<I>>::PalletId::get().into_account_truncating(), (1_000_000u32).into());
        add_vesting_schedule::<I, T>(&other)?;
        // At block zero, everything is vested.
        System::<T>::set_block_number(BlockNumberFor::<T>::zero());
        assert_eq!(
            Vesting::<T, I>::vesting_balance(&other),
            Some(100u32.into()),
            "Vesting schedule not added",
        );

        let caller: T::AccountId = account("caller", 0, SEED);
    }: vest_other(RawOrigin::Signed(caller.clone()), other_lookup)
    verify {
        // Nothing happened since everything is still vested.
        assert_eq!(
            Vesting::<T, I>::vesting_balance(&other),
            Some(100u32.into()),
            "Vesting schedule was removed",
        );
    }

    vest_other_unlocked {

        let other: T::AccountId = account("other", 0, SEED);
        let other_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(other.clone());
        T::Currency::make_free_balance_be(&<T as pallet::Config<I>>::PalletId::get().into_account_truncating(), (1_000_000u32).into());
        add_vesting_schedule::<I, T>(&other)?;
        // At block 20, everything is unvested.
        System::<T>::set_block_number(20u32.into());
        assert_eq!(
            Vesting::<T, I>::vesting_balance(&other),
            Some(BalanceOf::<T, I>::zero()),
            "Vesting schedule still active",
        );

        let caller: T::AccountId = account("caller", 0, SEED);
    }: vest_other(RawOrigin::Signed(caller.clone()), other_lookup)
    verify {
        // Vesting schedule is removed!
        assert_eq!(
            Vesting::<T,I>::vesting_balance(&other),
            None,
            "Vesting schedule was not removed",
        );
    }

    vested_transfer {
        let caller: T::AccountId = account("caller", 0, SEED);
        let caller_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(caller.clone());
        T::Currency::make_free_balance_be(&caller, (1_000_000_000_000u64).unique_saturated_into());
        let target: T::AccountId = account("target", 0, SEED);
        let target_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(target.clone());

        let transfer_amount = T::MinVestedTransfer::get();

        let vesting_schedule = VestingInfo {
            locked: transfer_amount,
            per_block: 100_000u32.into(),
            starting_block: 0u32.into(),
        };

        let price_setter: T::AccountId = whitelisted_caller();
        eq_whitelists::Pallet::<T>::add_to_whitelist(RawOrigin::Root.into(), price_setter.clone())
            .unwrap();
        <eq_oracle::Pallet::<T> as eq_primitives::PriceSetter<_>>::set_price(price_setter.clone(), asset::GENS, FixedI64::one())
            .unwrap();

        eq_balances::Pallet::<T>::enable_transfers(RawOrigin::Root.into())
            .unwrap();

    }: force_vested_transfer(RawOrigin::Root, caller_lookup, target_lookup, vesting_schedule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{ExtBuilder, Test};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default()
            .existential_deposit(256)
            .build()
            .execute_with(|| {
                assert_ok!(test_benchmark_vest_locked::<Test>());
                assert_ok!(test_benchmark_vest_unlocked::<Test>());
                assert_ok!(test_benchmark_vest_other_locked::<Test>());
                assert_ok!(test_benchmark_vest_other_unlocked::<Test>());
                assert_ok!(test_benchmark_force_vested_transfer::<Test>());
            });
    }
}
