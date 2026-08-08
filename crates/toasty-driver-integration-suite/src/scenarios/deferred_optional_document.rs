use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: uuid::Uuid,

        title: String,
        summary: toasty::Deferred<Option<String>>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Document)).await
    }
}
