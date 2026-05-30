use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        contact: ContactInfo,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User)).await
    }
}
