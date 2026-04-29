use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[version]
        version: u64,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Item)).await
    }
}
