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

use sp_runtime::offchain::storage::{StorageRetrievalError, StorageValueRef};

const ID_KEY: &[u8] = b"exec_id";
const LOCK_KEY: &[u8] = b"lock";
const EXEC_ID_KEY: &[u8] = b"execution-id/";

pub enum LockedExecResult {
    Locked,
    Executed,
}

pub fn accure_lock<F>(prefix: &[u8], f: F) -> LockedExecResult
where
    F: Fn(),
{
    let lock_key = [prefix, LOCK_KEY].concat();
    let mut lock_storage = StorageValueRef::persistent(&lock_key);

    let exec_id_opt = StorageValueRef::persistent(EXEC_ID_KEY).get();
    if let Ok(Some(exec_id)) = exec_id_opt {
        let id_key = [prefix, ID_KEY].concat();
        let id_storage = StorageValueRef::persistent(&id_key);
        let need_to_clear_lock = id_storage.mutate(
            |id: Result<Option<[u8; 32]>, StorageRetrievalError>| match id {
                Ok(Some(val)) => {
                    if val != exec_id {
                        // new id we need to clear lock because of first launch
                        Ok(exec_id)
                    } else {
                        Err(())
                    }
                }
                _ => {
                    // no id we need to clear lock because of first launch
                    Ok(exec_id)
                }
            },
        );

        if need_to_clear_lock.is_ok() {
            lock_storage.clear();
        }
    }

    let can_process = lock_storage.mutate(
        |is_locked: Result<Option<bool>, StorageRetrievalError>| match is_locked {
            Ok(Some(true)) => Err(()),
            _ => Ok(true),
        },
    );

    match can_process {
        Ok(true) => {
            f();
            lock_storage.clear();
            LockedExecResult::Executed
        }
        _ => LockedExecResult::Locked,
    }
}
