pub mod num;
pub mod option;
pub mod result;
pub mod slice;
pub mod str;

pub mod prelude {
    pub use crate::{
        assert_empty, assert_err, assert_none, assert_ok, assert_unique, num::NumUtil,
        slice::SliceUtil,
    };
}
