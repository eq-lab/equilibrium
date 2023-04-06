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

use crate::{Config, PalletManager};
use frame_support::traits::EnsureOrigin;

pub struct EnsureManager<T>(T);

impl<T: Config> EnsureOrigin<T::Origin> for EnsureManager<T> {
    type Success = T::AccountId;

    fn try_origin(o: T::Origin) -> Result<Self::Success, T::Origin> {
        use frame_system::RawOrigin;
        use RawOrigin::Signed;
        o.into().and_then(|raw| match raw {
            Signed(ref acc_id) => match <PalletManager<T>>::get() {
                Some(manager_id) => {
                    if manager_id == *acc_id {
                        Ok(manager_id)
                    } else {
                        Err(T::Origin::from(raw))
                    }
                }
                None => Err(T::Origin::from(raw)),
            },
            r => Err(T::Origin::from(r)),
        })
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn successful_origin() -> T::Origin {
        todo!()
    }
}
