use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: uuid::Uuid,

        title: String,

        metadata: toasty::Deferred<Metadata>,
    }

    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,
        notes: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Document)).await
    }
}
