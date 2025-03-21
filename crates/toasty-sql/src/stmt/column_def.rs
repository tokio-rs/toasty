use super::*;

use toasty_core::schema::db::Column;

use std::fmt;

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: Ident,
    pub ty: Type,
}

impl ColumnDef {
    pub(crate) fn from_schema(column: &Column) -> ColumnDef {
        ColumnDef {
            name: Ident::from(&column.name[..]),
            ty: Type::from_schema(&column.ty),
        }
    }
}

impl fmt::Display for ColumnDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.name, self.ty)
    }
}
