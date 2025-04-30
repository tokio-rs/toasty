use toasty_core::{
    driver,
    schema::db::{self, Column},
};

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub ty: db::Type,
}

impl ColumnDef {
    pub(crate) fn from_schema(column: &Column, storage_types: &driver::StorageTypes) -> ColumnDef {
        let ty = db::Type::from_app(&column.ty, &column.storage_ty, storage_types).unwrap();

        ColumnDef {
            name: column.name.clone(),
            ty,
        }
    }
}
