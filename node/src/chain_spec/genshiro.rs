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

use common_runtime::AccountId;
use gens_node_runtime::{
    eq_primitives::{
        asset::{self, AssetType, AssetXcmData, OtherReservedData},
        balance_number::EqFixedU128,
        XcmMode,
    },
    opaque::SessionKeys,
    AuraConfig, CollatorSelectionConfig, EqAssetsConfig, EqBalancesConfig, EqMultisigSudoConfig,
    EqTreasury, EqTreasuryConfig, GenesisConfig, ParachainInfoConfig, PolkadotXcmConfig,
    SessionConfig, SudoConfig, SystemConfig, VestingConfig, WASM_BINARY,
};
use sp_runtime::Percent;

fn session_keys(aura: AuraId, eq_rate: EqRateId) -> SessionKeys {
    SessionKeys { aura, eq_rate }
}

pub const GENSHIRO_PARACHAIN_ID: u32 = 2024;

pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

pub fn development_config() -> ChainSpec {
    // let wasm_binary = WASM_BINARY;

    ChainSpec::from_genesis(
        "Genshiro Development",
        "genshiro-dev",
        ChainType::Development,
        move || {
            testnet_genesis(
                vec![authority_keys_from_seed("Alice")],
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                vec![
                    //get_account_id_from_seed::<sr25519::Public>("Alice"),
                ],
                vec![
                    get_account_id_from_seed::<sr25519::Public>("Alice"),
                    get_account_id_from_seed::<sr25519::Public>("Bob"),
                    get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
                ],
                true,
                GENSHIRO_PARACHAIN_ID.into(),
            )
        },
        vec![],
        None,
        None,
        None,
        Some(get_properties()),
        Extensions {
            relay_chain: "kusama-local".into(), // Polkadot local testnet config (multivalidator Alice + Bob)
            para_id: GENSHIRO_PARACHAIN_ID,
        },
    )
}

pub fn local_testnet_config() -> ChainSpec {
    // let wasm_binary = WASM_BINARY;

    ChainSpec::from_genesis(
        "Genshiro Local Testnet",
        "genshiro-local",
        ChainType::Local,
        move || {
            testnet_genesis(
                vec![
                    authority_keys_from_seed("Alice"),
                    authority_keys_from_seed("Bob"),
                ],
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                vec![
                    get_account_id_from_seed::<sr25519::Public>("Alice"),
                    get_account_id_from_seed::<sr25519::Public>("Bob"),
                ],
                vec![
                    get_account_id_from_seed::<sr25519::Public>("Alice"),
                    get_account_id_from_seed::<sr25519::Public>("Bob"),
                    get_account_id_from_seed::<sr25519::Public>("Charlie"),
                    get_account_id_from_seed::<sr25519::Public>("Dave"),
                    get_account_id_from_seed::<sr25519::Public>("Eve"),
                    get_account_id_from_seed::<sr25519::Public>("Ferdie"),
                    get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
                    chainbridge::MODULE_ID.into_account_truncating(),
                ],
                true,
                GENSHIRO_PARACHAIN_ID.into(),
            )
        },
        vec![],
        None,
        None,
        None,
        Some(get_properties()),
        Extensions {
            relay_chain: "kusama-local".into(),
            para_id: GENSHIRO_PARACHAIN_ID,
        },
    )
}

pub fn mainnet_config() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(&include_bytes!("../../chain-specs/genshiro-mainnet.json")[..])
}

fn testnet_genesis(
    initial_authorities: Vec<(AccountId, AccountId, AuraId, EqRateId)>,
    root_key: AccountId,
    _whitelisted_accounts: Vec<AccountId>,
    endowed_accounts: Vec<AccountId>,
    _enable_println: bool,
    id: ParaId,
) -> GenesisConfig {
    let mut balances: Vec<(_, _, _)> = endowed_accounts
        .iter()
        .cloned()
        .map(|k| (k, 1 << 50, asset::GENS.get_id()))
        .collect();

    balances.push((EqTreasury::account_id(), 1u64 << 50, asset::GENS.get_id()));
    GenesisConfig {
        system: SystemConfig {
            code: WASM_BINARY.unwrap().to_vec(),
        },
        eq_assets: EqAssetsConfig {
            _runtime: PhantomData,
            assets: vec![
                (
                    asset::EQD.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::SelfReserved.encode(),
                    Permill::from_rational(2u32, 5u32),
                    1,
                    AssetType::Synthetic,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::BTC.get_id(),
                    EqFixedU128::saturating_from_rational(1845, 100000),
                    FixedI64::saturating_from_integer(1),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    vec![],
                    Permill::from_rational(2u32, 5u32),
                    2,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::ETH.get_id(),
                    EqFixedU128::saturating_from_rational(5612, 10000),
                    FixedI64::saturating_from_rational(1, 10),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    vec![],
                    Permill::from_rational(2u32, 5u32),
                    3,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::KSM.get_id(),
                    EqFixedU128::saturating_from_rational(331455, 10000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (1, Here).into(),
                        decimals: 10,
                    })
                    .encode(),
                    Permill::from_rational(2u32, 5u32),
                    4,
                    AssetType::Physical,
                    true,
                    Percent::from_rational(95u32, 100u32),
                    Permill::one(),
                ),
                (
                    asset::CRV.get_id(),
                    EqFixedU128::saturating_from_rational(3875969, 10000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    vec![],
                    Permill::from_rational(2u32, 5u32),
                    5,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::EOS.get_id(),
                    EqFixedU128::saturating_from_rational(2123142, 10000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    vec![],
                    Permill::from_rational(2u32, 5u32),
                    6,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::GENS.get_id(),
                    EqFixedU128::saturating_from_integer(25000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::SelfReserved.encode(),
                    Permill::from_rational(2u32, 5u32),
                    u64::MAX,
                    AssetType::Native,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::DAI.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    vec![],
                    Permill::from_rational(2u32, 5u32),
                    7,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::USDT.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (
                            1,
                            X3(Parachain(1000), PalletInstance(50), GeneralIndex(1984)),
                        )
                            .into(),
                        decimals: 6,
                    })
                    .encode(),
                    Permill::from_rational(2u32, 5u32),
                    8,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::USDC.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    vec![],
                    Permill::from_rational(2u32, 5u32),
                    11,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::BUSD.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    vec![],
                    Permill::from_rational(2u32, 5u32),
                    9,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::BNB.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::zero(),
                    Permill::zero(),
                    vec![],
                    Permill::zero(),
                    12,
                    AssetType::Physical,
                    true,
                    Percent::from_rational(85u32, 100u32),
                    Permill::one(),
                ),
                (
                    asset::WBTC.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::zero(),
                    Permill::zero(),
                    vec![],
                    Permill::zero(),
                    13,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::EQ.get_id(),
                    EqFixedU128::saturating_from_rational(47393, 10),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::SelfReserved.encode(),
                    Permill::zero(),
                    100,
                    AssetType::Physical,
                    true,
                    Percent::from_rational(4u32, 10u32),
                    Permill::one(),
                ),
                (
                    asset::KAR.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (
                            1,
                            X2(
                                Parachain(2000),
                                GeneralKey(WeakBoundedVec::force_from(vec![0x00, 0x80], None)),
                            ),
                        )
                            .into(),
                        decimals: 12,
                    })
                    .encode(),
                    Permill::zero(),
                    14,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::AUSD.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (
                            1,
                            X2(
                                Parachain(2000),
                                GeneralKey(WeakBoundedVec::force_from(vec![0x00, 0x81], None)),
                            ),
                        )
                            .into(),
                        decimals: 12,
                    })
                    .encode(),
                    Permill::zero(),
                    15,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::LKSM.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (
                            1,
                            X2(
                                Parachain(2000),
                                GeneralKey(WeakBoundedVec::force_from(vec![0x00, 0x83], None)),
                            ),
                        )
                            .into(),
                        decimals: 12,
                    })
                    .encode(),
                    Permill::zero(),
                    16,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::MOVR.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (1, X2(Parachain(2023), PalletInstance(10))).into(),
                        decimals: 18,
                    })
                    .encode(),
                    Permill::zero(),
                    17,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::HKO.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (
                            1,
                            X2(
                                Parachain(2085),
                                GeneralKey(WeakBoundedVec::force_from(b"HKO".to_vec(), None)),
                            ),
                        )
                            .into(),
                        decimals: 12,
                    })
                    .encode(),
                    Permill::zero(),
                    18,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::KBTC.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (
                            1,
                            X2(
                                Parachain(2092),
                                GeneralKey(WeakBoundedVec::force_from(vec![0x00, 0x0b], None)),
                            ),
                        )
                            .into(),
                        decimals: 8,
                    })
                    .encode(),
                    Permill::zero(),
                    19,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::SDN.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (1, X1(Parachain(2007))).into(),
                        decimals: 18,
                    })
                    .encode(),
                    Permill::zero(),
                    20,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
                (
                    asset::BNC.get_id(),
                    EqFixedU128::saturating_from_integer(1000),
                    FixedI64::saturating_from_rational(1, 100),
                    Permill::from_rational(5u32, 10_000u32),
                    Permill::from_rational(1u32, 1000u32),
                    AssetXcmData::OtherReserved(OtherReservedData {
                        multi_location: (
                            1,
                            X2(
                                Parachain(2001),
                                GeneralKey(WeakBoundedVec::force_from(vec![0x00, 0x01], None)),
                            ),
                        )
                            .into(),
                        decimals: 12,
                    })
                    .encode(),
                    Permill::zero(),
                    21,
                    AssetType::Physical,
                    true,
                    Percent::one(),
                    Permill::one(),
                ),
            ],
        },
        eq_balances: EqBalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, vec![(1 << 50, asset::GENS.get_id())]))
                .collect(),
            is_transfers_enabled: false,
            is_xcm_enabled: Some(XcmMode::Xcm(true)),
        },
        eq_treasury: EqTreasuryConfig { empty: () },
        vesting: VestingConfig::default(),
        aura: AuraConfig {
            authorities: vec![],
        },
        sudo: SudoConfig {
            key: Some(root_key.clone()),
        },
        session: SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| {
                    (
                        x.0.clone(),
                        x.0.clone(),
                        session_keys(x.2.clone(), x.3.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        },
        eq_session_manager: eq_session_manager::GenesisConfig {
            validators: initial_authorities
                .iter()
                .map(|(x, ..)| x.clone())
                .collect(),
        },

        chain_bridge: Default::default(),

        eq_bridge: Default::default(),

        treasury: Default::default(),

        eq_multisig_sudo: EqMultisigSudoConfig {
            keys: vec![root_key.clone()],
            threshold: 1,
        },

        aura_ext: Default::default(),

        parachain_system: Default::default(),

        parachain_info: ParachainInfoConfig { parachain_id: id },

        collator_selection: CollatorSelectionConfig {
            invulnerables: initial_authorities
                .iter()
                .cloned()
                .map(|(acc, _, _, _)| acc)
                .collect(),
            candidacy_bond: gens_node_runtime::EXISTENSIAL_DEPOSIT * 2,
            ..Default::default()
        },

        polkadot_xcm: PolkadotXcmConfig {
            safe_xcm_version: Some(SAFE_XCM_VERSION),
        },

        eq_lending: Default::default(),
    }
}

fn get_properties() -> Properties {
    let mut properties = sc_chain_spec::Properties::new();
    properties.insert("ss58Format".into(), 67.into());
    properties.insert("tokenDecimals".to_string(), 9.into());
    properties.insert("tokenSymbol".to_string(), "TOKEN".into());
    properties
}
