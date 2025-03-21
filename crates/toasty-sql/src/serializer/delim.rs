use super::{Formatter, Params, ToSql};

/// Comma delimited
pub(super) struct Comma<L>(pub(super) L);

/// Period delimited
pub(super) struct Period<L>(pub(super) L);

impl<L> ToSql for Comma<L>
where
    L: IntoIterator,
    L::Item: ToSql,
{
    fn to_sql<P: Params>(self, f: &mut Formatter<'_, P>) {
        let mut s = "";
        for i in self.0 {
            fmt!(f, s i);
            s = ", ";
        }
    }
}

impl<L, I> ToSql for Period<L>
where
    L: IntoIterator<Item = I>,
    I: ToSql,
{
    fn to_sql<P: Params>(self, f: &mut Formatter<'_, P>) {
        let mut s = "";
        for i in self.0.into_iter() {
            fmt!(f, s i);
            s = ".";
        }
    }
}
