use super::{Ident, Params, ToSql};

use crate::{serializer::ExprContext, stmt};

impl ToSql for &stmt::ColumnDef {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let name = Ident(&self.name);

        fmt!(cx, f, name " " self.ty);

        if self.not_null {
            fmt!(cx, f, " NOT NULL");
        }

        if self.auto_increment {
            fmt!(cx, f, " AUTO_INCREMENT");
        }
    }
}
