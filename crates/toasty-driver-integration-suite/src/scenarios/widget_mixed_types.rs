use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct Widget {
        #[key]
        #[auto]
        id: ID,

        label: String,
        count: i64,
        active: bool,
        description: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Widget)).await
    }
}
