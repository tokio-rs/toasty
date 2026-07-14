use crate::prelude::*;
use serde::{Deserialize, Serialize};

scenario! {
    #![id(ID)]

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Payload {
        name: String,
        version: u32,
    }

    #[derive(Debug, toasty::Model)]
    struct Repository {
        #[key]
        #[auto]
        id: ID,

        name: String,
        payload: toasty::Deferred<toasty::Json<Payload>>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Repository)).await
    }
}
