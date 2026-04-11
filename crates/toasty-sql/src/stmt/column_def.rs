use super::CheckConstraint;

use toasty_core::{
    driver::{self, Capability},
    schema::db::{self, Column},
    stmt::Expr,
};

/// A column definition used in `CREATE TABLE` and `ADD COLUMN` statements.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    /// Column name.
    pub name: String,
    /// Storage type (e.g. `INTEGER`, `TEXT`).
    pub ty: db::Type,
    /// When `true`, the column has a `NOT NULL` constraint.
    pub not_null: bool,
    /// When `true`, the column auto-increments.
    pub auto_increment: bool,
    /// Optional CHECK constraint on this column.
    pub check: Option<CheckConstraint>,
}

impl ColumnDef {
    pub(crate) fn from_schema(
        column: &Column,
        _storage_types: &driver::StorageTypes,
        capability: &Capability,
    ) -> Self {
        // For SQLite enum columns: store as TEXT with a CHECK constraint instead
        // of db::Type::Enum. The CHECK constraint restricts values to the
        // declared labels.
        if let db::Type::Enum { labels, .. } = &column.storage_ty
            && !capability.native_enum
        {
            return Self {
                name: column.name.clone(),
                ty: db::Type::Text,
                not_null: !column.nullable,
                auto_increment: column.auto_increment,
                check: Some(CheckConstraint {
                    name: None,
                    expr: Box::new(Expr::in_list(
                        Expr::Ident(column.name.clone()),
                        Expr::list(labels.iter().map(|l| Expr::from(l.clone()))),
                    )),
                }),
            };
        }

        Self {
            name: column.name.clone(),
            ty: column.storage_ty.clone(),
            not_null: !column.nullable,
            auto_increment: column.auto_increment,
            check: None,
        }
    }
}
