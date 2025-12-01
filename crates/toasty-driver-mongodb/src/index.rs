use mongodb::{Database, IndexModel};
use toasty_core::{
    schema::db::{Schema, Table},
    Result,
};

pub async fn create_indexes_for_table(
    database: &Database,
    _schema: &Schema,
    table: &Table,
) -> Result<()> {
    let collection = database.collection::<bson::Document>(&table.name);

    let mut indexes = Vec::new();

    for index in &table.indices {
        if index.primary_key {
            continue;
        }

        let mut keys = bson::Document::new();
        for column_ref in &index.columns {
            let column = table.column(column_ref.column);
            keys.insert(&column.name, 1);
        }

        let mut options = mongodb::options::IndexOptions::default();
        options.unique = Some(index.unique);

        options.name = Some(index.name.clone());

        indexes.push(IndexModel::builder().keys(keys).options(options).build());
    }

    if !indexes.is_empty() {
        collection.create_indexes(indexes).await?;
    }

    Ok(())
}
