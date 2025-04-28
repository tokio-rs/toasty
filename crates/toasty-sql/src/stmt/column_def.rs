use toasty_core::schema::db::{self, Column};

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub ty: db::Type,
}

impl ColumnDef {
    pub(crate) fn from_schema(column: &Column) -> ColumnDef {
        ColumnDef {
            name: column.name.clone(),
            ty: column
                .storage_ty
                .clone()
                .unwrap_or_else(|| db::Type::from_app(&column.ty)),
        }
    }
}
