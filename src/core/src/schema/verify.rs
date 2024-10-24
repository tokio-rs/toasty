mod relations_are_indexed;

use super::*;

use std::collections::HashSet;

struct Verify<'a> {
    schema: &'a Schema,
}

impl Schema {
    pub(super) fn verify(&self) {
        Verify { schema: self }.verify();
    }
}

impl Verify<'_> {
    fn verify(&self) {
        debug_assert!(self.verify_ids_populated());

        for model in &self.schema.inner.models {
            for field in &model.fields {
                self.verify_relations_are_indexed(field);
            }
        }

        self.verify_model_indices_are_scoped_correctly();
        self.verify_indices_have_columns();
        self.verify_index_names_are_unique().unwrap();
        self.verify_query_names_are_unique().unwrap();
        self.verify_table_indices_and_nullable();
    }

    // TODO: move these methods to separate modules?

    fn verify_ids_populated(&self) -> bool {
        for model in &self.schema.inner.models {
            assert_ne!(model.id, ModelId::placeholder());
            assert_ne!(model.lowering.table, TableId::placeholder());

            for field in &model.fields {
                if let Some(has_many) = field.ty.as_has_many() {
                    assert_ne!(has_many.pair, FieldId::placeholder());
                }

                if let Some(belongs_to) = field.ty.as_belongs_to() {
                    assert_ne!(belongs_to.target, ModelId::placeholder());
                    assert_ne!(belongs_to.pair, FieldId::placeholder());
                    assert_ne!(
                        belongs_to.expr_ty,
                        stmt::Type::Model(ModelId::placeholder())
                    );
                }

                if let FieldTy::Primitive(primitive) = &field.ty {
                    assert_ne!(primitive.column, ColumnId::placeholder());
                    assert_ne!(primitive.lowering, usize::MAX);
                }
            }
        }

        for table in &self.schema.inner.tables {
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
        for model in &self.schema.inner.models {
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
        for table in &self.schema.inner.tables {
            for index in &table.indices {
                assert!(!index.columns.is_empty(), "TABLE={:#?}", table);
            }
        }
    }

    fn verify_index_names_are_unique(&self) -> Result<()> {
        let mut names = HashSet::new();

        for table in &self.schema.inner.tables {
            for index in &table.indices {
                if !names.insert(&index.name) {
                    anyhow::bail!("duplicate index name `{}`", index.name);
                }
            }
        }

        Ok(())
    }

    fn verify_query_names_are_unique(&self) -> Result<()> {
        if false {
            let mut names = HashSet::new();

            for query in &self.schema.inner.queries {
                if !names.insert(&query.full_name) {
                    anyhow::bail!("duplicate query name `{}`", query.full_name);
                }
            }
        }

        Ok(())
    }

    fn verify_table_indices_and_nullable(&self) {
        for table in &self.schema.inner.tables {
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
}
