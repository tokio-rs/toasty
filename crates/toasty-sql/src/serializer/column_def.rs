use super::{Ident, Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::ColumnDef {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        let name = Ident(&self.name);

        fmt!(f, name " " self.ty)
    }
}
