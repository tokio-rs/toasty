use super::{Comma, Formatter, Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::With {
    fn to_sql<P: Params>(self, f: &mut Formatter<'_, P>) {
        fmt!(f, "WITH " Comma(self.ctes.iter().enumerate()) " ");
    }
}

impl ToSql for (usize, &stmt::Cte) {
    fn to_sql<P: Params>(self, f: &mut Formatter<'_, P>) {
        let depth = f.depth;

        f.depth += 1;

        fmt!(f, "cte_" depth "_" self.0 " as (" self.1.query ")");
        f.depth -= 1;
    }
}
