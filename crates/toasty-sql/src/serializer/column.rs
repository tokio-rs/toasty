use crate::serializer::ToSql;

#[derive(Debug)]
pub(crate) struct ColumnAlias(pub(crate) usize);

impl ToSql for ColumnAlias {
    fn to_sql<P: super::Params>(
        self,
        cx: &super::ExprContext<'_>,
        f: &mut super::Formatter<'_, P>,
    ) {
        if f.serializer.is_mysql() {
            let i = self.0;
            fmt!(cx, f, "column_" i);
        } else {
            let i = self.0 + 1;
            fmt!(cx, f, "column" i);
        }
    }
}
