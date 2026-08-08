use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Document {
        #[key]
        id: String,

        metadata: Option<Metadata>,
    }

    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        author: String,
        note: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Document)).await
    }
}
