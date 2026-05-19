use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    #[key(id, revision)]
    struct User {
        #[auto]
        id: uuid::Uuid,

        revision: i64,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[index(user_id, user_revision)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,

        user_id: uuid::Uuid,
        user_revision: i64,

        #[belongs_to(key = [user_id, user_revision], references = [id, revision])]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Todo)).await
    }
}
