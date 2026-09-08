//! Single `Item` model with one of each filterable type the `IN`-list tests
//! exercise: a String column (`name`), a numeric column (`n`), an Option
//! column (`bio`), and the auto-generated `id` (whose concrete type varies
//! by ID variant — `u64` or `uuid::Uuid`).

use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        name: String,

        #[index]
        n: i64,

        bio: Option<String>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Item)).await
    }
}
