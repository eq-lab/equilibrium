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

#![allow(unused_variables)]

use core::marker::PhantomData;

use crate::*;
use assets::*;
use eq_primitives::{
    asset::{self, Asset, AssetType, AssetXcmData, OtherReservedData},
    balance::{EqCurrency, XcmDestination, XcmTransferDealWithFee},
    xcm_origins, AccountType, XcmMode,
};
use eq_utils::balance_into_xcm;
use eq_xcm::*;
use frame_support::{
    assert_err, assert_noop, assert_ok,
    traits::{GenesisBuild, ProcessMessageError},
    PalletId,
};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{AccountIdConversion, TrailingZeroInput};
use sp_std::cell::RefCell;
use xcm::latest::{
    AssetId, Error as XcmError, ExecuteXcm, Fungibility,
    Instruction::*,
    Junction::{self},
    MultiAsset, MultiAssetFilter, MultiLocation, NetworkId, SendError, SendResult,
    Weight as XcmWeight,
    WeightLimit::*,
    Xcm,
};
use xcm_executor::traits::{Convert as _, ShouldExecute as _, WeightTrader as _};

use codec::Decode;
use eq_xcm::ParaId;
use polkadot_parachain::primitives::Sibling;
use xcm::v3::{Instruction, MultiAssets, Outcome, Parent, XcmHash};

type AccountPublic = <Signature as Verify>::Signer;
const EQ_PARACHAIN_ID: u32 = 2011;

#[macro_export]
macro_rules! assert_matches_event {
    ($( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {{
        assert!(crate::System::events().into_iter().any(|record| matches!(record.event, $( $pattern )|+ $( if $guard )?)))
    }}
}

thread_local! {
    static XCM_MESSAGE_CONTAINER: RefCell<Vec<(MultiLocation, xcm::latest::Xcm<()>)>> = RefCell::new(Vec::with_capacity(5));
}

pub fn xcm_message_container() -> Vec<(MultiLocation, xcm::latest::Xcm<()>)> {
    XCM_MESSAGE_CONTAINER.with(|c| c.borrow().clone())
}

pub const CURSED_MULTI_LOCATION: MultiLocation = MultiLocation {
    parents: 1,
    interior: X1(Parachain(666)),
};

fn hash_xcm<T>(msg: Xcm<T>) -> XcmHash {
    msg.using_encoded(sp_io::hashing::blake2_256)
}

fn general_key(id: &[u8; 3]) -> Junction {
    let id = id.to_vec();
    let mut data = [0u8; 32];
    data[..id.len()].copy_from_slice(&id[..]);
    GeneralKey {
        length: id.len() as u8,
        data,
    }
}

pub struct XcmRouterMock;
impl xcm::latest::SendXcm for XcmRouterMock {
    type Ticket = (MultiLocation, Xcm<()>);
    fn validate(
        destination: &mut Option<MultiLocation>,
        message: &mut Option<Xcm<()>>,
    ) -> SendResult<Self::Ticket> {
        match &destination {
            Some(CURSED_MULTI_LOCATION) => Err(SendError::ExceedsMaxMessageSize),
            Some(MultiLocation {
                parents: 1,
                interior: Here | X1(Parachain(_)),
            }) => Ok((
                (destination.unwrap(), message.clone().unwrap()),
                MultiAssets::new(),
            )),
            _ => Err(xcm::latest::SendError::Transport("")),
        }
    }

    fn deliver(ticket: Self::Ticket) -> Result<XcmHash, SendError> {
        let (dest, msg) = ticket;
        XCM_MESSAGE_CONTAINER.with(|c| c.borrow_mut().push((dest, msg.clone())));
        Ok(hash_xcm(msg))
    }
}

mod multi {
    use super::*;

    pub const EQ: OtherReservedData = OtherReservedData {
        multi_location: MultiLocation::here(),
        decimals: 9,
    };
    parameter_types! {
        pub EQD: OtherReservedData = OtherReservedData {
            multi_location: MultiLocation {
                parents: 0,
                interior: X1(general_key(b"eqd")),
            },
            decimals: 9,
        };
    }
    pub const DOT: OtherReservedData = OtherReservedData {
        multi_location: MultiLocation {
            parents: 1,
            interior: Here,
        },
        decimals: 10,
    };

    /// From statemine
    pub const USDT: OtherReservedData = OtherReservedData {
        multi_location: MultiLocation {
            parents: 1,
            interior: X3(Parachain(1000), PalletInstance(50), GeneralIndex(1984)),
        },
        decimals: 6,
    };
}

mod resources {
    pub const DOT: chainbridge::ResourceId = [0; 32];
    pub const EQ: chainbridge::ResourceId = [1; 32];
    pub const EQD: chainbridge::ResourceId = [2; 32];
    pub const USDT: chainbridge::ResourceId = [3; 32];
}

pub fn get_from_seed<P: Public>(seed: &str) -> <P::Pair as Pair>::Public {
    P::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

fn get_account_id_from_seed<P: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<P::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<P>(seed)).into_account()
}

fn parachain_test_ext() -> Result<sp_io::TestExternalities, String> {
    let mut storage = frame_system::GenesisConfig::default().build_storage::<Runtime>()?;

    chainbridge::GenesisConfig::<Runtime> {
        chains: vec![GENSHIRO_CHAIN_ID],
        ..Default::default()
    }
    .assimilate_storage(&mut storage)?;

    eq_assets::GenesisConfig::<Runtime> {
        _runtime: PhantomData,
        assets: vec![
            (
                asset::EQ.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                AssetXcmData::SelfReserved.encode(),
                Permill::from_rational(2u32, 5u32),
                4,
                AssetType::Native,
                true,
                Percent::zero(),
                Permill::one(),
            ),
            (
                asset::EQD.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                AssetXcmData::SelfReserved.encode(),
                Permill::from_rational(2u32, 5u32),
                3,
                AssetType::Synthetic,
                true,
                Percent::zero(),
                Permill::one(),
            ),
            (
                asset::USDT.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                AssetXcmData::OtherReserved(multi::USDT).encode(),
                Permill::from_rational(2u32, 5u32),
                1,
                AssetType::Physical,
                true,
                Percent::zero(),
                Permill::one(),
            ),
            (
                asset::DOT.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                AssetXcmData::OtherReserved(multi::DOT).encode(),
                Permill::from_rational(2u32, 5u32),
                2,
                AssetType::Physical,
                true,
                Percent::zero(),
                Permill::one(),
            ),
        ],
    }
    .assimilate_storage(&mut storage)?;

    eq_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (
                BRIDGE_MODULE_ID.into_account_truncating(),
                vec![
                    (INITIAL_AMOUNT, asset::EQ.get_id()),
                    (INITIAL_AMOUNT, asset::DOT.get_id()),
                    (INITIAL_AMOUNT, asset::USDT.get_id()),
                    (INITIAL_AMOUNT, asset::EQD.get_id()),
                ],
            ),
            (
                USER_X,
                vec![
                    (INITIAL_AMOUNT, asset::EQ.get_id()),
                    (INITIAL_AMOUNT, asset::DOT.get_id()),
                    (INITIAL_AMOUNT, asset::USDT.get_id()),
                    (INITIAL_AMOUNT, asset::EQD.get_id()),
                ],
            ),
            (
                USER_Y,
                vec![
                    (INITIAL_AMOUNT, asset::EQ.get_id()),
                    (INITIAL_AMOUNT, asset::DOT.get_id()),
                    (INITIAL_AMOUNT, asset::USDT.get_id()),
                    (INITIAL_AMOUNT, asset::EQD.get_id()),
                ],
            ),
            (
                USER_Z,
                vec![
                    (INITIAL_AMOUNT, asset::EQ.get_id()),
                    (INITIAL_AMOUNT, asset::DOT.get_id()),
                    (INITIAL_AMOUNT, asset::USDT.get_id()),
                    (INITIAL_AMOUNT, asset::EQD.get_id()),
                ],
            ),
        ],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(true)),
    }
    .assimilate_storage(&mut storage)?;

    let eq_treasury_config = eq_treasury::GenesisConfig { empty: () };
    <eq_treasury::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
        &eq_treasury_config,
        &mut storage,
    )?;

    eq_bridge::GenesisConfig::<Runtime> {
        _runtime: PhantomData,
        resources: vec![
            (resources::DOT, asset::DOT),
            (resources::USDT, asset::USDT),
            (resources::EQ, asset::EQ),
            (resources::EQD, asset::EQD),
        ],
        minimum_transfer_amount: vec![
            (GENSHIRO_CHAIN_ID, resources::DOT, 1),
            (GENSHIRO_CHAIN_ID, resources::USDT, 1),
            (GENSHIRO_CHAIN_ID, resources::EQ, 1),
            (GENSHIRO_CHAIN_ID, resources::EQD, 1),
        ],
        enabled_withdrawals: vec![
            (resources::DOT, vec![GENSHIRO_CHAIN_ID]),
            (resources::USDT, vec![GENSHIRO_CHAIN_ID]),
            (resources::EQ, vec![GENSHIRO_CHAIN_ID]),
            (resources::EQD, vec![GENSHIRO_CHAIN_ID]),
        ],
    }
    .assimilate_storage(&mut storage)?;

    eq_whitelists::GenesisConfig::<Runtime> {
        whitelist: vec![USER_X],
    }
    .assimilate_storage(&mut storage)?;

    let pallet_xcm_config = pallet_xcm::GenesisConfig {
        safe_xcm_version: Some(2),
    };
    <pallet_xcm::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
        &pallet_xcm_config,
        &mut storage,
    )?;

    let parachain_info_config = parachain_info::GenesisConfig {
        parachain_id: ParaId::from(EQ_PARACHAIN_ID),
    };
    <parachain_info::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
        &parachain_info_config,
        &mut storage,
    )?;

    Ok(storage.into())
}

#[test]
fn location_to_account_id_works() {
    let original_account: AccountId = get_account_id_from_seed::<sr25519::Public>("Alice");
    let dest_multi_location = MultiLocation {
        parents: 0,
        interior: X1(AccountId32 {
            id: original_account.clone().into(),
            network: None,
        }),
    };
    let result = LocationToAccountId::convert(dest_multi_location);

    assert_ok!(result.clone());

    assert_eq!(original_account, result.unwrap());
}

#[test]
fn location_to_account_id_fails_on_network() {
    let original_account: AccountId = get_account_id_from_seed::<sr25519::Public>("Alice");
    let network = Some(NetworkId::Polkadot);
    let dest_multi_location = MultiLocation {
        parents: 0,
        interior: X1(AccountId32 {
            id: original_account.into(),
            network,
        }),
    };
    let result = LocationToAccountId::convert(dest_multi_location.clone());

    assert_err!(result, dest_multi_location);
}

#[test]
fn location_to_account_id_fails_on_junction_parachain() {
    let dest_multi_location = MultiLocation {
        parents: 0,
        interior: X1(Parachain(2000)),
    };
    let result = LocationToAccountId::convert(dest_multi_location.clone());

    assert_err!(result, dest_multi_location);
}

#[test]
fn transact_dispatch_origin_not_allowed() {
    let account: AccountId = get_account_id_from_seed::<sr25519::Public>("Alice");
    for origin in [
        MultiLocation::parent(),
        MultiLocation {
            parents: 1,
            interior: X1(AccountId32 {
                network: None,
                id: account.clone().into(),
            }),
        },
        MultiLocation {
            parents: 0,
            interior: X1(AccountId32 {
                network: None,
                id: account.into(),
            }),
        },
    ] {
        for kind in [
            OriginKind::Native,
            OriginKind::SovereignAccount,
            OriginKind::Superuser,
            OriginKind::Xcm,
        ] {
            let result: Result<RuntimeOrigin, _> =
                XcmOriginToTransactDispatchOrigin::convert_origin(origin.clone(), kind);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap(), origin);
        }
    }
}

#[allow(dead_code)]
const USER_X: AccountId = AccountId::new([0x01; 32]);
#[allow(dead_code)]
const USER_Y: AccountId = AccountId::new([0x02; 32]);
#[allow(dead_code)]
const USER_Z: AccountId = AccountId::new([0x03; 32]);

const BRIDGE_MODULE_ID: PalletId = chainbridge::MODULE_ID;

#[allow(dead_code)]
const TO_SEND_AMOUNT: Balance = 1000 * ONE_TOKEN;
#[allow(dead_code)]
const TO_RECV_AMOUNT: Balance = 1400 * ONE_TOKEN;
#[allow(dead_code)]
const FEE_AMOUNT: Balance = 1 * ONE_TOKEN;
#[allow(dead_code)]
const INITIAL_AMOUNT: Balance = 10000 * ONE_TOKEN;
#[allow(dead_code)]
const XCM_MSG_WEIGHT: XcmWeight = BaseXcmWeight::get().saturating_mul(4);

const DOT_FEE: (Asset, Balance) = (asset::DOT, 2 * 469_417_452);
const ACALA_EQD_FEE: (Asset, Balance) = (asset::EQD, 185_392_000);

fn multi_asset_from<Balance>(amount: Balance, asset_xcm_data: &OtherReservedData) -> MultiAsset
where
    Balance: sp_std::convert::Into<XcmBalance>,
{
    MultiAsset {
        id: AssetId::Concrete(asset_xcm_data.multi_location.clone()),
        fun: Fungibility::Fungible(
            eq_utils::balance_into_xcm(amount, asset_xcm_data.decimals).unwrap(),
        ), // convert to parachain balance
    }
}

#[test]
fn xcm_message_transferred() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::balance::EqCurrency as _;

        assert_ok!(EqBalances::xcm_toggle(
            RuntimeOrigin::root(),
            Some(XcmMode::Bridge(true))
        ));

        System::set_block_number(1);

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&BRIDGE_MODULE_ID.into_account_truncating(), asset::DOT),
            INITIAL_AMOUNT
        );

        assert_ok!(EqBridge::xcm_transfer(
            RuntimeOrigin::signed(BRIDGE_MODULE_ID.into_account_truncating()).into(),
            (0, AccountType::Id32(USER_X.into())).encode(),
            TO_SEND_AMOUNT,
            resources::DOT,
        ));

        assert_eq!(
            xcm_message_container(),
            vec![(
                xcm_origins::RELAY,
                Xcm(vec![
                    WithdrawAsset(
                        vec![MultiAsset {
                            id: AssetId::Concrete(MultiLocation::here()),
                            fun: Fungibility::Fungible(
                                balance_into_xcm(TO_SEND_AMOUNT, 10).unwrap()
                            )
                        }]
                        .into()
                    ),
                    ClearOrigin,
                    BuyExecution {
                        fees: MultiAsset {
                            id: AssetId::Concrete(MultiLocation::here()),
                            fun: Fungibility::Fungible(DOT_FEE.1)
                        },
                        weight_limit: xcm::latest::WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: xcm::latest::WildMultiAsset::All.into(),
                        beneficiary: X1(AccountId32 {
                            network: None,
                            id: USER_X.into()
                        })
                        .into(),
                    },
                ])
            )]
        );

        let ksm_location = MultiLocation::here();
        println!("{:?}", System::events());
        System::assert_has_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                xcm_origins::RELAY,
                MultiLocation::new(
                    0,
                    AccountId32 {
                        network: None,
                        id: USER_X.into(),
                    },
                ),
            ),
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&BRIDGE_MODULE_ID.into_account_truncating(), asset::DOT),
            INITIAL_AMOUNT
        );
    })
}

#[test]
fn xcm_transfer_eq_native() {
    parachain_test_ext().unwrap().execute_with(|| {
        assert_ok!(EqBalances::xcm_toggle(
            RuntimeOrigin::root(),
            Some(XcmMode::Bridge(true))
        ));
        System::set_block_number(1);

        assert_ok!(EqBridge::xcm_transfer(
            RuntimeOrigin::signed(BRIDGE_MODULE_ID.into_account_truncating()).into(),
            (2000, AccountType::Id32(USER_X.into())).encode(),
            TO_SEND_AMOUNT,
            resources::EQ,
        ));

        assert_eq!(
            xcm_message_container(),
            vec![(
                xcm_origins::dot::PARACHAIN_ACALA,
                Xcm(vec![
                    ReserveAssetDeposited(
                        vec![MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X1(Parachain(EQ_PARACHAIN_ID))
                            )),
                            fun: Fungibility::Fungible(TO_SEND_AMOUNT),
                        }]
                        .into()
                    ),
                    ClearOrigin,
                    BuyExecution {
                        fees: MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X1(Parachain(EQ_PARACHAIN_ID))
                            )),
                            fun: Fungibility::Fungible(
                                2 * fees::acala::eq::WeightToFee::weight_to_fee(
                                    &fees::acala::BaseXcmWeight::get().saturating_mul(4)
                                )
                            ),
                        },
                        weight_limit: xcm::latest::WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: xcm::latest::WildMultiAsset::All.into(),
                        beneficiary: X1(AccountId32 {
                            network: None,
                            id: USER_X.into()
                        })
                        .into(),
                    },
                ])
            )]
        );
        System::assert_has_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                xcm_origins::dot::PARACHAIN_ACALA,
                MultiLocation::new(
                    0,
                    AccountId32 {
                        network: None,
                        id: USER_X.into(),
                    },
                ),
            ),
        ));
    });
}

#[test]
fn xcm_transfer_eqd_native_bridge() {
    parachain_test_ext().unwrap().execute_with(|| {
        assert_ok!(EqBalances::xcm_toggle(
            RuntimeOrigin::root(),
            Some(XcmMode::Bridge(true))
        ));

        System::set_block_number(1);

        let acala_acc: AccountId = Sibling::from(2000).into_account_truncating();
        assert_eq!(EqBalances::total_balance(&acala_acc, asset::EQD), 0,);

        assert_ok!(EqBridge::xcm_transfer(
            RuntimeOrigin::signed(BRIDGE_MODULE_ID.into_account_truncating()).into(),
            (2000, AccountType::Id32(USER_X.into())).encode(),
            TO_SEND_AMOUNT,
            resources::EQD,
        ));

        assert_eq!(
            xcm_message_container(),
            vec![(
                xcm_origins::dot::PARACHAIN_ACALA,
                Xcm(vec![
                    ReserveAssetDeposited(
                        vec![MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X2(Parachain(EQ_PARACHAIN_ID), general_key(b"eqd"))
                            )),
                            fun: Fungibility::Fungible(TO_SEND_AMOUNT),
                        }]
                        .into()
                    ),
                    ClearOrigin,
                    BuyExecution {
                        fees: MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X2(Parachain(EQ_PARACHAIN_ID), general_key(b"eqd"))
                            )),
                            fun: Fungibility::Fungible(2 * 70_392_000),
                        },
                        weight_limit: xcm::latest::WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: xcm::latest::WildMultiAsset::All.into(),
                        beneficiary: X1(AccountId32 {
                            network: None,
                            id: USER_X.into()
                        })
                        .into(),
                    },
                ])
            )]
        );
        System::assert_has_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                xcm_origins::dot::PARACHAIN_ACALA,
                MultiLocation::new(
                    0,
                    AccountId32 {
                        network: None,
                        id: USER_X.into(),
                    },
                ),
            ),
        ));

        // all funds stored in sovereign account
        assert_eq!(
            EqBalances::total_balance(&acala_acc, asset::EQD),
            TO_SEND_AMOUNT
        );
    });
}

#[test]
fn xcm_transfer_eqd_native() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        let acala_acc: AccountId = Sibling::from(2000).into_account_truncating();
        assert_eq!(EqBalances::total_balance(&acala_acc, asset::EQD), 0,);

        let to: MultiLocation = MultiLocation::new(
            1,
            X2(
                Parachain(2000),
                AccountId32 {
                    network: None,
                    id: USER_Y.into(),
                },
            ),
        );
        assert_ok!(EqBalances::do_xcm_transfer(
            USER_X,
            (asset::EQD, TO_SEND_AMOUNT),
            ACALA_EQD_FEE,
            XcmDestination::Common(to.clone()),
        ));

        assert_eq!(
            xcm_message_container(),
            vec![(
                xcm_origins::dot::PARACHAIN_ACALA,
                Xcm(vec![
                    ReserveAssetDeposited(
                        vec![MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X2(Parachain(EQ_PARACHAIN_ID), general_key(b"eqd"))
                            )),
                            fun: Fungibility::Fungible(TO_SEND_AMOUNT + ACALA_EQD_FEE.1),
                        }]
                        .into()
                    ),
                    ClearOrigin,
                    BuyExecution {
                        fees: MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X2(Parachain(EQ_PARACHAIN_ID), general_key(b"eqd"))
                            )),
                            fun: Fungibility::Fungible(2 * 92_696_000),
                        },
                        weight_limit: xcm::latest::WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: xcm::latest::WildMultiAsset::All.into(),
                        beneficiary: X1(AccountId32 {
                            network: None,
                            id: USER_Y.into()
                        })
                        .into(),
                    },
                ])
            )]
        );

        System::assert_has_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                xcm_origins::dot::PARACHAIN_ACALA,
                MultiLocation::new(
                    0,
                    AccountId32 {
                        network: None,
                        id: USER_Y.into(),
                    },
                ),
            ),
        ));

        // all funds stored in sovereign account
        assert_eq!(
            EqBalances::total_balance(&acala_acc, asset::EQD),
            TO_SEND_AMOUNT + ACALA_EQD_FEE.1
        );
    });
}

#[test]
fn xcm_transfer_native() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::{balance::EqCurrency as _, AccountType};
        //use frame_support::weights::WeightToFeePolynomial as _;
        use sp_std::convert::TryFrom as _;

        System::set_block_number(1);

        let relay_acc = AccountId::decode(&mut TrailingZeroInput::new(b"Parent")).unwrap();

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&relay_acc, asset::DOT), 0);

        assert_ok!(<EqBalances as EqCurrency<_, _>>::xcm_transfer(
            &USER_X,
            asset::DOT,
            TO_SEND_AMOUNT,
            XcmDestination::Native(AccountType::try_from(USER_Y.encode()).unwrap()),
        ));

        assert_eq!(
            xcm_message_container(),
            vec![(
                xcm_origins::RELAY,
                Xcm(vec![
                    WithdrawAsset(
                        vec![MultiAsset {
                            id: AssetId::Concrete(Here.into()),
                            fun: Fungibility::Fungible(
                                balance_into_xcm(TO_SEND_AMOUNT, 10).unwrap()
                            ),
                        }]
                        .into()
                    ),
                    ClearOrigin,
                    BuyExecution {
                        fees: MultiAsset {
                            id: AssetId::Concrete(Here.into()),
                            fun: Fungibility::Fungible(DOT_FEE.1),
                        },
                        weight_limit: xcm::latest::WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: xcm::latest::WildMultiAsset::All.into(),
                        beneficiary: X1(AccountId32 {
                            network: None,
                            id: USER_Y.into()
                        })
                        .into(),
                    },
                ])
            )]
        );

        let ksm_location = MultiLocation::here();
        System::assert_has_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                xcm_origins::RELAY,
                MultiLocation::new(
                    0,
                    AccountId32 {
                        network: None,
                        id: USER_Y.into(),
                    },
                ),
            ),
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT - TO_SEND_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&relay_acc, asset::DOT), 0);
    })
}

#[test]
fn xcm_transfer_native_gens_is_preserved() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::{balance::EqCurrency as _, AccountType};
        use sp_std::convert::TryFrom as _;

        System::set_block_number(1);

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::EQ),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::EQ),
            INITIAL_AMOUNT
        );

        assert_noop!(
            <EqBalances as EqCurrency<_, _>>::xcm_transfer(
                &USER_X,
                asset::EQ,
                TO_SEND_AMOUNT,
                XcmDestination::Native(AccountType::try_from(USER_Y.encode()).unwrap()),
            ),
            eq_balances::Error::<Runtime>::XcmInvalidDestination
        );

        assert_eq!(xcm_message_container(), vec![]);

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::EQ),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::EQ),
            INITIAL_AMOUNT
        );
    })
}

#[test]
fn xcm_transfer_self_reserved() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::balance::EqCurrency as _;

        System::set_block_number(1);

        let parallel_acc: AccountId = Sibling::from(2012).into_account_truncating();

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::EQ),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::EQ),
            INITIAL_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&parallel_acc, asset::EQ), 0);

        let to = MultiLocation {
            parents: 1,
            interior: X2(
                Parachain(2012),
                AccountId32 {
                    id: USER_Y.into(),
                    network: None,
                },
            ),
        };
        assert_ok!(<EqBalances as EqCurrency<_, _>>::xcm_transfer(
            &USER_X,
            asset::EQ,
            TO_SEND_AMOUNT,
            XcmDestination::Common(to),
        ));

        assert_eq!(
            xcm_message_container(),
            vec![(
                xcm_origins::dot::PARACHAIN_PARALLEL,
                Xcm(vec![
                    ReserveAssetDeposited(
                        vec![MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X1(Parachain(EQ_PARACHAIN_ID))
                            )),
                            fun: Fungibility::Fungible(TO_SEND_AMOUNT),
                        }]
                        .into()
                    ),
                    ClearOrigin,
                    BuyExecution {
                        fees: MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X1(Parachain(EQ_PARACHAIN_ID))
                            )),
                            fun: Fungibility::Fungible(0),
                        },
                        weight_limit: xcm::latest::WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: xcm::latest::WildMultiAsset::All.into(),
                        beneficiary: X1(AccountId32 {
                            network: None,
                            id: USER_Y.into()
                        })
                        .into(),
                    },
                ])
            )]
        );

        System::assert_has_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                xcm_origins::dot::PARACHAIN_PARALLEL,
                MultiLocation::new(
                    0,
                    AccountId32 {
                        id: USER_Y.into(),
                        network: None,
                    },
                ),
            ),
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::EQ),
            INITIAL_AMOUNT - TO_SEND_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::EQ),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&parallel_acc, asset::EQ),
            TO_SEND_AMOUNT
        );
    })
}

#[test]
fn xcm_transfer_other_reserved() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::balance::EqCurrency as _;

        System::set_block_number(1);

        let relay_acc = AccountId::decode(&mut TrailingZeroInput::new(b"Parent")).unwrap();

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&relay_acc, asset::DOT), 0);

        let to = MultiLocation {
            parents: 1,
            interior: X1(AccountId32 {
                id: USER_Y.into(),
                network: None,
            }),
        };
        assert_ok!(<EqBalances as EqCurrency<_, _>>::xcm_transfer(
            &USER_X,
            asset::DOT,
            TO_SEND_AMOUNT,
            XcmDestination::Common(to.clone()),
        ));

        assert_eq!(
            xcm_message_container(),
            vec![(
                xcm_origins::RELAY,
                Xcm(vec![
                    WithdrawAsset(
                        vec![MultiAsset {
                            id: AssetId::Concrete(Here.into()),
                            fun: Fungibility::Fungible(
                                balance_into_xcm(TO_SEND_AMOUNT, 10).unwrap()
                            ),
                        }]
                        .into()
                    ),
                    ClearOrigin,
                    BuyExecution {
                        fees: MultiAsset {
                            id: AssetId::Concrete(Here.into()),
                            fun: Fungibility::Fungible(DOT_FEE.1),
                        },
                        weight_limit: xcm::latest::WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: xcm::latest::WildMultiAsset::All.into(),
                        beneficiary: X1(AccountId32 {
                            network: None,
                            id: USER_Y.into()
                        })
                        .into(),
                    },
                ])
            )]
        );

        System::assert_has_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                xcm_origins::RELAY,
                MultiLocation::new(
                    0,
                    AccountId32 {
                        id: USER_Y.into(),
                        network: None,
                    },
                ),
            ),
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT - TO_SEND_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&relay_acc, asset::DOT), 0);
    })
}

#[test]
fn xcm_transfer_deal_with_fee_ok() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::USDT),
            INITIAL_AMOUNT
        );

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X).into(),
            asset::USDT,
            FixedI64::from_inner(1_000_000_000) // $1
        ));
        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X).into(),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000) // $5
        ));

        assert_ok!(EqBalances::do_xcm_transfer_old(
            USER_X,
            asset::USDT,
            TO_SEND_AMOUNT,
            XcmDestination::Native(AccountType::Id32(USER_Y.into())),
            XcmTransferDealWithFee::SovereignAccWillPay
        ));

        let (fee_asset, fee_amount) = XcmToFee::convert((
            asset::USDT,
            xcm_origins::dot::PARACHAIN_STATEMINT,
            &Xcm::<()>(vec![ClearOrigin; 4]),
        ))
        .unwrap();

        assert_eq!(fee_asset, asset::DOT);
        assert_eq!(fee_amount, 2 * 35_199_492);

        let fee_in_usdt = (fee_amount * 5) / 10_000; // DOT - 10 decimals; USDT - 6 decimals
        let fee_in_usdt_local = fee_in_usdt * 1_000;
        let actual_to_send_amount = TO_SEND_AMOUNT / 1_000 - fee_in_usdt;

        System::assert_has_event(RuntimeEvent::EqBalances(eq_balances::Event::XcmTransfer(
            xcm_origins::dot::PARACHAIN_STATEMINT,
            MultiLocation::new(
                0,
                AccountId32 {
                    network: None,
                    id: USER_Y.into(),
                },
            ),
        )));

        assert_eq!(
            xcm_message_container(),
            vec![(
                xcm_origins::dot::PARACHAIN_STATEMINT,
                Xcm(vec![
                    WithdrawAsset(
                        vec![
                            MultiAsset {
                                id: AssetId::Concrete(MultiLocation::new(
                                    0,
                                    X2(PalletInstance(50), GeneralIndex(1984))
                                )),
                                fun: Fungibility::Fungible(actual_to_send_amount),
                            },
                            MultiAsset {
                                id: AssetId::Concrete(Parent.into()),
                                fun: Fungibility::Fungible(fee_amount),
                            },
                        ]
                        .into()
                    ),
                    ClearOrigin,
                    BuyExecution {
                        fees: MultiAsset {
                            id: AssetId::Concrete(Parent.into()),
                            fun: Fungibility::Fungible(fee_amount),
                        },
                        weight_limit: xcm::latest::WeightLimit::Unlimited,
                    },
                    DepositAsset {
                        assets: xcm::latest::WildMultiAsset::All.into(),
                        beneficiary: X1(AccountId32 {
                            network: None,
                            id: USER_Y.into()
                        })
                        .into(),
                    },
                ])
            )]
        )
    });
}

#[test]
fn xcm_transfer_deal_with_fee_no_price() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::USDT),
            INITIAL_AMOUNT
        );

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X).into(),
            asset::USDT,
            FixedI64::from_inner(1_000_000_000) // $1
        ));

        assert_noop!(
            EqBalances::do_xcm_transfer_old(
                USER_X,
                asset::USDT,
                TO_SEND_AMOUNT,
                XcmDestination::Native(AccountType::Id32(USER_Y.into())),
                XcmTransferDealWithFee::SovereignAccWillPay
            ),
            eq_oracle::Error::<Runtime>::CurrencyNotFound,
        );
    });
}

#[test]
fn xcm_transfer_deal_with_fee_unimplemented() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::USDT),
            INITIAL_AMOUNT
        );

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X).into(),
            asset::USDT,
            FixedI64::from_inner(1_000_000_000) // $1
        ));
        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X).into(),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000) // $5
        ));

        assert_noop!(
            EqBalances::do_xcm_transfer_old(
                USER_X,
                asset::USDT,
                TO_SEND_AMOUNT,
                XcmDestination::Native(AccountType::Id32(USER_Y.into())),
                XcmTransferDealWithFee::AccOnTargetChainWillPay
            ),
            eq_balances::Error::<Runtime>::MethodUnimplemented,
        );
    });
}

// #[test]
// fn xcm_transfer_deal_with_fee_self_reserved_asset_failed() {
//     parachain_test_ext().unwrap().execute_with(|| {
//         System::set_block_number(1);

//         assert_eq!(
//             EqBalances::total_balance(&USER_X, asset::USDT),
//             INITIAL_AMOUNT
//         );

//         assert_ok!(Oracle::set_price(
//             RuntimeOrigin::signed(USER_X).into(),
//             asset::USDT,
//             FixedI64::from_inner(1_000_000_000) // $1
//         ));
//         assert_ok!(Oracle::set_price(
//             RuntimeOrigin::signed(USER_X).into(),
//             asset::DOT,
//             FixedI64::from_inner(5_000_000_000) // $5
//         ));

//         assert_noop!(
//             EqBalances::do_xcm_transfer(
//                 USER_X,
//                 asset::EQD,
//                 TO_SEND_AMOUNT,
//                 XcmDestination::Common(MultiLocation {
//                     parents: 1,
//                     interior: X1(AccountId32 {
//                         network: None,
//                         id: USER_Y.into()
//                     })
//                 }),
//                 XcmTransferDealWithFee::SovereignAccWillPay
//             ),
//             eq_balances::Error::<Runtime>::XcmWrongFeeAsset,
//         );
//     });
// }

#[test]
fn xcm_message_received_unlimited_weight_bridge() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::balance::EqCurrency as _;
        assert_ok!(EqBalances::xcm_toggle(
            RuntimeOrigin::root(),
            Some(XcmMode::Bridge(true))
        ));

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000)
        ));
        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::USDT,
            FixedI64::from_inner(1_000_000_000)
        ));

        let bridge: AccountId = BRIDGE_MODULE_ID.into_account_truncating();

        System::set_block_number(1);

        assert_eq!(
            EqBalances::total_balance(&USER_Z, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&bridge, asset::DOT),
            INITIAL_AMOUNT
        );

        let fee_in_usd = crate::fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT) as Balance;
        let fee_in_dot = (fee_in_usd * 10) / 5;
        let fee_asset = MultiAsset {
            id: AssetId::Concrete(multi::DOT.multi_location),
            fun: Fungibility::Fungible(2 * fee_in_dot),
        };
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![
                    multi_asset_from(TO_RECV_AMOUNT, &multi::DOT),
                    fee_asset.clone(),
                ]
                .into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: fee_asset,
                weight_limit: Unlimited,
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::DOT)].into(),
                ),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: USER_Z.into(),
                    }),
                },
            },
        ]);

        assert!(matches!(
            XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
                xcm_origins::RELAY,
                rcvd_xcm_message.clone(),
                hash_xcm(rcvd_xcm_message),
                XcmWeight::MAX,
                XcmWeight::MAX
            ),
            Outcome::Complete(_),
        ));

        System::assert_has_event(RuntimeEvent::EqBridge(
            eq_bridge::Event::<Runtime>::ToBridgeTransfer(
                bridge.clone(),
                asset::DOT,
                TO_RECV_AMOUNT,
            ),
        ));

        System::assert_has_event(RuntimeEvent::ChainBridge(
            chainbridge::Event::<Runtime>::FungibleTransfer(
                GENSHIRO_CHAIN_ID,
                1,
                resources::DOT,
                sp_core::U256::from(TO_RECV_AMOUNT),
                USER_Z.encode(),
            ),
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_Z, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&bridge, asset::DOT),
            INITIAL_AMOUNT
        );

        System::set_block_number(2);

        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&bridge, asset::DOT),
            INITIAL_AMOUNT
        );

        let fee_in_usd = crate::fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT) as Balance;
        let fee_in_usdt = fee_in_usd / 1000;
        let fee_asset = MultiAsset {
            id: AssetId::Concrete(multi::USDT.multi_location),
            fun: Fungibility::Fungible(2 * fee_in_usdt),
        };
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![
                    multi_asset_from(TO_RECV_AMOUNT, &multi::USDT),
                    fee_asset.clone(),
                ]
                .into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: fee_asset,
                weight_limit: Unlimited,
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::USDT)].into(),
                ),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: USER_Y.into(),
                    }),
                },
            },
        ]);

        let execute_result = XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
            xcm_origins::dot::PARACHAIN_STATEMINT,
            rcvd_xcm_message.clone(),
            hash_xcm(rcvd_xcm_message),
            XcmWeight::MAX,
            XcmWeight::MAX,
        );
        println!("{:?}", execute_result);
        assert!(matches!(execute_result, Outcome::Complete(_)));

        System::assert_has_event(RuntimeEvent::EqBridge(
            eq_bridge::Event::<Runtime>::ToBridgeTransfer(
                bridge.clone(),
                asset::USDT,
                TO_RECV_AMOUNT,
            ),
        ));

        System::assert_has_event(RuntimeEvent::ChainBridge(
            chainbridge::Event::<Runtime>::FungibleTransfer(
                GENSHIRO_CHAIN_ID,
                2,
                resources::USDT,
                sp_core::U256::from(TO_RECV_AMOUNT.saturated_into::<u64>()),
                USER_Y.encode(),
            ),
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&bridge, asset::DOT),
            INITIAL_AMOUNT
        );
    })
}

#[test]
fn xcm_message_reseived_limited_weight_ok_bridge() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::balance::EqCurrency as _;

        System::set_block_number(1);

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000)
        ));
        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::USDT,
            FixedI64::from_inner(1_000_000_000)
        ));

        assert_ok!(EqBalances::xcm_toggle(
            RuntimeOrigin::root(),
            Some(XcmMode::Bridge(true))
        ));

        let bridge = BRIDGE_MODULE_ID.into_account_truncating();

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&bridge, asset::DOT),
            INITIAL_AMOUNT
        );

        let fee_in_usd = crate::fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT) as Balance;
        let fee_in_dot = (fee_in_usd * 10) / 5;
        let fee_asset = MultiAsset {
            id: AssetId::Concrete(multi::DOT.multi_location),
            fun: Fungibility::Fungible(2 * fee_in_dot),
        };
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![
                    multi_asset_from(TO_RECV_AMOUNT, &multi::DOT),
                    fee_asset.clone(),
                ]
                .into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: fee_asset,
                weight_limit: Limited(XCM_MSG_WEIGHT),
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::DOT)].into(),
                ),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: USER_X.into(),
                    }),
                },
            },
        ]);

        assert!(matches!(
            XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
                xcm_origins::RELAY,
                rcvd_xcm_message.clone(),
                hash_xcm(rcvd_xcm_message),
                XcmWeight::MAX,
                XcmWeight::MAX
            ),
            Outcome::Complete(_),
        ));

        System::assert_has_event(RuntimeEvent::EqBridge(
            eq_bridge::Event::<Runtime>::ToBridgeTransfer(
                bridge.clone(),
                asset::DOT,
                TO_RECV_AMOUNT,
            ),
        ));

        System::assert_has_event(RuntimeEvent::ChainBridge(
            chainbridge::Event::<Runtime>::FungibleTransfer(
                GENSHIRO_CHAIN_ID,
                1,
                resources::DOT,
                sp_core::U256::from(TO_RECV_AMOUNT),
                USER_X.encode(),
            ),
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&bridge, asset::DOT),
            INITIAL_AMOUNT
        );
    })
}

#[test]
fn xcm_message_reseived_limited_weight_too_expensive_bridge() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(EqBalances::xcm_toggle(
            RuntimeOrigin::root(),
            Some(XcmMode::Bridge(true))
        ));

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000)
        ));
        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::USDT,
            FixedI64::from_inner(1_000_000_000)
        ));

        let fee_in_usd = crate::fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT) as Balance;
        let fee_in_dot = (fee_in_usd * 10) / 5 - 1;
        let fee_asset = MultiAsset {
            id: AssetId::Concrete(multi::DOT.multi_location),
            fun: Fungibility::Fungible(fee_in_dot),
        };
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![
                    multi_asset_from(TO_RECV_AMOUNT, &multi::DOT),
                    fee_asset.clone(),
                ]
                .into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: fee_asset,
                weight_limit: Limited(XCM_MSG_WEIGHT),
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::DOT)].into(),
                ),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: BRIDGE_MODULE_ID.into_account_truncating(),
                    }),
                },
            },
        ]);

        assert!(
            matches!(
                dbg!(XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
                    xcm_origins::RELAY,
                    rcvd_xcm_message.clone(),
                    hash_xcm(rcvd_xcm_message),
                    XcmWeight::MAX,
                    XcmWeight::MAX
                )),
                Outcome::Incomplete(_, XcmError::TooExpensive)
            ),
            "Should result with XcmError::TooExpensive"
        );
    })
}

#[test]
fn xcm_message_received_unlimited_weight() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::balance::EqCurrency as _;

        System::set_block_number(1);

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000)
        ));
        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::USDT,
            FixedI64::from_inner(1_000_000_000)
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT
        );

        let fee_in_usd = crate::fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT) as Balance;
        let fee_in_dot = (fee_in_usd * 10) / 5;
        let fee_asset = MultiAsset {
            id: AssetId::Concrete(multi::DOT.multi_location),
            fun: Fungibility::Fungible(2 * fee_in_dot),
        };
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![
                    multi_asset_from(TO_RECV_AMOUNT, &multi::DOT),
                    fee_asset.clone(),
                ]
                .into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: fee_asset,
                weight_limit: Unlimited,
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::DOT)].into(),
                ),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: USER_X.into(),
                    }),
                },
            },
        ]);

        assert!(matches!(
            XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
                xcm_origins::RELAY,
                rcvd_xcm_message.clone(),
                hash_xcm(rcvd_xcm_message),
                XcmWeight::MAX,
                XcmWeight::MAX
            ),
            Outcome::Complete(_),
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT + TO_RECV_AMOUNT
        );

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::USDT),
            INITIAL_AMOUNT
        );

        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![multi_asset_from(TO_RECV_AMOUNT + fee_in_usd, &multi::USDT)].into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(2 * fee_in_usd, &multi::USDT),
                weight_limit: Unlimited,
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::USDT)].into(),
                ),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: USER_X.into(),
                    }),
                },
            },
        ]);

        let execute_result = XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
            xcm_origins::dot::PARACHAIN_STATEMINT,
            rcvd_xcm_message.clone(),
            hash_xcm(rcvd_xcm_message),
            XcmWeight::MAX,
            XcmWeight::MAX,
        );
        println!("{:?}", execute_result);
        assert!(matches!(execute_result, Outcome::Complete(_),));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::USDT),
            INITIAL_AMOUNT + TO_RECV_AMOUNT
        );
    })
}

#[test]
fn xcm_message_reseived_limited_weight_ok() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::balance::EqCurrency as _;

        System::set_block_number(1);

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000)
        ));
        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::USDT,
            FixedI64::from_inner(1_000_000_000)
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT
        );

        let fee_in_usd = crate::fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT) as Balance;
        let fee_in_dot = (fee_in_usd * 10) / 5;
        let fee_asset = MultiAsset {
            id: AssetId::Concrete(multi::DOT.multi_location),
            fun: Fungibility::Fungible(2 * fee_in_dot),
        };
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![
                    multi_asset_from(TO_RECV_AMOUNT, &multi::DOT),
                    fee_asset.clone(),
                ]
                .into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: fee_asset,
                weight_limit: Limited(XCM_MSG_WEIGHT),
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::DOT)].into(),
                ),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: USER_X.into(),
                    }),
                },
            },
        ]);

        assert!(matches!(
            XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
                xcm_origins::RELAY,
                rcvd_xcm_message.clone(),
                hash_xcm(rcvd_xcm_message),
                XcmWeight::MAX,
                XcmWeight::MAX
            ),
            Outcome::Complete(_),
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::DOT),
            INITIAL_AMOUNT + TO_RECV_AMOUNT
        );
    })
}

#[test]
fn xcm_message_reseived_limited_weight_too_expensive() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000)
        ));
        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::USDT,
            FixedI64::from_inner(1_000_000_000)
        ));

        let fee_in_usd = crate::fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT) as Balance;
        let fee_in_dot = (fee_in_usd * 10) / 5 - 1;
        let fee_asset = MultiAsset {
            id: AssetId::Concrete(multi::DOT.multi_location),
            fun: Fungibility::Fungible(fee_in_dot),
        };
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![
                    multi_asset_from(TO_RECV_AMOUNT, &multi::DOT),
                    fee_asset.clone(),
                ]
                .into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: fee_asset,
                weight_limit: Limited(XCM_MSG_WEIGHT),
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::DOT)].into(),
                ),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: USER_X.into(),
                    }),
                },
            },
        ]);

        let execute_outcome = XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
            xcm_origins::RELAY,
            rcvd_xcm_message.clone(),
            hash_xcm(rcvd_xcm_message),
            XcmWeight::MAX,
            XcmWeight::MAX,
        );
        assert!(
            matches!(
                execute_outcome,
                Outcome::Incomplete(_, XcmError::TooExpensive),
            ),
            "Should result with XcmError::TooExpensive"
        );
    })
}

#[test]
fn xcm_router_send_error() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        fn send_to(to: impl Into<MultiLocation>) -> DispatchResult {
            EqBalances::do_xcm_transfer(
                USER_X,
                (asset::DOT, TO_SEND_AMOUNT),
                DOT_FEE,
                XcmDestination::Common(to.into()),
            )
        }

        assert_ok!(send_to(MultiLocation::new(
            1,
            X1(AccountId32 {
                id: USER_Y.into(),
                network: None
            })
        )));
        assert_ok!(send_to(MultiLocation::new(
            1,
            X2(
                Parachain(2000),
                AccountId32 {
                    id: USER_Y.into(),
                    network: None
                }
            )
        )));

        sp_io::storage::start_transaction();
        assert_err!(
            send_to(MultiLocation::new(
                1,
                X2(
                    Parachain(666),
                    AccountId32 {
                        id: USER_Y.into(),
                        network: None
                    }
                )
            )),
            eq_balances::Error::<Runtime>::XcmSend
        );
        let h = frame_support::storage_root(frame_support::StateVersion::V1);

        sp_io::storage::rollback_transaction();

        // storages differ only in XcmMessageSendError event
        assert_ne!(
            h,
            frame_support::storage_root(frame_support::StateVersion::V1)
        );
        System::deposit_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmMessageSendError(
                xcm::latest::SendError::ExceedsMaxMessageSize,
            ),
        ));

        assert_eq!(
            h,
            frame_support::storage_root(frame_support::StateVersion::V1)
        );
    });
}

#[test]
fn xcm_barrier() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        let mut error_xcm_message: Vec<Instruction<()>> = vec![
            ReceiveTeleportedAsset(vec![multi_asset_from(TO_SEND_AMOUNT, &multi::DOT)].into()),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(0u32, &multi::DOT),
                weight_limit: Unlimited,
            },
        ];
        let mut rcvd_xcm_message: Vec<Instruction<()>> = vec![
            ReserveAssetDeposited(vec![multi_asset_from(TO_SEND_AMOUNT, &multi::DOT)].into()),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(0u32, &multi::DOT),
                weight_limit: Unlimited,
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_SEND_AMOUNT, &multi::DOT)].into(),
                ),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: BRIDGE_MODULE_ID.into_account_truncating(),
                    }),
                },
            },
        ];
        let max_weight = XcmWeight::from_parts(1_000_000_000, 0);
        let mut weight_credit = XcmWeight::zero();

        assert_ok!(crate::Barrier::should_execute(
            &xcm_origins::RELAY,
            &mut rcvd_xcm_message,
            max_weight,
            &mut weight_credit,
        ));

        assert_ok!(crate::Barrier::should_execute(
            &xcm_origins::dot::PARACHAIN_STATEMINT,
            &mut rcvd_xcm_message,
            max_weight,
            &mut weight_credit,
        ));

        assert_noop!(
            crate::Barrier::should_execute(
                &MultiLocation {
                    parents: 1,
                    interior: X1(Parachain(1_000_000)),
                },
                &mut rcvd_xcm_message,
                max_weight,
                &mut weight_credit,
            ),
            ProcessMessageError::Unsupported
        );

        assert_noop!(
            crate::Barrier::should_execute(
                &xcm_origins::RELAY,
                &mut error_xcm_message[..],
                max_weight,
                &mut weight_credit,
            ),
            ProcessMessageError::Unsupported
        );
    })
}

#[test]
fn xcm_barrier_self_and_other_reserved() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        fn reserve_asset_deposited(xcm_data: OtherReservedData) -> Vec<Instruction<()>> {
            vec![
                ReserveAssetDeposited(vec![multi_asset_from(TO_SEND_AMOUNT, &xcm_data)].into()),
                ClearOrigin,
                BuyExecution {
                    fees: multi_asset_from(0u32, &xcm_data),
                    weight_limit: Unlimited,
                },
                DepositAsset {
                    assets: MultiAssetFilter::Definite(
                        vec![multi_asset_from(TO_SEND_AMOUNT, &xcm_data)].into(),
                    ),

                    beneficiary: MultiLocation {
                        parents: 0,
                        interior: X1(AccountId32 {
                            network: None,
                            id: BRIDGE_MODULE_ID.into_account_truncating(),
                        }),
                    },
                },
            ]
        }

        fn withdraw_asset(xcm_data: OtherReservedData) -> Vec<Instruction<()>> {
            vec![
                WithdrawAsset(vec![multi_asset_from(TO_SEND_AMOUNT, &xcm_data)].into()),
                ClearOrigin,
                BuyExecution {
                    fees: multi_asset_from(0u32, &xcm_data),
                    weight_limit: Unlimited,
                },
                DepositAsset {
                    assets: MultiAssetFilter::Definite(
                        vec![multi_asset_from(TO_SEND_AMOUNT, &xcm_data)].into(),
                    ),
                    beneficiary: MultiLocation {
                        parents: 0,
                        interior: X1(AccountId32 {
                            network: None,
                            id: BRIDGE_MODULE_ID.into_account_truncating(),
                        }),
                    },
                },
            ]
        }

        let max_weight = XcmWeight::from_parts(1_000_000_000, 0);
        let mut weight_credit = XcmWeight::zero();

        assert_ok!(Barrier::should_execute(
            &xcm_origins::dot::PARACHAIN_STATEMINT,
            &mut reserve_asset_deposited(multi::DOT)[..], // place holder
            max_weight,
            &mut weight_credit
        ));
        assert_noop!(
            Barrier::should_execute(
                &xcm_origins::dot::PARACHAIN_STATEMINT,
                &mut withdraw_asset(multi::DOT)[..], // place holder
                max_weight,
                &mut weight_credit
            ),
            ProcessMessageError::Unsupported
        );

        assert_noop!(
            Barrier::should_execute(
                &xcm_origins::dot::PARACHAIN_STATEMINT,
                &mut reserve_asset_deposited(multi::EQ)[..],
                max_weight,
                &mut weight_credit
            ),
            ProcessMessageError::Unsupported
        );
        assert_ok!(Barrier::should_execute(
            &xcm_origins::dot::PARACHAIN_STATEMINT,
            &mut withdraw_asset(multi::EQ)[..],
            max_weight,
            &mut weight_credit
        ));
    });
}

#[test]
fn eq_matches_fungible() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        // DOT TOKEN
        let multi_asset = multi_asset_from(1_000_000_000_u64, &multi::DOT);
        assert_eq!(
            EqAssets::matches_fungible(&multi_asset),
            Some((asset::DOT, 1_000_000_000_u64)),
        );

        // // MOONBASE TOKEN
        // let multi_asset = multi_asset_from(2_000_000_000_u64, &multi::MOVR);
        // assert_eq!(
        //     EqAssets::matches_fungible(&multi_asset),
        //     Some((asset::MOVR, 2_000_000_000_u64)),
        // );

        // // PARALLEL TOKEN
        // let multi_asset = multi_asset_from(3_000_000_000_u64, &multi::HKO);
        // assert_eq!(
        //     EqAssets::matches_fungible(&multi_asset),
        //     Some((asset::HKO, 3_000_000_000_u64)),
        // );

        // UNKNOWN TOKEN
        let multi_asset = MultiAsset {
            id: AssetId::Concrete(MultiLocation {
                parents: 1,
                interior: X1(Parachain(999)),
            }),
            fun: Fungibility::Fungible(3_000_000_000_000),
        };
        assert_eq!(
            EqAssets::matches_fungible(&multi_asset),
            Option::<(Asset, Balance)>::None,
        );

        // NON FUNGIBLE TOKEN
        let multi_asset = MultiAsset {
            id: AssetId::Concrete(multi::USDT.multi_location),
            fun: Fungibility::NonFungible(().into()),
        };
        assert_eq!(
            EqAssets::matches_fungible(&multi_asset),
            Option::<(Asset, Balance)>::None,
        );
    })
}

#[test]
fn buy_weight_with_no_price_fallback() {
    parachain_test_ext().unwrap().execute_with(|| {
        EqAssets::do_update_asset(
            eq_primitives::asset::DOT,
            None,
            None,
            None,
            None,
            Some(AssetXcmData::OtherReserved(multi::DOT)),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let mut payment = xcm_executor::Assets::new();
        payment
            .fungible
            .insert(AssetId::Concrete(MultiLocation::parent()), 469_417_452 - 1);

        let mut trader = crate::EqTrader::new();
        assert_noop!(
            trader.buy_weight(XCM_MSG_WEIGHT, payment),
            XcmError::TooExpensive
        );

        let mut payment = xcm_executor::Assets::new();
        payment
            .fungible
            .insert(AssetId::Concrete(MultiLocation::parent()), 469_417_452);

        let mut trader = crate::EqTrader::new();
        assert_ok!(
            trader.buy_weight(XCM_MSG_WEIGHT, payment),
            xcm_executor::Assets::new()
        );
    })
}

#[test]
fn buy_weight_with_no_assets() {
    parachain_test_ext().unwrap().execute_with(|| {
        EqAssets::do_update_asset(
            eq_primitives::asset::DOT,
            None,
            None,
            None,
            None,
            Some(AssetXcmData::None),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let mut payment = xcm_executor::Assets::new();
        payment
            .fungible
            .insert(AssetId::Concrete(MultiLocation::parent()), 469_417_452);

        let mut trader = crate::EqTrader::new();
        assert_noop!(
            trader.buy_weight(XCM_MSG_WEIGHT, payment),
            XcmError::AssetNotFound
        );
    })
}

#[test]
fn buy_weight_too_expensive() {
    parachain_test_ext().unwrap().execute_with(|| {
        EqAssets::do_update_asset(
            eq_primitives::asset::DOT,
            None,
            None,
            None,
            None,
            Some(AssetXcmData::OtherReserved(multi::DOT)),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000)
        ));

        let mut payment = xcm_executor::Assets::new();

        let payment_value = fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT) - 1;

        payment
            .fungible
            .insert(AssetId::Concrete(MultiLocation::parent()), payment_value);

        let mut trader = crate::EqTrader::new();

        assert_noop!(
            trader.buy_weight(XCM_MSG_WEIGHT, payment),
            XcmError::TooExpensive,
        );
    })
}

#[test]
fn buy_weight_unknown_asset_as_fee() {
    parachain_test_ext().unwrap().execute_with(|| {
        EqAssets::do_update_asset(
            eq_primitives::asset::DOT,
            None,
            None,
            None,
            None,
            Some(AssetXcmData::OtherReserved(multi::DOT)),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000)
        ));

        let mut payment = xcm_executor::Assets::new();

        payment.fungible.insert(
            AssetId::Concrete(MultiLocation::new(1, X1(Parachain(1234)))),
            1_000_000_000_000_000_000,
        );

        assert_eq!(
            crate::fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT),
            100_000_000
        );
        let mut trader = crate::EqTrader::new();
        assert_noop!(
            trader.buy_weight(XCM_MSG_WEIGHT, payment),
            XcmError::AssetNotFound,
        );
    });
}

#[test]
fn buy_weight_ok() {
    parachain_test_ext().unwrap().execute_with(|| {
        EqAssets::do_update_asset(
            eq_primitives::asset::DOT,
            None,
            None,
            None,
            None,
            Some(AssetXcmData::OtherReserved(multi::DOT)),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Oracle::set_price(
            RuntimeOrigin::signed(USER_X),
            asset::DOT,
            FixedI64::from_inner(5_000_000_000)
        ));

        let mut payment = xcm_executor::Assets::new();

        payment.fungible.insert(
            AssetId::Concrete(MultiLocation::parent()),
            (100_000_000 * 10) / 5,
        );

        assert_eq!(
            crate::fee::XcmWeightToFee::weight_to_fee(&XCM_MSG_WEIGHT),
            100_000_000
        );
        let mut trader = crate::EqTrader::new();
        assert_eq!(
            trader.buy_weight(XCM_MSG_WEIGHT, payment),
            Ok(xcm_executor::Assets::new())
        );
    });
}

#[test]
fn expected_fees() {
    use xcm_executor::traits::WeightBounds;

    let xcm = Xcm::<()>(vec![ClearOrigin; 4]);

    for i in 1..=4 {
        let weight = crate::Weigher::weight(&mut Xcm::<RuntimeCall>(vec![ClearOrigin; i])).unwrap();
        assert_eq!(
            crate::fee::XcmWeightToFee::weight_to_fee(&weight),
            25_000_000 * i as u128
        );
    }
}

#[test]
fn parachain_sovereign_accounts() {
    // SS58:    5Dt6dpkWPwLaH4BBCKJwjiWrFVAGyYk3tLUabvyn4v7KtESG
    // HEX:     0x506172656e740000000000000000000000000000000000000000000000000000
    let polkadot_acc = AccountId::decode(&mut TrailingZeroInput::new(b"Parent")).unwrap();
    println!("Polkadot relay");
    println!("ss58: {}", polkadot_acc);
    println!("hex:  0x{:?}", polkadot_acc);
    println!();

    // SS58:    5Eg2fntLDxSsJAuwHC7GpsPcPCW1NidnVuuueRNigVuRwREd
    // HEX:     0x7369626cdb070000000000000000000000000000000000000000000000000000
    let equilibrium_acc: AccountId = Sibling::from(EQ_PARACHAIN_ID).into_account_truncating();
    println!("Equilibrium as sibling");
    println!("ss58: {}", equilibrium_acc);
    println!("hex:  0x{:?}", equilibrium_acc);
    println!();

    // SS58:    5Ec4AhPX9EFyAaBnjCABiE3o8iphfJagSGGUPTrDomss5bxQ
    // HEX:     0x70617261db070000000000000000000000000000000000000000000000000000
    let equilibrium_acc: AccountId = ParaId::from(EQ_PARACHAIN_ID).into_account_truncating();
    println!("Equilibrium as child");
    println!("ss58: {}", equilibrium_acc);
    println!("hex:  0x{:?}", equilibrium_acc);
    println!();

    // SS58:    5Eg2fnshks6aHYtoYbmVvQMDEY5C4XtFeteTR9awyHqaFUvV
    // HEX:     0x7369626c25080000000000000000000000000000000000000000000000000000
    let parallel_acc: AccountId = Sibling::from(2012).into_account_truncating();
    println!("Parallel as sibling");
    println!("ss58: {}", parallel_acc);
    println!("hex:  0x{:?}", parallel_acc);
    println!();

    // SS58:    5Ec4AhNtg8ug9xAezbpQom1Pz4PtM7q9bF12AC4T6Zp1PoCB
    // HEX:     0x7061726125080000000000000000000000000000000000000000000000000000
    let parallel_acc: AccountId = ParaId::from(2012).into_account_truncating();
    println!("Parallel as child");
    println!("ss58: {}", parallel_acc);
    println!("hex:  0x{:?}", parallel_acc);
    println!();

    // SS58:    5Eg2fntJ27qsari4FGrGhrMqKFDRnkNSR6UshkZYBGXmSuC8
    // HEX:     0x7369626cd0070000000000000000000000000000000000000000000000000000
    let acala_acc: AccountId = Sibling::from(2000).into_account_truncating();
    println!("Acala as sibling");
    println!("ss58: {}", acala_acc);
    println!("hex:  0x{:?}", acala_acc);
    println!();

    // SS58:    5Ec4AhPUwPeyTFyuhGuBbD224mY85LKLMSqSSo33JYWCazU4
    // HEX:     0x70617261d0070000000000000000000000000000000000000000000000000000
    let acala_acc: AccountId = ParaId::from(2000).into_account_truncating();
    println!("Acala as child");
    println!("ss58: {}", acala_acc);
    println!("hex:  0x{:?}", acala_acc);
    println!();

    // SS58:    5Eg2fntJpc4bfLRHXnmBUav7hms6NC6vowNAEBDnYSfeMPCw
    // HEX:     0x7369626cd4070000000000000000000000000000000000000000000000000000
    let moonbeam_acc: AccountId = Sibling::from(2004).into_account_truncating();
    println!("Moonbeam as sibling");
    println!("ss58: {}", moonbeam_acc);
    println!("hex:  0x{:?}", moonbeam_acc);
    println!();

    // SS58:    5Ec4AhPVjsshXjh8ynp6MwaJTJBnen3pkHiiyDhHfie5VWkN
    // HEX:     0x70617261d4070000000000000000000000000000000000000000000000000000
    let moonbeam_acc: AccountId = ParaId::from(2004).into_account_truncating();
    println!("Moonbeam as child");
    println!("ss58: {}", moonbeam_acc);
    println!("hex:  0x{:?}", moonbeam_acc);
    println!();

    let cursed: AccountId = Sibling::from(666).into_account_truncating();
    println!("ss58: {}", cursed);
    println!("hex:  0x{:?}", cursed);
}

#[test]
fn eq_bridge_recipient_encoder() {
    let para_id: u32 = 2000;
    let account_id: [u8; 32] =
        hex_literal::hex!("c2636483b8eb649b283db08dde646b60ba6da8eb7138a5275910eaa9e140fe17");

    print!("0x");
    for byte in (para_id, AccountType::Id32(account_id)).encode() {
        print!("{:02x}", byte);
    }
    println!();
}
