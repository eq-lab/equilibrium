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

pub use codec::Encode;
pub use core::marker::PhantomData;
pub use eq_rate::ed25519::AuthorityId as EqRateId;
pub use eq_xcm::ParaId;
pub use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
pub use sc_service::{ChainType, Properties};
pub use serde::{Deserialize, Serialize};
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
pub use sp_core::{sr25519, Pair, Public};
pub use sp_runtime::{
    traits::{AccountIdConversion, IdentifyAccount, Verify},
    AccountId32, FixedPointNumber, MultiSignature, Permill,
};
pub use std::time::SystemTime;
pub use substrate_fixed::types::I64F64;
pub use xcm::v3::{Junction::*, Junctions::*, MultiLocation};

#[cfg(feature = "with-eq-runtime")]
pub mod equilibrium;
#[cfg(feature = "with-gens-runtime")]
pub mod genshiro;

const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

// Note this is the URL for the telemetry server
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

pub type RawChainSpec = sc_service::GenericChainSpec<(), Extensions>;

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
    /// The relay chain of the Parachain.
    pub relay_chain: String,
    /// The id of the Parachain.
    pub para_id: u32,
}

impl Extensions {
    /// Try to get the extension from the given `ChainSpec`.
    pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
        sc_chain_spec::get_extension(chain_spec.extensions())
    }
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <MultiSignature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId32
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate an authority key for Aura
pub fn authority_keys_from_seed(s: &str) -> (AccountId32, AccountId32, AuraId, EqRateId) {
    (
        get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", s)),
        get_account_id_from_seed::<sr25519::Public>(s),
        get_from_seed::<AuraId>(s),
        get_from_seed::<EqRateId>(s),
    )
}

/// Can be called for a `Configuration` to check if it is a configuration for
/// the `Equilibrium` network.
pub trait IdentifyVariant {
    /// Returns `true` if this is a configuration for the `Equilibeium` network.
    fn is_equilibrium(&self) -> bool;

    /// Returns `true` if this is a configuration for the `Genshiro` network.
    fn is_genshiro(&self) -> bool;
}

impl IdentifyVariant for Box<dyn sc_service::ChainSpec> {
    fn is_equilibrium(&self) -> bool {
        self.id().starts_with("equilibrium")
    }

    fn is_genshiro(&self) -> bool {
        self.id().starts_with("genshiro")
    }
}
