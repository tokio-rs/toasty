use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User)).await
    }
}
