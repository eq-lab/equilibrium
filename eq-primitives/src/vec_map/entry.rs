use super::VecMap;
use core::fmt;

pub struct OccupiedEntry<'a, K, V> {
    pub(super) map: &'a mut VecMap<K, V>,
    pub(super) idx: usize,
}

impl<'a, K: fmt::Debug, V: fmt::Debug> fmt::Debug for OccupiedEntry<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (key, value) = &self.map.0[self.idx];
        f.debug_map().entry(key, &Some(value)).finish()
    }
}

impl<K, V> PartialEq for OccupiedEntry<'_, K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx
    }
}

impl<K, V> Eq for OccupiedEntry<'_, K, V> {}

impl<'a, K, V> OccupiedEntry<'a, K, V> {
    pub fn key(&self) -> &K {
        unsafe { &self.map.0.get_unchecked(self.idx).0 }
    }

    pub fn get(&self) -> &V {
        unsafe { &self.map.0.get_unchecked(self.idx).1 }
    }

    pub fn get_mut(&mut self) -> &mut V {
        unsafe { &mut self.map.0.get_unchecked_mut(self.idx).1 }
    }

    pub fn into_mut(self) -> &'a mut V {
        unsafe { &mut self.map.0.get_unchecked_mut(self.idx).1 }
    }

    pub fn insert(&mut self, value: V) -> V {
        core::mem::replace(self.get_mut(), value)
    }

    pub fn remove(self) -> V {
        unsafe {
            if self.idx >= self.map.0.len() {
                core::hint::unreachable_unchecked();
            }
            self.map.0.remove(self.idx).1
        }
    }
}

pub struct VacantEntry<'a, K, V> {
    pub(super) map: &'a mut VecMap<K, V>,
    pub(super) idx: usize,
    pub(super) key: K,
}

impl<'a, K: fmt::Debug, V: fmt::Debug> fmt::Debug for VacantEntry<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entry(&self.key, &Option::<V>::None).finish()
    }
}

impl<K, V> PartialEq for VacantEntry<'_, K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx
    }
}

impl<K, V> Eq for VacantEntry<'_, K, V> {}

impl<'a, K: Ord, V> VacantEntry<'a, K, V> {
    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn into_key(self) -> K {
        self.key
    }

    pub fn insert(self, value: V) -> &'a mut V {
        unsafe {
            if self.idx > self.map.0.len() {
                core::hint::unreachable_unchecked();
            }
            self.map.0.insert(self.idx, (self.key, value));
            &mut self.map.0.get_unchecked_mut(self.idx).1
        }
    }
}

#[derive(Debug)]
pub enum Entry<'a, K: 'a, V: 'a> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K: 'a, V: 'a> Entry<'a, K, V> {
    pub(super) fn occupied(map: &'a mut VecMap<K, V>, idx: usize) -> Self {
        Self::Occupied(OccupiedEntry { map, idx })
    }

    pub(super) fn vacant(map: &'a mut VecMap<K, V>, idx: usize, key: K) -> Self {
        Self::Vacant(VacantEntry { map, idx, key })
    }
}

impl<'a, K: Ord + 'a, V: 'a> PartialEq for Entry<'a, K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.key().eq(other.key())
    }
}

impl<'a, K: Ord + 'a, V: 'a> Eq for Entry<'a, K, V> {}

impl<'a, K: Ord + 'a, V: 'a> PartialOrd for Entry<'a, K, V> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.key().cmp(other.key()))
    }
}

impl<'a, K: Ord + 'a, V: 'a> Ord for Entry<'a, K, V> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.key().cmp(other.key())
    }
}

impl<'a, K: Ord, V> Entry<'a, K, V> {
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default),
        }
    }

    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default()),
        }
    }

    pub fn or_insert_with_key<F: FnOnce(&K) -> V>(self, default: F) -> &'a mut V {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(value)
            }
        }
    }

    pub fn key(&self) -> &K {
        match *self {
            Self::Occupied(ref entry) => entry.key(),
            Self::Vacant(ref entry) => entry.key(),
        }
    }

    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            Self::Occupied(mut entry) => {
                f(entry.get_mut());
                Self::Occupied(entry)
            }
            Self::Vacant(entry) => Self::Vacant(entry),
        }
    }
}

impl<'a, K: Ord, V: Default> Entry<'a, K, V> {
    pub fn or_default(self) -> &'a mut V {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(Default::default()),
        }
    }
}
