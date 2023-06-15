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

use eq_primitives::AccountType;
use sp_std::marker::PhantomData;
use xcm::v3::{Junction::*, Junctions::*, MultiLocation, NetworkId};

/// Extracts the `AccountId32` from the passed `location` if the network matches.
pub struct AccountIdConversion<AccountId>(PhantomData<AccountId>);

impl<AccountId: From<[u8; 32]> + Into<[u8; 32]> + Clone> AccountIdConversion<AccountId> {
    fn account_id(location: MultiLocation) -> Result<AccountId, MultiLocation> {
        match location {
            MultiLocation {
                parents: 0,
                interior:
                    X1(AccountId32 {
                        id,
                        network: None | Some(NetworkId::Kusama), // Kusama here for frontend and init scripts backward compatibility
                    }),
            } => Ok(id.into()),
            _ => Err(location),
        }
    }

    fn multi_location(who: AccountType) -> MultiLocation {
        who.multi_location().into()
    }
}

impl<AccountId: From<[u8; 32]> + Into<[u8; 32]> + Clone>
    xcm_executor::traits::Convert<MultiLocation, AccountId> for AccountIdConversion<AccountId>
{
    fn convert(location: MultiLocation) -> Result<AccountId, MultiLocation> {
        Self::account_id(location)
    }

    fn reverse(who: AccountId) -> Result<MultiLocation, AccountId> {
        Ok(Self::multi_location(AccountType::Id32(who.into())).into())
    }
}

impl<AccountId: From<[u8; 32]> + Into<[u8; 32]> + Clone>
    sp_runtime::traits::Convert<AccountType, MultiLocation> for AccountIdConversion<AccountId>
{
    fn convert(a: AccountType) -> MultiLocation {
        Self::multi_location(a)
    }
}

frame_support::parameter_types! {
    pub const AnyNetwork: Option<NetworkId> = None;
}

/// Converts signed origin to AccountId32 multilocation
pub type LocalOriginToLocation<Origin, AccountId> =
    xcm_builder::SignedToAccountId32<Origin, AccountId, AnyNetwork>;
