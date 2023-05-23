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

use xcm::latest::{Junction::*, Junctions::*};
use xcm::v1::MultiLocation;

pub const RELAY: MultiLocation = MultiLocation {
    parents: 1,
    interior: Here,
};

// Polkadot parachains
pub mod dot {
    use super::*;

    pub const PARACHAIN_STATEMINT: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(1000)),
    };
    pub const PARACHAIN_ACALA: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2000)),
    };
    pub const PARACHAIN_MOONBEAM: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2004)),
    };
    pub const PARACHAIN_PARALLEL: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2012)),
    };
    pub const PARACHAIN_INTERLAY: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2032)),
    };
    pub const PARACHAIN_ASTAR: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2006)),
    };
    pub const PARACHAIN_BIFROST: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2030)),
    };
    pub const PARACHAIN_CRUST: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2008)),
    };
    pub const PARACHAIN_PHALA: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2035)),
    };

    pub const PARACHAIN_LITENTRY: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2013)),
    };

    pub const PARACHAIN_POLKADEX: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2040)),
    };

    pub const PARACHAIN_COMPOSABLE:  MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2019)),
    };
}

// Kusama parachains
pub mod ksm {
    use super::*;

    pub const PARACHAIN_KARURA: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2000)),
    };
    pub const PARACHAIN_MOONRIVER: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2023)),
    };
    pub const PARACHAIN_HEIKO: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2085)),
    };
    pub const PARACHAIN_KINTSUGI: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2092)),
    };
    pub const PARACHAIN_SHIDEN: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2007)),
    };
    pub const PARACHAIN_BIFROST: MultiLocation = MultiLocation {
        parents: 1,
        interior: X1(Parachain(2001)),
    };
}
