use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,

        #[has_many(via = todos.tags.name)]
        tag_names: toasty::Deferred<Vec<String>>,

        #[has_many(via = todos.tags.name)]
        eager_tag_names: Vec<String>,

        #[has_one]
        profile: toasty::Deferred<Option<Profile>>,

        #[has_one(via = profile.display_name)]
        display_name: toasty::Deferred<String>,

        #[has_one(via = profile.nickname)]
        nickname: toasty::Deferred<Option<String>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,

        #[has_many]
        tags: toasty::Deferred<Vec<Tag>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Tag {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::Deferred<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        display_name: String,

        nickname: Option<String>,

        #[unique]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Todo, Tag, Profile)).await
    }
}
