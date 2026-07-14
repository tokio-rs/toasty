use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        /// Indexed so the planner uses FindPkByIndex to collect keys.
        #[index]
        tag: String,

        /// Not indexed; becomes the `result_filter` on UpdateByKey.
        status: String,

        name: String,

        #[version]
        version: u64,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Item)).await
    }
}
