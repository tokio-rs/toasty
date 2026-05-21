use super::{Formatter, ToSql};

/// Comma delimited
pub(super) struct Comma<L>(pub(super) L);

/// Period delimited
pub(super) struct Period<L>(pub(super) L);

/// Separated by a custom delimiter
pub(super) struct Delimited<L>(pub(super) L, pub(super) &'static str);

impl<L> ToSql for Comma<L>
where
    L: IntoIterator,
    L::Item: ToSql,
{
    fn to_sql(self, f: &mut Formatter<'_>) {
        Delimited(self.0, ", ").to_sql(f);
    }
}

impl<L> ToSql for Period<L>
where
    L: IntoIterator,
    L::Item: ToSql,
{
    fn to_sql(self, f: &mut Formatter<'_>) {
        Delimited(self.0, ".").to_sql(f);
    }
}

impl<L> ToSql for Delimited<L>
where
    L: IntoIterator,
    L::Item: ToSql,
{
    fn to_sql(self, f: &mut Formatter<'_>) {
        let mut s = "";
        for i in self.0.into_iter() {
            fmt!(f, s i);
            s = self.1;
        }
    }
}
