use super::{Comma, Formatter, Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::With {
    fn to_sql<P: Params>(self, f: &mut Formatter<'_, P>) {
        fmt!(f, "WITH " Comma(&self.ctes) " ");
    }
}

impl ToSql for &stmt::Cte {
    fn to_sql<P: Params>(self, f: &mut Formatter<'_, P>) {
        fmt!(f, "cte_tbl_lol as (" self.query ")");
    }
}
