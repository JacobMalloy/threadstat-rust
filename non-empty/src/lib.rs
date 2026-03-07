use std::iter::FromIterator;
use std::ops::{Deref, DerefMut, Index, IndexMut};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonEmpty<U>(Box<[U]>);

impl<U> NonEmpty<U> {
    pub fn new_single(val: U) -> Self {
        Self(Box::new([val]))
    }

    pub fn first(&self) -> &U {
        unsafe { self.0.get_unchecked(0) }
    }

    pub fn is_empty() -> bool {
        false
    }

    pub fn first_mut(&mut self) -> &mut U {
        unsafe { self.0.get_unchecked_mut(0) }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, U> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, U> {
        self.0.iter_mut()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaybeNonEmpty<U>(pub Option<NonEmpty<U>>);

impl<U> MaybeNonEmpty<U> {
    pub fn into_option(self) -> Option<NonEmpty<U>> {
        self.0
    }
}

impl<U> Deref for MaybeNonEmpty<U> {
    type Target = Option<NonEmpty<U>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<U> From<MaybeNonEmpty<U>> for Option<NonEmpty<U>> {
    fn from(m: MaybeNonEmpty<U>) -> Self {
        m.0
    }
}

impl<U> FromIterator<U> for MaybeNonEmpty<U> {
    fn from_iter<T: IntoIterator<Item = U>>(iter: T) -> Self {
        let vals: Box<[U]> = iter.into_iter().collect();
        MaybeNonEmpty(if vals.is_empty() {
            None
        } else {
            Some(NonEmpty(vals))
        })
    }
}

impl<U> Deref for NonEmpty<U> {
    type Target = [U];

    fn deref(&self) -> &[U] {
        &self.0
    }
}

impl<U> DerefMut for NonEmpty<U> {
    fn deref_mut(&mut self) -> &mut [U] {
        &mut self.0
    }
}

impl<U> AsRef<[U]> for NonEmpty<U> {
    fn as_ref(&self) -> &[U] {
        &self.0
    }
}

impl<U> AsMut<[U]> for NonEmpty<U> {
    fn as_mut(&mut self) -> &mut [U] {
        &mut self.0
    }
}

impl<U, I: std::slice::SliceIndex<[U]>> Index<I> for NonEmpty<U> {
    type Output = I::Output;

    fn index(&self, index: I) -> &I::Output {
        &self.0[index]
    }
}

impl<U, I: std::slice::SliceIndex<[U]>> IndexMut<I> for NonEmpty<U> {
    fn index_mut(&mut self, index: I) -> &mut I::Output {
        &mut self.0[index]
    }
}

impl<'a, U> IntoIterator for &'a NonEmpty<U> {
    type Item = &'a U;
    type IntoIter = std::slice::Iter<'a, U>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, U> IntoIterator for &'a mut NonEmpty<U> {
    type Item = &'a mut U;
    type IntoIter = std::slice::IterMut<'a, U>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl<U> IntoIterator for NonEmpty<U> {
    type Item = U;
    type IntoIter = std::vec::IntoIter<U>;

    fn into_iter(self) -> Self::IntoIter {
        Vec::from(self.0).into_iter()
    }
}

impl<U> From<NonEmpty<U>> for Box<[U]> {
    fn from(ne: NonEmpty<U>) -> Box<[U]> {
        ne.0
    }
}

impl<U> From<NonEmpty<U>> for Vec<U> {
    fn from(ne: NonEmpty<U>) -> Vec<U> {
        ne.0.into()
    }
}
