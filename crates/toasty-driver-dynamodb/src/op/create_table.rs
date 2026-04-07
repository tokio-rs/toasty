use super::{
    AttributeDefinition, BillingMode, Connection, GlobalSecondaryIndex, Projection, ProjectionType,
    Result, Table, TypeExt, db, ddb_key_schema,
};

impl Connection {
    pub(crate) async fn create_table(
        &mut self,
        schema: &db::Schema,
        table: &Table,
        reset: bool,
    ) -> Result<()> {
        if reset {
            let _ = self
                .client
                .delete_table()
                .table_name(&table.name)
                .send()
                .await;

            for index in &table.indices {
                if !index.primary_key && index.unique {
                    let _ = self
                        .client
                        .delete_table()
                        .table_name(&index.name)
                        .send()
                        .await;
                }
            }
        }

        // Calculate which attributes need to be defined
        let mut defined_attributes = std::collections::HashSet::new();

        let pk_cols: Vec<&db::Column> = table.primary_key_columns().collect();

        assert!(
            !pk_cols.is_empty() && pk_cols.len() <= 2,
            "TABLE={table:#?}"
        );

        for col in &pk_cols {
            defined_attributes.insert(col.id);
        }

        let pk_partition_cols = &pk_cols[..1];
        let pk_range_cols = if pk_cols.len() > 1 {
            &pk_cols[1..]
        } else {
            &[][..]
        };

        let mut gsis = vec![];

        for index in &table.indices {
            if index.primary_key || index.unique {
                continue;
            }

            let partition_cols: Vec<&db::Column> = index
                .columns
                .iter()
                .filter(|ic| ic.scope.is_partition())
                .map(|ic| table.column(ic.column))
                .collect();

            let range_cols: Vec<&db::Column> = index
                .columns
                .iter()
                .filter(|ic| ic.scope.is_local())
                .map(|ic| table.column(ic.column))
                .collect();

            assert!(
                !partition_cols.is_empty() && partition_cols.len() <= 4,
                "GSI '{}' must have 1 to 4 partition (HASH) columns, got {}",
                index.name,
                partition_cols.len()
            );

            assert!(
                range_cols.len() <= 4,
                "GSI '{}' must have at most 4 range (RANGE) columns, got {}",
                index.name,
                range_cols.len()
            );

            for col in &partition_cols {
                defined_attributes.insert(col.id);
            }

            for col in &range_cols {
                defined_attributes.insert(col.id);
            }

            gsis.push(
                GlobalSecondaryIndex::builder()
                    .index_name(&index.name)
                    .set_key_schema(Some(ddb_key_schema(&partition_cols, &range_cols)))
                    .projection(
                        Projection::builder()
                            .projection_type(ProjectionType::All)
                            .build(),
                    )
                    .build()
                    .unwrap(),
            );
        }

        let attribute_definitions = defined_attributes
            .iter()
            .map(|column_id| {
                let column = table.column(*column_id);
                let ty = column.ty.to_ddb_type();

                AttributeDefinition::builder()
                    .attribute_name(&column.name)
                    .attribute_type(ty)
                    .build()
                    .unwrap()
            })
            .collect();

        self.client
            .create_table()
            .table_name(&table.name)
            .set_attribute_definitions(Some(attribute_definitions))
            .set_key_schema(Some(ddb_key_schema(pk_partition_cols, pk_range_cols)))
            .set_global_secondary_indexes(if gsis.is_empty() { None } else { Some(gsis) })
            .billing_mode(BillingMode::PayPerRequest)
            .send()
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Now, create separate tables for each unique index
        for index in table.indices.iter().filter(|i| !i.primary_key && i.unique) {
            // TODO: handle more than one column
            assert_eq!(1, index.columns.len());

            let pk = schema.column(index.columns[0].column);

            self.client
                .create_table()
                .table_name(&index.name)
                .set_key_schema(Some(ddb_key_schema(&[pk], &[])))
                .attribute_definitions(
                    AttributeDefinition::builder()
                        .attribute_name(&pk.name)
                        .attribute_type(pk.ty.to_ddb_type())
                        .build()
                        .unwrap(),
                )
                .billing_mode(BillingMode::PayPerRequest)
                .send()
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;
        }

        Ok(())
    }
}
