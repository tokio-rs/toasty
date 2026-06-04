use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: ID,

        title: String,

        // Newtype embed whose single field is deferred — the nullable head
        // reuses that one (deferred) leaf column.
        body: Option<Body>,
    }

    #[derive(Debug, toasty::Embed)]
    struct Body(toasty::Deferred<String>);

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Document)).await
    }
}
