use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        id: String,

        address: Address,
    }

    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User)).await
    }
}
