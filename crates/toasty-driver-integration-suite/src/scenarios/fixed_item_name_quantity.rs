use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,

        name: String,

        quantity: i64,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Item)).await
    }
}
