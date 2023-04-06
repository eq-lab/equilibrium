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

use crate::mock::{new_test_ext, ModuleWhitelists, Test};
use crate::CheckWhitelisted;
use frame_support::assert_ok;

#[test]
fn add_whitelist() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;
        let refs_before = frame_system::Pallet::<Test>::providers(&account_id);

        let in_whitelist = ModuleWhitelists::in_whitelist(&account_id);
        assert_eq!(in_whitelist, false);

        assert_ok!(ModuleWhitelists::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id
        ));

        let in_whitelist = ModuleWhitelists::in_whitelist(&account_id);
        assert_eq!(in_whitelist, true);
        assert!(frame_system::Pallet::<Test>::providers(&account_id) == refs_before + 1);
    });
}

#[test]
fn remove_from_whitelist() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;

        assert_ok!(ModuleWhitelists::add_to_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id
        ));

        let refs_before = frame_system::Pallet::<Test>::providers(&account_id);

        let in_whitelist = ModuleWhitelists::in_whitelist(&account_id);
        assert_eq!(in_whitelist, true);

        assert_ok!(ModuleWhitelists::remove_from_whitelist(
            frame_system::RawOrigin::Root.into(),
            account_id
        ));

        let in_whitelist = ModuleWhitelists::in_whitelist(&account_id);
        assert_eq!(in_whitelist, false);
        assert!(frame_system::Pallet::<Test>::providers(&account_id) == refs_before - 1);
    });
}
