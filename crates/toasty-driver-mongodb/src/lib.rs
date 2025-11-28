mod aggregation;
mod index;
mod op;
mod value;

use toasty_core::{
    driver::{operation::Operation, Capability, Driver, Response},
    schema::db::Schema,
};

use anyhow::Result;
use mongodb::{Client, Database};
use std::sync::Arc;
use url::Url;

#[derive(Debug)]
pub struct MongoDb {
    client: Client,
    database: Database,
}

impl MongoDb {
    pub fn new(client: Client, database: Database) -> Self {
        Self { client, database }
    }

    pub async fn connect(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;

        if url.scheme() != "mongodb" {
            return Err(anyhow::anyhow!(
                "connection URL does not have a `mongodb` scheme; url={url}"
            ));
        }

        let client = Client::with_uri_str(url.as_str()).await?;

        let db_name = url
            .path()
            .trim_start_matches('/')
            .split('?')
            .next()
            .unwrap_or("toasty");

        let database = client.database(db_name);

        Ok(Self { client, database })
    }
}

#[toasty_core::async_trait]
impl Driver for MongoDb {
    fn capability(&self) -> &Capability {
        &Capability::MONGODB
    }

    async fn register_schema(&mut self, schema: &Schema) -> Result<()> {
        for table in &schema.tables {
            index::create_indexes_for_table(&self.database, schema, table).await?;
        }
        Ok(())
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        op::execute_operation(self, schema, op).await
    }

    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        for table in &schema.tables {
            let collection_name = &table.name;

            self.database
                .collection::<bson::Document>(collection_name)
                .drop()
                .await
                .ok();

            index::create_indexes_for_table(&self.database, schema, table).await?;
        }
        Ok(())
    }
}
