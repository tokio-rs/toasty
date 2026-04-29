use hashbrown::HashSet;
use rand::seq::SliceRandom;
use std::hash::Hash;

pub(crate) trait NumUtil: PartialOrd + Ord + Eq {
    fn is_even(&self) -> bool;
}

macro_rules! impl_num_util {
    ( $($t:ty),+ ) => {
        $(
            impl NumUtil for $t {
                fn is_even(&self) -> bool {
                    *self % 2 == 0
                }
            }
        )+
    };
}

impl_num_util!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

pub(crate) trait SliceUtil {
    fn is_unique<T>(&self) -> bool
    where
        Self: AsRef<[T]>,
        T: Eq + Hash,
    {
        let slice = self.as_ref();
        let mut s = HashSet::new();
        for el in slice {
            if !s.insert(el) {
                return false;
            }
        }
        true
    }

    fn shuffle<T>(&mut self)
    where
        Self: AsMut<[T]>,
    {
        SliceRandom::shuffle(self.as_mut(), &mut rand::rng());
    }
}

impl<T> SliceUtil for [T] {}

macro_rules! assert_unique {
    ($slice:expr) => {{
        use $crate::util::SliceUtil;
        let slice = &$slice;
        assert!(
            slice.is_unique(),
            "expected `{}` to be unique, but it wasn't; actual={:?}",
            stringify!($slice),
            slice,
        );
    }};
}

macro_rules! assert_err {
    ($e:expr $(, $($t:tt)* )?) => {
        match $e {
            Err(e) => e,
            actual => {
                #[allow(unused_imports)]
                use std::fmt::Write;
                #[allow(unused_mut)]
                let mut msg = format!("expected `Err`; actual={:?}", actual);

                $(
                    write!(msg, ", ").unwrap();
                    write!(msg, $($t)*).unwrap();
                )?

                panic!("{}", msg);
            }
        }
    };
}

macro_rules! assert_ok {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            actual => panic!("expected `Ok`; actual={:?}", actual),
        }
    };
}

macro_rules! assert_none {
    ($e:expr) => {
        match &$e {
            None => {}
            actual => panic!("expected `None`; actual={:?}", actual),
        }
    };
}
