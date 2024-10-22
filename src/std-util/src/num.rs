pub trait NumUtil: PartialOrd + Ord + Eq {
    fn is_even(&self) -> bool;

    fn is_odd(&self) -> bool {
        !self.is_even()
    }
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
