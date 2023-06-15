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

use crate::mock::{
    get_basic_balance, get_eqd_balance, get_eth_balance, get_lpt0_balance, get_synth_balance,
    AccountId, Balance, EqdTokenId, EthTokenId, Lpt0TokenId, ModuleBalances, SyntheticTokenId,
    LPT0, SYNT,
};

use super::mock::{
    assert_events, event_exists, expect_event, new_test_ext, ChainBridge, EqBridge, NativeTokenId,
    ProposalLifetime, RuntimeCall, RuntimeEvent, RuntimeOrigin, Test, DEFAULT_FEE, ENDOWED_BALANCE,
    RELAYER_A, RELAYER_B, RELAYER_C, USER,
};
use super::*;
use codec::Encode;
use eq_primitives::SignedBalance::Positive;
use frame_support::dispatch::DispatchError;
use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_core::{blake2_256, H256};

const TEST_THRESHOLD: u32 = 2;

fn make_remark_proposal(hash: H256) -> RuntimeCall {
    RuntimeCall::EqBridge(crate::Call::remark { hash })
}

fn make_transfer_proposal(
    to: AccountId,
    amount: Balance,
    resource_id: chainbridge::ResourceId,
) -> RuntimeCall {
    RuntimeCall::EqBridge(crate::Call::transfer {
        to,
        amount,
        resource_id,
    })
}

#[test]
fn transfer_native() {
    use sp_runtime::traits::AccountIdConversion;

    new_test_ext().execute_with(|| {
        let dest_chain = 0;
        let resource_id = NativeTokenId::get();
        let amount = 100;
        let fee = chainbridge::Fees::<Test>::get(dest_chain);
        let recipient = vec![99];
        let fee_id = chainbridge::FEE_MODULE_ID.into_account_truncating();
        let asset = eq_primitives::asset::EQ;

        assert_eq!(get_basic_balance(fee_id), Positive(0));
        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(
            RawOrigin::Root.into(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_ok!(EqBridge::transfer_native(
            RuntimeOrigin::signed(USER),
            amount.clone(),
            recipient.clone(),
            dest_chain,
            resource_id,
        ));

        assert_eq!(
            get_basic_balance(USER),
            Positive(ENDOWED_BALANCE - amount - fee)
        );
        assert_eq!(get_basic_balance(fee_id), Positive(fee));

        expect_event(chainbridge::Event::FungibleTransfer(
            dest_chain,
            1,
            resource_id,
            amount.into(),
            recipient,
        ));
    })
}

#[test]
fn transfer_native_with_disabled_transfers() {
    use sp_runtime::traits::AccountIdConversion;

    new_test_ext().execute_with(|| {
        let dest_chain = 0;
        let resource_id = NativeTokenId::get();
        let amount = 100;
        let recipient = vec![99];
        let fee_id = chainbridge::FEE_MODULE_ID.into_account_truncating();
        let asset = eq_primitives::asset::EQ;

        assert_eq!(get_basic_balance(fee_id), Positive(0));
        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(
            RawOrigin::Root.into(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_ok!(ChainBridge::toggle_chain(
            RuntimeOrigin::root(),
            dest_chain,
            false
        ));

        assert_eq!(ChainBridge::chain_enabled(dest_chain), false);

        assert_err!(
            EqBridge::transfer_native(
                RuntimeOrigin::signed(USER),
                amount.clone(),
                recipient.clone(),
                dest_chain,
                resource_id,
            ),
            Error::<Test>::DisabledChain
        );
    })
}

#[test]
fn execute_remark() {
    new_test_ext().execute_with(|| {
        let hash: H256 = "ABC".using_encoded(blake2_256).into();
        let proposal = make_remark_proposal(hash.clone());
        let prop_id = 1;
        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"hash");
        let resource = b"EqBridge.remark".to_vec();

        assert_ok!(ChainBridge::set_threshold(
            RuntimeOrigin::root(),
            TEST_THRESHOLD,
        ));
        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_A));
        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_B));
        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            src_id,
            DEFAULT_FEE
        ));
        assert_ok!(ChainBridge::set_resource(
            RuntimeOrigin::root(),
            r_id,
            resource
        ));
        assert_ok!(ChainBridge::set_min_nonce(RuntimeOrigin::root(), src_id, 0));

        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_B),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));

        event_exists(crate::Event::Remark(hash));
    })
}

#[test]
fn execute_remark_bad_origin() {
    new_test_ext().execute_with(|| {
        let hash: H256 = "ABC".using_encoded(blake2_256).into();

        assert_ok!(EqBridge::remark(
            RuntimeOrigin::signed(ChainBridge::account_id()),
            hash
        ));
        // Don't allow any signed origin except from bridge addr
        assert_noop!(
            EqBridge::remark(RuntimeOrigin::signed(RELAYER_A), hash),
            DispatchError::BadOrigin
        );
        // Don't allow root calls
        assert_noop!(
            EqBridge::remark(RuntimeOrigin::root(), hash),
            DispatchError::BadOrigin
        );
    })
}

#[test]
fn transfer() {
    new_test_ext().execute_with(|| {
        // Check inital state
        let bridge_id: AccountId = ChainBridge::account_id();
        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));
        assert_eq!(get_eth_balance(bridge_id), Positive(0));
        assert_eq!(get_eth_balance(RELAYER_A), Positive(0));

        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
        let asset = eq_primitives::asset::ETH;

        assert_ok!(EqBridge::set_resource(RawOrigin::Root.into(), r_id, asset));

        // Transfer and check result
        assert_ok!(EqBridge::transfer(
            RuntimeOrigin::signed(ChainBridge::account_id()),
            RELAYER_A,
            10,
            r_id
        ));
        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));
        assert_eq!(get_eth_balance(bridge_id), Positive(0));
        assert_eq!(get_eth_balance(RELAYER_A), Positive(10));
    })
}

#[test]
fn transfer_basic() {
    new_test_ext().execute_with(|| {
        // Check inital state
        let bridge_id: AccountId = ChainBridge::account_id();
        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));

        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
        let asset = eq_primitives::asset::EQ;

        assert_ok!(EqBridge::set_resource(RawOrigin::Root.into(), r_id, asset));

        // Transfer and check result
        assert_ok!(EqBridge::transfer(
            RuntimeOrigin::signed(ChainBridge::account_id()),
            RELAYER_A,
            10,
            r_id
        ));
        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE - 10));
        assert_eq!(get_basic_balance(RELAYER_A), Positive(ENDOWED_BALANCE + 10));
    })
}

#[test]
fn create_successful_transfer_proposal() {
    new_test_ext().execute_with(|| {
        let prop_id = 1;
        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
        let resource = b"EqBridge.transfer".to_vec();
        let proposal = make_transfer_proposal(RELAYER_A, 10, r_id);
        let asset = eq_primitives::asset::EQ;

        assert_ok!(EqBridge::set_resource(RawOrigin::Root.into(), r_id, asset));
        assert_ok!(ChainBridge::set_threshold(
            RuntimeOrigin::root(),
            TEST_THRESHOLD,
        ));
        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_A));
        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_B));
        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_C));
        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            src_id,
            DEFAULT_FEE
        ));
        assert_ok!(ChainBridge::set_resource(
            RuntimeOrigin::root(),
            r_id,
            resource
        ));
        assert_ok!(ChainBridge::set_min_nonce(RuntimeOrigin::root(), src_id, 0));

        // Create proposal (& vote)
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = chainbridge::ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: chainbridge::ProposalStatus::Initiated,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        // Second relayer votes against
        assert_ok!(ChainBridge::reject_proposal(
            RuntimeOrigin::signed(RELAYER_B),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = chainbridge::ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![RELAYER_B],
            status: chainbridge::ProposalStatus::Initiated,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        // Third relayer votes in favour
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_C),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = chainbridge::ProposalVotes {
            votes_for: vec![RELAYER_A, RELAYER_C],
            votes_against: vec![RELAYER_B],
            status: chainbridge::ProposalStatus::Approved,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        assert_eq!(get_basic_balance(RELAYER_A), Positive(ENDOWED_BALANCE + 10));
        assert_eq!(
            get_basic_balance(ChainBridge::account_id()),
            Positive(ENDOWED_BALANCE - 10)
        );

        assert_events(vec![
            RuntimeEvent::ChainBridge(chainbridge::Event::VoteFor(src_id, prop_id, RELAYER_A)),
            RuntimeEvent::ChainBridge(chainbridge::Event::VoteAgainst(src_id, prop_id, RELAYER_B)),
            RuntimeEvent::ChainBridge(chainbridge::Event::VoteFor(src_id, prop_id, RELAYER_C)),
            RuntimeEvent::ChainBridge(chainbridge::Event::ProposalApproved(src_id, prop_id)),
            RuntimeEvent::Balances(eq_balances::Event::Transfer(
                ChainBridge::account_id(),
                RELAYER_A,
                eq_primitives::asset::EQ,
                10,
                eq_primitives::TransferReason::Common,
            )),
            RuntimeEvent::ChainBridge(chainbridge::Event::ProposalSucceeded(src_id, prop_id)),
        ]);
    })
}

#[test]
fn set_minimum_transfer_amount_successful() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();
        let minimum_amount = 100;
        let asset = eq_primitives::asset::EQ;

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_ok!(EqBridge::set_minimum_transfer_amount(
            RuntimeOrigin::root(),
            dest_chain,
            resource_id,
            minimum_amount
        ));

        assert_events(vec![
            RuntimeEvent::ChainBridge(chainbridge::Event::ChainWhitelisted(dest_chain)),
            RuntimeEvent::EqBridge(crate::Event::WithdrawalsToggled(
                resource_id,
                dest_chain,
                true,
            )),
            RuntimeEvent::EqBridge(crate::Event::MinimumTransferAmountChanged(
                dest_chain,
                resource_id,
                minimum_amount,
            )),
        ]);
    })
}

#[test]
fn set_minimum_transfer_amount_bad_origin() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();
        let minimum_amount = 100;
        let asset = eq_primitives::asset::EQ;

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_err!(
            EqBridge::set_minimum_transfer_amount(
                RuntimeOrigin::signed(USER),
                dest_chain,
                resource_id,
                minimum_amount
            ),
            DispatchError::BadOrigin
        );

        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_A));

        assert_err!(
            EqBridge::set_minimum_transfer_amount(
                RuntimeOrigin::signed(RELAYER_A),
                dest_chain,
                resource_id,
                minimum_amount
            ),
            DispatchError::BadOrigin
        );

        assert_events(vec![
            RuntimeEvent::ChainBridge(chainbridge::Event::ChainWhitelisted(dest_chain)),
            RuntimeEvent::EqBridge(crate::Event::WithdrawalsToggled(
                resource_id,
                dest_chain,
                true,
            )),
            RuntimeEvent::ChainBridge(chainbridge::Event::RelayerAdded(RELAYER_A)),
        ]);
    })
}

#[test]
fn set_minimum_transfer_amount_unsuccessful() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();
        let minimum_amount = 100;
        let asset = eq_primitives::asset::EQ;

        assert_err!(
            EqBridge::set_minimum_transfer_amount(
                RuntimeOrigin::root(),
                dest_chain,
                resource_id,
                minimum_amount
            ),
            Error::<Test>::ChainNotWhitelisted
        );

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_err!(
            EqBridge::set_minimum_transfer_amount(
                RuntimeOrigin::root(),
                dest_chain,
                resource_id,
                minimum_amount
            ),
            Error::<Test>::InvalidResourceId
        );

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(EqBridge::set_minimum_transfer_amount(
            RuntimeOrigin::root(),
            dest_chain,
            resource_id,
            minimum_amount
        ));
    })
}

#[test]
fn transfer_native_without_minimum() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();
        let amount = 10;
        let recipient = vec![99];
        let asset = eq_primitives::asset::EQ;

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_ok!(EqBridge::transfer_native(
            RuntimeOrigin::signed(USER),
            amount.clone(),
            recipient.clone(),
            dest_chain,
            resource_id,
        ));

        expect_event(RuntimeEvent::ChainBridge(
            chainbridge::Event::FungibleTransfer(
                dest_chain,
                1,
                resource_id,
                amount.into(),
                recipient,
            ),
        ));
    })
}

#[test]
fn transfer_native_lower_than_minimum() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();
        let minimum_amount = 100;
        let amount = minimum_amount - 1;
        let recipient = vec![99];
        let asset = eq_primitives::asset::EQ;

        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_ok!(EqBridge::update_minimum_transfer_amount(
            dest_chain,
            resource_id,
            minimum_amount
        ));

        assert_err!(
            EqBridge::transfer_native(
                RuntimeOrigin::signed(USER),
                amount.clone(),
                recipient.clone(),
                dest_chain,
                resource_id,
            ),
            Error::<Test>::TransferAmountLowerMinimum
        );

        expect_event(RuntimeEvent::EqBridge(
            crate::Event::MinimumTransferAmountChanged(dest_chain, resource_id, minimum_amount),
        ));
    })
}

#[test]
fn transfer_native_amount_equal_to_minimum() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();
        let minimum_amount = 100;
        let amount = minimum_amount;
        let recipient = vec![99];
        let asset = eq_primitives::asset::EQ;

        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_ok!(EqBridge::update_minimum_transfer_amount(
            dest_chain,
            resource_id,
            minimum_amount
        ));

        assert_ok!(EqBridge::transfer_native(
            RuntimeOrigin::signed(USER),
            amount.clone(),
            recipient.clone(),
            dest_chain,
            resource_id,
        ));

        expect_event(RuntimeEvent::ChainBridge(
            chainbridge::Event::FungibleTransfer(
                dest_chain,
                1,
                resource_id,
                amount.into(),
                recipient,
            ),
        ));
    })
}

#[test]
fn transfer_native_amount_more_than_minimum() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();
        let minimum_amount = 100;
        let amount = minimum_amount + 1;
        let recipient = vec![99];
        let asset = eq_primitives::asset::EQ;

        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_ok!(EqBridge::update_minimum_transfer_amount(
            dest_chain,
            resource_id,
            minimum_amount
        ));

        assert_ok!(EqBridge::transfer_native(
            RuntimeOrigin::signed(USER),
            amount.clone(),
            recipient.clone(),
            dest_chain,
            resource_id,
        ));

        expect_event(RuntimeEvent::ChainBridge(
            chainbridge::Event::FungibleTransfer(
                dest_chain,
                1,
                resource_id,
                amount.into(),
                recipient,
            ),
        ));
    })
}

#[test]
fn transfer_native_with_disabled_withdrawals() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();
        let amount = 100_u64;
        let recipient = vec![99];
        let asset = eq_primitives::asset::EQ;

        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));

        assert_err!(
            EqBridge::transfer_native(
                RuntimeOrigin::signed(USER),
                amount.clone() as u128,
                recipient.clone(),
                dest_chain,
                resource_id,
            ),
            Error::<Test>::DisabledWithdrawals
        );
    })
}

#[test]
fn transfer_native_with_enabled_withdrawals() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();
        let amount = 100_u64;
        let recipient = vec![99];
        let asset = eq_primitives::asset::EQ;

        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));

        assert_err!(
            EqBridge::transfer_native(
                RuntimeOrigin::signed(USER),
                amount.clone() as u128,
                recipient.clone(),
                dest_chain,
                resource_id,
            ),
            Error::<Test>::DisabledWithdrawals
        );

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_ok!(EqBridge::transfer_native(
            RuntimeOrigin::signed(USER),
            amount.clone() as u128,
            recipient.clone(),
            dest_chain,
            resource_id,
        ));

        expect_event(RuntimeEvent::ChainBridge(
            chainbridge::Event::FungibleTransfer(
                dest_chain,
                1,
                resource_id,
                amount.into(),
                recipient,
            ),
        ));
    })
}

#[test]
fn enable_withdrawals_successful() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_events(vec![RuntimeEvent::EqBridge(
            crate::Event::WithdrawalsToggled(resource_id, dest_chain, true),
        )]);
    })
}

#[test]
fn enabled_withdrawals_storage_update() {
    new_test_ext().execute_with(|| {
        let dest_chain_1 = 2;
        let dest_chain_2 = 3;
        let dest_chain_3 = 6;
        let resource_id = NativeTokenId::get();

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_1
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_1]
        );

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_2
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_1, dest_chain_2]
        );

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_3
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_1, dest_chain_2, dest_chain_3]
        );

        assert_ok!(EqBridge::disable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_2
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_1, dest_chain_3]
        );
    })
}

#[test]
fn enable_withdrawals_unsuccessful() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));

        assert_events(vec![RuntimeEvent::EqBridge(
            crate::Event::WithdrawalsToggled(resource_id, dest_chain, true),
        )]);

        assert_err!(
            EqBridge::enable_withdrawals(RawOrigin::Root.into(), resource_id, dest_chain),
            Error::<Test>::WithdrawalsAllowabilityEqual
        );
    })
}

#[test]
fn disable_withdrawals_successful() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_events(vec![RuntimeEvent::EqBridge(
            crate::Event::WithdrawalsToggled(resource_id, dest_chain, true),
        )]);

        assert_ok!(EqBridge::disable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));
        assert_events(vec![RuntimeEvent::EqBridge(
            crate::Event::WithdrawalsToggled(resource_id, dest_chain, false),
        )]);
    })
}

#[test]
fn disable_withdrawals_unsuccessful() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();

        assert_err!(
            EqBridge::disable_withdrawals(RawOrigin::Root.into(), resource_id, dest_chain),
            Error::<Test>::WithdrawalsAllowabilityEqual
        );
    })
}

#[test]
fn toggle_withdrawals_bad_origin() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = NativeTokenId::get();

        assert_err!(
            EqBridge::enable_withdrawals(RuntimeOrigin::signed(USER), resource_id, dest_chain),
            DispatchError::BadOrigin
        );

        assert_err!(
            EqBridge::disable_withdrawals(RuntimeOrigin::signed(USER), resource_id, dest_chain),
            DispatchError::BadOrigin
        );
    })
}

#[test]
fn enabled_withdrawals_sorting() {
    new_test_ext().execute_with(|| {
        let dest_chain_2 = 2;
        let dest_chain_3 = 3;
        let dest_chain_6 = 6;
        let dest_chain_7 = 7;
        let resource_id = NativeTokenId::get();

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_3
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_3]
        );

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_2
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_2, dest_chain_3]
        );

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_7
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_2, dest_chain_3, dest_chain_7]
        );

        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_6
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_2, dest_chain_3, dest_chain_6, dest_chain_7]
        );

        assert_ok!(EqBridge::disable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_3
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_2, dest_chain_6, dest_chain_7]
        );

        assert_ok!(EqBridge::disable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_2
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_6, dest_chain_7]
        );

        assert_ok!(EqBridge::disable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_6
        ));
        assert_eq!(
            EqBridge::enabled_withdrawals(resource_id),
            vec![dest_chain_7]
        );

        assert_ok!(EqBridge::disable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain_7
        ));
        assert_eq!(EqBridge::enabled_withdrawals(resource_id).len(), 0);
    })
}

#[test]
fn transfer_native_physical_asset() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = EthTokenId::get();
        let amount = 100_u128;
        let recipient = vec![99];
        let bridge_id: u64 = ChainBridge::account_id();
        let asset = eq_primitives::asset::ETH;

        assert_ok!(ModuleBalances::deposit(
            RuntimeOrigin::root(),
            asset,
            USER,
            amount
        ));

        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));
        assert_eq!(get_eth_balance(USER), Positive(amount));
        assert_eq!(get_eth_balance(bridge_id), Positive(0));

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));

        assert_ok!(EqBridge::transfer_native(
            RuntimeOrigin::signed(USER),
            amount.clone(),
            recipient.clone(),
            dest_chain,
            resource_id,
        ));

        expect_event(chainbridge::Event::FungibleTransfer(
            dest_chain,
            1,
            resource_id,
            amount.into(),
            recipient,
        ));

        assert_eq!(get_eth_balance(bridge_id), Positive(0));
        assert_eq!(get_eth_balance(USER), Positive(0));
    })
}

#[test]
fn transfer_native_synthetic_asset() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = SyntheticTokenId::get();
        let amount = 100_u128;
        let recipient = vec![99];
        let bridge_id: u64 = ChainBridge::account_id();
        let asset = SYNT;

        assert_ok!(ModuleBalances::deposit(
            RuntimeOrigin::root(),
            asset,
            USER,
            amount
        ));

        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));
        assert_eq!(get_synth_balance(USER), Positive(amount));
        assert_eq!(get_synth_balance(bridge_id), Positive(0));

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));

        assert_ok!(EqBridge::transfer_native(
            RuntimeOrigin::signed(USER),
            amount.clone(),
            recipient.clone(),
            dest_chain,
            resource_id,
        ));

        expect_event(chainbridge::Event::FungibleTransfer(
            dest_chain,
            1,
            resource_id,
            amount.into(),
            recipient,
        ));

        assert_eq!(get_synth_balance(bridge_id), Positive(amount));
        assert_eq!(get_synth_balance(USER), Positive(0));
    })
}

#[test]
fn transfer_native_eqd_asset() {
    new_test_ext().execute_with(|| {
        let dest_chain = 6;
        let resource_id = EqdTokenId::get();
        let amount = 100_u128;
        let recipient = vec![99];
        let bridge_id: u64 = ChainBridge::account_id();
        let asset = eq_primitives::asset::EQD;

        assert_ok!(ModuleBalances::deposit(
            RuntimeOrigin::root(),
            asset,
            USER,
            amount
        ));

        assert_eq!(get_basic_balance(USER), Positive(ENDOWED_BALANCE));
        assert_eq!(get_eqd_balance(USER), Positive(amount));
        assert_eq!(get_eqd_balance(bridge_id), Positive(0));

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));

        assert_ok!(EqBridge::transfer_native(
            RuntimeOrigin::signed(USER),
            amount.clone(),
            recipient.clone(),
            dest_chain,
            resource_id,
        ));

        expect_event(chainbridge::Event::FungibleTransfer(
            dest_chain,
            1,
            resource_id,
            amount.into(),
            recipient,
        ));

        assert_eq!(get_eqd_balance(bridge_id), Positive(0));
        assert_eq!(get_eqd_balance(USER), Positive(0));
    })
}

#[test]
fn transfer_native_invalid_asset_type() {
    new_test_ext().execute_with(|| {
        // Check inital state
        let dest_chain = 6;
        let resource_id = Lpt0TokenId::get();
        let amount = 100_u128;
        let recipient = vec![99];
        let bridge_id: u64 = ChainBridge::account_id();
        let asset = LPT0;

        assert_ok!(ModuleBalances::deposit(
            RuntimeOrigin::root(),
            asset,
            USER,
            amount
        ));

        assert_eq!(get_lpt0_balance(bridge_id), Positive(0));
        assert_eq!(get_lpt0_balance(USER), Positive(amount));
        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(
            RuntimeOrigin::root(),
            resource_id,
            asset
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_chain.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(EqBridge::enable_withdrawals(
            RawOrigin::Root.into(),
            resource_id,
            dest_chain
        ));

        assert_err!(
            EqBridge::transfer_native(
                RuntimeOrigin::signed(USER),
                amount.clone(),
                recipient.clone(),
                dest_chain,
                resource_id,
            ),
            Error::<Test>::InvalidAssetType
        );

        assert_eq!(get_lpt0_balance(bridge_id), Positive(0));
        assert_eq!(get_lpt0_balance(USER), Positive(amount));
    })
}

#[test]
fn transfer_physical_asset() {
    new_test_ext().execute_with(|| {
        // Check inital state
        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
        let asset = eq_primitives::asset::ETH;
        let amount = 100_u128;
        let bridge_id: u64 = ChainBridge::account_id();

        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));
        assert_eq!(get_eth_balance(bridge_id), Positive(0));
        assert_eq!(get_eth_balance(USER), Positive(0));

        assert_ok!(EqBridge::set_resource(RawOrigin::Root.into(), r_id, asset));

        // Transfer and check result
        assert_ok!(EqBridge::transfer(
            RuntimeOrigin::signed(ChainBridge::account_id()),
            USER,
            amount,
            r_id
        ));

        assert_events(vec![RuntimeEvent::EqBridge(
            crate::Event::FromBridgeTransfer(USER, asset, amount),
        )]);

        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));
        assert_eq!(get_eth_balance(bridge_id), Positive(0));
        assert_eq!(get_eth_balance(USER), Positive(amount));
    })
}

#[test]
fn transfer_synthetic_asset() {
    new_test_ext().execute_with(|| {
        // Check inital state
        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
        let asset = SYNT;
        let amount = 100_u128;
        let bridge_id: u64 = ChainBridge::account_id();

        assert_ok!(ModuleBalances::deposit(
            RuntimeOrigin::root(),
            asset,
            bridge_id,
            amount
        ));
        assert_eq!(get_synth_balance(bridge_id), Positive(amount));

        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));
        assert_eq!(get_synth_balance(USER), Positive(0));

        assert_ok!(EqBridge::set_resource(RawOrigin::Root.into(), r_id, asset));

        // Transfer and check result
        assert_ok!(EqBridge::transfer(
            RuntimeOrigin::signed(ChainBridge::account_id()),
            USER,
            amount,
            r_id
        ));

        assert_events(vec![RuntimeEvent::Balances(eq_balances::Event::Transfer(
            bridge_id,
            USER,
            asset,
            amount,
            eq_primitives::TransferReason::Common,
        ))]);

        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));
        assert_eq!(get_synth_balance(bridge_id), Positive(0));
        assert_eq!(get_synth_balance(USER), Positive(amount));
    })
}

#[test]
fn transfer_eqd_asset() {
    new_test_ext().execute_with(|| {
        // Check inital state
        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
        let asset = eq_primitives::asset::EQD;
        let amount = 100_u128;
        let bridge_id: u64 = ChainBridge::account_id();

        assert_eq!(get_eqd_balance(bridge_id), Positive(0));
        assert_eq!(get_eqd_balance(USER), Positive(0));
        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(RawOrigin::Root.into(), r_id, asset));

        // Transfer and check result
        assert_ok!(EqBridge::transfer(
            RuntimeOrigin::signed(ChainBridge::account_id()),
            USER,
            amount,
            r_id
        ));

        assert_events(vec![RuntimeEvent::EqBridge(
            crate::Event::FromBridgeTransfer(USER, asset, amount),
        )]);

        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));
        assert_eq!(get_eqd_balance(bridge_id), Positive(0));
        assert_eq!(get_eqd_balance(USER), Positive(amount));
    })
}

#[test]
fn transfer_invalid_asset_type() {
    new_test_ext().execute_with(|| {
        // Check inital state
        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
        let asset = LPT0;
        let amount = 100_u128;
        let bridge_id: u64 = ChainBridge::account_id();

        assert_ok!(ModuleBalances::deposit(
            RuntimeOrigin::root(),
            asset,
            bridge_id,
            amount
        ));
        assert_eq!(get_lpt0_balance(bridge_id), Positive(amount));
        assert_eq!(get_lpt0_balance(USER), Positive(0));
        assert_eq!(get_basic_balance(bridge_id), Positive(ENDOWED_BALANCE));

        assert_ok!(EqBridge::set_resource(RawOrigin::Root.into(), r_id, asset));

        // Transfer and check error
        assert_err!(
            EqBridge::transfer(
                RuntimeOrigin::signed(ChainBridge::account_id()),
                USER,
                amount,
                r_id
            ),
            Error::<Test>::InvalidAssetType
        );

        assert_eq!(get_lpt0_balance(bridge_id), Positive(amount));
        assert_eq!(get_lpt0_balance(USER), Positive(0));
    })
}
