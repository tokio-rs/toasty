use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    #[unique(tenant, slug)]
    struct Entry {
        #[key]
        #[auto]
        id: ID,

        tenant: String,
        slug: String,
        value: String,
    }

    #[derive(Debug, toasty::Model)]
    struct DefaultedItem {
        #[key]
        #[auto]
        id: ID,

        value: String,

        #[default("created".to_string())]
        created_only: String,

        #[update("always".to_string())]
        updated_always: String,
    }

    #[derive(Debug, toasty::Model)]
    struct AssignedItem {
        #[key]
        #[auto]
        id: ID,

        count: i64,
        tags: Vec<String>,
        note: Option<String>,
    }

    async fn setup_entry(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Entry)).await
    }

    async fn setup_defaulted_item(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(DefaultedItem)).await
    }

    async fn setup_assigned_item(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(AssignedItem)).await
    }
}
