use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: ID,

        title: String,

        status: Status,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        #[column(variant = "pending")]
        Pending,
        #[column(variant = "active")]
        Active,
        #[column(variant = "done")]
        Done,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Task)).await
    }
}
