use super::{Comma, Formatter, ToSql};

use crate::{serializer::ExprContext, stmt};

impl ToSql for &stmt::With {
    fn to_sql(self, cx: &ExprContext<'_>, f: &mut Formatter<'_>) {
        fmt!(cx, f, "WITH " Comma(self.ctes.iter().enumerate()) " ");
    }
}

impl ToSql for (usize, &stmt::Cte) {
    fn to_sql(self, cx: &ExprContext<'_>, f: &mut Formatter<'_>) {
        let depth = f.depth;

        f.depth += 1;

        fmt!(cx, f, "cte_" depth "_" self.0 " as (" self.1.query ")");
        f.depth -= 1;
    }
}
