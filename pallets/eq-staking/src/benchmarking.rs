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

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use eq_primitives::balance_number::EqFixedU128;
use eq_utils::ONE_TOKEN;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use sp_runtime::{traits::Get, FixedI64, Percent, Permill};
use sp_std::vec;

const BALANCE: u128 = 100 * ONE_TOKEN;

pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config:
    eq_assets::Config + eq_balances::Config + pallet_timestamp::Config + eq_rate::Config + crate::Config
{
}

fn add_asset_and_deposit<T: Config>(who: &T::AccountId, amount: u128) {
    let _ = eq_assets::Pallet::<T>::do_add_asset(
        asset::EQ,
        EqFixedU128::zero(),
        FixedI64::zero(),
        Permill::zero(),
        Permill::zero(),
        eq_primitives::asset::AssetXcmData::None,
        Permill::zero(),
        0,
        eq_primitives::asset::AssetType::Physical,
        true,
        Percent::zero(),
        Permill::one(),
        vec![],
    );

    eq_balances::Pallet::<T>::make_free_balance_be(
        who,
        asset::EQ,
        SignedBalance::Positive(
            amount
                .try_into()
                .map_err(|_| "balance convertion error")
                .unwrap(),
        ),
    );
}

benchmarks! {
    stake {
        let caller: T::AccountId = whitelisted_caller();
        let amount = ONE_TOKEN.try_into().map_err(|_| "balance convertion error").unwrap();
        let period = StakePeriod::One;
        add_asset_and_deposit::<T>(&caller, BALANCE);
        let expected: BoundedVec<Stake<_>, T::MaxStakesCount> = vec![Stake{ amount, period, start: 0}]
            .try_into()
            .unwrap();
    }: _(RawOrigin::Signed(caller.clone()), amount, period)
    verify {
        assert_eq!(Stakes::<T>::get(caller), expected);
    }

    reward {
        let who: T::AccountId = whitelisted_caller();
        let amount = ONE_TOKEN.try_into().map_err(|_| "balance convertion error").unwrap();
        add_asset_and_deposit::<T>(&who, BALANCE);
        eq_balances::Pallet::<T>::make_free_balance_be(
            &T::LiquidityAccount::get(),
            asset::EQ,
            SignedBalance::Positive(
                BALANCE
                    .try_into()
                    .map_err(|_| "balance convertion error")
                    .unwrap(),
            ),
        );
    }: _(RawOrigin::Root, who.clone(), amount, 0)
    verify {
        assert_eq!(Rewards::<T>::get(who), Some(Stake{ amount, start: 0, period: T::RewardsLockPeriod::get() }));
    }

    unlock_stake {
        let caller: T::AccountId = whitelisted_caller();
        let amount = ONE_TOKEN.try_into().map_err(|_| "balance convertion error").unwrap();
        let period = StakePeriod::One;
        add_asset_and_deposit::<T>(&caller, BALANCE);
        let _ = crate::Pallet::<T>::stake(RawOrigin::Signed(caller.clone()).into(), amount, period);
        let _ = eq_rate::Pallet::<T>::set_now_millis_offset(
            RawOrigin::Root.into(),
            (period.as_secs() * 1000).try_into().map_err(|_| "").unwrap())
            .unwrap();
    }: unlock(RawOrigin::Signed(caller.clone()), Some(0))
    verify {
        assert_eq!(Stakes::<T>::get(caller).len(), 0);
    }

    unlock_reward {
        let caller: T::AccountId = whitelisted_caller();
        let amount = ONE_TOKEN.try_into().map_err(|_| "balance convertion error").unwrap();
        eq_balances::Pallet::<T>::make_free_balance_be(
            &T::LiquidityAccount::get(),
            asset::EQ,
            SignedBalance::Positive(
                BALANCE
                    .try_into()
                    .map_err(|_| "balance convertion error")
                    .unwrap(),
            ),
        );
        let _ = crate::Pallet::<T>::reward(RawOrigin::Root.into(), caller.clone(), amount, 0 as u64).unwrap();
        let _ = eq_rate::Pallet::<T>::set_now_millis_offset(
            RawOrigin::Root.into(),
            (T::RewardsLockPeriod::get().as_secs() * 1000).try_into().map_err(|_| "").unwrap())
            .unwrap();
    }: unlock(RawOrigin::Signed(caller.clone()), None)
    verify {
        assert_eq!(Rewards::<T>::get(caller), None);
    }

    // impl_benchmark_test_suite!(crate::Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
