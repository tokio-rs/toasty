use super::{Ident, Period, ToSql};

use crate::stmt;

impl ToSql for &stmt::Name {
    fn to_sql(self, f: &mut super::Formatter<'_>) {
        let parts = Period(self.0.iter().map(Ident));
        fmt!(f, parts);
    }
}
