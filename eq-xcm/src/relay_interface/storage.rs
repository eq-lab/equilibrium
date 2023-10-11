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

use codec::{Decode, Encode};
use cumulus_primitives_core::{
    relay_chain::{self, BlakeTwo256},
    PersistedValidationData,
};
use frame_support::storage::unhashed;
use hex_literal::hex;
use sp_runtime::traits::HashingFor;
use sp_state_machine::{Backend, TrieBackendBuilder};
use sp_std::prelude::*;
use sp_trie::{HashDBT, StorageProof, EMPTY_PREFIX};

const LOG_TARGET: &'static str = "relay_db";

pub mod known_keys {
    use super::*;

    pub const STAKING_ACTIVE_ERA: &'static [u8] =
        &hex!("5f3e4907f716ac89b6347d15ececedca487df464e44a534ba6b0cbb32407b587");

    pub const STAKING_CURRENT_ERA: &'static [u8] =
        &hex!("5f3e4907f716ac89b6347d15ececedca0b6a45321efae92aea15e0740ec7afe7");

    pub fn system_account_maybe_derivative(
        acc: sp_runtime::AccountId32,
        maybe_index: Option<u16>,
    ) -> Vec<u8> {
        const SYSTEM_ACCOUNT_PREFIX: [u8; 32] =
            hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da9");
        let acc = if let Some(index) = maybe_index {
            // Derived account as in `frame_utility`
            (b"modlpy/utilisuba", acc, index).using_encoded(sp_io::hashing::blake2_256)
        } else {
            acc.into()
        };
        let acc_hash = sp_io::hashing::blake2_128(&acc);

        [&SYSTEM_ACCOUNT_PREFIX[..], &acc_hash[..], &acc[..]].concat()
    }

    pub fn staking_ledger_maybe_derivative(
        acc: sp_runtime::AccountId32,
        maybe_index: Option<u16>,
    ) -> Vec<u8> {
        const STAKING_LEDGER_PREFIX: [u8; 32] =
            hex_literal::hex!("5f3e4907f716ac89b6347d15ececedca422adb579f1dbf4f3886c5cfa3bb8cc4");
        let acc = if let Some(index) = maybe_index {
            // Derived account as in `frame_utility`
            (b"modlpy/utilisuba", acc, index).using_encoded(sp_io::hashing::blake2_256)
        } else {
            acc.into()
        };
        let acc_hash = sp_io::hashing::blake2_128(&acc);

        [&STAKING_LEDGER_PREFIX[..], &acc_hash[..], &acc[..]].concat()
    }
}

#[derive(
    Encode, Decode, Clone, Copy, PartialEq, Eq, sp_core::RuntimeDebug, scale_info::TypeInfo,
)]
pub enum RelayChainStorageError {
    NoValidationData,
    NoStateProof,
    RootMismatch,
    NotFound,
}

pub fn create_relay_backend() -> Result<impl Backend<BlakeTwo256>, RelayChainStorageError> {
    const VALIDATION_DATA_KEY: &'static [u8] =
        &hex!("45323df7cc47150b3930e2666b0aa313d422e17d2affdce4a912d187a734dd67");
    const RELAY_STATE_PROOF_KEY: &'static [u8] =
        &hex!("45323df7cc47150b3930e2666b0aa3138399ce59b27ca884649213623132836d");

    let relay_parent_storage_root = unhashed::get::<PersistedValidationData>(VALIDATION_DATA_KEY)
        .ok_or_else(|| {
            log::error!(target: LOG_TARGET, "No validation data is provided");
            RelayChainStorageError::NoValidationData
        })?
        .relay_parent_storage_root;
    let relay_state_proof =
        unhashed::get::<StorageProof>(RELAY_STATE_PROOF_KEY).ok_or_else(|| {
            log::error!(target: LOG_TARGET, "No relay state proof is provided");
            RelayChainStorageError::NoStateProof
        })?;

    let relay_db = relay_state_proof.into_memory_db::<HashingFor<relay_chain::Block>>();
    if !relay_db.contains(&relay_parent_storage_root, EMPTY_PREFIX) {
        log::error!(target: LOG_TARGET, "Relay state root mismatch",);
        return Err(RelayChainStorageError::RootMismatch);
    }

    Ok(TrieBackendBuilder::new(relay_db, relay_parent_storage_root).build())
}

pub fn get_raw_with(
    backend: &impl Backend<BlakeTwo256>,
    key: impl AsRef<[u8]>,
) -> Result<Vec<u8>, RelayChainStorageError> {
    backend.storage(key.as_ref()).ok().flatten().ok_or_else(|| {
        log::error!(
            target: LOG_TARGET,
            "Value is not found for corresponding key. key: {:?}",
            key.as_ref(),
        );
        RelayChainStorageError::NotFound
    })
}

pub fn get_raw(key: impl AsRef<[u8]>) -> Result<Vec<u8>, RelayChainStorageError> {
    get_raw_with(&create_relay_backend()?, key)
}

pub fn get_with<T: Decode>(
    backend: &impl Backend<BlakeTwo256>,
    key: impl AsRef<[u8]>,
) -> Result<Option<T>, RelayChainStorageError> {
    let raw = get_raw_with(backend, key.as_ref())?;
    let maybe_value = Decode::decode(&mut &raw[..])
        .map_err(|e| {
            log::error!(
                target: LOG_TARGET,
                "Failed to decode: {:?}. key: {:?}, raw: {:?}",
                e,
                key.as_ref(),
                raw,
            );
        })
        .ok();

    Ok(maybe_value)
}

pub fn get<T: Decode>(key: impl AsRef<[u8]>) -> Result<Option<T>, RelayChainStorageError> {
    get_with(&create_relay_backend()?, key)
}
