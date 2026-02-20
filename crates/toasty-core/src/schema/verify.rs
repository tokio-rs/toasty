mod relations_are_indexed;

use super::{
    app::{FieldId, ModelId},
    db::{ColumnId, IndexId},
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
                self.verify_auto_field_type(field);
            }
        }

        self.verify_model_indices_are_scoped_correctly();
        self.verify_each_table_has_one_primary_key();
        self.verify_indices_have_columns();
        self.verify_index_names_are_unique()?;
        self.verify_table_indices_and_nullable();
        self.verify_auto_increment_columns()?;

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
                    return Err(crate::Error::invalid_schema(format!(
                        "duplicate index name `{}`",
                        index.name
                    )));
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

    fn verify_auto_increment_columns(&self) -> Result<()> {
        for table in &self.schema.db.tables {
            for column in &table.columns {
                if column.auto_increment {
                    // Verify the column has a numeric type
                    if !column.ty.is_numeric() {
                        return Err(crate::Error::invalid_schema(format!(
                            "auto_increment column `{}` in table `{}` must have a numeric type, found {:?}",
                            column.name,
                            table.name,
                            column.ty
                        )));
                    }

                    // Verify it's the only column in the primary key
                    if table.primary_key.columns.len() != 1 {
                        return Err(crate::Error::invalid_schema(format!(
                            "auto_increment column `{}` in table `{}` cannot be used with composite primary keys (partition/local keys). Use UUID or remove the composite key.",
                            column.name,
                            table.name
                        )));
                    }

                    // Verify the auto_increment column is actually in the primary key
                    let pk_column = &table.columns[table.primary_key.columns[0].index];
                    if pk_column.id != column.id {
                        return Err(crate::Error::invalid_schema(format!(
                            "auto_increment column `{}` in table `{}` must be part of the primary key",
                            column.name,
                            table.name
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    fn verify_auto_field_type(&self, field: &super::app::Field) {
        use super::app::AutoStrategy;

        let Some(auto) = &field.auto else {
            return;
        };

        // Only verify primitive fields
        let Some(primitive) = field.ty.as_primitive() else {
            return;
        };

        let field_ty = &primitive.ty;

        match auto {
            AutoStrategy::Increment => {
                assert!(
                    field_ty.is_numeric(),
                    "field `{}` has Auto::Increment but type is not numeric: {:?}",
                    field.name.app_name,
                    field_ty
                );
            }
            AutoStrategy::Uuid(_) => {
                assert!(
                    field_ty.is_uuid(),
                    "field `{}` has Auto::Uuid but type is not Uuid: {:?}",
                    field.name.app_name,
                    field_ty
                );
            }
        }
    }
}
