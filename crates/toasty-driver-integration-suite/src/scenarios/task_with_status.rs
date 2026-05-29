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
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Failed { reason: String },
        #[column(variant = 3)]
        Done,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Task)).await
    }
}
