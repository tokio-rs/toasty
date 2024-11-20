use toasty::codegen_support::*;
#[derive(Debug)]
pub struct Todo {
    pub id: Id<Todo>,
    pub user_id: Id<super::user::User>,
    user: BelongsTo<super::user::User>,
    pub title: String,
}
impl Todo {
    pub const ID: Path<Id<Todo>> = Path::from_field_index::<Self>(0);
    pub const USER_ID: Path<Id<super::user::User>> = Path::from_field_index::<Self>(1);
    pub const USER: self::fields::User =
        self::fields::User::from_path(Path::from_field_index::<Self>(2));
    pub const TITLE: Path<String> = Path::from_field_index::<Self>(3);
    pub fn create() -> CreateTodo {
        CreateTodo::default()
    }
    pub fn create_many() -> CreateMany<Todo> {
        CreateMany::default()
    }
    pub fn filter(expr: stmt::Expr<bool>) -> Query {
        Query::from_stmt(stmt::Select::from_expr(expr))
    }
    pub fn update(&mut self) -> UpdateTodo<'_> {
        let query = UpdateQuery::from(self.into_select());
        UpdateTodo { model: self, query }
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        let stmt = self.into_select().delete();
        db.exec(stmt).await?;
        Ok(())
    }
}
impl Model for Todo {
    const ID: ModelId = ModelId(1);
    type Key = Id<Todo>;
    fn load(mut record: Record) -> Result<Self, Error> {
        Ok(Todo {
            id: Id::from_untyped(record[0].take().to_id()?),
            user_id: Id::from_untyped(record[1].take().to_id()?),
            user: BelongsTo::load(record[2].take())?,
            title: record[3].take().to_string()?,
        })
    }
}
impl stmt::IntoSelect for &Todo {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Todo::find_by_id(&self.id).into_select()
    }
}
impl stmt::IntoSelect for &mut Todo {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<Self::Model> {
        (&*self).into_select()
    }
}
impl stmt::AsSelect for Todo {
    type Model = Todo;
    fn as_select(&self) -> stmt::Select<Self::Model> {
        Todo::find_by_id(&self.id).into_select()
    }
}
impl stmt::IntoSelect for Todo {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Todo::find_by_id(self.id).into_select()
    }
}
impl stmt::IntoExpr<Todo> for &Todo {
    fn into_expr(self) -> stmt::Expr<Todo> {
        stmt::Key::from_expr(&self.id).into()
    }
}
impl stmt::IntoExpr<[Todo]> for &Todo {
    fn into_expr(self) -> stmt::Expr<[Todo]> {
        stmt::Key::from_expr(&self.id).into()
    }
}
#[derive(Debug)]
pub struct Query {
    stmt: stmt::Select<Todo>,
}
impl Query {
    pub const fn from_stmt(stmt: stmt::Select<Todo>) -> Query {
        Query { stmt }
    }
    pub async fn all(self, db: &Db) -> Result<Cursor<Todo>> {
        db.all(self.stmt).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<Todo>> {
        db.first(self.stmt).await
    }
    pub async fn get(self, db: &Db) -> Result<Todo> {
        db.get(self.stmt).await
    }
    pub fn update(self) -> UpdateQuery {
        UpdateQuery::from(self)
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        db.exec(self.stmt.delete()).await?;
        Ok(())
    }
    pub async fn collect<A>(self, db: &Db) -> Result<A>
    where
        A: FromCursor<Todo>,
    {
        self.all(db).await?.collect().await
    }
    pub fn filter(self, expr: stmt::Expr<bool>) -> Query {
        Query {
            stmt: self.stmt.and(expr),
        }
    }
    pub fn user(mut self) -> super::user::Query {
        todo!()
    }
}
impl stmt::IntoSelect for Query {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<Todo> {
        self.stmt
    }
}
impl stmt::IntoSelect for &Query {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<Todo> {
        self.stmt.clone()
    }
}
impl Default for Query {
    fn default() -> Query {
        Query {
            stmt: stmt::Select::all(),
        }
    }
}
#[derive(Debug)]
pub struct CreateTodo {
    pub(super) stmt: stmt::Insert<Todo>,
}
impl CreateTodo {
    pub fn id(mut self, id: impl Into<Id<Todo>>) -> Self {
        self.stmt.set_value(0, id.into());
        self
    }
    pub fn user_id(mut self, user_id: impl Into<Id<super::user::User>>) -> Self {
        self.stmt.set_value(1, user_id.into());
        self
    }
    pub fn user<'b>(mut self, user: impl IntoExpr<self::relation::User<'b>>) -> Self {
        self.stmt.set_expr(2, user.into_expr());
        self
    }
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.stmt.set_value(3, title.into());
        self
    }
    pub async fn exec(self, db: &Db) -> Result<Todo> {
        db.exec_insert_one(self.stmt).await
    }
}
impl IntoInsert for CreateTodo {
    type Model = Todo;
    fn into_insert(self) -> stmt::Insert<Todo> {
        self.stmt
    }
}
impl IntoExpr<Todo> for CreateTodo {
    fn into_expr(self) -> stmt::Expr<Todo> {
        self.stmt.into()
    }
}
impl IntoExpr<[Todo]> for CreateTodo {
    fn into_expr(self) -> stmt::Expr<[Todo]> {
        self.stmt.into_list_expr()
    }
}
impl Default for CreateTodo {
    fn default() -> CreateTodo {
        CreateTodo {
            stmt: stmt::Insert::blank(),
        }
    }
}
#[derive(Debug)]
pub struct UpdateTodo<'a> {
    model: &'a mut Todo,
    query: UpdateQuery,
}
#[derive(Debug)]
pub struct UpdateQuery {
    stmt: stmt::Update<Todo>,
}
impl UpdateTodo<'_> {
    pub fn id(mut self, id: impl Into<Id<Todo>>) -> Self {
        self.query.set_id(id);
        self
    }
    pub fn user_id(mut self, user_id: impl Into<Id<super::user::User>>) -> Self {
        self.query.set_user_id(user_id);
        self
    }
    pub fn user<'b>(mut self, user: impl IntoExpr<self::relation::User<'b>>) -> Self {
        self.query.set_user(user);
        self
    }
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.query.set_title(title);
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let mut stmt = self.query.stmt;
        let mut result = db.exec_one(stmt.into()).await?;
        for (field, value) in result.into_sparse_record().into_iter() {
            match field.into_usize() {
                0 => self.model.id = stmt::Id::from_untyped(value.to_id()?),
                1 => self.model.user_id = stmt::Id::from_untyped(value.to_id()?),
                2 => todo!("should not be set; {} = {value:#?}", 2),
                3 => self.model.title = value.to_string()?,
                _ => todo!("handle unknown field id in reload after update"),
            }
        }
        Ok(())
    }
}
impl UpdateQuery {
    pub fn id(mut self, id: impl Into<Id<Todo>>) -> Self {
        self.set_id(id);
        self
    }
    pub fn set_id(&mut self, id: impl Into<Id<Todo>>) -> &mut Self {
        self.stmt.set_expr(0, id.into());
        self
    }
    pub fn user_id(mut self, user_id: impl Into<Id<super::user::User>>) -> Self {
        self.set_user_id(user_id);
        self
    }
    pub fn set_user_id(&mut self, user_id: impl Into<Id<super::user::User>>) -> &mut Self {
        self.stmt.set_expr(1, user_id.into());
        self
    }
    pub fn user<'b>(mut self, user: impl IntoExpr<self::relation::User<'b>>) -> Self {
        self.set_user(user);
        self
    }
    pub fn set_user<'b>(&mut self, user: impl IntoExpr<self::relation::User<'b>>) -> &mut Self {
        self.stmt.set_expr(2, user.into_expr());
        self
    }
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.set_title(title);
        self
    }
    pub fn set_title(&mut self, title: impl Into<String>) -> &mut Self {
        self.stmt.set_expr(3, title.into());
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let stmt = self.stmt;
        let mut cursor = db.exec(stmt.into()).await?;
        Ok(())
    }
}
impl From<Query> for UpdateQuery {
    fn from(value: Query) -> UpdateQuery {
        UpdateQuery {
            stmt: stmt::Update::new(value),
        }
    }
}
impl From<stmt::Select<Todo>> for UpdateQuery {
    fn from(src: stmt::Select<Todo>) -> UpdateQuery {
        UpdateQuery {
            stmt: stmt::Update::new(src),
        }
    }
}
pub mod fields {
    use super::*;
    pub struct User {
        pub(super) path: Path<super::super::user::User>,
    }
    impl User {
        pub const fn from_path(path: Path<super::super::user::User>) -> User {
            User { path }
        }
        pub fn id(mut self) -> Path<Id<super::super::user::User>> {
            self.path.chain(super::super::user::User::ID)
        }
        pub fn name(mut self) -> Path<String> {
            self.path.chain(super::super::user::User::NAME)
        }
        pub fn email(mut self) -> Path<String> {
            self.path.chain(super::super::user::User::EMAIL)
        }
        pub fn todos(mut self) -> super::super::user::fields::Todos {
            let path = self.path.chain(super::super::user::User::TODOS);
            super::super::user::fields::Todos::from_path(path)
        }
        pub fn moto(mut self) -> Path<String> {
            self.path.chain(super::super::user::User::MOTO)
        }
        pub fn eq<'a, T>(self, rhs: T) -> stmt::Expr<bool>
        where
            T: toasty::stmt::IntoExpr<super::relation::user::User<'a>>,
        {
            self.path.eq(rhs.into_expr().cast())
        }
        pub fn in_query<Q>(self, rhs: Q) -> toasty::stmt::Expr<bool>
        where
            Q: stmt::IntoSelect<Model = super::super::user::User>,
        {
            self.path.in_query(rhs)
        }
    }
    impl From<User> for Path<super::super::user::User> {
        fn from(val: User) -> Path<super::super::user::User> {
            val.path
        }
    }
    impl<'a> stmt::IntoExpr<super::relation::user::User<'a>> for User {
        fn into_expr(self) -> stmt::Expr<super::relation::user::User<'a>> {
            todo!("into_expr for {} (field path struct)", stringify!(User));
        }
    }
}
pub mod relation {
    use super::*;
    use toasty::Cursor;
    pub mod user {
        use super::*;
        #[derive(Debug)]
        pub struct User<'a> {
            scope: &'a Todo,
        }
        impl super::Todo {
            pub fn user(&self) -> User<'_> {
                User { scope: self }
            }
        }
        impl User<'_> {
            pub fn get(&self) -> &super::super::super::user::User {
                self.scope.user.get()
            }
        }
        impl stmt::IntoSelect for &User<'_> {
            type Model = super::super::super::user::User;
            fn into_select(self) -> stmt::Select<Self::Model> {
                super::super::super::user::User::find_by_id(&self.scope.user_id).into_select()
            }
        }
        impl<'a> stmt::IntoExpr<User<'a>> for User<'a> {
            fn into_expr(self) -> stmt::Expr<User<'a>> {
                todo!(
                    "stmt::IntoExpr for {} (belongs_to Fk struct) - self = {:#?}",
                    stringify!(User),
                    self
                );
            }
        }
        impl<'a> stmt::IntoExpr<User<'a>> for &User<'a> {
            fn into_expr(self) -> stmt::Expr<User<'a>> {
                todo!(
                    "stmt::IntoExpr for &'a {} (belongs_to Fk struct) - self = {:#?}",
                    stringify!(User),
                    self
                );
            }
        }
        impl<'a> stmt::IntoExpr<User<'a>> for &super::super::super::user::User {
            fn into_expr(self) -> stmt::Expr<User<'a>> {
                stmt::Expr::from_untyped(&self.id)
            }
        }
        impl<'a> stmt::IntoExpr<User<'a>> for super::super::super::user::CreateUser {
            fn into_expr(self) -> stmt::Expr<User<'a>> {
                let expr: stmt::Expr<super::super::super::user::User> = self.stmt.into();
                expr.cast()
            }
        }
        impl User<'_> {
            pub async fn find(&self, db: &Db) -> Result<super::super::super::user::User> {
                db.get(self.into_select()).await
            }
        }
    }
    pub use user::User;
}
pub mod queries {
    use super::*;
    impl super::Todo {
        pub fn find_by_id(id: impl stmt::IntoExpr<Id<Todo>>) -> FindById {
            FindById {
                query: Query::from_stmt(stmt::Select::from_expr(Todo::ID.eq(id))),
            }
        }
    }
    pub struct FindById {
        query: Query,
    }
    impl FindById {
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Todo>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Todo>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Todo> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> FindById {
            let path = path.into();
            self.query.stmt.include(path);
            self
        }
        pub fn filter(self, filter: stmt::Expr<bool>) -> Query {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &Db) -> Result<A>
        where
            A: FromCursor<super::Todo>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindById {
        type Model = super::Todo;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Todo {
        pub fn find_many_by_id() -> FindManyById {
            FindManyById { items: vec![] }
        }
    }
    pub struct FindManyById {
        items: Vec<stmt::Expr<Id<Todo>>>,
    }
    impl FindManyById {
        pub fn item(mut self, id: impl stmt::IntoExpr<Id<Todo>>) -> Self {
            self.items.push(id.into_expr());
            self
        }
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Todo>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Todo>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Todo> {
            db.get(self.into_select()).await
        }
        pub fn update(self) -> super::UpdateQuery {
            super::UpdateQuery::from(self.into_select())
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            db.delete(self.into_select()).await
        }
        pub fn filter(self, filter: stmt::Expr<bool>) -> Query {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &Db) -> Result<A>
        where
            A: FromCursor<super::Todo>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindManyById {
        type Model = super::Todo;
        fn into_select(self) -> stmt::Select<Self::Model> {
            stmt::Select::from_expr(stmt::in_set(Todo::ID, self.items))
        }
    }
    impl super::Todo {
        pub fn find_by_user_id(
            user_id: impl stmt::IntoExpr<Id<super::super::user::User>>,
        ) -> FindByUserId {
            FindByUserId {
                query: Query::from_stmt(stmt::Select::from_expr(Todo::USER_ID.eq(user_id))),
            }
        }
    }
    pub struct FindByUserId {
        query: Query,
    }
    impl FindByUserId {
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Todo>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Todo>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Todo> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> FindByUserId {
            let path = path.into();
            self.query.stmt.include(path);
            self
        }
        pub fn filter(self, filter: stmt::Expr<bool>) -> Query {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &Db) -> Result<A>
        where
            A: FromCursor<super::Todo>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindByUserId {
        type Model = super::Todo;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.query.into_select()
        }
    }
}
