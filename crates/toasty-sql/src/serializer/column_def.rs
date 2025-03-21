use super::{Ident, Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::ColumnDef {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let name = Ident(&self.name);

        fmt!(f, name " " self.ty)
    }
}
