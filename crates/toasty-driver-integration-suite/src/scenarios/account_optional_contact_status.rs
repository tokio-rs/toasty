use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        id: String,

        // Data-carrying enum.
        contact: Option<Contact>,

        // Unit-only enum.
        status: Option<Status>,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Contact {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Active,
        #[column(variant = 2)]
        Inactive,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Account)).await
    }
}
