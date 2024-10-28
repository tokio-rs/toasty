use toasty::codegen_support::*;
#[derive(Debug)]
pub struct Todo {
    pub id: Id<Todo>,
    pub user: relation::user::User,
    pub category: relation::category::Category,
    pub title: String,
}
#[derive(Debug)]
pub struct ExprKey<'a, M> {
    pub id: stmt::Expr<'a, M, Id<Todo>>,
}
impl Todo {
    pub const ALL: Query<'static> = Query::new();
    pub const FIELDS: Fields<Self> = Fields::from_path(Path::root());
    pub fn create<'a>() -> CreateTodo<'a> {
        CreateTodo::default()
    }
    pub fn create_many<'a>() -> CreateMany<'a, Todo> {
        CreateMany::default()
    }
    pub fn filter<'a>(expr: stmt::Expr<'a, Self, bool>) -> Query<'a> {
        Query::from_stmt(stmt::Select::from_expr(expr))
    }
    pub fn update<'a>(&'a mut self) -> UpdateTodo<'a> {
        UpdateTodo {
            model: self,
            fields: Default::default(),
            expr: toasty::schema::stmt::ExprRecord::from_vec(vec![Value::Null.into(); 4usize]),
        }
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        let _ = db
            .exec(Statement::delete(Todo::FIELDS.id().eq(&self.id)))
            .await?;
        Ok(())
    }
}
impl<'a> Model<'a> for Todo {
    const ID: ModelId = ModelId(1);
    const FIELD_COUNT: usize = 4;
    type Create = CreateTodo<'a>;
    fn load(mut record: toasty::driver::Record<'_>) -> Result<Self, Error> {
        let mut values = record.into_iter();
        Ok(Todo {
            id: Id::from_untyped(values.next_as_id()?),
            user: relation::user::User::load(values.next().unwrap())?,
            category: relation::category::Category::load(values.next().unwrap())?,
            title: values.next_as_string()?,
        })
    }
}
impl<'a> stmt::IntoSelect<'a> for &'a Todo {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<'a, Self::Model> {
        Todo::find_by_id(&self.id).into_select()
    }
}
impl stmt::IntoSelect<'static> for Todo {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<'static, Self::Model> {
        Todo::find_by_id(self.id).into_select()
    }
}
impl<'a, M> stmt::IntoExpr<'a, M, Todo> for &'a Todo {
    fn into_expr(self) -> stmt::Expr<'a, M, Todo> {
        stmt::Expr::from_untyped(&self.id)
    }
}
impl<'a, M> stmt::IntoExpr<'a, M, Todo> for ExprKey<'a, M> {
    fn into_expr(self) -> stmt::Expr<'a, M, Todo> {
        stmt::Expr::from_untyped(self.id)
    }
}
pub struct Fields<M> {
    path: Path<M, Todo>,
}
pub struct Collection<M> {
    path: Path<M, Todo>,
}
impl<M> Fields<M> {
    pub const fn from_path(path: Path<M, Todo>) -> Fields<M> {
        Fields { path }
    }
    pub fn in_query<'a, Q>(self, rhs: Q) -> toasty::stmt::Expr<'a, M, bool>
    where
        Q: stmt::IntoSelect<'a, Model = Todo>,
    {
        self.path.in_query(rhs)
    }
    pub fn eq<'a, T>(self, rhs: T) -> toasty::stmt::Expr<'a, M, bool>
    where
        T: toasty::stmt::IntoExpr<'a, M, Todo>,
    {
        self.path.eq(rhs)
    }
    pub fn id(mut self) -> Path<M, Id<Todo>> {
        self.path.concat(Path::from_field_index(0))
    }
    pub fn user(mut self) -> super::user::Fields<M> {
        let path = self.path.concat(Path::from_field_index(1));
        super::user::Fields::from_path(path)
    }
    pub fn category(mut self) -> super::category::Fields<M> {
        let path = self.path.concat(Path::from_field_index(2));
        super::category::Fields::from_path(path)
    }
    pub fn title(mut self) -> Path<M, String> {
        self.path.concat(Path::from_field_index(3))
    }
}
impl<M> Collection<M> {
    pub const fn from_path(path: Path<M, Todo>) -> Collection<M> {
        Collection { path }
    }
    pub fn id(mut self) -> Path<M, Id<Todo>> {
        self.path.concat(Path::from_field_index(0))
    }
    pub fn user(mut self) -> super::user::Fields<M> {
        let path = self.path.concat(Path::from_field_index(1));
        super::user::Fields::from_path(path)
    }
    pub fn category(mut self) -> super::category::Fields<M> {
        let path = self.path.concat(Path::from_field_index(2));
        super::category::Fields::from_path(path)
    }
    pub fn title(mut self) -> Path<M, String> {
        self.path.concat(Path::from_field_index(3))
    }
}
#[derive(Debug)]
pub struct Query<'a> {
    stmt: stmt::Select<'a, Todo>,
}
impl<'a> Query<'a> {
    pub const fn from_stmt(stmt: stmt::Select<'a, Todo>) -> Query<'a> {
        Query { stmt }
    }
    pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, Todo>> {
        db.all(self).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<Todo>> {
        db.first(self).await
    }
    pub async fn get(self, db: &Db) -> Result<Todo> {
        db.get(self).await
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        db.exec(self.stmt.delete()).await?;
        Ok(())
    }
    pub async fn collect<A>(self, db: &'a Db) -> Result<A>
    where
        A: FromCursor<Todo>,
    {
        self.all(db).await?.collect().await
    }
    pub fn filter(self, expr: stmt::Expr<'a, Todo, bool>) -> Query<'a> {
        Query {
            stmt: self.stmt.and(expr),
        }
    }
    pub fn user(mut self) -> super::user::Query<'a> {
        todo!()
    }
    pub fn category(mut self) -> super::category::Query<'a> {
        todo!()
    }
}
impl Query<'static> {
    pub const fn new() -> Query<'static> {
        Query {
            stmt: stmt::Select::all(),
        }
    }
}
impl<'a> stmt::IntoSelect<'a> for Query<'a> {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<'a, Todo> {
        self.stmt
    }
}
impl<'a> stmt::IntoSelect<'a> for &Query<'a> {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<'a, Todo> {
        self.stmt.clone()
    }
}
impl Default for Query<'static> {
    fn default() -> Query<'static> {
        Query::new()
    }
}
#[derive(Debug)]
pub struct CreateTodo<'a> {
    pub(super) stmt: stmt::Insert<'a, Todo>,
}
impl<'a> CreateTodo<'a> {
    pub fn id(mut self, id: impl Into<Id<Todo>>) -> Self {
        self.stmt.set_value(0, id.into());
        self
    }
    pub fn user(mut self, user: impl IntoExpr<'a, Todo, super::user::User>) -> Self {
        self.stmt.set_expr(1, user.into_expr());
        self
    }
    pub fn category(
        mut self,
        category: impl IntoExpr<'a, Todo, super::category::Category>,
    ) -> Self {
        self.stmt.set_expr(2, category.into_expr());
        self
    }
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.stmt.set_value(3, title.into());
        self
    }
    pub async fn exec(self, db: &'a Db) -> Result<Todo> {
        db.exec_insert_one::<'_, Todo>(self.stmt).await
    }
}
impl<'a> CreateModel<'a, Todo> for CreateTodo<'a> {
    fn as_insert_statement(&self) -> &stmt::Insert<'a, Todo> {
        &self.stmt
    }
    fn as_insert_statement_mut(&mut self) -> &mut stmt::Insert<'a, Todo> {
        &mut self.stmt
    }
}
impl<'a, M> IntoExpr<'a, M, Todo> for CreateTodo<'a> {
    fn into_expr(self) -> stmt::Expr<'a, M, Todo> {
        self.stmt.into()
    }
}
impl<'a> Default for CreateTodo<'a> {
    fn default() -> CreateTodo<'a> {
        CreateTodo {
            stmt: stmt::Insert::new(Todo::ALL, vec![]),
        }
    }
}
#[derive(Debug)]
pub struct UpdateTodo<'a> {
    model: &'a mut Todo,
    fields: toasty::schema::stmt::PathFieldSet,
    expr: toasty::schema::stmt::ExprRecord<'a>,
}
#[derive(Debug)]
pub struct UpdateQuery<'a> {
    stmt: stmt::Update<'a, Todo>,
}
impl<'a> UpdateTodo<'a> {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.fields.insert(3);
        self.expr[3] = Value::from(title.into()).into();
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let expr = self.expr;
        let fields = self.fields.clone();
        let mut into_iter = {
            let mut records = db
                .exec::<Todo>(Statement::update(expr, fields, &*self.model))
                .await?;
            records.next().await.unwrap()?.into_owned().into_iter()
        };
        for field in self.fields.iter() {
            match field.as_index() {
                0 => self.model.id = toasty::Id::from_untyped(into_iter.next().unwrap().to_id()?),
                1 => todo!(),
                2 => todo!(),
                3 => self.model.title = into_iter.next().unwrap().to_string()?,
                _ => todo!(),
            }
        }
        Ok(())
    }
}
impl<'a> UpdateQuery<'a> {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.stmt.set(3, Value::from(title.into()));
        self
    }
    pub async fn exec(&mut self, db: &Db) -> Result<()> {
        let stmt = self.stmt.clone();
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
impl<'a> From<stmt::Select<'a, Todo>> for UpdateQuery<'a> {
    fn from(src: stmt::Select<'a, Todo>) -> UpdateQuery<'a> {
        UpdateQuery {
            stmt: stmt::Update::new(src),
        }
    }
}
pub mod relation {
    use super::*;
    use toasty::Cursor;
    pub mod user {
        use super::*;
        #[derive(Debug)]
        pub struct User {
            pub id: Id<super::super::super::user::User>,
        }
        impl User {
            pub fn load(value: Value<'_>) -> Result<Self> {
                Ok(Self {
                    id: Id::from_untyped(value.to_id()?),
                })
            }
        }
        impl<'a> stmt::IntoSelect<'a> for &'a User {
            type Model = super::super::super::user::User;
            fn into_select(self) -> stmt::Select<'a, Self::Model> {
                super::super::super::user::User::find_by_id(&self.id).into_select()
            }
        }
        impl<'a, M> stmt::IntoExpr<'a, M, super::super::super::user::User> for &'a User {
            fn into_expr(self) -> stmt::Expr<'a, M, super::super::super::user::User> {
                todo!()
            }
        }
        impl<'a, M> stmt::IntoExpr<'a, M, super::super::super::user::User> for User {
            fn into_expr(self) -> stmt::Expr<'a, M, super::super::super::user::User> {
                todo!()
            }
        }
        impl User {
            pub async fn get<'a>(&self, db: &'a Db) -> Result<super::super::super::user::User> {
                db.get(self).await
            }
        }
    }
    pub use user::User;
    pub mod category {
        use super::*;
        #[derive(Debug)]
        pub struct Category {
            pub id: Id<super::super::super::category::Category>,
        }
        impl Category {
            pub fn load(value: Value<'_>) -> Result<Self> {
                Ok(Self {
                    id: Id::from_untyped(value.to_id()?),
                })
            }
        }
        impl<'a> stmt::IntoSelect<'a> for &'a Category {
            type Model = super::super::super::category::Category;
            fn into_select(self) -> stmt::Select<'a, Self::Model> {
                super::super::super::category::Category::find_by_id(&self.id).into_select()
            }
        }
        impl<'a, M> stmt::IntoExpr<'a, M, super::super::super::category::Category> for &'a Category {
            fn into_expr(self) -> stmt::Expr<'a, M, super::super::super::category::Category> {
                todo!()
            }
        }
        impl<'a, M> stmt::IntoExpr<'a, M, super::super::super::category::Category> for Category {
            fn into_expr(self) -> stmt::Expr<'a, M, super::super::super::category::Category> {
                todo!()
            }
        }
        impl Category {
            pub async fn get<'a>(
                &self,
                db: &'a Db,
            ) -> Result<super::super::super::category::Category> {
                db.get(self).await
            }
        }
    }
    pub use category::Category;
}
pub mod queries {
    use super::*;
    impl super::Todo {
        pub fn find_by_id<'a>(id: impl toasty::stmt::IntoExpr<'a, Todo, Id<Todo>>) -> FindById<'a> {
            FindById {
                query: Query::from_stmt(stmt::Select::from_expr(
                    Todo::FIELDS.id().eq(id.into_expr()),
                )),
            }
        }
    }
    pub struct FindById<'a> {
        query: Query<'a>,
    }
    impl<'a> FindById<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Todo>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Todo>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Todo> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn filter(self, filter: stmt::Expr<'a, Todo, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::Todo>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindById<'a> {
        type Model = super::Todo;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Todo {
        pub fn find_by_user<'a>(
            user: impl toasty::stmt::IntoExpr<'a, Todo, super::super::user::User>,
        ) -> FindByUser<'a> {
            FindByUser {
                query: Query::from_stmt(stmt::Select::from_expr(
                    Todo::FIELDS.user().eq(user.into_expr()),
                )),
            }
        }
    }
    pub struct FindByUser<'a> {
        query: Query<'a>,
    }
    impl<'a> FindByUser<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Todo>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Todo>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Todo> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn filter(self, filter: stmt::Expr<'a, Todo, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::Todo>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindByUser<'a> {
        type Model = super::Todo;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Todo {
        pub fn find_by_user_id<'a>(
            user_id: impl toasty::stmt::IntoExpr<'a, Todo, Id<super::super::user::User>>,
        ) -> FindByUserId<'a> {
            FindByUserId {
                query: Query::from_stmt(stmt::Select::from_expr(Todo::FIELDS.user().eq(
                    super::super::user::ExprKey {
                        id: user_id.into_expr(),
                    },
                ))),
            }
        }
    }
    pub struct FindByUserId<'a> {
        query: Query<'a>,
    }
    impl<'a> FindByUserId<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Todo>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Todo>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Todo> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn filter(self, filter: stmt::Expr<'a, Todo, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::Todo>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindByUserId<'a> {
        type Model = super::Todo;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Todo {
        pub fn find_by_category<'a>(
            category: impl toasty::stmt::IntoExpr<'a, Todo, super::super::category::Category>,
        ) -> FindByCategory<'a> {
            FindByCategory {
                query: Query::from_stmt(stmt::Select::from_expr(
                    Todo::FIELDS.category().eq(category.into_expr()),
                )),
            }
        }
    }
    pub struct FindByCategory<'a> {
        query: Query<'a>,
    }
    impl<'a> FindByCategory<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Todo>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Todo>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Todo> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn filter(self, filter: stmt::Expr<'a, Todo, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::Todo>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindByCategory<'a> {
        type Model = super::Todo;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Todo {
        pub fn find_by_category_id<'a>(
            category_id: impl toasty::stmt::IntoExpr<'a, Todo, Id<super::super::category::Category>>,
        ) -> FindByCategoryId<'a> {
            FindByCategoryId {
                query: Query::from_stmt(stmt::Select::from_expr(Todo::FIELDS.category().eq(
                    super::super::category::ExprKey {
                        id: category_id.into_expr(),
                    },
                ))),
            }
        }
    }
    pub struct FindByCategoryId<'a> {
        query: Query<'a>,
    }
    impl<'a> FindByCategoryId<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Todo>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Todo>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Todo> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn filter(self, filter: stmt::Expr<'a, Todo, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::Todo>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindByCategoryId<'a> {
        type Model = super::Todo;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
}
