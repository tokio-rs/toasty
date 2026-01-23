use super::{
    ddb_key_schema, AttributeDefinition, Connection, GlobalSecondaryIndex, Projection,
    ProjectionType, ProvisionedThroughput, Result, Schema, Table, TypeExt,
};

impl Connection {
    pub(crate) async fn create_table(
        &mut self,
        schema: &Schema,
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

        let pt = ProvisionedThroughput::builder()
            .read_capacity_units(10)
            .write_capacity_units(5)
            .build()
            .unwrap();

        // Calculate which attributes need to be defined
        let mut defined_attributes = std::collections::HashSet::new();

        let mut pk_columns = table.primary_key_columns();

        // TODO: for now, up to 2 columns are supported as part of the PK.
        assert!(
            pk_columns.len() >= 1 && pk_columns.len() <= 2,
            "TABLE={table:#?}"
        );

        let partition_column = pk_columns.next().unwrap();
        defined_attributes.insert(partition_column.id);

        let range_column = pk_columns.next();

        if let Some(range_column) = &range_column {
            defined_attributes.insert(range_column.id);
        }

        let mut gsis = vec![];

        for index in &table.indices {
            if index.primary_key || index.unique {
                continue;
            }

            assert_eq!(1, index.columns.len());
            let field = &table.column(index.columns[0].column);
            defined_attributes.insert(field.id);

            gsis.push(
                GlobalSecondaryIndex::builder()
                    .index_name(&index.name)
                    .set_key_schema(Some(ddb_key_schema(field, None)))
                    .projection(
                        Projection::builder()
                            .projection_type(ProjectionType::All)
                            .build(),
                    )
                    .provisioned_throughput(pt.clone())
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
            .set_key_schema(Some(ddb_key_schema(partition_column, range_column)))
            .set_global_secondary_indexes(if gsis.is_empty() { None } else { Some(gsis) })
            .provisioned_throughput(pt.clone())
            .send()
            .await
            .map_err(toasty_core::Error::driver)?;

        // Now, create separate tables for each unique index
        for index in table.indices.iter().filter(|i| !i.primary_key && i.unique) {
            // TODO: handle more than one column
            assert_eq!(1, index.columns.len());

            let pk = schema.column(index.columns[0].column);

            self.client
                .create_table()
                .table_name(&index.name)
                .set_key_schema(Some(ddb_key_schema(pk, None)))
                .attribute_definitions(
                    AttributeDefinition::builder()
                        .attribute_name(&pk.name)
                        .attribute_type(pk.ty.to_ddb_type())
                        .build()
                        .unwrap(),
                )
                .provisioned_throughput(pt.clone())
                .send()
                .await
                .map_err(toasty_core::Error::driver)?;
        }

        Ok(())
    }
}
