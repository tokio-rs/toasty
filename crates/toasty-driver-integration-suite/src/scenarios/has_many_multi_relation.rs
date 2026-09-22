use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,

        #[index]
        category_id: uuid::Uuid,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::Deferred<Category>,

        title: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Category {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Todo, Category)).await
    }
}
