use toasty::stmt::Id;

#[derive(Debug, toasty::Model)]
pub struct User {
    #[key]
    #[auto]
    pub id: Id<Self>,

    pub name: String,

    #[unique]
    pub email: String,

    #[has_many]
    pub todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
pub struct Todo {
    #[key]
    #[auto]
    pub id: Id<Self>,

    #[index]
    pub user_id: Id<User>,

    #[belongs_to(key = user_id, references = id)]
    pub user: toasty::BelongsTo<User>,

    #[index]
    pub title: String,

    pub completed: bool,
}

/// Helper function to create a database instance with the schema
pub async fn create_db() -> toasty::Result<toasty::Db> {
    let db = toasty::Db::builder()
        .register::<User>()
        .register::<Todo>()
        .connect("sqlite:./test.db")
        .await?;

    Ok(db)
}
