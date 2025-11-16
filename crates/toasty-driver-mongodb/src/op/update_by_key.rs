use crate::{value, MongoDb};
use toasty_core::{
    driver::{operation::UpdateByKey, Response},
    schema::db::Schema,
    stmt,
    Result,
};
use std::sync::Arc;

pub async fn execute(
    driver: &MongoDb,
    schema: &Arc<Schema>,
    op: UpdateByKey,
) -> Result<Response> {
    let table = schema.table(op.table);
    let collection = driver.database.collection::<bson::Document>(&table.name);

    // Build update document using $set operator
    let mut set_doc = bson::Document::new();

    for (index, assignment) in op.assignments.iter() {
        // The index represents the column index in the table
        let column = &table.columns[index];
        let value = match &assignment.expr {
            stmt::Expr::Value(value) => value,
            _ => todo!("non-value assignment expressions: {:?}", assignment.expr),
        };
        set_doc.insert(&column.name, value::to_bson(value));
    }

    let mut update_doc = bson::Document::new();
    update_doc.insert("$set", set_doc);

    if op.keys.len() == 1 {
        // Single key update
        let key = &op.keys[0];
        let mut filter = bson::Document::new();
        filter.insert("_id", value::to_bson(key));

        // Add filter conditions if present
        if let Some(filter_expr) = &op.filter {
            // TODO: Convert filter expression to MongoDB query
            todo!("filter expression conversion: {:?}", filter_expr);
        }

        if let Some(condition_expr) = &op.condition {
            // TODO: Convert condition expression to MongoDB query
            todo!("condition expression conversion: {:?}", condition_expr);
        }

        let result = collection.update_one(filter, update_doc).await?;
        Ok(Response::count(result.modified_count))
    } else {
        // Multiple keys update
        let mut key_values = Vec::new();
        for key in &op.keys {
            key_values.push(value::to_bson(key));
        }

        let mut filter = bson::Document::new();
        filter.insert("_id", bson::doc! { "$in": key_values });

        let result = collection.update_many(filter, update_doc).await?;
        Ok(Response::count(result.modified_count))
    }
}
