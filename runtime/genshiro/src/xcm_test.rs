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

use crate::*;
use assets::*;
use codec::Decode;
use eq_primitives::{
    asset::{self, Asset, AssetType, AssetXcmData, OtherReservedData},
    balance::{EqCurrency, XcmDestination},
    balance_number::EqFixedU128,
    xcm_origins as origins, XcmMode,
};
use eq_xcm::*;
use frame_support::{
    assert_err, assert_ok,
    traits::{GenesisBuild, ProcessMessageError},
    weights::WeightToFee,
    PalletId,
};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{
    traits::{AccountIdConversion, TrailingZeroInput},
    Percent,
};
use sp_std::sync::{Arc, RwLock};
use xcm::v3::{
    AssetId, Error as XcmError, ExecuteXcm, Fungibility,
    Instruction::{self, *},
    MultiAsset, MultiAssetFilter, MultiAssets, MultiLocation, Outcome, SendError, SendResult,
    WeightLimit::*,
    Xcm, XcmHash,
};
use xcm_executor::traits::{Convert as _, ShouldExecute as _, WeightTrader as _};

use polkadot_parachain::primitives::Sibling;
use sp_arithmetic::FixedI64;

type AccountPublic = <Signature as Verify>::Signer;
const GENS_PARACHAIN_ID: u32 = 2024;

#[macro_export]
macro_rules! assert_matches_event {
    ($( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {{
        assert!(crate::System::events().into_iter().any(|record| matches!(record.event, $( $pattern )|+ $( if $guard )?)))
    }}
}

thread_local! {
    static XCM_MESSAGE_CONTAINER: Arc<RwLock<Vec<(MultiLocation, xcm::latest::Xcm<()>)>>> = Arc::new(RwLock::new(Vec::new()));
}

pub fn xcm_message_container() -> Vec<(MultiLocation, xcm::latest::Xcm<()>)> {
    XCM_MESSAGE_CONTAINER
        .try_with(|a| a.clone())
        .unwrap()
        .try_read()
        .map(|lock| (&*lock).clone())
        .unwrap_or_default()
}

fn hash_xcm<T>(msg: Xcm<T>) -> XcmHash {
    msg.using_encoded(sp_io::hashing::blake2_256)
}

pub struct XcmRouterMock;
impl xcm::latest::SendXcm for XcmRouterMock {
    type Ticket = (MultiLocation, Xcm<()>);

    fn validate(
        destination: &mut Option<MultiLocation>,
        message: &mut Option<Xcm<()>>,
    ) -> SendResult<Self::Ticket> {
        match &destination {
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
        if let Ok(mut vec) = XCM_MESSAGE_CONTAINER
            .try_with(|a| a.clone())
            .unwrap()
            .try_write()
        {
            vec.push((dest, msg.clone()));
            Ok(hash_xcm(msg))
        } else {
            Err(SendError::NotApplicable)
        }
    }
}

mod multi {
    use super::*;

    pub const KSM: OtherReservedData = OtherReservedData {
        multi_location: MultiLocation {
            parents: 1,
            interior: Here,
        },
        decimals: 12,
    };
    pub const MOVR: OtherReservedData = OtherReservedData {
        multi_location: MultiLocation {
            parents: 1,
            interior: X2(Parachain(2023), PalletInstance(3)),
        },
        decimals: 18,
    };
    pub const HKO: OtherReservedData = OtherReservedData {
        multi_location: MultiLocation {
            parents: 1,
            interior: X1(Parachain(2085)),
        },
        decimals: 12,
    };
}

mod resources {
    pub const GENS: chainbridge::ResourceId = [0; 32];
    pub const EQD: chainbridge::ResourceId = [1; 32];
    pub const KSM: chainbridge::ResourceId = [2; 32];
    pub const MOVR: chainbridge::ResourceId = [3; 32];
    pub const HKO: chainbridge::ResourceId = [4; 32];
    pub const USDT: chainbridge::ResourceId = [5; 32];
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
                asset::GENS.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                AssetXcmData::SelfReserved.encode(),
                Permill::from_rational(2u32, 5u32),
                1,
                AssetType::Native,
                false,
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
                2,
                AssetType::Synthetic,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::KSM.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                AssetXcmData::OtherReserved(multi::KSM).encode(),
                Permill::from_rational(2u32, 5u32),
                3,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::MOVR.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                AssetXcmData::OtherReserved(multi::MOVR).encode(),
                Permill::from_rational(2u32, 5u32),
                4,
                AssetType::Physical,
                true,
                Percent::one(),
                Permill::one(),
            ),
            (
                asset::HKO.get_id(),
                EqFixedU128::from(0),
                FixedI64::from(0),
                Permill::zero(),
                Permill::zero(),
                AssetXcmData::OtherReserved(multi::HKO).encode(),
                Permill::from_rational(2u32, 5u32),
                5,
                AssetType::Physical,
                true,
                Percent::one(),
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
                    (INITIAL_AMOUNT, asset::GENS.get_id()),
                    (INITIAL_AMOUNT, asset::KSM.get_id()),
                    (INITIAL_AMOUNT, asset::MOVR.get_id()),
                    (INITIAL_AMOUNT, asset::HKO.get_id()),
                ],
            ),
            (
                USER_X,
                vec![
                    (INITIAL_AMOUNT, asset::GENS.get_id()),
                    (INITIAL_AMOUNT, asset::KSM.get_id()),
                    (INITIAL_AMOUNT, asset::MOVR.get_id()),
                    (INITIAL_AMOUNT, asset::HKO.get_id()),
                ],
            ),
            (
                USER_Y,
                vec![
                    (INITIAL_AMOUNT, asset::GENS.get_id()),
                    (INITIAL_AMOUNT, asset::KSM.get_id()),
                    (INITIAL_AMOUNT, asset::MOVR.get_id()),
                    (INITIAL_AMOUNT, asset::HKO.get_id()),
                ],
            ),
            (
                USER_Z,
                vec![
                    (INITIAL_AMOUNT, asset::GENS.get_id()),
                    (INITIAL_AMOUNT, asset::KSM.get_id()),
                    (INITIAL_AMOUNT, asset::MOVR.get_id()),
                    (INITIAL_AMOUNT, asset::HKO.get_id()),
                ],
            ),
        ],
        is_transfers_enabled: true,
        is_xcm_enabled: Some(XcmMode::Xcm(true)),
    }
    .assimilate_storage(&mut storage)?;

    eq_bridge::GenesisConfig::<Runtime> {
        _runtime: PhantomData,
        resources: vec![
            (resources::KSM, asset::KSM),
            (resources::MOVR, asset::MOVR),
            (resources::HKO, asset::HKO),
        ],
        minimum_transfer_amount: vec![
            (GENSHIRO_CHAIN_ID, resources::KSM, 1),
            (GENSHIRO_CHAIN_ID, resources::USDT, 1),
            (GENSHIRO_CHAIN_ID, resources::GENS, 1),
            (GENSHIRO_CHAIN_ID, resources::EQD, 1),
        ],
        enabled_withdrawals: vec![
            (resources::KSM, vec![GENSHIRO_CHAIN_ID]),
            (resources::USDT, vec![GENSHIRO_CHAIN_ID]),
            (resources::GENS, vec![GENSHIRO_CHAIN_ID]),
            (resources::EQD, vec![GENSHIRO_CHAIN_ID]),
        ],
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
        parachain_id: ParaId::from(2024),
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
const XCM_MSG_WEIGHT: XcmWeight = XcmWeight::from_parts(4_000_000, 0);

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

        System::set_block_number(1);
        assert_ok!(EqBalances::xcm_toggle(
            RuntimeOrigin::root(),
            Some(XcmMode::Bridge(true))
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&BRIDGE_MODULE_ID.into_account_truncating(), asset::KSM),
            INITIAL_AMOUNT
        );

        assert_ok!(EqBridge::xcm_transfer(
            frame_system::RawOrigin::Signed(BRIDGE_MODULE_ID.into_account_truncating()).into(),
            USER_X.encode(),
            TO_SEND_AMOUNT,
            resources::KSM,
        ));

        let ksm_location = MultiLocation::here();

        System::assert_has_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                origins::RELAY,
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
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&BRIDGE_MODULE_ID.into_account_truncating(), asset::KSM),
            INITIAL_AMOUNT
        );
    })
}

#[test]
fn xcm_transfer_native() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::{balance::EqCurrency as _, AccountType};
        use sp_std::convert::TryFrom as _;

        System::set_block_number(1);

        let relay_acc = AccountId::decode(&mut TrailingZeroInput::new(b"Parent")).unwrap();

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&relay_acc, asset::KSM), 0);

        assert_ok!(<EqBalances as EqCurrency<_, _>>::xcm_transfer(
            &USER_X,
            asset::KSM,
            TO_SEND_AMOUNT,
            XcmDestination::Native(AccountType::try_from(USER_Y.encode()).unwrap()),
        ));

        System::assert_has_event(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                origins::RELAY,
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
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT - TO_SEND_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&relay_acc, asset::KSM), 0);
    })
}

#[test]
fn xcm_transfer_native_gens_is_preserved() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::{balance::EqCurrency as _, AccountType};
        use sp_std::convert::TryFrom as _;

        System::set_block_number(1);

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::GENS),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::GENS),
            INITIAL_AMOUNT
        );

        assert_err!(
            <EqBalances as EqCurrency<_, _>>::xcm_transfer(
                &USER_X,
                asset::GENS,
                TO_SEND_AMOUNT,
                XcmDestination::Native(AccountType::try_from(USER_Y.encode()).unwrap()),
            ),
            eq_balances::Error::<Runtime>::XcmInvalidDestination
        );

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::GENS),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::GENS),
            INITIAL_AMOUNT
        );
    })
}

#[test]
fn xcm_transfer_self_reserved() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::balance::EqCurrency as _;

        System::set_block_number(1);

        let parallel_acc = Sibling::from(2085).into_account_truncating();

        assert_eq!(
            <EqAssets as asset::AssetGetter>::get_asset_data(&asset::GENS)
                .unwrap()
                .asset_xcm_data,
            AssetXcmData::SelfReserved,
        );
        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::GENS),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::GENS),
            INITIAL_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&parallel_acc, asset::GENS), 0);

        let to = MultiLocation {
            parents: 1,
            interior: X2(
                Parachain(2085),
                AccountId32 {
                    id: USER_Y.into(),
                    network: None,
                },
            ),
        };

        let send_amount = TO_SEND_AMOUNT * 3;

        assert_ok!(<EqBalances as EqCurrency<_, _>>::xcm_transfer(
            &USER_X,
            asset::GENS,
            send_amount,
            XcmDestination::Common(to),
        ));

        assert_eq!(
            xcm_message_container(),
            vec![(
                origins::ksm::PARACHAIN_HEIKO,
                Xcm(vec![
                    ReserveAssetDeposited(
                        vec![MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X1(Parachain(GENS_PARACHAIN_ID))
                            )),
                            fun: Fungibility::Fungible(send_amount as u128),
                        }]
                        .into()
                    ),
                    ClearOrigin,
                    BuyExecution {
                        fees: MultiAsset {
                            id: AssetId::Concrete(MultiLocation::new(
                                1,
                                X1(Parachain(GENS_PARACHAIN_ID))
                            )),
                            fun: Fungibility::Fungible(2 * 527_992_396_909),
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

        assert_matches_event!(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                origins::ksm::PARACHAIN_HEIKO, // target_chain
                to
            )
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::GENS),
            INITIAL_AMOUNT - send_amount
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::GENS),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&parallel_acc, asset::GENS),
            send_amount
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
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::KSM),
            INITIAL_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&relay_acc, asset::KSM), 0);

        let to = MultiLocation {
            parents: 1,
            interior: X1(AccountId32 {
                id: USER_Y.into(),
                network: None,
            }),
        };
        assert_ok!(<EqBalances as EqCurrency<_, _>>::xcm_transfer(
            &USER_X,
            asset::KSM,
            TO_SEND_AMOUNT,
            XcmDestination::Common(to),
        ));

        // println!("{:#?}", System::events());

        assert_matches_event!(RuntimeEvent::EqBalances(
            eq_balances::Event::<Runtime>::XcmTransfer(
                origins::RELAY, // target_chain = Kusama
                to
            )
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT - TO_SEND_AMOUNT
        );
        assert_eq!(
            EqBalances::total_balance(&USER_Y, asset::KSM),
            INITIAL_AMOUNT
        );
        assert_eq!(EqBalances::total_balance(&relay_acc, asset::KSM), 0);
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

        let weight = crate::BaseXcmWeight::get().saturating_mul(4);
        let fee = 0;
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(vec![multi_asset_from(TO_RECV_AMOUNT + fee, &multi::KSM)].into()),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(fee, &multi::KSM),
                weight_limit: Limited(XCM_MSG_WEIGHT),
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::KSM)].into(),
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

        let execute_result = XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
            origins::RELAY,
            rcvd_xcm_message.clone(),
            hash_xcm(rcvd_xcm_message),
            weight,
            weight,
        );
        assert!(
            matches!(
                execute_result,
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

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT
        );

        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![multi_asset_from(TO_RECV_AMOUNT + FEE_AMOUNT, &multi::KSM)].into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(FEE_AMOUNT, &multi::KSM),
                weight_limit: Unlimited,
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::KSM)].into(),
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
            origins::RELAY,
            rcvd_xcm_message.clone(),
            hash_xcm(rcvd_xcm_message),
            XcmWeight::MAX,
            XcmWeight::MAX,
        );
        assert!(matches!(execute_outcome, Outcome::Complete(_),));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT + TO_RECV_AMOUNT
        );

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::MOVR),
            INITIAL_AMOUNT
        );

        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![multi_asset_from(TO_RECV_AMOUNT + FEE_AMOUNT, &multi::MOVR)].into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(FEE_AMOUNT, &multi::MOVR),
                weight_limit: Unlimited,
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::MOVR)].into(),
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
                origins::ksm::PARACHAIN_MOONRIVER,
                rcvd_xcm_message.clone(),
                hash_xcm(rcvd_xcm_message),
                XcmWeight::MAX,
                XcmWeight::MAX
            ),
            Outcome::Complete(_)
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::MOVR),
            INITIAL_AMOUNT + TO_RECV_AMOUNT
        );
    })
}

#[test]
fn xcm_message_reseived_limited_weight_ok() {
    parachain_test_ext().unwrap().execute_with(|| {
        use eq_primitives::balance::EqCurrency as _;

        System::set_block_number(1);

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT
        );

        let weight = crate::BaseXcmWeight::get().saturating_mul(4);
        let fee_amount = crate::fee::XcmWeightToFee::weight_to_fee(&weight) as Balance;
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![multi_asset_from(TO_RECV_AMOUNT + fee_amount, &multi::KSM)].into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(fee_amount, &multi::KSM),
                weight_limit: Limited(XCM_MSG_WEIGHT),
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::KSM)].into(),
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
                origins::RELAY,
                rcvd_xcm_message.clone(),
                hash_xcm(rcvd_xcm_message),
                XcmWeight::MAX,
                XcmWeight::MAX
            ),
            Outcome::Complete(_)
        ));

        assert_eq!(
            EqBalances::total_balance(&USER_X, asset::KSM),
            INITIAL_AMOUNT + TO_RECV_AMOUNT
        );
    })
}

#[test]
fn xcm_message_reseived_limited_weight_too_expensive() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        let weight = XCM_MSG_WEIGHT;
        let fee_amount: Balance =
            eq_utils::balance_from_xcm(crate::fee::XcmWeightToFee::weight_to_fee(&weight) - 1, 12)
                .unwrap();
        let rcvd_xcm_message = Xcm::<RuntimeCall>(vec![
            ReserveAssetDeposited(
                vec![multi_asset_from(TO_RECV_AMOUNT + fee_amount, &multi::KSM)].into(),
            ),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(fee_amount, &multi::KSM),
                weight_limit: Limited(XCM_MSG_WEIGHT),
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_RECV_AMOUNT, &multi::KSM)].into(),
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

        assert!(
            matches!(
                XcmExecutor::<XcmConfig>::execute_xcm_in_credit(
                    origins::RELAY,
                    rcvd_xcm_message.clone(),
                    hash_xcm(rcvd_xcm_message),
                    XcmWeight::MAX,
                    XcmWeight::MAX
                ),
                Outcome::Incomplete(_, XcmError::TooExpensive)
            ),
            "Should result with XcmError::TooExpensive"
        );
    })
}

#[test]
fn xcm_barrier() {
    parachain_test_ext().unwrap().execute_with(|| {
        let mut error_xcm_message: Vec<Instruction<()>> = vec![
            ReceiveTeleportedAsset(vec![multi_asset_from(TO_SEND_AMOUNT, &multi::KSM)].into()),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(0u32, &multi::KSM),
                weight_limit: Unlimited,
            },
        ];
        let mut rcvd_xcm_message: Vec<Instruction<()>> = vec![
            ReserveAssetDeposited(vec![multi_asset_from(TO_SEND_AMOUNT, &multi::KSM)].into()),
            ClearOrigin,
            BuyExecution {
                fees: multi_asset_from(0u32, &multi::KSM),
                weight_limit: Unlimited,
            },
            DepositAsset {
                assets: MultiAssetFilter::Definite(
                    vec![multi_asset_from(TO_SEND_AMOUNT, &multi::KSM)].into(),
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
            &origins::RELAY,
            &mut rcvd_xcm_message,
            max_weight,
            &mut weight_credit,
        ));

        assert_ok!(crate::Barrier::should_execute(
            &origins::ksm::PARACHAIN_MOONRIVER,
            &mut rcvd_xcm_message,
            max_weight,
            &mut weight_credit,
        ));

        assert_ok!(crate::Barrier::should_execute(
            &origins::ksm::PARACHAIN_HEIKO,
            &mut rcvd_xcm_message,
            max_weight,
            &mut weight_credit,
        ));

        assert_err!(
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

        assert_err!(
            crate::Barrier::should_execute(
                &origins::RELAY,
                &mut error_xcm_message,
                max_weight,
                &mut weight_credit,
            ),
            ProcessMessageError::Unsupported
        );
    })
}

#[test]
fn eq_matches_fungible() {
    parachain_test_ext().unwrap().execute_with(|| {
        System::set_block_number(1);

        // KUSAMA TOKEN
        let multi_asset = multi_asset_from(1_000_000_000_u64, &multi::KSM);
        assert_eq!(
            EqAssets::matches_fungible(&multi_asset),
            Some((asset::KSM, 1_000_000_000_u64)),
        );

        // MOONBASE TOKEN
        let multi_asset = multi_asset_from(2_000_000_000_u64, &multi::MOVR);
        assert_eq!(
            EqAssets::matches_fungible(&multi_asset),
            Some((asset::MOVR, 2_000_000_000_u64)),
        );

        // PARALLEL TOKEN
        let multi_asset = multi_asset_from(3_000_000_000_u64, &multi::HKO);
        assert_eq!(
            EqAssets::matches_fungible(&multi_asset),
            Some((asset::HKO, 3_000_000_000_u64)),
        );

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
            id: AssetId::Concrete(multi::MOVR.multi_location),
            fun: Fungibility::NonFungible(().into()),
        };
        assert_eq!(
            EqAssets::matches_fungible(&multi_asset),
            Option::<(Asset, Balance)>::None,
        );
    })
}

#[test]
fn buy_weight_with_no_assets() {
    parachain_test_ext().unwrap().execute_with(|| {
        EqAssets::do_update_asset(
            eq_primitives::asset::KSM,
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

        let payment_value = 99_654_685 * 1000;
        let weight = 4_000_000;

        payment
            .fungible
            .insert(AssetId::Concrete(MultiLocation::parent()), payment_value);

        let mut trader = crate::EqTrader::new();
        assert_err!(
            trader.buy_weight(XCM_MSG_WEIGHT, payment),
            XcmError::AssetNotFound,
        );
    })
}

#[test]
fn buy_weight_too_expensive() {
    parachain_test_ext().unwrap().execute_with(|| {
        assert_ok!(EqAssets::do_update_asset(
            eq_primitives::asset::KSM,
            None,
            None,
            None,
            None,
            Some(AssetXcmData::OtherReserved(multi::KSM)),
            None,
            None,
            None,
            None,
            None,
            None,
        ));

        let mut payment = xcm_executor::Assets::new();

        let payment_value = 117_331_640 - 1;

        payment
            .fungible
            .insert(AssetId::Concrete(MultiLocation::parent()), payment_value);

        let mut trader = crate::EqTrader::new();
        assert_err!(
            trader.buy_weight(XCM_MSG_WEIGHT, payment),
            XcmError::TooExpensive,
        );
    })
}

#[test]
fn buy_weight_ok() {
    parachain_test_ext().unwrap().execute_with(|| {
        assert_ok!(EqAssets::do_update_asset(
            eq_primitives::asset::KSM,
            None,
            None,
            None,
            None,
            Some(AssetXcmData::OtherReserved(multi::KSM)),
            None,
            None,
            None,
            None,
            None,
            None,
        ));

        let mut payment = xcm_executor::Assets::new();

        payment
            .fungible
            .insert(AssetId::Concrete(MultiLocation::parent()), 117331640);

        let mut trader = crate::EqTrader::new();
        assert_ok!(
            trader.buy_weight(XCM_MSG_WEIGHT, payment),
            xcm_executor::Assets::new()
        );
    })
}

#[test]
#[ignore]
fn expected_fees() {
    use eq_xcm::fees::*;

    let xcm = Xcm::<()>(vec![ClearOrigin; 4]);

    // assert_eq!(kusama::XcmToFee::convert(xcm()), 165_940_672);
    // assert_eq!(moonbeam::movr::XcmToFee::convert(xcm()), 20_000_000_000_000);
    // assert_eq!(parallel::hko::XcmToFee::convert(xcm()), 2_880_000_000);
    // assert_eq!(parallel::gens::XcmToFee::convert(xcm()), 480_000_000_000);
    // assert_eq!(acala::kar::XcmToFee::convert(xcm()), 6_400_000_000);
    // assert_eq!(acala::kusd::XcmToFee::convert(xcm()), 51_200_000_000);
    // assert_eq!(acala::lksm::XcmToFee::convert(xcm()), 1_280_000_000);
    // assert_eq!(acala::gens::XcmToFee::convert(xcm()), 64_000_000_000);
    // assert_eq!(acala::eqd::XcmToFee::convert(xcm()), 640_000_000);
    assert_eq!(interlay::kbtc::XcmToFee::convert(&xcm), 124);
    assert_eq!(astar::sdn::XcmToFee::convert(&xcm), 4_662_276_356_431_024);
    assert_eq!(astar::gens::XcmToFee::convert(&xcm), 2_800_000);
    assert_eq!(astar::eqd::XcmToFee::convert(&xcm), 2_800_000);
}

#[test]
fn parachain_sovereign_accounts() {
    // GENS:    cZgYFAiY544M6N23GRuvwkB1HMfGXEQhqLdFErabC9Czjqfuc
    // ID:      5Dt6dpkWPwLaH4BBCKJwjiWrFVAGyYk3tLUabvyn4v7KtESG
    // HEX:     0x506172656e740000000000000000000000000000000000000000000000000000
    let kusama_acc = AccountId::decode(&mut TrailingZeroInput::new(b"Parent")).unwrap();
    println!("0x{:?}", kusama_acc);

    // GENS:    cZhLBCgfwVAAk8gqXBxwjLiBxkbDfWGjAESZF6x8SScESqLzK
    // ID:      5Eg2fntNq3AE3iyRxNKjLFhXeR7RFQmNn9narJX2NKM2t3MQ
    // HEX:     0x7369626ce8070000000000000000000000000000000000000000000000000000
    let genshiro_acc: AccountId = Sibling::from(2024).into_account_truncating();
    println!("0x{:?}", genshiro_acc);

    // GENS:    cZhGChbB8QRyr167NdxzeE4r9W7YMnrg4AnuoqzbwZtCsyjUt
    // ID:      5Ec4AhPZkJyKv8FHQNNeDcMiPwS7XziGiW99bLzXVbKU2TBw
    // HEX:     0x70617261e8070000000000000000000000000000000000000000000000000000
    let genshiro_acc: AccountId = ParaId::from(2024).into_account_truncating();
    println!("0x{:?}", genshiro_acc);

    // GENS:    cZhLBCgfGQz76NWktnCPVvrqeLiBSKPr37BR7foCN3aizCrAJ
    // ID:      5Eg2fnshks6aHYtoYbmVvQMDEY5C4XtFeteTR9awyHqaFUvV
    // HEX:     0x7369626c25080000000000000000000000000000000000000000000000000000
    let parallel_acc: AccountId = Sibling::from(2085).into_account_truncating();
    println!("0x{:?}", parallel_acc);

    // GENS:    cZhGChbATLFvCEv2kECSQpDVq6EW8bynw3XmgQqfsArhRMDau
    // ID:      5Ec4AhNtg8ug9xAezbpQom1Pz4PtM7q9bF12AC4T6Zp1PoCB
    // HEX:     0x7061726125080000000000000000000000000000000000000000000000000000
    let parallel_acc: AccountId = ParaId::from(2085).into_account_truncating();
    println!("0x{:?}", parallel_acc);

    // GENS:    cZhLBCgfrgErPfpa9UsUGiJrGRRKg3cLDsPFXxQAxFZRBQFcz
    // ID:      5Eg2fntJ27qsari4FGrGhrMqKFDRnkNSR6UshkZYBGXmSuC8
    // HEX:     0x7369626cd0070000000000000000000000000000000000000000000000000000
    let karura_acc: AccountId = Sibling::from(2000).into_account_truncating();
    println!("0x{:?}", karura_acc);

    // GENS:    cZhGChbB3bWfVYDqzvsXBbfWTAweNLCH7ojc6hSeTNqPcYnAx
    // ID:      5Ec4AhPUwPeyTFyuhGuBbD224mY85LKLMSqSSo33JYWCazU4
    // HEX:     0x70617261d0070000000000000000000000000000000000000000000000000000
    let karura_acc: AccountId = ParaId::from(2000).into_account_truncating();
    println!("0x{:?}", karura_acc);
}
