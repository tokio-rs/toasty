use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        title: String,

        metadata: Option<Metadata>,
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
