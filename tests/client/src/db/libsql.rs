use toasty::driver::Capability;
use toasty::schema::app::Schema;
use toasty::Db;

use crate::Setup;

pub struct SetupLibSQL;

#[async_trait::async_trait]
impl Setup for SetupLibSQL {
    async fn setup(&self, schema: Schema) -> Db {
        let driver = toasty_libsql::LibSQL::local(":memory:".to_string())
            .await
            .unwrap();
        let db = toasty::Db::new(schema, driver).await.unwrap();
        db.reset_db().await.unwrap();
        db
    }

    fn capability(&self) -> &Capability {
        &Capability::Sql
    }
}
