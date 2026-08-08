use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Document {
        #[key]
        id: String,

        // Single-field newtype embed: flattens to one column named after the
        // field, so the nullable head reuses that column rather than a
        // dedicated (and colliding) presence column.
        code: Option<Code>,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    struct Code(String);

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Document)).await
    }
}
