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

use codec::Decode;
use cumulus_primitives_core::{
    relay_chain::{well_known_keys as relay_well_known_keys, Hash as PHash},
    PersistedValidationData,
};
use cumulus_primitives_parachain_inherent::ParachainInherentData;
use cumulus_relay_chain_interface::RelayChainInterface;
use eq_xcm::ParaId;
use polkadot_primitives::HrmpChannelId;

const LOG_TARGET: &str = "parachain-inherent";

/// Collect the relevant relay chain state in form of a proof for putting it into the validation
/// data inherent.
async fn collect_relay_storage_proof(
    relay_chain_interface: &impl RelayChainInterface,
    para_id: ParaId,
    relay_parent: PHash,
    extra_keys: impl IntoIterator<Item = Vec<u8>>,
) -> Option<sp_state_machine::StorageProof> {
    let ingress_channels = relay_chain_interface
        .get_storage_by_key(
            relay_parent,
            &relay_well_known_keys::hrmp_ingress_channel_index(para_id),
        )
        .await
        .map_err(|e| {
            tracing::error!(
                target: LOG_TARGET,
                relay_parent = ?relay_parent,
                error = ?e,
                "Cannot obtain the hrmp ingress channel."
            )
        })
        .ok()?;

    let ingress_channels = ingress_channels
        .map(|raw| <Vec<ParaId>>::decode(&mut &raw[..]))
        .transpose()
        .map_err(|e| {
            tracing::error!(
                target: LOG_TARGET,
                error = ?e,
                "Cannot decode the hrmp ingress channel index.",
            )
        })
        .ok()?
        .unwrap_or_default();

    let egress_channels = relay_chain_interface
        .get_storage_by_key(
            relay_parent,
            &relay_well_known_keys::hrmp_egress_channel_index(para_id),
        )
        .await
        .map_err(|e| {
            tracing::error!(
                target: LOG_TARGET,
                error = ?e,
                "Cannot obtain the hrmp egress channel.",
            )
        })
        .ok()?;

    let egress_channels = egress_channels
        .map(|raw| <Vec<ParaId>>::decode(&mut &raw[..]))
        .transpose()
        .map_err(|e| {
            tracing::error!(
                target: LOG_TARGET,
                error = ?e,
                "Cannot decode the hrmp egress channel index.",
            )
        })
        .ok()?
        .unwrap_or_default();

    let mut relevant_keys = Vec::new();
    relevant_keys.push(relay_well_known_keys::CURRENT_BLOCK_RANDOMNESS.to_vec());
    relevant_keys.push(relay_well_known_keys::ONE_EPOCH_AGO_RANDOMNESS.to_vec());
    relevant_keys.push(relay_well_known_keys::TWO_EPOCHS_AGO_RANDOMNESS.to_vec());
    relevant_keys.push(relay_well_known_keys::CURRENT_SLOT.to_vec());
    relevant_keys.push(relay_well_known_keys::ACTIVE_CONFIG.to_vec());
    relevant_keys.push(relay_well_known_keys::dmq_mqc_head(para_id));
    relevant_keys.push(relay_well_known_keys::relay_dispatch_queue_size(para_id));
    relevant_keys.push(relay_well_known_keys::hrmp_ingress_channel_index(para_id));
    relevant_keys.push(relay_well_known_keys::hrmp_egress_channel_index(para_id));
    relevant_keys.push(relay_well_known_keys::upgrade_go_ahead_signal(para_id));
    relevant_keys.push(relay_well_known_keys::upgrade_restriction_signal(para_id));
    relevant_keys.extend(ingress_channels.into_iter().map(|sender| {
        relay_well_known_keys::hrmp_channels(HrmpChannelId {
            sender,
            recipient: para_id,
        })
    }));
    relevant_keys.extend(egress_channels.into_iter().map(|recipient| {
        relay_well_known_keys::hrmp_channels(HrmpChannelId {
            sender: para_id,
            recipient,
        })
    }));
    relevant_keys.extend(extra_keys);

    relay_chain_interface
        .prove_read(relay_parent, &relevant_keys)
        .await
        .map_err(|e| {
            tracing::error!(
                target: LOG_TARGET,
                relay_parent = ?relay_parent,
                error = ?e,
                "Cannot obtain read proof from relay chain.",
            );
        })
        .ok()
}

/// Create the [`ParachainInherentData`] at the given `relay_parent`.
///
/// Returns `None` if the creation failed.
pub async fn create_at(
    relay_parent: PHash,
    relay_chain_interface: &impl RelayChainInterface,
    validation_data: &PersistedValidationData,
    para_id: ParaId,
    extra_keys: impl IntoIterator<Item = Vec<u8>>,
) -> Option<ParachainInherentData> {
    let relay_chain_state =
        collect_relay_storage_proof(relay_chain_interface, para_id, relay_parent, extra_keys)
            .await?;

    let downward_messages = relay_chain_interface
        .retrieve_dmq_contents(para_id, relay_parent)
        .await
        .map_err(|e| {
            tracing::error!(
                target: LOG_TARGET,
                relay_parent = ?relay_parent,
                error = ?e,
                "An error occured during requesting the downward messages.",
            );
        })
        .ok()?;
    let horizontal_messages = relay_chain_interface
        .retrieve_all_inbound_hrmp_channel_contents(para_id, relay_parent)
        .await
        .map_err(|e| {
            tracing::error!(
                target: LOG_TARGET,
                relay_parent = ?relay_parent,
                error = ?e,
                "An error occured during requesting the inbound HRMP messages.",
            );
        })
        .ok()?;

    Some(ParachainInherentData {
        downward_messages,
        horizontal_messages,
        validation_data: validation_data.clone(),
        relay_chain_state,
    })
}
