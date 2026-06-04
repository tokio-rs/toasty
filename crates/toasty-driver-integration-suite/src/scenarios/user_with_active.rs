use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[allow(dead_code)]
        age: i64,

        #[allow(dead_code)]
        active: bool,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User)).await
    }
}
