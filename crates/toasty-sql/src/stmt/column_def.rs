use super::*;

use toasty_core::schema::db::Column;

use std::fmt;

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub ty: ColumnType,
}

impl ColumnDef {
    pub(crate) fn from_schema(column: &Column, indexed: bool) -> ColumnDef {
        ColumnDef {
            name: column.name.clone(),
            ty: ColumnType::from_schema(&column.ty, indexed),
        }
    }
}

impl fmt::Display for ColumnDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.name, self.ty)
    }
}
