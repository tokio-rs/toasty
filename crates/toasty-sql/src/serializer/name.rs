use super::{Ident, Params, Period, ToSql};

use crate::{serializer::ExprContext, stmt};

impl ToSql for &stmt::Name {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let parts = Period(self.0.iter().map(Ident));
        fmt!(cx, f, parts);
    }
}
