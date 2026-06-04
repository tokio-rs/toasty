use crate::serializer::ToSql;

#[derive(Debug)]
pub(crate) struct ColumnAlias(pub(crate) usize);

impl ToSql for ColumnAlias {
    fn to_sql(self, f: &mut super::Formatter<'_>) {
        // The per-flavor format matches each engine's auto-naming convention
        // for derived-table columns. MySQL's `VALUES ROW(...) AS t` exposes
        // columns as `column_0`, `column_1`, ...; PG and SQLite use
        // `column1`, `column2`, ... in equivalent contexts. The engine emits
        // VALUES-derived tables in include lowering, and outer references
        // (e.g. `tbl_1_0.column_0`) must match what the engine sees.
        if f.serializer.is_mysql() {
            let i = self.0;
            fmt!(f, "column_" i);
        } else {
            let i = self.0 + 1;
            fmt!(f, "column" i);
        }
    }
}
