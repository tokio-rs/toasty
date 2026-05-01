use super::{
    AttributeDefinition, BillingMode, Connection, GlobalSecondaryIndex, Projection, ProjectionType,
    Result, Table, TypeExt, db, ddb_key_schema,
};
use toasty_core::schema::db::IndexScope;

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

        let pk_index = &table.indices[table.primary_key.index.index];
        let all_pk_cols: Vec<&db::Column> = pk_index
            .columns
            .iter()
            .map(|c| table.column(c.column))
            .collect();
        let local_cols: Vec<&db::Column> = pk_index
            .columns
            .iter()
            .filter(|c| matches!(c.scope, IndexScope::Local))
            .map(|c| table.column(c.column))
            .collect();

        // If there are Local-scoped columns (item-collection / new-style composite key),
        // the first Partition column is the hash key and Local columns become __sk.
        // Otherwise fall back to positional: first column = hash, rest = range.
        let (partition_col, range_name): (&db::Column, Option<String>) = if !local_cols.is_empty() {
            let partition_cols: Vec<&db::Column> = pk_index
                .columns
                .iter()
                .filter(|c| matches!(c.scope, IndexScope::Partition))
                .map(|c| table.column(c.column))
                .collect();
            assert_eq!(
                partition_cols.len(),
                1,
                "table '{}' must have exactly one partition key",
                table.name
            );
            let range = if local_cols.len() > 1 {
                Some("__sk".to_string())
            } else {
                Some(local_cols[0].name.clone())
            };
            (partition_cols[0], range)
        } else {
            // Legacy positional: first = hash, second (if any) = range.
            assert!(
                !all_pk_cols.is_empty(),
                "table '{}' has no primary key columns",
                table.name
            );
            let range = if all_pk_cols.len() > 1 {
                Some(all_pk_cols[1].name.clone())
            } else {
                None
            };
            (all_pk_cols[0], range)
        };

        // Collect attributes that need to be declared in the DynamoDB table schema.
        // Maps attribute name → DynamoDB type string.
        let mut defined_attributes: std::collections::HashMap<
            String,
            aws_sdk_dynamodb::types::ScalarAttributeType,
        > = std::collections::HashMap::new();
        defined_attributes.insert(partition_col.name.clone(), partition_col.ty.to_ddb_type());
        if let Some(ref rn) = range_name {
            if rn == "__sk" {
                // __sk is always a String (synthesised composite sort key).
                defined_attributes.insert(
                    "__sk".to_string(),
                    aws_sdk_dynamodb::types::ScalarAttributeType::S,
                );
            } else {
                // Single-column range key: find it by name in all_pk_cols.
                let range_col = all_pk_cols
                    .iter()
                    .find(|c| c.name == *rn)
                    .expect("range column must be in pk_cols");
                defined_attributes.insert(range_col.name.clone(), range_col.ty.to_ddb_type());
            }
        }

        let mut gsis = vec![];

        for index in &table.indices {
            if index.primary_key {
                continue;
            }

            if index.unique {
                // Unique indices are materialized as separate tables below. Each
                // index table is keyed on a single column and enforced with an
                // `attribute_not_exists` condition, so composite (multi-column)
                // unique indices are not supported. Reject them here, before any
                // table is created, rather than partway through. SQL backends
                // support them via `CREATE UNIQUE INDEX`.
                if index.columns.len() != 1 {
                    return Err(toasty_core::Error::unsupported_feature(format!(
                        "DynamoDB does not support composite unique indices; \
                         index '{}' spans {} columns",
                        index.name,
                        index.columns.len()
                    )));
                }

                continue;
            }

            let gsi_partition_cols: Vec<&db::Column> = index
                .columns
                .iter()
                .filter(|ic| ic.scope.is_partition())
                .map(|ic| table.column(ic.column))
                .collect();

            let gsi_range_cols: Vec<&db::Column> = index
                .columns
                .iter()
                .filter(|ic| ic.scope.is_local())
                .map(|ic| table.column(ic.column))
                .collect();

            if gsi_partition_cols.is_empty() || gsi_partition_cols.len() > 4 {
                return Err(toasty_core::Error::invalid_schema(format!(
                    "GSI '{}' must have 1 to 4 partition (HASH) columns, got {}",
                    index.name,
                    gsi_partition_cols.len()
                )));
            }

            if gsi_range_cols.len() > 4 {
                return Err(toasty_core::Error::invalid_schema(format!(
                    "GSI '{}' must have at most 4 range (RANGE) columns, got {}",
                    index.name,
                    gsi_range_cols.len()
                )));
            }

            for col in &gsi_partition_cols {
                defined_attributes
                    .entry(col.name.clone())
                    .or_insert_with(|| col.ty.to_ddb_type());
            }
            for col in &gsi_range_cols {
                defined_attributes
                    .entry(col.name.clone())
                    .or_insert_with(|| col.ty.to_ddb_type());
            }

            let gsi_partition_names: Vec<&str> =
                gsi_partition_cols.iter().map(|c| c.name.as_str()).collect();
            let gsi_range_names: Vec<&str> =
                gsi_range_cols.iter().map(|c| c.name.as_str()).collect();

            gsis.push(
                GlobalSecondaryIndex::builder()
                    .index_name(&index.name)
                    .set_key_schema(Some(ddb_key_schema(&gsi_partition_names, &gsi_range_names)))
                    .projection(
                        Projection::builder()
                            .projection_type(ProjectionType::All)
                            .build(),
                    )
                    .build()
                    .unwrap(),
            );
        }

        let attribute_definitions: Vec<_> = defined_attributes
            .iter()
            .map(|(name, ty)| {
                AttributeDefinition::builder()
                    .attribute_name(name)
                    .attribute_type(ty.clone())
                    .build()
                    .unwrap()
            })
            .collect();

        self.client
            .create_table()
            .table_name(&table.name)
            .set_attribute_definitions(Some(attribute_definitions))
            .set_key_schema(Some(ddb_key_schema(
                &[partition_col.name.as_str()],
                &range_name.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )))
            .set_global_secondary_indexes(if gsis.is_empty() { None } else { Some(gsis) })
            .billing_mode(BillingMode::PayPerRequest)
            .send()
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Now, create separate tables for each unique index. Composite unique
        // indices were already rejected above, so each index has one column.
        for index in table.indices.iter().filter(|i| !i.primary_key && i.unique) {
            let pk = schema.column(index.columns[0].column);

            self.client
                .create_table()
                .table_name(&index.name)
                .set_key_schema(Some(ddb_key_schema(&[pk.name.as_str()], &[])))
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
