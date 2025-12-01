use crate::{value, MongoDb};
use toasty_core::{
    driver::{operation::Insert, Response},
    schema::db::Schema,
    stmt,
    Result,
};
use std::sync::Arc;

pub async fn execute(driver: &MongoDb, schema: &Arc<Schema>, op: Insert) -> Result<Response> {
    // Extract the INSERT statement from the operation
    let insert_stmt = match op.stmt {
        stmt::Statement::Insert(insert) => insert,
        _ => return Err(anyhow::anyhow!("Expected Insert statement")),
    };

    // Get the target table
    let insert_table = insert_stmt.target.as_table_unwrap();
    let table = &schema.table(insert_table.table);

    // Get the MongoDB collection
    let collection = driver.database.collection::<bson::Document>(&table.name);

    // Extract rows from the insert source
    let source = insert_stmt.source.body.into_values();
    let mut documents = Vec::new();

    for row in source.rows {
        let mut doc = bson::Document::new();

        // Map each column value to the document
        for (i, column_id) in insert_table.columns.iter().enumerate() {
            let column = schema.column(*column_id);
            let entry = row.entry(i);
            let value = entry.as_value();

            // Skip null values (MongoDB handles missing fields gracefully)
            if !value.is_null() {
                let bson_value = value::to_bson(value);

                // Use "_id" for primary key field
                let field_name = if table.primary_key_columns().any(|pk| pk.id == *column_id) {
                    "_id".to_string()
                } else {
                    column.name.clone()
                };

                doc.insert(field_name, bson_value);
            }
        }

        documents.push(doc);
    }

    let count = documents.len();

    // Insert documents
    if count == 1 {
        // Single insert
        collection.insert_one(documents.into_iter().next().unwrap()).await?;
    } else if count > 1 {
        // Batch insert
        collection.insert_many(documents).await?;
    }

    Ok(Response::count(count as u64))
}
