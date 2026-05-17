use crate::prelude::*;

scenario! {
    /// Multi-relation chain scenario with composite keys at two of the three
    /// hops:
    ///
    /// - `User` has a single-column auto PK.
    /// - `Todo` has a single-column PK, a single-column FK back to `User`,
    ///   and a *composite* FK `(category_id, category_revision)` pointing at
    ///   `Category`.
    /// - `Category` has a *composite* PK `(id, revision)`.
    ///
    /// This lets the chain tests exercise composite-key handling at two
    /// distinct positions:
    ///
    /// - `user.todos().category()` — the second hop is a `BelongsTo` with a
    ///   composite FK.
    /// - `category.todos()` — the first hop is a `HasMany` whose paired
    ///   `BelongsTo` (on `Todo`) is composite.
    /// - `category.todos().user()` — chains the composite-pair first hop into
    ///   a single-column second hop.

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[index(category_id, category_revision)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        category_id: uuid::Uuid,
        category_revision: i64,

        #[belongs_to(key = [category_id, category_revision], references = [id, revision])]
        category: toasty::BelongsTo<Category>,

        title: String,
    }

    #[derive(Debug, toasty::Model)]
    #[key(id, revision)]
    struct Category {
        id: uuid::Uuid,
        revision: i64,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Todo, Category)).await
    }
}
