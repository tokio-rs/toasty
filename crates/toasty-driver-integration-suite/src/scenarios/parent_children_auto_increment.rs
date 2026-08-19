use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto(increment)]
        id: u32,

        #[has_many]
        children: toasty::Deferred<Vec<Child>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        #[auto(increment)]
        id: u32,

        #[index]
        parent_id: u32,

        #[belongs_to(key = parent_id, references = id)]
        #[allow(dead_code)]
        parent: toasty::Deferred<Parent>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Parent, Child)).await
    }
}
