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

use super::config::RelaySystemConfig as Config;
use codec::{Decode, Encode, FullCodec};
use cumulus_primitives_core::ParaId;
use eq_primitives::balance::Balance;
use frame_support::weights::{Weight, WeightToFee};
use frame_support::RuntimeDebug;
use sp_runtime::traits::{Get, StaticLookup};
use sp_std::boxed::Box;
use sp_std::marker::PhantomData;
use sp_std::{vec, vec::Vec};
use xcm::latest::prelude::*;

use eq_utils::XcmBalance;
pub use pallet_staking::RewardDestination;

#[derive(Encode, Decode, RuntimeDebug)]
pub enum BalancesCall<T: Config> {
    #[codec(index = 3)]
    TransferKeepAlive(
        <T::Lookup as StaticLookup>::Source,
        #[codec(compact)] Balance,
    ),
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum UtilityCall<RelayChainCall> {
    #[codec(index = 1)]
    AsDerivative(u16, Box<RelayChainCall>),
    #[codec(index = 2)]
    BatchAll(Vec<RelayChainCall>),
}

/// Weights from polkadot runtime multiplied by 2
/// https://github.com/paritytech/polkadot/blob/94078b44fb6c9767bf60ffcaaa3be40681be5a76/runtime/polkadot/src/weights/pallet_utility.rs
pub struct UtilityWeights<T>(PhantomData<T>);
impl<T: frame_system::Config> UtilityWeights<T> {
    pub fn as_derivative() -> Weight {
        Weight::from_ref_time(5_533_000 as u64).saturating_mul(2)
    }

    pub fn batch_all(c: u32) -> Weight {
        Weight::from_ref_time(26_834_000 as u64)
            .saturating_add(Weight::from_ref_time(3_527_000 as u64).saturating_mul(c as u64))
            .saturating_mul(2)
    }
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum StakingCall<T: Config> {
    #[codec(index = 0)]
    Bond(
        <<T as Config>::Lookup as StaticLookup>::Source,
        #[codec(compact)] Balance, /* Need to convert our balance to RelayChain balance, because of different decimals*/
        RewardDestination<T::AccountId>,
    ),
    #[codec(index = 1)]
    BondExtra(#[codec(compact)] Balance), /* Need to convert our balance to RelayChain balance, because of different decimals*/
    #[codec(index = 2)]
    Unbond(#[codec(compact)] Balance), /* Need to convert our balance to RelayChain balance, because of different decimals */
    #[codec(index = 3)]
    WithdrawUnbonded(u32),
}

/// Wights from polkadot runtime multiplied by 2
/// https://github.com/paritytech/polkadot/blob/v0.9.37/runtime/polkadot/src/weights/pallet_staking.rs
pub struct StakingWeights<T>(PhantomData<T>);
impl<T: frame_system::Config> StakingWeights<T> {
    pub fn bond() -> Weight {
        Weight::from_ref_time(46_793_000 as u64)
            .saturating_add(T::DbWeight::get().reads(4 as u64))
            .saturating_add(T::DbWeight::get().writes(4 as u64))
            .saturating_mul(2)
    }

    pub fn bond_extra() -> Weight {
        Weight::from_ref_time(85_921_000 as u64)
            .saturating_add(T::DbWeight::get().reads(8 as u64))
            .saturating_add(T::DbWeight::get().writes(7 as u64))
            .saturating_mul(2)
    }

    pub fn unbond() -> Weight {
        Weight::from_ref_time(92_682_000 as u64)
            // было в сумме 1_190_775_000
            // у нас было 2_009_062_000
            // сейчас в dispatch_info 5_058_339_947
            // было в dispatch_info 998_576_000
            .saturating_add(T::DbWeight::get().reads(12 as u64))
            .saturating_add(T::DbWeight::get().writes(8 as u64))
            .saturating_mul(2)
    }

    pub fn nominate(n: u32) -> Weight {
        Weight::from_ref_time(64_480_887 as u64)
            .saturating_add(Weight::from_ref_time(2_387_381 as u64).saturating_mul(n as u64))
            .saturating_add(T::DbWeight::get().reads(12 as u64))
            .saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(n as u64)))
            .saturating_add(T::DbWeight::get().writes(6 as u64))
            .saturating_mul(2)
    }

    pub fn withdraw_unbonded_kill(s: u32) -> Weight {
        Weight::from_ref_time(83_564_707 as u64)
            .saturating_add(Weight::from_ref_time(910_195).saturating_mul(s.into()))
            .saturating_add(T::DbWeight::get().reads(13 as u64))
            .saturating_add(T::DbWeight::get().writes(11 as u64))
            .saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(s.into())))
            .saturating_mul(2)
    }
}

/// The encoded index correspondes to Polkadot's Runtime module configuration.
/// https://github.com/paritytech/polkadot/blob/84a3962e76151ac5ed3afa4ef1e0af829531ab42/runtime/polkadot/src/lib.rs#L1040
#[cfg(not(feature = "kusama"))]
#[derive(Encode, Decode, RuntimeDebug)]
pub enum RelayChainCall<T: Config> {
    #[codec(index = 5)]
    Balances(BalancesCall<T>),
    #[codec(index = 7)]
    Staking(StakingCall<T>),
    #[codec(index = 26)]
    Utility(UtilityCall<Self>),
}

/// The encoded index correspondes to Kusama's Runtime module configuration.
/// https://github.com/paritytech/polkadot/blob/444e96ae34bcec8362f0f947a07bd912b32ca48f/runtime/kusama/src/lib.rs#L1379
#[cfg(feature = "kusama")]
#[derive(Encode, Decode, RuntimeDebug)]
pub enum RelayChainCall<T: Config> {
    #[codec(index = 4)]
    Balances(BalancesCall<T>),
    #[codec(index = 6)]
    Staking(StakingCall<T>),
    #[codec(index = 24)]
    Utility(UtilityCall<Self>),
}

pub struct RelayChainCallBuilder<T: Config, ParachainId: Get<ParaId>>(
    PhantomData<(T, ParachainId)>,
);

impl<T: Config, ParachainId: Get<ParaId>> CallBuilder<T::AccountId, Balance>
    for RelayChainCallBuilder<T, ParachainId>
where
    T::AccountId: FullCodec,
    RelayChainCall<T>: FullCodec,
{
    type RelayChainCall = RelayChainCall<T>;

    fn balances_transfer_keep_alive(to: T::AccountId, amount: Balance) -> Self::RelayChainCall {
        RelayChainCall::Balances(BalancesCall::TransferKeepAlive(
            T::Lookup::unlookup(to),
            amount,
        ))
    }

    fn utility_batch_call(calls: Vec<Self::RelayChainCall>) -> Self::RelayChainCall {
        RelayChainCall::Utility(UtilityCall::BatchAll(calls))
    }

    fn utility_as_derivative_call(call: Self::RelayChainCall, index: u16) -> Self::RelayChainCall {
        RelayChainCall::Utility(UtilityCall::AsDerivative(index, Box::new(call)))
    }

    fn staking_bond(
        controller: T::AccountId,
        amount: Balance,
        payee: RewardDestination<T::AccountId>,
    ) -> Self::RelayChainCall {
        RelayChainCall::Staking(StakingCall::Bond(
            T::Lookup::unlookup(controller),
            amount,
            payee,
        ))
    }

    fn staking_bond_extra(amount: Balance) -> Self::RelayChainCall {
        RelayChainCall::Staking(StakingCall::BondExtra(amount))
    }

    fn staking_unbond(amount: Balance) -> Self::RelayChainCall {
        RelayChainCall::Staking(StakingCall::Unbond(amount))
    }

    fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelayChainCall {
        RelayChainCall::Staking(StakingCall::WithdrawUnbonded(num_slashing_spans))
    }

    fn finalize_call_into_xcm_message(
        call: Self::RelayChainCall,
        transact_weight: Weight,
    ) -> Xcm<()> {
        let xcm_weight = Weight::from_ref_time(crate::fees::polkadot::BaseXcmWeight::get())
            .saturating_mul(5)
            .saturating_add(transact_weight);
        let xcm_fee: XcmBalance =
            crate::fees::polkadot::WeightToFee::weight_to_fee(&xcm_weight).into();

        let asset = MultiAsset {
            id: Concrete(MultiLocation::here()),
            fun: Fungibility::Fungible(xcm_fee),
        };
        Xcm(vec![
            WithdrawAsset(asset.clone().into()),
            BuyExecution {
                fees: asset,
                weight_limit: Unlimited,
            },
            Transact {
                origin_type: OriginKind::SovereignAccount,
                require_weight_at_most: transact_weight.ref_time(),
                call: call.encode().into(),
            },
            RefundSurplus,
            DepositAsset {
                assets: All.into(),
                max_assets: 1,
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(Parachain(ParachainId::get().into())),
                },
            },
        ])
    }
}

pub trait CallBuilder<AccountId: FullCodec, Balance: FullCodec> {
    type RelayChainCall: FullCodec;

    /// Transfer Staking currency to another account, disallowing "death".
    ///  params:
    /// - to: The destination for the transfer
    /// - amount: The amount of staking currency to be transferred.
    fn balances_transfer_keep_alive(to: AccountId, amount: Balance) -> Self::RelayChainCall;

    /// Prepare utility::batch call on relay-chain
    /// Param:
    /// - calls: List of calls to be executed
    fn utility_batch_call(calls: Vec<Self::RelayChainCall>) -> Self::RelayChainCall;

    /// Execute a call, replacing the `Origin` with a sub-account.
    ///  params:
    /// - call: The call to be executed. Can be nested with `utility_batch_call`
    /// - index: The index of sub-account to be used as the new origin.
    fn utility_as_derivative_call(call: Self::RelayChainCall, index: u16) -> Self::RelayChainCall;

    /// Prepare pallet_staking::bond call on relay-chain
    /// params:
    /// - controller:
    /// - amount: amount to stake
    /// - payee: destination of rewards
    fn staking_bond(
        controller: AccountId,
        amount: Balance,
        payee: RewardDestination<AccountId>,
    ) -> Self::RelayChainCall;

    /// Prepare pallet_staking::bond_extra call on relay-chain.
    ///  params:
    /// - amount: The amount of staking currency to bond.
    fn staking_bond_extra(amount: Balance) -> Self::RelayChainCall;

    /// Prepare pallet_staking::unbond call on relay-chain.
    ///  params:
    /// - amount: The amount of staking currency to unbond.
    fn staking_unbond(amount: Balance) -> Self::RelayChainCall;

    /// Withdraw unbonded staking on the relay-chain.
    ///  params:
    /// - num_slashing_spans: The number of slashing spans to withdraw from.
    fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelayChainCall;

    /// Wrap the final calls into the Xcm format.
    ///  params:
    /// - call: The call to be executed
    /// - transact_weight: the weight limit used for XCM.
    fn finalize_call_into_xcm_message(
        call: Self::RelayChainCall,
        transact_weight: Weight,
    ) -> Xcm<()>;
}

/// Implementation for testing purposes only
impl<AccountId: FullCodec, Balance: FullCodec> CallBuilder<AccountId, Balance> for () {
    type RelayChainCall = ();

    fn balances_transfer_keep_alive(_to: AccountId, _amount: Balance) -> Self::RelayChainCall {
        ()
    }

    fn utility_batch_call(_calls: Vec<Self::RelayChainCall>) -> Self::RelayChainCall {
        ()
    }

    fn utility_as_derivative_call(
        _call: Self::RelayChainCall,
        _index: u16,
    ) -> Self::RelayChainCall {
        ()
    }

    fn staking_bond(
        _controller: AccountId,
        _amount: Balance,
        _payee: RewardDestination<AccountId>,
    ) -> Self::RelayChainCall {
        ()
    }

    fn staking_bond_extra(_: Balance) -> Self::RelayChainCall {
        ()
    }

    fn staking_unbond(_: Balance) -> Self::RelayChainCall {
        ()
    }

    fn staking_withdraw_unbonded(_: u32) -> Self::RelayChainCall {
        ()
    }

    fn finalize_call_into_xcm_message(
        _call: Self::RelayChainCall,
        _transact_weight: Weight,
    ) -> Xcm<()> {
        Xcm(vec![])
    }
}
