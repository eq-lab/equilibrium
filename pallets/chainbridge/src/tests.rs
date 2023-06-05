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

use super::*;

use super::mock::{
    assert_events, new_test_ext, ChainBridge, RuntimeCall, RuntimeEvent, RuntimeOrigin, System,
    Test, TestChainId, ENDOWED_BALANCE, RELAYER_A, RELAYER_B, RELAYER_C, TEST_THRESHOLD,
};
use crate::mock::{new_test_ext_initialized, BasicCurrency, DEFAULT_FEE, TEST_PROPOSAL_LIFETIME};
use crate::mock::{new_test_ext_params, AccountId};
use frame_support::{assert_err, assert_noop, assert_ok};
use sp_runtime::DispatchError;

#[test]
fn derive_ids() {
    let chain = 1;
    let id = [
        0x21, 0x60, 0x5f, 0x71, 0x84, 0x5f, 0x37, 0x2a, 0x9e, 0xd8, 0x42, 0x53, 0xd2, 0xd0, 0x24,
        0xb7, 0xb1, 0x09, 0x99, 0xf4,
    ];
    let r_id = derive_resource_id(chain, &id);
    let expected = [
        0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x21, 0x60, 0x5f, 0x71, 0x84, 0x5f,
        0x37, 0x2a, 0x9e, 0xd8, 0x42, 0x53, 0xd2, 0xd0, 0x24, 0xb7, 0xb1, 0x09, 0x99, 0xf4, chain,
    ];
    assert_eq!(r_id, expected);
}

#[test]
fn complete_proposal_approved() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let mut prop = ProposalVotes {
            votes_for: vec![1, 2],
            votes_against: vec![3],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get(),
        };

        prop.try_to_complete(2, 3);
        assert_eq!(prop.status, ProposalStatus::Approved);
    });
}

#[test]
fn complete_proposal_rejected() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let mut prop = ProposalVotes {
            votes_for: vec![1],
            votes_against: vec![2, 3],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get(),
        };

        prop.try_to_complete(2, 3);
        assert_eq!(prop.status, ProposalStatus::Rejected);
    });
}

#[test]
fn complete_proposal_bad_threshold() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let mut prop = ProposalVotes {
            votes_for: vec![1, 2],
            votes_against: vec![],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get(),
        };
        prop.try_to_complete(3, 2);
        assert_eq!(prop.status, ProposalStatus::Initiated);

        let mut prop = ProposalVotes {
            votes_for: vec![],
            votes_against: vec![1, 2],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get(),
        };

        prop.try_to_complete(3, 2);
        assert_eq!(prop.status, ProposalStatus::Initiated);
    });
}

#[test]
fn setup_resources() {
    new_test_ext().execute_with(|| {
        let id: ResourceId = [1; 32];
        let method = "Pallet.do_something".as_bytes().to_vec();
        let method2 = "Pallet.do_somethingElse".as_bytes().to_vec();

        assert_ok!(ChainBridge::set_resource(
            RuntimeOrigin::root(),
            id,
            method.clone()
        ));
        assert_eq!(ChainBridge::resources(id), Some(method));

        assert_ok!(ChainBridge::set_resource(
            RuntimeOrigin::root(),
            id,
            method2.clone()
        ));
        assert_eq!(ChainBridge::resources(id), Some(method2));

        assert_ok!(ChainBridge::remove_resource(RuntimeOrigin::root(), id));
        assert_eq!(ChainBridge::resources(id), None);
    })
}

#[test]
fn whitelist_chain() {
    new_test_ext().execute_with(|| {
        assert!(!ChainBridge::chain_whitelisted(0));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            0,
            DEFAULT_FEE
        ));
        assert_noop!(
            ChainBridge::whitelist_chain(RuntimeOrigin::root(), TestChainId::get(), DEFAULT_FEE),
            Error::<Test>::InvalidChainId
        );

        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::FeeChanged(0, DEFAULT_FEE)),
            RuntimeEvent::ChainBridge(crate::Event::ChainWhitelisted(0)),
        ]);
    })
}

#[test]
fn set_get_threshold() {
    new_test_ext().execute_with(|| {
        assert_eq!(<RelayerThreshold<Test>>::get(), 1);

        assert_ok!(ChainBridge::set_threshold(
            RuntimeOrigin::root(),
            TEST_THRESHOLD
        ));
        assert_eq!(<RelayerThreshold<Test>>::get(), TEST_THRESHOLD);

        assert_ok!(ChainBridge::set_threshold(RuntimeOrigin::root(), 5));
        assert_eq!(<RelayerThreshold<Test>>::get(), 5);

        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::RelayerThresholdChanged(TEST_THRESHOLD)),
            RuntimeEvent::ChainBridge(crate::Event::RelayerThresholdChanged(5)),
        ]);
    })
}

#[test]
fn set_proposal_lifetime_success() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            <ProposalLifetime<Test>>::get(),
            DEFAULT_PROPOSAL_LIFETIME as u64
        );

        assert_ok!(ChainBridge::set_proposal_lifetime(
            RuntimeOrigin::root(),
            TEST_PROPOSAL_LIFETIME
        ));
        assert_eq!(<ProposalLifetime<Test>>::get(), TEST_PROPOSAL_LIFETIME);

        assert_ok!(ChainBridge::set_proposal_lifetime(
            RuntimeOrigin::root(),
            50
        ));
        assert_eq!(<ProposalLifetime<Test>>::get(), 50);

        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::ProposalLifetimeChanged(
                TEST_PROPOSAL_LIFETIME,
            )),
            RuntimeEvent::ChainBridge(crate::Event::ProposalLifetimeChanged(50)),
        ]);
    })
}

#[test]
fn set_proposal_lifetime_fail() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            <ProposalLifetime<Test>>::get(),
            DEFAULT_PROPOSAL_LIFETIME as u64
        );
        let account_id_1: AccountId = 0;
        assert_err!(
            ChainBridge::set_proposal_lifetime(
                RuntimeOrigin::signed(account_id_1),
                TEST_PROPOSAL_LIFETIME
            ),
            DispatchError::BadOrigin
        );
        assert_eq!(
            <ProposalLifetime<Test>>::get(),
            DEFAULT_PROPOSAL_LIFETIME as u64
        );

        assert_err!(
            ChainBridge::set_proposal_lifetime(RuntimeOrigin::root(), 0),
            Error::<Test>::InvalidProposalLifetime
        );
        assert_eq!(
            <ProposalLifetime<Test>>::get(),
            DEFAULT_PROPOSAL_LIFETIME as u64
        );
    })
}

#[test]
fn asset_transfer_success() {
    new_test_ext().execute_with(|| {
        let dest_id = 2;
        let to = vec![2];
        let resource_id = [1; 32];
        let metadata = vec![];
        let amount = 100;
        let token_id = vec![1, 2, 3, 4];

        assert_ok!(ChainBridge::set_threshold(
            RuntimeOrigin::root(),
            TEST_THRESHOLD,
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_id.clone(),
            DEFAULT_FEE
        ));
        assert_ok!(ChainBridge::transfer_fungible(
            dest_id.clone(),
            resource_id.clone(),
            to.clone(),
            amount.into()
        ));
        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::FeeChanged(dest_id, DEFAULT_FEE)),
            RuntimeEvent::ChainBridge(crate::Event::ChainWhitelisted(dest_id.clone())),
            RuntimeEvent::ChainBridge(crate::Event::FungibleTransfer(
                dest_id.clone(),
                1,
                resource_id.clone(),
                amount.into(),
                to.clone(),
            )),
        ]);

        assert_ok!(ChainBridge::transfer_nonfungible(
            dest_id.clone(),
            resource_id.clone(),
            token_id.clone(),
            to.clone(),
            metadata.clone()
        ));
        assert_events(vec![RuntimeEvent::ChainBridge(
            crate::Event::NonFungibleTransfer(
                dest_id.clone(),
                2,
                resource_id.clone(),
                token_id,
                to.clone(),
                metadata.clone(),
            ),
        )]);

        assert_ok!(ChainBridge::transfer_generic(
            dest_id.clone(),
            resource_id.clone(),
            metadata.clone()
        ));
        assert_events(vec![RuntimeEvent::ChainBridge(
            crate::Event::GenericTransfer(dest_id.clone(), 3, resource_id, metadata),
        )]);
    })
}

#[test]
fn asset_transfer_invalid_chain() {
    new_test_ext().execute_with(|| {
        let chain_id = 2;
        let bad_dest_id = 3;
        let resource_id = [4; 32];

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            chain_id.clone(),
            DEFAULT_FEE
        ));
        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::FeeChanged(chain_id, DEFAULT_FEE)),
            RuntimeEvent::ChainBridge(crate::Event::ChainWhitelisted(chain_id.clone())),
        ]);

        assert_noop!(
            ChainBridge::transfer_fungible(bad_dest_id, resource_id.clone(), vec![], U256::zero()),
            Error::<Test>::ChainNotWhitelisted
        );

        assert_noop!(
            ChainBridge::transfer_nonfungible(
                bad_dest_id,
                resource_id.clone(),
                vec![],
                vec![],
                vec![]
            ),
            Error::<Test>::ChainNotWhitelisted
        );

        assert_noop!(
            ChainBridge::transfer_generic(bad_dest_id, resource_id.clone(), vec![]),
            Error::<Test>::ChainNotWhitelisted
        );
    })
}

#[test]
fn asset_transfer_disabled_chain() {
    new_test_ext().execute_with(|| {
        let dest_id = 2;
        let to = vec![2];
        let resource_id = [1; 32];
        let amount = 100;

        assert_ok!(ChainBridge::set_threshold(
            RuntimeOrigin::root(),
            TEST_THRESHOLD,
        ));

        assert_ok!(ChainBridge::whitelist_chain(
            RuntimeOrigin::root(),
            dest_id.clone(),
            DEFAULT_FEE
        ));

        assert_ok!(ChainBridge::toggle_chain(
            RuntimeOrigin::root(),
            dest_id.clone(),
            false
        ));

        assert_err!(
            ChainBridge::transfer_fungible(dest_id, resource_id, to, amount.into()),
            Error::<Test>::DisabledChain
        );
    })
}

#[test]
fn add_remove_relayer() {
    new_test_ext().execute_with(|| {
        assert_ok!(ChainBridge::set_threshold(
            RuntimeOrigin::root(),
            TEST_THRESHOLD,
        ));
        assert_eq!(ChainBridge::relayer_count(), 0);

        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_A));
        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_B));
        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_C));
        assert_eq!(ChainBridge::relayer_count(), 3);

        // Already exists
        assert_noop!(
            ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_A),
            Error::<Test>::RelayerAlreadyExists
        );

        // Confirm removal
        assert_ok!(ChainBridge::remove_relayer(
            RuntimeOrigin::root(),
            RELAYER_B
        ));
        assert_eq!(ChainBridge::relayer_count(), 2);
        assert_noop!(
            ChainBridge::remove_relayer(RuntimeOrigin::root(), RELAYER_B),
            Error::<Test>::RelayerInvalid
        );
        assert_eq!(ChainBridge::relayer_count(), 2);

        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::RelayerAdded(RELAYER_A)),
            RuntimeEvent::ChainBridge(crate::Event::RelayerAdded(RELAYER_B)),
            RuntimeEvent::ChainBridge(crate::Event::RelayerAdded(RELAYER_C)),
            RuntimeEvent::ChainBridge(crate::Event::RelayerRemoved(RELAYER_B)),
        ]);
    })
}

fn make_proposal(remark: Vec<u8>) -> mock::RuntimeCall {
    RuntimeCall::System(system::Call::remark { remark })
}

#[test]
fn create_successful_proposal() {
    let src_id = 1;
    let r_id = derive_resource_id(src_id, b"remark");

    new_test_ext_initialized(
        src_id,
        r_id,
        b"System.remark".to_vec(),
        DEFAULT_PROPOSAL_LIFETIME as u64,
    )
    .execute_with(|| {
        let prop_id = 1;
        let proposal = make_proposal(vec![10]);

        // Create proposal (& vote)
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get() + 1,
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
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![RELAYER_B],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get() + 1,
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
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A, RELAYER_C],
            votes_against: vec![RELAYER_B],
            status: ProposalStatus::Approved,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);

        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::VoteFor(src_id, prop_id, RELAYER_A)),
            RuntimeEvent::ChainBridge(crate::Event::VoteAgainst(src_id, prop_id, RELAYER_B)),
            RuntimeEvent::ChainBridge(crate::Event::VoteFor(src_id, prop_id, RELAYER_C)),
            RuntimeEvent::ChainBridge(crate::Event::ProposalApproved(src_id, prop_id)),
            RuntimeEvent::ChainBridge(crate::Event::ProposalSucceeded(src_id, prop_id)),
        ]);
    })
}

#[test]
fn create_unsuccessful_proposal() {
    let src_id = 1;
    let r_id = derive_resource_id(src_id, b"transfer");

    new_test_ext_initialized(
        src_id,
        r_id,
        b"System.remark".to_vec(),
        DEFAULT_PROPOSAL_LIFETIME as u64,
    )
    .execute_with(|| {
        let prop_id = 1;
        let proposal = make_proposal(vec![11]);

        // Create proposal (& vote)
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get() + 1,
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
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![RELAYER_B],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);

        // Third relayer votes against
        assert_ok!(ChainBridge::reject_proposal(
            RuntimeOrigin::signed(RELAYER_C),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![RELAYER_B, RELAYER_C],
            status: ProposalStatus::Rejected,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);

        assert_eq!(BasicCurrency::free_balance(&RELAYER_B), 0);
        assert_eq!(
            BasicCurrency::free_balance(&ChainBridge::account_id()),
            ENDOWED_BALANCE
        );

        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::VoteFor(src_id, prop_id, RELAYER_A)),
            RuntimeEvent::ChainBridge(crate::Event::VoteAgainst(src_id, prop_id, RELAYER_B)),
            RuntimeEvent::ChainBridge(crate::Event::VoteAgainst(src_id, prop_id, RELAYER_C)),
            RuntimeEvent::ChainBridge(crate::Event::ProposalRejected(src_id, prop_id)),
        ]);
    })
}

#[test]
fn toggle_chain_success() {
    let src_id = 1;
    let r_id = derive_resource_id(src_id, b"transfer");

    new_test_ext_initialized(
        src_id,
        r_id,
        b"System.remark".to_vec(),
        DEFAULT_PROPOSAL_LIFETIME as u64,
    )
    .execute_with(|| {
        assert!(!<DisabledChains<Test>>::contains_key(src_id));

        assert_ok!(ChainBridge::toggle_chain(
            RuntimeOrigin::root(),
            src_id,
            false
        ));
        assert!(<DisabledChains<Test>>::contains_key(src_id));

        assert_ok!(ChainBridge::toggle_chain(
            RuntimeOrigin::root(),
            src_id,
            true
        ));
        assert!(!<DisabledChains<Test>>::contains_key(src_id));

        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::ChainToggled(src_id, false)),
            RuntimeEvent::ChainBridge(crate::Event::ChainToggled(src_id, true)),
        ]);
    });
}

#[test]
fn toggle_chain_fail() {
    let src_id = 1;
    let r_id = derive_resource_id(src_id, b"transfer");
    let acc: AccountId = 6;
    new_test_ext_initialized(
        src_id,
        r_id,
        b"System.remark".to_vec(),
        DEFAULT_PROPOSAL_LIFETIME as u64,
    )
    .execute_with(|| {
        assert!(!<DisabledChains<Test>>::contains_key(src_id));

        assert_err!(
            ChainBridge::toggle_chain(RuntimeOrigin::signed(acc), src_id, false),
            DispatchError::BadOrigin
        );
        assert!(!<DisabledChains<Test>>::contains_key(src_id));

        assert_err!(
            ChainBridge::toggle_chain(RuntimeOrigin::root(), src_id, true),
            Error::<Test>::AllowabilityEqual
        );
        assert!(!<DisabledChains<Test>>::contains_key(src_id));
    });
}

#[test]
fn acknowledge_proposal_with_disabled_transfers() {
    let src_id = 1;
    let r_id = derive_resource_id(src_id, b"transfer");

    new_test_ext_initialized(
        src_id,
        r_id,
        b"System.remark".to_vec(),
        DEFAULT_PROPOSAL_LIFETIME as u64,
    )
    .execute_with(|| {
        let prop_id = 1;
        let proposal = make_proposal(vec![11]);

        assert_ok!(ChainBridge::toggle_chain(
            RuntimeOrigin::root(),
            src_id,
            false
        ));
        assert!(<DisabledChains<Test>>::contains_key(src_id));

        assert_err!(
            ChainBridge::acknowledge_proposal(
                RuntimeOrigin::signed(RELAYER_A),
                prop_id,
                src_id,
                r_id,
                Box::new(proposal.clone())
            ),
            Error::<Test>::DisabledChain
        );
    });
}

#[test]
fn reject_proposal_with_disabled_transfers() {
    let src_id = 1;
    let r_id = derive_resource_id(src_id, b"transfer");

    new_test_ext_initialized(
        src_id,
        r_id,
        b"System.remark".to_vec(),
        DEFAULT_PROPOSAL_LIFETIME as u64,
    )
    .execute_with(|| {
        let prop_id = 1;
        let proposal = make_proposal(vec![10]);

        // Create proposal (& vote)
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);

        assert_ok!(ChainBridge::toggle_chain(
            RuntimeOrigin::root(),
            src_id,
            false
        ));
        assert!(<DisabledChains<Test>>::contains_key(src_id));

        assert_err!(
            ChainBridge::reject_proposal(
                RuntimeOrigin::signed(RELAYER_B),
                prop_id,
                src_id,
                r_id,
                Box::new(proposal.clone())
            ),
            Error::<Test>::DisabledChain
        );

        assert_ok!(ChainBridge::toggle_chain(
            RuntimeOrigin::root(),
            src_id,
            true
        ));
        assert!(!<DisabledChains<Test>>::contains_key(src_id));

        assert_ok!(ChainBridge::reject_proposal(
            RuntimeOrigin::signed(RELAYER_B),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));

        assert_ok!(ChainBridge::reject_proposal(
            RuntimeOrigin::signed(RELAYER_C),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));

        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![RELAYER_B, RELAYER_C],
            status: ProposalStatus::Rejected,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);
    });
}

#[test]
fn execute_after_threshold_change() {
    let src_id = 1;
    let r_id = derive_resource_id(src_id, b"transfer");

    new_test_ext_initialized(
        src_id,
        r_id,
        b"System.remark".to_vec(),
        DEFAULT_PROPOSAL_LIFETIME as u64,
    )
    .execute_with(|| {
        let prop_id = 1;
        let proposal = make_proposal(vec![11]);

        // Create proposal (& vote)
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);

        // Change threshold
        assert_ok!(ChainBridge::set_threshold(RuntimeOrigin::root(), 1));

        // Attempt to execute
        assert_ok!(ChainBridge::eval_vote_state(
            RuntimeOrigin::signed(RELAYER_A),
            prop_id,
            src_id,
            Box::new(proposal.clone())
        ));

        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: ProposalStatus::Approved,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);

        assert_eq!(BasicCurrency::free_balance(&RELAYER_B), 0);
        assert_eq!(
            BasicCurrency::free_balance(&ChainBridge::account_id()),
            ENDOWED_BALANCE
        );

        assert_events(vec![
            RuntimeEvent::ChainBridge(crate::Event::VoteFor(src_id, prop_id, RELAYER_A)),
            RuntimeEvent::ChainBridge(crate::Event::RelayerThresholdChanged(1)),
            RuntimeEvent::ChainBridge(crate::Event::ProposalApproved(src_id, prop_id)),
            RuntimeEvent::ChainBridge(crate::Event::ProposalSucceeded(src_id, prop_id)),
        ]);
    })
}

#[test]
fn proposal_expires() {
    let src_id = 1;
    let r_id = derive_resource_id(src_id, b"remark");

    new_test_ext_initialized(
        src_id,
        r_id,
        b"System.remark".to_vec(),
        DEFAULT_PROPOSAL_LIFETIME as u64,
    )
    .execute_with(|| {
        let prop_id = 1;
        let proposal = make_proposal(vec![10]);

        // Create proposal (& vote)
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);

        // Increment enough blocks such that now == expiry
        System::set_block_number(<ProposalLifetime<Test>>::get() + 1);

        // Attempt to submit a vote should fail
        assert_noop!(
            ChainBridge::reject_proposal(
                RuntimeOrigin::signed(RELAYER_B),
                prop_id,
                src_id,
                r_id,
                Box::new(proposal.clone())
            ),
            Error::<Test>::ProposalExpired
        );

        // Proposal state should remain unchanged
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);

        // eval_vote_state should have no effect
        assert_noop!(
            ChainBridge::eval_vote_state(
                RuntimeOrigin::signed(RELAYER_C),
                prop_id,
                src_id,
                Box::new(proposal.clone())
            ),
            Error::<Test>::ProposalExpired
        );
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: ProposalStatus::Initiated,
            expiry: <ProposalLifetime<Test>>::get() + 1,
        };
        assert_eq!(prop, expected);

        assert_events(vec![RuntimeEvent::ChainBridge(crate::Event::VoteFor(
            src_id, prop_id, RELAYER_A,
        ))]);
    })
}

#[test]
fn test_redistribute_fees() {
    use eq_primitives::asset::EQ;
    use eq_primitives::SignedBalance;
    use sp_runtime::traits::Zero;
    new_test_ext().execute_with(|| {
        // No relayers, no fees (noop)
        assert_ok!(ChainBridge::do_redistribute_fees());

        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_A));
        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_B));
        assert_ok!(ChainBridge::add_relayer(RuntimeOrigin::root(), RELAYER_C));
        assert_eq!(ChainBridge::relayer_count(), 3);

        // There are some relayers, but no fees (noop)
        assert_ok!(ChainBridge::do_redistribute_fees());

        let a = <Test as Config>::BalanceGetter::get_balance(&RELAYER_A, &EQ);
        let b = <Test as Config>::BalanceGetter::get_balance(&RELAYER_B, &EQ);
        let c = <Test as Config>::BalanceGetter::get_balance(&RELAYER_C, &EQ);
        assert!(a.is_zero());
        assert!(b.is_zero());
        assert!(c.is_zero());

        // Throw in some fees
        assert_ok!(<Test as Config>::Currency::transfer(
            &ChainBridge::account_id(),
            &ChainBridge::fee_account_id(),
            100u32.into(),
            ExistenceRequirement::AllowDeath
        ));

        // Real redistribution happens here
        assert_ok!(ChainBridge::do_redistribute_fees());

        let a = <Test as Config>::BalanceGetter::get_balance(&RELAYER_A, &EQ);
        let b = <Test as Config>::BalanceGetter::get_balance(&RELAYER_B, &EQ);
        let c = <Test as Config>::BalanceGetter::get_balance(&RELAYER_C, &EQ);
        assert_eq!(a, SignedBalance::Positive(33));
        assert_eq!(b, SignedBalance::Positive(33));
        assert_eq!(c, SignedBalance::Positive(33));
    });
}

#[test]
fn relays_count_is_valid_after_genesis_init() {
    new_test_ext_params(vec![RELAYER_A, RELAYER_B, RELAYER_C]).execute_with(|| {
        assert_eq!(ChainBridge::relayer_count(), 3);
    })
}

#[test]
fn minimal_nonce_check() {
    let src_id = 1;
    let r_id = derive_resource_id(src_id, b"remark");

    new_test_ext_initialized(
        src_id,
        r_id,
        b"System.remark".to_vec(),
        DEFAULT_PROPOSAL_LIFETIME as u64,
    )
    .execute_with(|| {
        let proposal = make_proposal(vec![10]);
        assert_ok!(ChainBridge::set_min_nonce(RuntimeOrigin::root(), src_id, 0));
        // Create proposal (& vote)
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_A),
            1,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        assert_ok!(ChainBridge::set_min_nonce(RuntimeOrigin::root(), src_id, 1));
        assert_ok!(ChainBridge::acknowledge_proposal(
            RuntimeOrigin::signed(RELAYER_B),
            1,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        assert_noop!(
            ChainBridge::acknowledge_proposal(
                RuntimeOrigin::signed(RELAYER_C),
                0,
                src_id,
                r_id,
                Box::new(proposal.clone())
            ),
            Error::<Test>::MinimalNonce
        );
    });
}
