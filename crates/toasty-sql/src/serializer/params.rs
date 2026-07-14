use super::{Formatter, ToSql};

use std::fmt;
use toasty_core::driver::SqlPlaceholder;

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
    fn to_sql(self, f: &mut Formatter<'_>) {
        write_sql_placeholder(&mut f.dst, f.serializer.flavor.sql_placeholder(), self.0).unwrap();
    }
}

fn write_sql_placeholder(
    dst: &mut impl fmt::Write,
    placeholder: SqlPlaceholder,
    index: usize,
) -> fmt::Result {
    match placeholder {
        SqlPlaceholder::QuestionMark => dst.write_str("?"),
        SqlPlaceholder::NumberedQuestionMark => write!(dst, "?{index}"),
        SqlPlaceholder::DollarNumber => write!(dst, "${index}"),
    }
}
