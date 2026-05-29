use crate::prelude::*;

scenario! {
    //! A 3-step has_many via chain: `User` → `Organization` → `Project` → `Todo`.
    //!
    //! Used by tests that need a `via` path longer than the 2-step
    //! `user_comment_article` scenario.
    //!
    //! `User::todos` is the flat 3-step via (`organizations.projects.todos`).
    //! `User::nested_todos` reaches the same todos through `organizations.todos`,
    //! where `Organization::todos` is itself a via — a via-of-via, used to test
    //! recursive flattening of a via path whose step names another via.

    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        organizations: toasty::Deferred<Vec<Organization>>,

        // User → organizations → projects → todos
        #[has_many(via = organizations.projects.todos)]
        todos: toasty::Deferred<Vec<Todo>>,

        // User → organizations → Organization::todos, which is itself a via.
        // The second step expands into another via path (via-of-via).
        #[has_many(via = organizations.todos)]
        nested_todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Organization {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,

        #[has_many]
        projects: toasty::Deferred<Vec<Project>>,

        // Organization → projects → todos. The via that `User::nested_todos`
        // routes through, making that relation a via-of-via.
        #[has_many(via = projects.todos)]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Project {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        organization_id: ID,

        #[belongs_to(key = organization_id, references = id)]
        organization: toasty::Deferred<Organization>,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        project_id: ID,

        #[belongs_to(key = project_id, references = id)]
        project: toasty::Deferred<Project>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Organization, Project, Todo)).await
    }
}
