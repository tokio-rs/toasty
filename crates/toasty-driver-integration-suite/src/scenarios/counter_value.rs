use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct Counter {
        #[key]
        id: uuid::Uuid,

        value: i64,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Counter)).await
    }
}
