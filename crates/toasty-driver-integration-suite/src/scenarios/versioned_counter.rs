use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct Counter {
        #[key]
        #[auto]
        id: uuid::Uuid,

        value: i64,

        #[version]
        version: u64,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Counter)).await
    }
}
