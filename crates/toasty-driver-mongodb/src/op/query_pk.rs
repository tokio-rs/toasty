use crate::{value, MongoDb};
use toasty_core::{
    driver::{operation::QueryPk, Response},
    schema::db::Schema,
    stmt,
    Result,
};
use std::sync::Arc;
use futures::stream::StreamExt;

pub async fn execute(driver: &MongoDb, schema: &Arc<Schema>, op: QueryPk) -> Result<Response> {
    let table = schema.table(op.table);
    let collection = driver.database.collection::<bson::Document>(&table.name);

    // Build filter from pk_filter expression
    let filter = build_filter_document(&op.pk_filter, schema, table)?;

    // Build projection for selected columns
    let mut projection = bson::Document::new();
    for column_id in &op.select {
        let column = schema.column(*column_id);
        let field_name = if table.primary_key_columns().any(|pk| pk.id == *column_id) {
            "_id".to_string()
        } else {
            column.name.clone()
        };
        projection.insert(field_name, 1);
    }

    let options = mongodb::options::FindOptions::builder()
        .projection(projection)
        .build();

    let cursor = collection.find(filter).with_options(options).await?;
    let docs: Vec<_> = cursor.collect().await;

    let schema = schema.clone();
    let select = op.select.clone();

    Ok(Response::value_stream(stmt::ValueStream::from_iter(
        docs.into_iter().map(move |result| {
            result
                .map_err(|e| anyhow::anyhow!("MongoDB error: {}", e))
                .and_then(|doc| document_to_record(&doc, &select, &schema))
        }),
    )))
}

fn build_filter_document(
    expr: &stmt::Expr,
    _schema: &Schema,
    _table: &toasty_core::schema::db::Table,
) -> Result<bson::Document> {
    // For now, implement basic expression conversion
    // TODO: Full expression conversion for complex queries
    match expr {
        stmt::Expr::Value(val) => {
            let mut doc = bson::Document::new();
            doc.insert("_id", value::to_bson(val));
            Ok(doc)
        }
        stmt::Expr::BinaryOp(binary_op) => {
            // Handle binary operations like equality, comparison, etc.
            todo!("Binary operation conversion: {:?}", binary_op)
        }
        _ => {
            todo!("Expression conversion not yet implemented: {:?}", expr)
        }
    }
}

fn document_to_record(
    doc: &bson::Document,
    select: &[toasty_core::schema::db::ColumnId],
    schema: &Schema,
) -> Result<stmt::Value> {
    let mut values = Vec::new();

    for column_id in select {
        let column = schema.column(*column_id);
        let field_name = if doc.contains_key("_id") && column.name == "_id" {
            "_id"
        } else {
            &column.name
        };

        let bson_value = doc.get(field_name).unwrap_or(&bson::Bson::Null);
        let value = value::from_bson(bson_value, &column.ty);
        values.push(value);
    }

    Ok(stmt::Value::Record(stmt::ValueRecord { fields: values }))
}
