use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        title: String,

        metadata: Metadata,
    }

    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,
        notes: toasty::Deferred<String>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Document)).await
    }
}
