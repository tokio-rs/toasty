use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: uuid::Uuid,

        title: String,

        priority: Priority,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Bug {
        #[key]
        #[auto]
        id: uuid::Uuid,

        summary: String,

        priority: Priority,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Priority {
        Low,
        Medium,
        High,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Task, Bug)).await
    }
}
