use super::*;

pub struct SetupSqlite;

#[async_trait::async_trait]
impl Setup for SetupSqlite {
    async fn setup(&self, schema: Schema) -> Db {
        let driver = toasty_sqlite::Sqlite::in_memory();
        let db = toasty::Db::new(schema, driver).await.unwrap();
        db.reset_db().await.unwrap();
        db
    }

    fn capability(&self) -> &Capability {
        &Capability::Sql
    }
}
