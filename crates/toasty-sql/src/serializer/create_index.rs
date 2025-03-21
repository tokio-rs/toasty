use super::{Comma, Params, ToSql};

use crate::stmt;

impl ToSql for stmt::CreateIndex {
    fn fmt<T: Params>(&self, f: &mut super::Formatter<'_, T>) {
        let table_name = f.serializer.table_name(self.on);
        let columns = Comma(&self.columns);

        fmt!(
            f, "CREATE INDEX" self.name "ON" table_name "(" columns ")"
        );
    }
}
