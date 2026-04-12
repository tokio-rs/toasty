use crate::serializer::ExprContext;

use super::{Formatter, ToSql};

/// A positional bind-parameter placeholder.
///
/// The inner `usize` is the 1-based parameter index. The serializer renders
/// it in the target dialect's format (`$1`, `?1`, or `?`).
///
/// # Example
///
/// ```
/// use toasty_sql::serializer::Placeholder;
///
/// let p = Placeholder(3);
/// assert_eq!(p.0, 3);
/// ```
pub struct Placeholder(pub usize);

impl ToSql for Placeholder {
    fn to_sql(self, _cx: &ExprContext<'_>, f: &mut Formatter<'_>) {
        use std::fmt::Write;

        match f.serializer.flavor {
            super::Flavor::Mysql => write!(&mut f.dst, "?").unwrap(),
            super::Flavor::Postgresql => write!(&mut f.dst, "${}", self.0).unwrap(),
            super::Flavor::Sqlite => write!(&mut f.dst, "?{}", self.0).unwrap(),
        }
    }
}
