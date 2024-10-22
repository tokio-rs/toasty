use super::*;

use std::{ops, slice};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Punctuated<T, P> {
    pub items: Vec<(T, Option<P>)>,
}

pub(crate) struct Iter<'a, T, S> {
    inner: slice::Iter<'a, (T, Option<S>)>,
}

impl<T, P> Punctuated<T, P> {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn first(&self) -> Option<&T> {
        self.items.first().map(|(item, _)| item)
    }

    pub fn last(&self) -> Option<&T> {
        self.items.last().map(|(item, _)| item)
    }

    /// Iterate over elements in the group.
    pub fn iter(&self) -> Iter<'_, T, P> {
        Iter {
            inner: self.items.iter(),
        }
    }
}

impl<T: Parse, P: Peek> Parse for Punctuated<T, P> {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        let mut items = vec![];

        loop {
            let item = p.parse()?;
            let sep = p.parse::<Option<P>>()?;
            let is_end = sep.is_none();

            items.push((item, sep));

            if is_end {
                break;
            }
        }

        Ok(Punctuated { items })
    }
}

impl<'a, T, S> IntoIterator for &'a Punctuated<T, S> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T, S>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, S> Iterator for Iter<'a, T, S> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if let Some((t, _)) = self.inner.next() {
            Some(t)
        } else {
            None
        }
    }
}

impl<T, S> ops::Index<usize> for Punctuated<T, S> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.items[index].0
    }
}
