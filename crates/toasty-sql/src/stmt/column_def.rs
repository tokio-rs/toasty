use toasty_core::{
    driver,
    schema::db::{self, Column},
    stmt,
};

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub ty: db::Type,
}

impl ColumnDef {
    pub(crate) fn from_schema(column: &Column, storage_types: &driver::StorageTypes) -> ColumnDef {
        let ty = match column.storage_ty.clone() {
            Some(ty) => ty,
            None => match &column.ty {
                stmt::Type::Bool => db::Type::Boolean,
                stmt::Type::I64 => db::Type::Integer,
                stmt::Type::String => storage_types.default_string_type.clone(),
                ty => todo!("ty={:#?}", ty),
            },
        };

        ColumnDef {
            name: column.name.clone(),
            ty,
        }
    }
}
