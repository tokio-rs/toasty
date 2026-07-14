use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct Person {
        #[key]
        #[auto]
        id: ID,

        name: String,

        contact: ContactInfo,
    }

    #[derive(Debug, toasty::Embed)]
    enum ContactInfo {
        Email { address: String, metadata: Metadata },
        Phone { number: String },
    }

    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,
        notes: toasty::Deferred<String>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Person)).await
    }
}
