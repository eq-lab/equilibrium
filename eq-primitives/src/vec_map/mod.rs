use codec::{Compact, Decode, DecodeLength, Encode, EncodeLike, MaxEncodedLen};
use core::{
    borrow::Borrow,
    convert::TryInto,
    fmt,
    iter::FromIterator,
    ops::{Deref, Index, IndexMut},
};
use scale_info::TypeInfo;

pub(self) const SIZE: usize = 0x20;
pub(self) type Vec<T> = smallvec::SmallVec<[T; SIZE]>;
// extern crate alloc;
// pub(self) type Vec<T> = alloc::vec::Vec<T>;

use self::entry::Entry;

pub mod entry;
pub mod iter;
pub mod macros;
#[cfg(test)]
mod tests;

/// Inner representation of `VecMap` allocated on heap.
/// Could be used as replacement for `VecMap` inside values of another `VecMap` to minimaze stack size.
#[derive(
    Encode,
    Decode,
    Default,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    scale_info::TypeInfo,
)]
#[repr(transparent)]
pub struct SortedVec<T>(sp_std::vec::Vec<T>);

impl<T> SortedVec<T> {
    pub fn into_vec(self) -> sp_std::vec::Vec<T> {
        self.0
    }
}

impl<T> Deref for SortedVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.0.deref()
    }
}

/// Structure that store key-value pairs with ordering by key.
/// Unlike `BTreeMap`, stores pairs in contiguous memory allocated on stack, if contains < 32 elems, or on heap.
/// Since it could be allocated on stack, the following statement holds: size_of VecMap<K, V> = 32 * size_of (K, V).
/// Thus, storing values with large stack size (e.g. VecMap itself) may quickly lead to `stack overflow`.
///
/// Bad example: size_of VecMap<u32, VecMap<u32, u64>> = 1056 * 4 + 1024 * 8 = 5504 B ~ 5 kB
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VecMap<K, V>(pub(crate) Vec<(K, V)>);

#[cfg(feature = "std")]
impl<K: serde::Serialize, V: serde::Serialize> serde::Serialize for VecMap<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self)
    }
}

#[cfg(feature = "std")]
impl<'de, K: Ord + serde::Deserialize<'de>, V: serde::Deserialize<'de>> serde::Deserialize<'de>
    for VecMap<K, V>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MapVisitor<K, V> {
            marker: core::marker::PhantomData<VecMap<K, V>>,
        }

        impl<'de, K, V> serde::de::Visitor<'de> for MapVisitor<K, V>
        where
            K: Ord + serde::Deserialize<'de>,
            V: serde::Deserialize<'de>,
        {
            type Value = VecMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }

            #[inline]
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut values = VecMap::new();
                while let Some((k, v)) = map.next_entry()? {
                    assert!(matches!(values.insert(k, v), None), "KV already added");
                }
                Ok(values)
            }
        }

        deserializer.deserialize_map(MapVisitor {
            marker: core::marker::PhantomData,
        })
    }
}

impl<K, V> VecMap<K, V> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    pub fn reserve_exact(&mut self, additional: usize) {
        self.0.reserve_exact(additional);
    }

    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn into_keys(self) -> impl Iterator<Item = K> {
        iter::IntoKeys {
            iter: self.0.into_iter(),
        }
    }

    pub fn into_values(self) -> impl Iterator<Item = V> {
        iter::IntoValues {
            iter: self.0.into_iter(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        iter::Iter {
            iter: self.0.iter(),
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        iter::Keys {
            iter: self.0.iter(),
        }
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        iter::Values {
            iter: self.0.iter(),
        }
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        iter::IterMut {
            iter: self.0.iter_mut(),
        }
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        iter::ValuesMut {
            iter: self.0.iter_mut(),
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<K: Ord, V> VecMap<K, V> {
    pub(self) fn find<Q: ?Sized>(&self, key: &Q) -> Result<usize, usize>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.0.binary_search_by(|(k, _)| k.borrow().cmp(key))
    }

    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        let idx = self.find(key).ok()?;
        // SAFETY: `idx` returned by Ok(...) from `binary_search` so it is in range 0..len
        let (_, v) = unsafe { self.0.get_unchecked(idx) };
        Some(v)
    }

    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        let idx = self.find(key).ok()?;
        // SAFETY: `idx` returned by Ok(...) from `binary_search` so it is in range 0..len
        let (_, v) = unsafe { self.0.get_unchecked_mut(idx) };
        Some(v)
    }

    pub fn get_key_value<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        let idx = self.find(key).ok()?;
        // SAFETY: `idx` returned by Ok(...) from `binary_search` so it is in range 0..len
        let (k, v) = unsafe { self.0.get_unchecked(idx) };
        Some((&*k, v))
    }

    pub fn get_key_value_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<(&K, &mut V)>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        let idx = self.find(key).ok()?;
        // SAFETY: `idx` returned by Ok(...) from `binary_search` so it is in range 0..len
        let (k, v) = unsafe { self.0.get_unchecked_mut(idx) };
        Some((&*k, v))
    }

    pub fn first_key(&self) -> Option<&K> {
        self.0.first().map(|(k, _)| k)
    }

    pub fn first_value(&self) -> Option<&V> {
        self.0.first().map(|(_, v)| v)
    }

    pub fn first_value_mut(&mut self) -> Option<&mut V> {
        self.0.first_mut().map(|(_, v)| v)
    }

    pub fn first_key_value(&self) -> Option<(&K, &V)> {
        self.0.first().map(|(k, v)| (k, v))
    }

    pub fn first_key_value_mut(&mut self) -> Option<(&K, &mut V)> {
        self.0.first_mut().map(|(k, v)| (&*k, v))
    }

    pub fn pop_first(&mut self) -> Option<(K, V)> {
        if self.0.len() > 0 {
            Some(self.0.remove(0))
        } else {
            None
        }
    }

    pub fn last_key(&self) -> Option<&K> {
        self.0.last().map(|(k, _)| k)
    }

    pub fn last_value(&self) -> Option<&V> {
        self.0.last().map(|(_, v)| v)
    }

    pub fn last_value_mut(&mut self) -> Option<&mut V> {
        self.0.last_mut().map(|(_, v)| v)
    }

    pub fn last_key_value(&mut self) -> Option<(&K, &V)> {
        self.0.last().map(|(k, v)| (k, v))
    }

    pub fn last_key_value_mut(&mut self) -> Option<(&K, &mut V)> {
        self.0.last_mut().map(|(k, v)| (&*k, v))
    }

    pub fn pop_last(&mut self) -> Option<(K, V)> {
        self.0.pop()
    }

    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.find(key).is_ok()
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.find(&key) {
            Ok(idx) => {
                // SAFETY: `idx` returned by Ok(...) from `binary_search` so it is in range 0..len
                let (_, v) = unsafe { self.0.get_unchecked_mut(idx) };
                Some(core::mem::replace(v, value))
            }
            Err(idx) => unsafe {
                // SAFETY: `idx` returned by Err(...) from `binary_search` so it is in range 0..=len
                if idx > self.0.len() {
                    core::hint::unreachable_unchecked();
                }
                self.0.insert(idx, (key, value));
                None
            },
        }
    }

    /// Caller must ensure that `key` is greater than any key presented in map
    /// Otherwise `value` will be discarded
    pub fn push(&mut self, key: K, value: V) -> bool {
        if let Some((k, _)) = self.0.last() {
            if &key <= k {
                return false;
            }
        }
        self.0.push((key, value));
        true
    }

    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        let idx = self.find(key).ok()?;
        unsafe {
            // SAFETY: `idx` returned by Ok(...) from `binary_search` so it is in range 0..len
            if idx >= self.0.len() {
                core::hint::unreachable_unchecked();
            }
            Some(self.0.remove(idx).1)
        }
    }

    pub fn remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        let idx = self.find(key).ok()?;
        unsafe {
            // SAFETY: `idx` returned by Ok(...) from `binary_search` so it is in range 0..len
            if idx >= self.0.len() {
                core::hint::unreachable_unchecked();
            }
            Some(self.0.remove(idx))
        }
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        K: Ord,
        F: FnMut(&K, &mut V) -> bool,
    {
        return self.0.retain(|(k, v)| f(k, v));
    }

    pub fn append(&mut self, other: &mut Self) {
        // if keys are equal - take value from other
        *self = core::mem::take(self).merge(core::mem::take(other), |_, m| match m {
            MergeOption::Left(l) => Some(l),
            MergeOption::Right(r) | MergeOption::Both(_, r) => Some(r),
        });
    }

    /// Implementation assume, that all ellements in VecMap are arranged in ascending order
    /// Which is always true due to methods that preserve ordering
    pub fn merge(
        self,
        other: Self,
        mut merge_fn: impl for<'k> FnMut(&'k K, MergeOption<V, V>) -> Option<V>,
    ) -> Self {
        let max_len = self.len() + other.len();
        let (mut left, mut right) = (self.0.into_iter(), other.0.into_iter());
        let mut new = Vec::with_capacity(max_len);
        let (mut maybe_l, mut maybe_r) = (left.next(), right.next());
        loop {
            match (maybe_l, maybe_r) {
                (None, None) => {
                    break;
                }
                (None, Some(r)) => {
                    new.extend(
                        Some(r).into_iter().chain(right).filter_map(|(k, v)| {
                            merge_fn(&k, MergeOption::Right(v)).map(|v| (k, v))
                        }),
                    );
                    break;
                }
                (Some(l), None) => {
                    new.extend(
                        Some(l).into_iter().chain(left).filter_map(|(k, v)| {
                            merge_fn(&k, MergeOption::Left(v)).map(|v| (k, v))
                        }),
                    );
                    break;
                }
                (Some(l), Some(r)) => match l.0.cmp(&r.0) {
                    core::cmp::Ordering::Less => {
                        if let Some(new_v) = merge_fn(&l.0, MergeOption::Left(l.1)) {
                            new.push((l.0, new_v));
                        }
                        maybe_l = left.next();
                        maybe_r = Some(r);
                    }
                    core::cmp::Ordering::Equal => {
                        if let Some(new_v) = merge_fn(&r.0, MergeOption::Both(l.1, r.1)) {
                            new.push((r.0, new_v));
                        }
                        maybe_l = left.next();
                        maybe_r = right.next();
                    }
                    core::cmp::Ordering::Greater => {
                        if let Some(new_v) = merge_fn(&r.0, MergeOption::Right(r.1)) {
                            new.push((r.0, new_v));
                        }
                        maybe_l = Some(l);
                        maybe_r = right.next();
                    }
                },
            }
        }

        Self(new)
    }

    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        match self.find(&key) {
            Ok(idx) => Entry::occupied(self, idx),
            Err(idx) => Entry::vacant(self, idx, key),
        }
    }

    pub fn split_off<Q: ?Sized + Ord>(&mut self, key: &Q) -> Self
    where
        K: Borrow<Q>,
    {
        pub fn split_off<K, V>(this: &mut Vec<(K, V)>, at: usize) -> Vec<(K, V)> {
            if at == 0 {
                // the new vector can take over the original buffer and avoid the copy
                return core::mem::replace(this, Vec::with_capacity(this.capacity()));
            } else if at == this.len() {
                return Vec::new();
            }

            let other_len = this.len() - at;
            let mut other = Vec::with_capacity(other_len);

            // Unsafely `set_len` and copy items to `other`.
            unsafe {
                this.set_len(at);
                other.set_len(other_len);

                core::ptr::copy_nonoverlapping(
                    this.as_ptr().add(at),
                    other.as_mut_ptr(),
                    other.len(),
                );

                this.shrink_to_fit();
            }
            other
        }

        match self.find(key) {
            Ok(idx) | Err(idx) => Self(split_off(&mut self.0, idx)),
        }
    }
}

impl<K, V> Default for VecMap<K, V> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<K: EncodeLike<KL>, KL: Encode, V: EncodeLike<VL>, VL: Encode>
    EncodeLike<sp_std::vec::Vec<(KL, VL)>> for VecMap<K, V>
{
}
impl<K: EncodeLike<KL>, KL: Encode, V: EncodeLike<VL>, VL: Encode>
    EncodeLike<sp_std::collections::btree_map::BTreeMap<KL, VL>> for VecMap<K, V>
{
}
impl<K: EncodeLike<KL>, KL: Encode, V: EncodeLike<VL>, VL: Encode> EncodeLike<VecMap<KL, VL>>
    for sp_std::collections::btree_map::BTreeMap<K, V>
{
}
impl<K: EncodeLike<KL>, KL: Encode, V: EncodeLike<VL>, VL: Encode> EncodeLike<VecMap<KL, VL>>
    for VecMap<K, V>
{
}

impl<K: Encode, V: Encode> Encode for VecMap<K, V> {
    fn size_hint(&self) -> usize {
        self.0.size_hint()
    }

    fn encode_to<T: codec::Output + ?Sized>(&self, dest: &mut T) {
        Compact(self.len() as u32).encode_to(dest);
        for (k, v) in &self.0 {
            (k, v).encode_to(dest);
        }
    }
}

impl<K: Decode, V: Decode> Decode for VecMap<K, V> {
    fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
        let result = if let Ok(Compact(len)) = Compact::<u32>::decode(input) {
            let mut new = Self::with_capacity(len as usize + 1);
            input.descend_ref()?;
            for _ in 0..len {
                new.0.push(Decode::decode(input)?);
            }
            input.ascend_ref();
            new
        } else {
            VecMap::<K, V>::new()
        };

        Ok(result)
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for VecMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K: Borrow<Q> + Ord, Q: ?Sized + Ord, V> Index<&Q> for VecMap<K, V> {
    type Output = V;

    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

impl<K: Borrow<Q> + Ord, Q: ?Sized + Ord, V> IndexMut<&Q> for VecMap<K, V> {
    fn index_mut(&mut self, key: &Q) -> &mut V {
        self.get_mut(key).expect("no entry found for key")
    }
}

impl<K, V> From<VecMap<K, V>> for SortedVec<(K, V)> {
    fn from(this: VecMap<K, V>) -> SortedVec<(K, V)> {
        SortedVec(this.0.into_vec())
    }
}

impl<K, V> From<VecMap<K, V>> for Vec<(K, V)> {
    fn from(this: VecMap<K, V>) -> Vec<(K, V)> {
        this.0
    }
}

impl<K, V> From<VecMap<K, V>> for sp_std::vec::Vec<(K, V)> {
    fn from(this: VecMap<K, V>) -> sp_std::vec::Vec<(K, V)> {
        this.0.into_vec()
    }
}

impl<K, V> From<SortedVec<(K, V)>> for VecMap<K, V> {
    fn from(inner: SortedVec<(K, V)>) -> Self {
        Self(inner.0.into())
    }
}

impl<K: Ord, V> From<Vec<(K, V)>> for VecMap<K, V> {
    fn from(mut inner: Vec<(K, V)>) -> Self {
        inner.sort_by(|a, b| a.0.cmp(&b.0));
        inner.dedup_by(|a, b| a.0 == b.0);
        Self(inner)
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for VecMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let vec: Vec<_> = iter.into_iter().collect();
        vec.into()
    }
}

impl<'a, K: Clone + Ord, V: Clone> FromIterator<(&'a K, &'a V)> for VecMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (&'a K, &'a V)>>(iter: T) -> Self {
        let vec: Vec<_> = iter
            .into_iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        vec.into()
    }
}

impl<'a, K: Clone + Ord + 'a, V: Clone + 'a> FromIterator<&'a (K, V)> for VecMap<K, V> {
    fn from_iter<T: IntoIterator<Item = &'a (K, V)>>(iter: T) -> Self {
        let vec: Vec<_> = iter.into_iter().cloned().collect();
        vec.into()
    }
}

impl<K: Ord, V, T> Extend<T> for VecMap<K, V>
where
    Self: FromIterator<T>,
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.append(&mut iter.into_iter().collect());
    }
}

impl<K, V> IntoIterator for VecMap<K, V> {
    type Item = (K, V);
    type IntoIter = iter::IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        iter::IntoIter {
            iter: self.0.into_iter(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MergeOption<L, R> {
    Left(L),
    Right(R),
    Both(L, R),
}

impl<L, R> MergeOption<L, R> {
    pub fn left(&self) -> Option<&L> {
        match self {
            MergeOption::Left(l) | MergeOption::Both(l, _) => Some(l),
            _ => None,
        }
    }

    pub fn right(&self) -> Option<&R> {
        match self {
            MergeOption::Right(r) | MergeOption::Both(_, r) => Some(r),
            _ => None,
        }
    }

    pub fn lr(&self) -> (Option<&L>, Option<&R>) {
        match self {
            MergeOption::Left(l) => (Some(l), None),
            MergeOption::Right(r) => (None, Some(r)),
            MergeOption::Both(l, r) => (Some(l), Some(r)),
        }
    }

    pub fn to_lr(self) -> (Option<L>, Option<R>) {
        match self {
            MergeOption::Left(l) => (Some(l), None),
            MergeOption::Right(r) => (None, Some(r)),
            MergeOption::Both(l, r) => (Some(l), Some(r)),
        }
    }
}

impl<'a, K: 'a, V: 'a> IntoIterator for &'a VecMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = iter::Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        iter::Iter {
            iter: self.0.iter(),
        }
    }
}

impl<'a, K: 'a, V: 'a> IntoIterator for &'a mut VecMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = iter::IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        iter::IterMut {
            iter: self.0.iter_mut(),
        }
    }
}

impl<K: 'static + TypeInfo, V: 'static + TypeInfo> scale_info::TypeInfo for VecMap<K, V> {
    type Identity = Self;

    fn type_info() -> scale_info::Type {
        scale_info::Type::builder()
            .path(scale_info::Path::new("VecMap", module_path!()))
            .type_params(scale_info::type_params![K, V])
            .composite(
                scale_info::build::Fields::unnamed().field(|f| f.ty::<sp_std::vec::Vec<(K, V)>>()),
            )
    }
}

impl<K: MaxEncodedLen, V: MaxEncodedLen> MaxEncodedLen for VecMap<K, V> {
    fn max_encoded_len() -> usize {
        let max_len = 128_usize;
        Compact(max_len as u32).encoded_size().saturating_add(
            max_len.saturating_mul(K::max_encoded_len().saturating_add(V::max_encoded_len())),
        )
    }
}

impl<K, V> DecodeLength for VecMap<K, V> {
    fn len(mut self_encoded: &[u8]) -> Result<usize, codec::Error> {
        let Compact(size) = Compact::<u32>::decode(&mut self_encoded)?;
        size.try_into()
            .map_err(|_| "Failed to convert `u32` to `usize`".into())
    }
}
