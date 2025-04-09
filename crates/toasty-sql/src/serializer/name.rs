use super::{Ident, Params, Period, ToSql};

use crate::stmt;

impl ToSql for &stmt::Name {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let parts = Period(self.0.iter().map(Ident));
        fmt!(f, parts);
    }
}
