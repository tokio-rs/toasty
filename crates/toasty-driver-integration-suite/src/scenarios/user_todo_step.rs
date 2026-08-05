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

        title: String,

        #[index]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,

        #[has_many]
        steps: toasty::Deferred<Vec<Step>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Step {
        #[key]
        #[auto]
        id: uuid::Uuid,

        description: String,

        #[index]
        todo_id: uuid::Uuid,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::Deferred<Todo>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Todo, Step)).await
    }
}
