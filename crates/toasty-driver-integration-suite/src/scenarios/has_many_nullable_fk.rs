use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        user_id: Option<uuid::Uuid>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,

        title: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Todo)).await
    }
}
