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

use std::collections::BTreeMap;

use criterion::*;
use eq_primitives::vec_map::VecMap;
use rand::{distributions::Uniform, thread_rng, Rng};

trait Map {
    type Key;
    type Value;

    fn insert(&mut self, key: Self::Key, value: Self::Value) -> Option<Self::Value>;

    fn get<'a>(&'a self, key: &'_ Self::Key) -> Option<&'a Self::Value>;
}

// Heap allocated vector for comparisson with stack allocated VecMap
impl<K: Ord, V> Map for Vec<(K, V)> {
    type Key = K;
    type Value = V;

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(idx) => {
                // SAFETY: `idx` returned by Ok(...) from `binary_search` so it is in range 0..len
                let (_, v) = unsafe { self.get_unchecked_mut(idx) };
                Some(core::mem::replace(v, value))
            }
            Err(idx) => unsafe {
                // SAFETY: `idx` returned by Err(...) from `binary_search` so it is in range 0..=len
                if idx > self.len() {
                    core::hint::unreachable_unchecked();
                }
                self.insert(idx, (key, value));
                None
            },
        }
    }

    fn get<'a>(&'a self, key: &'_ Self::Key) -> Option<&'a Self::Value> {
        let idx = self.binary_search_by(|(k, _)| k.cmp(&key)).ok()?;
        // SAFETY: `idx` returned by Ok(...) from `binary_search` so it is in range 0..len
        let (_, v) = unsafe { self.get_unchecked(idx) };
        Some(v)
    }
}

fn map(c: &mut Criterion) {
    for bits in 0..8 {
        let mut rng = thread_rng();
        let uniform = Uniform::new(0, 1024);

        let size = 1 << bits;

        let mut vec_map = VecMap::<u64, usize>::with_capacity(size + 1);
        let mut heap_map = Vec::<(u64, usize)>::with_capacity(size + 1);
        let mut btree_map = BTreeMap::<u64, usize>::new();

        let mut v = 1;
        let mut contained_k = 0;
        while vec_map.len() < size {
            let k = (&mut rng).sample(&uniform);

            vec_map.insert(k, v);
            Map::insert(&mut heap_map, k, v);
            btree_map.insert(k, v);

            contained_k = k;
            v += 1;
        }
        assert_eq!(vec_map.len(), heap_map.len());
        assert_eq!(vec_map.len(), btree_map.len());

        let k = loop {
            let v = (&mut rng).sample(&uniform);
            if !vec_map.contains_key(&v) {
                break v;
            }
        };

        c.bench_function(&format!("vec::[{size}]::clone"), |b| {
            b.iter(|| {
                let new = black_box(vec_map.clone());
                assert_eq!(new, vec_map);
            })
        });

        c.bench_function(&format!("vec::[{size}]::insert"), |b| {
            b.iter(|| {
                assert_eq!(black_box(vec_map.clone()).insert(k, usize::MAX), None);
            })
        });

        c.bench_function(&format!("vec::[{size}]::get_none"), |b| {
            b.iter(|| {
                assert_eq!(vec_map.get(&k), None);
            })
        });

        c.bench_function(&format!("vec::[{size}]::get_some"), |b| {
            b.iter(|| {
                assert_ne!(vec_map.get(&contained_k), None);
            })
        });

        c.bench_function(&format!("btree::[{size}]::clone"), |b| {
            b.iter(|| {
                let new = black_box(btree_map.clone());
                assert_eq!(new, btree_map);
            })
        });

        c.bench_function(&format!("btree::[{size}]::insert"), |b| {
            b.iter(|| {
                assert_eq!(black_box(btree_map.clone()).insert(k, usize::MAX), None);
            })
        });

        c.bench_function(&format!("btree::[{size}]::get_none"), |b| {
            b.iter(|| {
                assert_eq!(btree_map.get(&k), None);
            })
        });

        c.bench_function(&format!("btree::[{size}]::get_some"), |b| {
            b.iter(|| {
                assert_ne!(btree_map.get(&contained_k), None);
            })
        });
    }
}

criterion_group!(benches, map);
criterion_main!(benches);
