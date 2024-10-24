use toasty::codegen_support::*;
#[derive(Debug)]
pub struct User {
    pub id: Id<User>,
    pub name: String,
    pub email: String,
    todos: HasMany<super::todo::Todo>,
}
impl User {
    pub const ID: Path<Id<User>> = Path::from_field_index::<Self>(0);
    pub const NAME: Path<String> = Path::from_field_index::<Self>(1);
    pub const EMAIL: Path<String> = Path::from_field_index::<Self>(2);
    pub const TODOS: self::fields::Todos =
        self::fields::Todos::from_path(Path::from_field_index::<Self>(3));
    pub fn create<'a>() -> CreateUser<'a> {
        CreateUser::default()
    }
    pub fn create_many<'a>() -> CreateMany<'a, User> {
        CreateMany::default()
    }
    pub fn filter<'a>(expr: stmt::Expr<'a, bool>) -> Query<'a> {
        Query::from_stmt(stmt::Select::from_expr(expr))
    }
    pub fn update<'a>(&'a mut self) -> UpdateUser<'a> {
        UpdateUser {
            model: self,
            query: UpdateQuery {
                stmt: stmt::Update::default(),
            },
        }
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        let stmt = self.into_select().delete();
        db.exec(stmt).await?;
        Ok(())
    }
}
impl Model for User {
    const ID: ModelId = ModelId(0);
    const FIELD_COUNT: usize = 4;
    type Key = Id<User>;
    fn load(mut record: Record<'_>) -> Result<Self, Error> {
        Ok(User {
            id: Id::from_untyped(record[0].take().to_id()?),
            name: record[1].take().to_string()?,
            email: record[2].take().to_string()?,
            todos: HasMany::load(record[3].take())?,
        })
    }
}
impl<'a> stmt::IntoSelect<'a> for &'a User {
    type Model = User;
    fn into_select(self) -> stmt::Select<'a, Self::Model> {
        User::find_by_id(&self.id).into_select()
    }
}
impl stmt::AsSelect for User {
    type Model = User;
    fn as_select(&self) -> stmt::Select<'_, Self::Model> {
        User::find_by_id(&self.id).into_select()
    }
}
impl stmt::IntoSelect<'static> for User {
    type Model = User;
    fn into_select(self) -> stmt::Select<'static, Self::Model> {
        User::find_by_id(self.id).into_select()
    }
}
impl<'a> stmt::IntoExpr<'a, User> for &'a User {
    fn into_expr(self) -> stmt::Expr<'a, User> {
        stmt::Key::from_expr(&self.id).into()
    }
}
impl<'a> stmt::IntoExpr<'a, [User]> for &'a User {
    fn into_expr(self) -> stmt::Expr<'a, [User]> {
        stmt::Key::from_expr(&self.id).into()
    }
}
#[derive(Debug)]
pub struct Query<'a> {
    stmt: stmt::Select<'a, User>,
}
impl<'a> Query<'a> {
    pub const fn from_stmt(stmt: stmt::Select<'a, User>) -> Query<'a> {
        Query { stmt }
    }
    pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, User>> {
        db.all(self.stmt).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<User>> {
        db.first(self.stmt).await
    }
    pub async fn get(self, db: &Db) -> Result<User> {
        db.get(self.stmt).await
    }
    pub fn update(self) -> UpdateQuery<'a> {
        UpdateQuery::from(self)
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        db.exec(self.stmt.delete()).await?;
        Ok(())
    }
    pub async fn collect<A>(self, db: &'a Db) -> Result<A>
    where
        A: FromCursor<User>,
    {
        self.all(db).await?.collect().await
    }
    pub fn filter(self, expr: stmt::Expr<'a, bool>) -> Query<'a> {
        Query {
            stmt: self.stmt.and(expr),
        }
    }
}
impl<'a> stmt::IntoSelect<'a> for Query<'a> {
    type Model = User;
    fn into_select(self) -> stmt::Select<'a, User> {
        self.stmt
    }
}
impl<'a> stmt::IntoSelect<'a> for &Query<'a> {
    type Model = User;
    fn into_select(self) -> stmt::Select<'a, User> {
        self.stmt.clone()
    }
}
impl Default for Query<'static> {
    fn default() -> Query<'static> {
        Query {
            stmt: stmt::Select::all(),
        }
    }
}
#[derive(Debug)]
pub struct CreateUser<'a> {
    pub(super) stmt: stmt::Insert<'a, User>,
}
impl<'a> CreateUser<'a> {
    pub fn id(mut self, id: impl Into<Id<User>>) -> Self {
        self.stmt.set_value(0, id.into());
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.stmt.set_value(1, name.into());
        self
    }
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.stmt.set_value(2, email.into());
        self
    }
    pub fn todo(mut self, todo: impl IntoExpr<'a, super::todo::Todo>) -> Self {
        self.stmt.push_expr(3, todo.into_expr());
        self
    }
    pub async fn exec(self, db: &'a Db) -> Result<User> {
        db.exec_insert_one::<User>(self.stmt).await
    }
}
impl<'a> IntoInsert<'a> for CreateUser<'a> {
    type Model = User;
    fn into_insert(self) -> stmt::Insert<'a, User> {
        self.stmt
    }
}
impl<'a> IntoExpr<'a, User> for CreateUser<'a> {
    fn into_expr(self) -> stmt::Expr<'a, User> {
        self.stmt.into()
    }
}
impl<'a> IntoExpr<'a, [User]> for CreateUser<'a> {
    fn into_expr(self) -> stmt::Expr<'a, [User]> {
        self.stmt.into_list_expr()
    }
}
impl<'a> Default for CreateUser<'a> {
    fn default() -> CreateUser<'a> {
        CreateUser {
            stmt: stmt::Insert::blank(),
        }
    }
}
#[derive(Debug)]
pub struct UpdateUser<'a> {
    model: &'a mut User,
    query: UpdateQuery<'a>,
}
#[derive(Debug)]
pub struct UpdateQuery<'a> {
    stmt: stmt::Update<'a, User>,
}
impl<'a> UpdateUser<'a> {
    pub fn id(mut self, id: impl Into<Id<User>>) -> Self {
        self.query.set_id(id);
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.query.set_name(name);
        self
    }
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.query.set_email(email);
        self
    }
    pub fn todo(mut self, todo: impl IntoExpr<'a, super::todo::Todo>) -> Self {
        self.query.add_todo(todo);
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let fields;
        let mut into_iter;
        {
            let mut stmt = self.query.stmt;
            fields = stmt.fields().clone();
            stmt.set_selection(&*self.model);
            let mut records = db.exec::<User>(stmt.into()).await?;
            into_iter = records
                .next()
                .await
                .unwrap()?
                .into_record()
                .into_owned()
                .into_iter();
        }
        for field in fields.iter() {
            match field.into_usize() {
                0 => self.model.id = stmt::Id::from_untyped(into_iter.next().unwrap().to_id()?),
                1 => self.model.name = into_iter.next().unwrap().to_string()?,
                2 => self.model.email = into_iter.next().unwrap().to_string()?,
                3 => {}
                _ => todo!("handle unknown field id in reload after update"),
            }
        }
        Ok(())
    }
}
impl<'a> UpdateQuery<'a> {
    pub fn id(mut self, id: impl Into<Id<User>>) -> Self {
        self.set_id(id);
        self
    }
    pub fn set_id(&mut self, id: impl Into<Id<User>>) -> &mut Self {
        self.stmt.set_expr(0, id.into().into_expr());
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.set_name(name);
        self
    }
    pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.stmt.set_expr(1, name.into().into_expr());
        self
    }
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.set_email(email);
        self
    }
    pub fn set_email(&mut self, email: impl Into<String>) -> &mut Self {
        self.stmt.set_expr(2, email.into().into_expr());
        self
    }
    pub fn todo(mut self, todo: impl IntoExpr<'a, super::todo::Todo>) -> Self {
        self.add_todo(todo);
        self
    }
    pub fn add_todo(&mut self, todo: impl IntoExpr<'a, super::todo::Todo>) -> &mut Self {
        self.stmt.push_expr(3, todo.into_expr());
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let stmt = self.stmt;
        let mut cursor = db.exec(stmt.into()).await?;
        Ok(())
    }
}
impl<'a> From<Query<'a>> for UpdateQuery<'a> {
    fn from(value: Query<'a>) -> UpdateQuery<'a> {
        UpdateQuery {
            stmt: stmt::Update::new(value),
        }
    }
}
impl<'a> From<stmt::Select<'a, User>> for UpdateQuery<'a> {
    fn from(src: stmt::Select<'a, User>) -> UpdateQuery<'a> {
        UpdateQuery {
            stmt: stmt::Update::new(src),
        }
    }
}
pub mod fields {
    use super::*;
    pub struct Todos {
        pub(super) path: Path<[super::super::todo::Todo]>,
    }
    impl Todos {
        pub const fn from_path(path: Path<[super::super::todo::Todo]>) -> Todos {
            Todos { path }
        }
        pub fn id(mut self) -> Path<Id<super::super::todo::Todo>> {
            self.path.chain(super::super::todo::Todo::ID)
        }
        pub fn title(mut self) -> Path<String> {
            self.path.chain(super::super::todo::Todo::TITLE)
        }
        pub fn order(mut self) -> Path<i64> {
            self.path.chain(super::super::todo::Todo::ORDER)
        }
        pub fn user(mut self) -> super::super::todo::fields::User {
            let path = self.path.chain(super::super::todo::Todo::USER);
            super::super::todo::fields::User::from_path(path)
        }
        pub fn user_id(mut self) -> Path<Id<User>> {
            self.path.chain(super::super::todo::Todo::USER_ID)
        }
    }
    impl From<Todos> for Path<[super::super::todo::Todo]> {
        fn from(val: Todos) -> Path<[super::super::todo::Todo]> {
            val.path
        }
    }
    impl<'stmt> stmt::IntoExpr<'stmt, super::relation::todos::Todos<'stmt>> for Todos {
        fn into_expr(self) -> stmt::Expr<'stmt, super::relation::todos::Todos<'stmt>> {
            todo!("into_expr for {} (field path struct)", stringify!(Todos));
        }
    }
}
pub mod relation {
    use super::*;
    use toasty::Cursor;
    pub mod todos {
        use super::*;
        #[derive(Debug)]
        pub struct Todos<'a> {
            scope: &'a User,
        }
        #[derive(Debug)]
        pub struct Query<'a> {
            pub(super) scope: super::Query<'a>,
        }
        #[derive(Debug)]
        pub struct Remove<'a> {
            stmt: stmt::Unlink<'a, super::User>,
        }
        #[derive(Debug)]
        pub struct Add<'a> {
            stmt: stmt::Link<'a, super::User>,
        }
        impl super::User {
            pub fn todos(&self) -> Todos<'_> {
                Todos { scope: self }
            }
        }
        impl<'a> super::Query<'a> {
            pub fn todos(self) -> Query<'a> {
                Query::with_scope(self)
            }
        }
        impl<'a> Todos<'a> {
            pub fn get(&self) -> &[super::super::super::todo::Todo] {
                self.scope.todos.get()
            }
            #[doc = r" Iterate all entries in the relation"]
            pub async fn all(
                self,
                db: &'a Db,
            ) -> Result<Cursor<'a, super::super::super::todo::Todo>> {
                db.all(self.into_select()).await
            }
            pub async fn collect<A>(self, db: &'a Db) -> Result<A>
            where
                A: FromCursor<super::super::super::todo::Todo>,
            {
                self.all(db).await?.collect().await
            }
            #[doc = r" Create a new associated record"]
            pub fn create(self) -> super::super::super::todo::CreateTodo<'a> {
                let mut builder = super::super::super::todo::CreateTodo::default();
                builder.stmt.set_scope(self);
                builder
            }
            pub fn query(
                self,
                filter: stmt::Expr<'a, bool>,
            ) -> super::super::super::todo::Query<'a> {
                let query = self.into_select();
                super::super::super::todo::Query::from_stmt(query.and(filter))
            }
            #[doc = r" Add an item to the association"]
            pub fn add(
                self,
                todos: impl IntoSelect<'a, Model = super::super::super::todo::Todo>,
            ) -> Add<'a> {
                Add {
                    stmt: stmt::Link::new(self.scope, super::User::TODOS, todos),
                }
            }
            #[doc = r" Remove items from the association"]
            pub fn remove(
                self,
                todos: impl IntoSelect<'a, Model = super::super::super::todo::Todo>,
            ) -> Remove<'a> {
                Remove {
                    stmt: stmt::Unlink::new(self.scope, super::User::TODOS, todos),
                }
            }
            pub fn find_by_id(
                self,
                id: impl stmt::IntoExpr<'a, Id<super::super::super::todo::Todo>>,
            ) -> FindByUserAndId<'a> {
                FindByUserAndId {
                    stmt: stmt::Select::from_expr(
                        super::super::super::todo::Todo::USER
                            .in_query(self.scope)
                            .and(super::super::super::todo::Todo::ID.eq(id)),
                    ),
                }
            }
        }
        impl<'a> stmt::IntoSelect<'a> for Todos<'a> {
            type Model = super::super::super::todo::Todo;
            fn into_select(self) -> stmt::Select<'a, super::super::super::todo::Todo> {
                super::super::super::todo::Todo::filter(
                    super::super::super::todo::Todo::USER.in_query(self.scope),
                )
                .into_select()
            }
        }
        impl<'a> Query<'a> {
            pub fn with_scope<S>(scope: S) -> Query<'a>
            where
                S: IntoSelect<'a, Model = User>,
            {
                Query {
                    scope: super::Query::from_stmt(scope.into_select()),
                }
            }
            pub fn find_by_id(
                self,
                id: impl stmt::IntoExpr<'a, Id<super::super::super::todo::Todo>>,
            ) -> FindByUserAndId<'a> {
                FindByUserAndId {
                    stmt: stmt::Select::from_expr(
                        super::super::super::todo::Todo::USER
                            .in_query(self.scope)
                            .and(super::super::super::todo::Todo::ID.eq(id)),
                    ),
                }
            }
        }
        impl<'a> Add<'a> {
            pub async fn exec(self, db: &'a Db) -> Result<()> {
                let mut cursor = db.exec(self.stmt.into()).await?;
                assert!(cursor.next().await.is_none());
                Ok(())
            }
        }
        impl<'a> Remove<'a> {
            pub async fn exec(self, db: &'a Db) -> Result<()> {
                let mut cursor = db.exec(self.stmt.into()).await?;
                assert!(cursor.next().await.is_none());
                Ok(())
            }
        }
        pub struct FindByUserAndId<'a> {
            stmt: stmt::Select<'a, super::super::super::todo::Todo>,
        }
        impl<'a> FindByUserAndId<'a> {
            pub async fn all(
                self,
                db: &'a Db,
            ) -> Result<Cursor<'a, super::super::super::todo::Todo>> {
                db.all(self.stmt).await
            }
            pub async fn first(self, db: &Db) -> Result<Option<super::super::super::todo::Todo>> {
                db.first(self.stmt).await
            }
            pub async fn get(self, db: &Db) -> Result<super::super::super::todo::Todo> {
                db.get(self.stmt).await
            }
            pub fn update(self) -> super::super::super::todo::UpdateQuery<'a> {
                super::super::super::todo::UpdateQuery::from(self.stmt)
            }
            pub async fn delete(self, db: &Db) -> Result<()> {
                db.exec(self.stmt.delete()).await?;
                Ok(())
            }
        }
        impl<'a> stmt::IntoSelect<'a> for FindByUserAndId<'a> {
            type Model = super::super::super::todo::Todo;
            fn into_select(self) -> stmt::Select<'a, Self::Model> {
                self.stmt
            }
        }
    }
    pub use todos::Todos;
}
pub mod queries {
    use super::*;
    impl super::User {
        pub fn find_by_id<'a>(id: impl stmt::IntoExpr<'a, Id<User>>) -> FindById<'a> {
            FindById {
                query: Query::from_stmt(stmt::Select::from_expr(User::ID.eq(id))),
            }
        }
    }
    pub struct FindById<'a> {
        query: Query<'a>,
    }
    impl<'a> FindById<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::User>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::User>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::User> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> FindById<'a> {
            let path = path.into();
            self.query.stmt.include(path);
            self
        }
        pub fn filter(self, filter: stmt::Expr<'a, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::User>,
        {
            self.all(db).await?.collect().await
        }
        pub fn todos(mut self) -> self::relation::todos::Query<'a> {
            self::relation::todos::Query::with_scope(self)
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindById<'a> {
        type Model = super::User;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::User {
        pub fn find_many_by_id<'a>() -> FindManyById<'a> {
            FindManyById { items: vec![] }
        }
    }
    pub struct FindManyById<'a> {
        items: Vec<stmt::Expr<'a, Id<User>>>,
    }
    impl<'a> FindManyById<'a> {
        pub fn item(mut self, id: impl stmt::IntoExpr<'a, Id<User>>) -> Self {
            self.items.push(id.into_expr());
            self
        }
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::User>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::User>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::User> {
            db.get(self.into_select()).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.into_select())
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            db.delete(self.into_select()).await
        }
        pub fn filter(self, filter: stmt::Expr<'a, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::User>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindManyById<'a> {
        type Model = super::User;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            stmt::Select::from_expr(stmt::in_set(User::ID, self.items))
        }
    }
    impl super::User {
        pub fn find_by_email<'a>(email: impl stmt::IntoExpr<'a, String>) -> FindByEmail<'a> {
            FindByEmail {
                query: Query::from_stmt(stmt::Select::from_expr(User::EMAIL.eq(email))),
            }
        }
    }
    pub struct FindByEmail<'a> {
        query: Query<'a>,
    }
    impl<'a> FindByEmail<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::User>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::User>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::User> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> FindByEmail<'a> {
            let path = path.into();
            self.query.stmt.include(path);
            self
        }
        pub fn filter(self, filter: stmt::Expr<'a, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::User>,
        {
            self.all(db).await?.collect().await
        }
        pub fn todos(mut self) -> self::relation::todos::Query<'a> {
            self::relation::todos::Query::with_scope(self)
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindByEmail<'a> {
        type Model = super::User;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::User {
        pub fn find_many_by_email<'a>() -> FindManyByEmail<'a> {
            FindManyByEmail { items: vec![] }
        }
    }
    pub struct FindManyByEmail<'a> {
        items: Vec<stmt::Expr<'a, String>>,
    }
    impl<'a> FindManyByEmail<'a> {
        pub fn item(mut self, email: impl stmt::IntoExpr<'a, String>) -> Self {
            self.items.push(email.into_expr());
            self
        }
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::User>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::User>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::User> {
            db.get(self.into_select()).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.into_select())
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            db.delete(self.into_select()).await
        }
        pub fn filter(self, filter: stmt::Expr<'a, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::User>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindManyByEmail<'a> {
        type Model = super::User;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            stmt::Select::from_expr(stmt::in_set(User::EMAIL, self.items))
        }
    }
}
