use core::iter::FusedIterator;

type IntoIterInner<K, V> = <super::Vec<(K, V)> as IntoIterator>::IntoIter;

#[derive(Clone)]
pub struct IntoIter<K, V> {
    pub(super) iter: IntoIterInner<K, V>,
}

impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last()
    }
}

impl<K, V> DoubleEndedIterator for IntoIter<K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl<K, V> ExactSizeIterator for IntoIter<K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for IntoIter<K, V> {}

#[derive(Clone)]
pub struct IntoKeys<K, V> {
    pub(super) iter: IntoIterInner<K, V>,
}

impl<K, V> Iterator for IntoKeys<K, V> {
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(k, _)| k)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(k, _)| k)
    }

    fn min(mut self) -> Option<Self::Item>
    where
        K: Ord,
    {
        self.next()
    }

    fn max(mut self) -> Option<Self::Item>
    where
        K: Ord,
    {
        self.next_back()
    }
}

impl<K, V> DoubleEndedIterator for IntoKeys<K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(k, _)| k)
    }
}

impl<K, V> ExactSizeIterator for IntoKeys<K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for IntoKeys<K, V> {}

#[derive(Clone)]
pub struct IntoValues<K, V> {
    pub(super) iter: IntoIterInner<K, V>,
}

impl<K, V> Iterator for IntoValues<K, V> {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(_, v)| v)
    }
}

impl<K, V> DoubleEndedIterator for IntoValues<K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(_, v)| v)
    }
}

impl<K, V> ExactSizeIterator for IntoValues<K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for IntoValues<K, V> {}

#[derive(Clone)]
pub struct Iter<'a, K, V> {
    pub(super) iter: core::slice::Iter<'a, (K, V)>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(k, v)| (&*k, v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(k, v)| (&*k, v))
    }
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for Iter<'a, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(k, v)| (&*k, v))
    }
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for Iter<'_, K, V> {}

#[derive(Clone)]
pub struct Keys<'a, K, V> {
    pub(super) iter: core::slice::Iter<'a, (K, V)>,
}

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(k, _)| k)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(k, _)| k)
    }

    fn min(mut self) -> Option<Self::Item>
    where
        Self::Item: Ord,
    {
        self.next()
    }

    fn max(mut self) -> Option<Self::Item>
    where
        Self::Item: Ord,
    {
        self.next_back()
    }
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for Keys<'a, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(k, _)| k)
    }
}

impl<K, V> ExactSizeIterator for Keys<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for Keys<'_, K, V> {}

#[derive(Clone)]
pub struct Values<'a, K, V> {
    pub(super) iter: core::slice::Iter<'a, (K, V)>,
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(_, v)| v)
    }
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for Values<'a, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(_, v)| v)
    }
}

impl<K, V> ExactSizeIterator for Values<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for Values<'_, K, V> {}

pub struct IterMut<'a, K, V> {
    pub(super) iter: core::slice::IterMut<'a, (K, V)>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(k, v)| (&*k, v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(k, v)| (&*k, v))
    }
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for IterMut<'a, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(k, v)| (&*k, v))
    }
}

impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for IterMut<'_, K, V> {}

pub struct ValuesMut<'a, K, V> {
    pub(super) iter: core::slice::IterMut<'a, (K, V)>,
}

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(_, v)| v)
    }
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for ValuesMut<'a, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(_, v)| v)
    }
}

impl<K, V> ExactSizeIterator for ValuesMut<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for ValuesMut<'_, K, V> {}
