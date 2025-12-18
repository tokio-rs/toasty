use toasty_core::{
    driver,
    schema::db::{self, Column},
};

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub ty: db::Type,
    pub not_null: bool,
    pub auto_increment: bool,
}

impl ColumnDef {
    pub(crate) fn from_schema(column: &Column, _storage_types: &driver::StorageTypes) -> Self {
        Self {
            name: column.name.clone(),
            ty: column.storage_ty.clone(),
            not_null: !column.nullable,
            auto_increment: column.auto_increment,
        }
    }
}
