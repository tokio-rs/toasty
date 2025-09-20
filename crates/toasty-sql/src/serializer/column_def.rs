use super::{Ident, Params, ToSql};

use crate::{serializer::ExprContext, stmt};

impl ToSql for &stmt::ColumnDef {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let name = Ident(&self.name);

        fmt!(cx, f, name " " self.ty)
    }
}
