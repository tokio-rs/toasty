use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct Company {
        #[key]
        #[auto]
        id: ID,

        name: String,

        headquarters: Office,
    }

    #[derive(Debug, toasty::Embed)]
    struct Office {
        name: String,
        address: Address,
    }

    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Company)).await
    }
}
