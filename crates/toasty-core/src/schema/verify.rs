mod relations_are_indexed;

use super::{
    app::{FieldId, FieldTy, ModelId},
    db::{ColumnId, IndexId, Type as DbType},
    Result, Schema,
};
use crate::stmt;

use std::collections::HashSet;

struct Verify<'a> {
    schema: &'a Schema,
}

impl Schema {
    pub(super) fn verify(&self) -> Result<()> {
        Verify { schema: self }.verify()
    }
}

impl Verify<'_> {
    fn verify(&self) -> Result<()> {
        debug_assert!(self.verify_ids_populated());

        for model in self.schema.app.models() {
            for field in &model.fields {
                self.verify_relations_are_indexed(field);
            }
        }

        self.verify_model_indices_are_scoped_correctly();
        self.verify_each_table_has_one_primary_key();
        self.verify_indices_have_columns();
        self.verify_index_names_are_unique()?;
        self.verify_table_indices_and_nullable();
        self.verify_column_type_compatibility()?;
        Ok(())
    }

    // TODO: move these methods to separate modules?

    fn verify_ids_populated(&self) -> bool {
        for model in self.schema.app.models() {
            assert_ne!(model.id, ModelId::placeholder());

            for field in &model.fields {
                if let Some(has_many) = field.ty.as_has_many() {
                    assert_ne!(has_many.pair, FieldId::placeholder());
                }

                if let Some(belongs_to) = field.ty.as_belongs_to() {
                    assert_ne!(belongs_to.target, ModelId::placeholder());

                    if let Some(pair) = belongs_to.pair {
                        assert_ne!(pair, FieldId::placeholder());
                    }

                    assert_ne!(
                        belongs_to.expr_ty,
                        stmt::Type::Model(ModelId::placeholder())
                    );
                }
            }
        }

        for table in &self.schema.db.tables {
            assert_ne!(table.primary_key.index, IndexId::placeholder());
            assert!(!table.primary_key.columns.is_empty());

            for index in &table.indices {
                for index_column in &index.columns {
                    assert_ne!(index_column.column, ColumnId::placeholder());
                }
            }
        }

        true
    }

    fn verify_model_indices_are_scoped_correctly(&self) {
        for model in self.schema.app.models() {
            for index in &model.indices {
                let mut seen_local = false;

                for field in &index.fields {
                    match (seen_local, field.scope.is_local()) {
                        (false, false) => {}
                        (false, true) => seen_local = true,
                        (true, true) => {}
                        (true, false) => panic!(),
                    }
                }
            }
        }
    }

    fn verify_indices_have_columns(&self) {
        for table in &self.schema.db.tables {
            for index in &table.indices {
                assert!(
                    !index.columns.is_empty(),
                    "table={table:#?}; schema={:#?}",
                    self.schema
                );
            }
        }
    }

    fn verify_index_names_are_unique(&self) -> Result<()> {
        let mut names = HashSet::new();

        for table in &self.schema.db.tables {
            for index in &table.indices {
                if !names.insert(&index.name) {
                    anyhow::bail!("duplicate index name `{}`", index.name);
                }
            }
        }

        Ok(())
    }

    fn verify_table_indices_and_nullable(&self) {
        for table in &self.schema.db.tables {
            for index in &table.indices {
                let nullable = index
                    .columns
                    .iter()
                    .any(|index_column| table.column(index_column.column).nullable);

                if nullable {
                    // If there are nullable columns, then (for now) the index
                    // should only have one column
                    assert_eq!(
                        index.columns.len(),
                        1,
                        "table index with multiple columns includes a nullable column"
                    );
                }
            }
        }
    }

    fn verify_each_table_has_one_primary_key(&self) {
        for table in &self.schema.db.tables {
            assert_eq!(1, table.indices.iter().filter(|i| i.primary_key).count());
        }
    }

    fn verify_column_type_compatibility(&self) -> Result<()> {
        for model in self.schema.app.models() {
            for field in &model.fields {
                // Only validate primitive fields that have an explicit storage type
                if let FieldTy::Primitive(primitive) = &field.ty {
                    if let Some(storage_ty) = &primitive.storage_ty {
                        if !self.is_storage_compatible(&primitive.ty, storage_ty) {
                            let field_name = &field.name.app_name;
                            let model_name = &model.name.upper_camel_case();

                            anyhow::bail!(
                                "Invalid column type '{}' for field '{}' of type '{}' in model '{}'.\n\n\
                                 = note: {} fields are compatible with: {}\n\
                                 = help: {}",
                                 self.format_db_type(storage_ty),
                                field_name,
                                self.format_stmt_type(&primitive.ty),
                                model_name,
                                self.format_stmt_type(&primitive.ty),
                                self.get_compatible_types(&primitive.ty),
                                self.get_suggestion(&primitive.ty)
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Check if a statement primitive type is compatible with a storage type
    ///
    /// This validation ensures that the storage type specified in a `#[column(type = ...)]`
    /// annotation is compatible with the Rust field type. The validation is based on the
    /// default storage type mappings defined in driver capabilities:
    ///
    /// - **SQLite/DynamoDB**: Use TEXT for most custom types (no native date/time support)
    /// - **PostgreSQL**: Native support for TIMESTAMP, DATE, TIME, DATETIME with precision
    /// - **MySQL**: Native support similar to PostgreSQL, uses DATETIME for timestamps
    ///
    /// # Database Compatibility Matrix
    ///
    /// | Rust Type | SQLite | PostgreSQL | MySQL | DynamoDB |
    /// |-----------|--------|------------|-------|----------|
    /// | String    | TEXT   | TEXT       | TEXT  | TEXT     |
    /// | bool      | INTEGER| BOOLEAN    | BOOLEAN| TEXT     |
    /// | i32/i64   | INTEGER| INTEGER    | INTEGER| TEXT     |
    /// | Uuid      | TEXT   | UUID       | TEXT  | TEXT     |
    /// | Timestamp | TEXT   | TIMESTAMP  | DATETIME| TEXT    |
    /// | Date      | TEXT   | DATE       | DATE  | TEXT     |
    /// | Time      | TEXT   | TIME       | TIME  | TEXT     |
    /// | DateTime  | TEXT   | DATETIME   | DATETIME| TEXT    |
    ///
    /// Note: This hardcodes database knowledge that ideally should come from
    /// driver capabilities, but requires architectural changes to pass capabilities
    /// through to schema validation. See issue for future improvements.
    fn is_storage_compatible(&self, stmt_ty: &stmt::Type, storage_ty: &DbType) -> bool {
        use stmt::Type;
        match (stmt_ty, storage_ty) {
            // Integers: Support any integer storage type regardless of size/signedness
            (
                Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64,
                DbType::Integer(_),
            ) => true,
            (Type::U8 | Type::U16 | Type::U32 | Type::U64, DbType::UnsignedInteger(_)) => true,

            // String types
            (Type::String, DbType::Text) => true,
            (Type::String, DbType::VarChar(_)) => true,

            // Boolean
            (Type::Bool, DbType::Boolean) => true,
            (Type::Bool, DbType::Integer(_)) => true, // SQLite uses INTEGER for bool

            // UUID
            (Type::Uuid, DbType::Text) => true,
            (Type::Uuid, DbType::VarChar(_)) => true,
            (Type::Uuid, DbType::Uuid) => true,
            (Type::Uuid, DbType::Blob) => true,
            (Type::Uuid, DbType::Binary(_)) => true,

            // BigDecimal: arbitrary-precision decimal numbers
            #[cfg(feature = "bigdecimal")]
            (Type::BigDecimal, DbType::Text) => true,

            // Jiff date/time types: use helper method for maintainability
            #[cfg(feature = "jiff")]
            (Type::Timestamp | Type::Date | Type::Time | Type::DateTime, _) => {
                self.is_jiff_type_compatible(stmt_ty, storage_ty)
            }

            // Handle custom type strings (case-insensitive)
            (Type::String, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower == "text" || s_lower.starts_with("varchar(")
            }
            (Type::Bool, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower == "boolean" || s_lower.starts_with("integer(")
            }
            (Type::I8 | Type::I16 | Type::I32 | Type::I64, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower.starts_with("integer(")
            }
            (Type::U8 | Type::U16 | Type::U32 | Type::U64, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower.starts_with("integer(") || s_lower.starts_with("unsignedinteger(")
            }
            (Type::Uuid, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower == "uuid"
                    || s_lower == "text"
                    || s_lower.starts_with("varchar(")
                    || s_lower == "blob"
                    || s_lower.starts_with("binary(")
            }
            #[cfg(feature = "bigdecimal")]
            (Type::BigDecimal, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower == "text"
            }

            // All other combinations are incompatible
            _ => false,
        }
    }

    /// Helper to check if a jiff date/time type is compatible with a database storage type
    #[cfg(feature = "jiff")]
    fn is_jiff_type_compatible(&self, stmt_ty: &stmt::Type, storage_ty: &DbType) -> bool {
        match (stmt_ty, storage_ty) {
            // All jiff types support TEXT storage (universal fallback)
            (
                stmt::Type::Timestamp | stmt::Type::Date | stmt::Type::Time | stmt::Type::DateTime,
                DbType::Text,
            ) => true,

            // Native database type support
            (stmt::Type::Timestamp, DbType::Timestamp(_)) => true,
            (stmt::Type::Timestamp, DbType::DateTime(_)) => true, // MySQL uses DATETIME for timestamp
            (stmt::Type::Date, DbType::Date) => true,
            (stmt::Type::Time, DbType::Time(_)) => true,
            (stmt::Type::DateTime, DbType::DateTime(_)) => true,

            // Custom type strings (case-insensitive)
            (stmt::Type::Timestamp, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower == "text"
                    || s_lower.starts_with("timestamp(")
                    || s_lower.starts_with("datetime(")
            }
            (stmt::Type::Date, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower == "text" || s_lower == "date"
            }
            (stmt::Type::Time, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower == "text" || s_lower.starts_with("time(")
            }
            (stmt::Type::DateTime, DbType::Custom(s)) => {
                let s_lower = s.to_lowercase();
                s_lower == "text" || s_lower.starts_with("datetime(")
            }

            _ => false,
        }
    }

    /// Format a database type for error messages
    fn format_db_type(&self, db_ty: &DbType) -> String {
        match db_ty {
            DbType::Integer(size) => format!("INTEGER({})", size),
            DbType::UnsignedInteger(size) => format!("UNSIGNED_INTEGER({})", size),
            DbType::Text => "TEXT".to_string(),
            DbType::VarChar(len) => format!("VARCHAR({})", len),
            DbType::Boolean => "BOOLEAN".to_string(),
            DbType::Uuid => "UUID".to_string(),
            DbType::Blob => "BLOB".to_string(),
            DbType::Binary(size) => format!("BINARY({})", size),
            DbType::Custom(s) => s.to_uppercase(),
            _ => "UNKNOWN".to_string(),
        }
    }

    /// Format a statement type for error messages  
    fn format_stmt_type(&self, stmt_ty: &stmt::Type) -> &'static str {
        use stmt::Type;
        match stmt_ty {
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::I64 => "i64",
            Type::U8 => "u8",
            Type::U16 => "u16",
            Type::U32 => "u32",
            Type::U64 => "u64",
            Type::String => "String",
            Type::Bool => "bool",
            Type::Uuid => "Uuid",
            #[cfg(feature = "bigdecimal")]
            Type::BigDecimal => "BigDecimal",
            #[cfg(feature = "jiff")]
            Type::Timestamp => "Timestamp",
            #[cfg(feature = "jiff")]
            Type::Date => "Date",
            #[cfg(feature = "jiff")]
            Type::Time => "Time",
            #[cfg(feature = "jiff")]
            Type::DateTime => "DateTime",
            _ => "unknown",
        }
    }

    /// Get compatible storage types for a statement type
    fn get_compatible_types(&self, stmt_ty: &stmt::Type) -> &'static str {
        use stmt::Type;
        match stmt_ty {
            Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64 => "INTEGER, UNSIGNED_INTEGER",
            Type::String => "TEXT, VARCHAR",
            Type::Bool => "BOOLEAN, INTEGER",
            Type::Uuid => "TEXT, VARCHAR, UUID, BLOB, BINARY",
            #[cfg(feature = "bigdecimal")]
            Type::BigDecimal => "TEXT",
            #[cfg(feature = "jiff")]
            Type::Timestamp => "TEXT, TIMESTAMP, DATETIME",
            #[cfg(feature = "jiff")]
            Type::Date => "TEXT, DATE",
            #[cfg(feature = "jiff")]
            Type::Time => "TEXT, TIME",
            #[cfg(feature = "jiff")]
            Type::DateTime => "TEXT, DATETIME",
            _ => "none",
        }
    }

    /// Get a helpful suggestion for fixing type compatibility issues
    fn get_suggestion(&self, stmt_ty: &stmt::Type) -> &'static str {
        use stmt::Type;
        match stmt_ty {
            Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64 => {
                "Consider removing the column_type annotation to use the default integer mapping."
            }
            Type::String => {
                "Consider using `column_type = text` or remove the column_type annotation."
            }
            Type::Bool => {
                "Consider using `column_type = boolean` or remove the column_type annotation."
            }
            Type::Uuid => {
                "Consider using `column_type = uuid`, `column_type = blob`, or remove the column_type annotation."
            }
            #[cfg(feature = "bigdecimal")]
            Type::BigDecimal => {
                "BigDecimal fields use TEXT storage by default. You can remove the column_type annotation."
            }
            #[cfg(feature = "jiff")]
            Type::Timestamp => {
                "Timestamp fields use database-native types by default (TEXT for SQLite/DynamoDB, TIMESTAMP/DATETIME for PostgreSQL/MySQL). You can remove the column_type annotation."
            }
            #[cfg(feature = "jiff")]
            Type::Date => {
                "Date fields use database-native types by default (TEXT for SQLite/DynamoDB, DATE for PostgreSQL/MySQL). You can remove the column_type annotation."
            }
            #[cfg(feature = "jiff")]
            Type::Time => {
                "Time fields use database-native types by default (TEXT for SQLite/DynamoDB, TIME for PostgreSQL/MySQL). You can remove the column_type annotation."
            }
            #[cfg(feature = "jiff")]
            Type::DateTime => {
                "DateTime fields use database-native types by default (TEXT for SQLite/DynamoDB, DATETIME for PostgreSQL/MySQL). You can remove the column_type annotation."
            }
            _ => "Remove the column_type annotation to use the default type mapping.",
        }
    }
}
