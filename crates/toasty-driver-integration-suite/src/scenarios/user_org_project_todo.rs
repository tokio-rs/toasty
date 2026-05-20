use crate::prelude::*;

scenario! {
    //! A 3-step has_many via chain: `User` → `Organization` → `Project` → `Todo`.
    //!
    //! Used by tests that need a `via` path longer than the 2-step
    //! `user_comment_article` scenario.

    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        organizations: toasty::HasMany<Organization>,

        // User → organizations → projects → todos
        #[has_many(via = organizations.projects.todos)]
        todos: toasty::HasMany<Todo>,
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
        user: toasty::BelongsTo<User>,

        #[has_many]
        projects: toasty::HasMany<Project>,
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
        organization: toasty::BelongsTo<Organization>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
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
        project: toasty::BelongsTo<Project>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Organization, Project, Todo)).await
    }
}
