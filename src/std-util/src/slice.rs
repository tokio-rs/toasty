use rand::{seq::SliceRandom, thread_rng};
use std::{collections::HashSet, hash::Hash};

pub trait SliceUtil {
    /// Returns `true` if the slice only contains unique values
    fn is_unique<T>(&self) -> bool
    where
        Self: AsRef<[T]>,
        T: Eq + Hash,
    {
        is_unique(self.as_ref())
    }

    /// Shuffle the slice
    fn shuffle<T>(&mut self)
    where
        Self: AsMut<[T]>,
    {
        shuffle(self.as_mut())
    }
}

impl<T> SliceUtil for [T] {}

#[macro_export]
macro_rules! assert_unique {
    ($slice:expr) => {{
        use $crate::slice::SliceUtil;
        let slice = &$slice;
        assert!(
            slice.is_unique(),
            "expected `{}` to be unique, but it wasn't; actual={:?}",
            stringify!($slice),
            slice,
        );
    }};
}

#[macro_export]
macro_rules! assert_empty {
    ($slice:expr) => {{
        match &$slice[..] {
            [] => {}
            actual => panic!("expected slice to be empty; actual={:?}", actual),
        }
    }};
}

pub fn is_unique<T: Eq + Hash>(slice: &[T]) -> bool {
    let mut s = HashSet::new();

    for el in slice {
        if !s.insert(el) {
            return false;
        }
    }

    true
}

pub fn shuffle<T>(slice: &mut [T]) {
    SliceRandom::shuffle(slice, &mut thread_rng());
}
