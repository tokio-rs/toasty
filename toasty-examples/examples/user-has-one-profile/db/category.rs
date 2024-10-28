use toasty::codegen_support::*;
#[derive(Debug)]
pub struct Category {
    pub id: Id<Category>,
    pub name: String,
}
#[derive(Debug)]
pub struct ExprKey<'a, M> {
    pub id: stmt::Expr<'a, M, Id<Category>>,
}
impl Category {
    pub const ALL: Query<'static> = Query::new();
    pub const FIELDS: Fields<Self> = Fields::from_path(Path::root());
    pub fn create<'a>() -> CreateCategory<'a> {
        CreateCategory::default()
    }
    pub fn create_many<'a>() -> CreateMany<'a, Category> {
        CreateMany::default()
    }
    pub fn filter<'a>(expr: stmt::Expr<'a, Self, bool>) -> Query<'a> {
        Query::from_stmt(stmt::Select::from_expr(expr))
    }
    pub fn update<'a>(&'a mut self) -> UpdateCategory<'a> {
        UpdateCategory {
            model: self,
            fields: Default::default(),
            expr: toasty::schema::stmt::ExprRecord::from_vec(vec![Value::Null.into(); 3usize]),
        }
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        let _ = db
            .exec(Statement::delete(Category::FIELDS.id().eq(&self.id)))
            .await?;
        Ok(())
    }
}
impl<'a> Model<'a> for Category {
    const ID: ModelId = ModelId(2);
    const FIELD_COUNT: usize = 3;
    type Create = CreateCategory<'a>;
    fn load(mut record: toasty::driver::Record<'_>) -> Result<Self, Error> {
        let mut values = record.into_iter();
        Ok(Category {
            id: Id::from_untyped(values.next_as_id()?),
            name: values.next_as_string()?,
        })
    }
}
impl<'a> stmt::IntoSelect<'a> for &'a Category {
    type Model = Category;
    fn into_select(self) -> stmt::Select<'a, Self::Model> {
        Category::find_by_id(&self.id).into_select()
    }
}
impl stmt::IntoSelect<'static> for Category {
    type Model = Category;
    fn into_select(self) -> stmt::Select<'static, Self::Model> {
        Category::find_by_id(self.id).into_select()
    }
}
impl<'a, M> stmt::IntoExpr<'a, M, Category> for &'a Category {
    fn into_expr(self) -> stmt::Expr<'a, M, Category> {
        stmt::Expr::from_untyped(&self.id)
    }
}
impl<'a, M> stmt::IntoExpr<'a, M, Category> for ExprKey<'a, M> {
    fn into_expr(self) -> stmt::Expr<'a, M, Category> {
        stmt::Expr::from_untyped(self.id)
    }
}
pub struct Fields<M> {
    path: Path<M, Category>,
}
pub struct Collection<M> {
    path: Path<M, Category>,
}
impl<M> Fields<M> {
    pub const fn from_path(path: Path<M, Category>) -> Fields<M> {
        Fields { path }
    }
    pub fn in_query<'a, Q>(self, rhs: Q) -> toasty::stmt::Expr<'a, M, bool>
    where
        Q: stmt::IntoSelect<'a, Model = Category>,
    {
        self.path.in_query(rhs)
    }
    pub fn eq<'a, T>(self, rhs: T) -> toasty::stmt::Expr<'a, M, bool>
    where
        T: toasty::stmt::IntoExpr<'a, M, Category>,
    {
        self.path.eq(rhs)
    }
    pub fn id(mut self) -> Path<M, Id<Category>> {
        self.path.concat(Path::from_field_index(0))
    }
    pub fn name(mut self) -> Path<M, String> {
        self.path.concat(Path::from_field_index(1))
    }
    pub fn todos(mut self) -> super::todo::Collection<M> {
        let path = self.path.concat(Path::from_field_index(2));
        super::todo::Collection::from_path(path)
    }
}
impl<M> Collection<M> {
    pub const fn from_path(path: Path<M, Category>) -> Collection<M> {
        Collection { path }
    }
    pub fn id(mut self) -> Path<M, Id<Category>> {
        self.path.concat(Path::from_field_index(0))
    }
    pub fn name(mut self) -> Path<M, String> {
        self.path.concat(Path::from_field_index(1))
    }
    pub fn todos(mut self) -> super::todo::Collection<M> {
        let path = self.path.concat(Path::from_field_index(2));
        super::todo::Collection::from_path(path)
    }
}
#[derive(Debug)]
pub struct Query<'a> {
    stmt: stmt::Select<'a, Category>,
}
impl<'a> Query<'a> {
    pub const fn from_stmt(stmt: stmt::Select<'a, Category>) -> Query<'a> {
        Query { stmt }
    }
    pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, Category>> {
        db.all(self).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<Category>> {
        db.first(self).await
    }
    pub async fn get(self, db: &Db) -> Result<Category> {
        db.get(self).await
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        db.exec(self.stmt.delete()).await?;
        Ok(())
    }
    pub async fn collect<A>(self, db: &'a Db) -> Result<A>
    where
        A: FromCursor<Category>,
    {
        self.all(db).await?.collect().await
    }
    pub fn filter(self, expr: stmt::Expr<'a, Category, bool>) -> Query<'a> {
        Query {
            stmt: self.stmt.and(expr),
        }
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
    type Model = Category;
    fn into_select(self) -> stmt::Select<'a, Category> {
        self.stmt
    }
}
impl<'a> stmt::IntoSelect<'a> for &Query<'a> {
    type Model = Category;
    fn into_select(self) -> stmt::Select<'a, Category> {
        self.stmt.clone()
    }
}
impl Default for Query<'static> {
    fn default() -> Query<'static> {
        Query::new()
    }
}
#[derive(Debug)]
pub struct CreateCategory<'a> {
    pub(super) stmt: stmt::Insert<'a, Category>,
}
impl<'a> CreateCategory<'a> {
    pub fn id(mut self, id: impl Into<Id<Category>>) -> Self {
        self.stmt.set_value(0, id.into());
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.stmt.set_value(1, name.into());
        self
    }
    pub fn todo(mut self, todo: impl IntoExpr<'a, Category, super::todo::Todo>) -> Self {
        self.stmt.push_expr(2, todo.into_expr());
        self
    }
    pub async fn exec(self, db: &'a Db) -> Result<Category> {
        db.exec_insert_one::<'_, Category>(self.stmt).await
    }
}
impl<'a> CreateModel<'a, Category> for CreateCategory<'a> {
    fn as_insert_statement(&self) -> &stmt::Insert<'a, Category> {
        &self.stmt
    }
    fn as_insert_statement_mut(&mut self) -> &mut stmt::Insert<'a, Category> {
        &mut self.stmt
    }
}
impl<'a, M> IntoExpr<'a, M, Category> for CreateCategory<'a> {
    fn into_expr(self) -> stmt::Expr<'a, M, Category> {
        self.stmt.into()
    }
}
impl<'a> Default for CreateCategory<'a> {
    fn default() -> CreateCategory<'a> {
        CreateCategory {
            stmt: stmt::Insert::new(Category::ALL, vec![]),
        }
    }
}
#[derive(Debug)]
pub struct UpdateCategory<'a> {
    model: &'a mut Category,
    fields: toasty::schema::stmt::PathFieldSet,
    expr: toasty::schema::stmt::ExprRecord<'a>,
}
#[derive(Debug)]
pub struct UpdateQuery<'a> {
    stmt: stmt::Update<'a, Category>,
}
impl<'a> UpdateCategory<'a> {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.fields.insert(1);
        self.expr[1] = Value::from(name.into()).into();
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let expr = self.expr;
        let fields = self.fields.clone();
        let mut into_iter = {
            let mut records = db
                .exec::<Category>(Statement::update(expr, fields, &*self.model))
                .await?;
            records.next().await.unwrap()?.into_owned().into_iter()
        };
        for field in self.fields.iter() {
            match field.as_index() {
                0 => self.model.id = toasty::Id::from_untyped(into_iter.next().unwrap().to_id()?),
                1 => self.model.name = into_iter.next().unwrap().to_string()?,
                _ => todo!(),
            }
        }
        Ok(())
    }
}
impl<'a> UpdateQuery<'a> {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.stmt.set(1, Value::from(name.into()));
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
impl<'a> From<stmt::Select<'a, Category>> for UpdateQuery<'a> {
    fn from(src: stmt::Select<'a, Category>) -> UpdateQuery<'a> {
        UpdateQuery {
            stmt: stmt::Update::new(src),
        }
    }
}
pub mod relation {
    use super::*;
    use toasty::Cursor;
    #[derive(Debug)]
    pub struct Todos<'a> {
        pub(super) scope: Query<'a>,
    }
    impl super::Category {
        pub fn todos(&self) -> Todos<'_> {
            let scope = Query::from_stmt(self.into_select());
            Todos { scope }
        }
    }
    impl<'a> super::Query<'a> {
        pub fn todos(self) -> Todos<'a> {
            Todos { scope: self }
        }
    }
    impl<'a> Todos<'a> {
        #[doc = r" Iterate all entries in the relation"]
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::super::todo::Todo>> {
            db.all(self.into_select()).await
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::super::todo::Todo>,
        {
            self.all(db).await?.collect().await
        }
        #[doc = r" Create a new associated record"]
        pub fn create(self) -> super::super::todo::CreateTodo<'a> {
            let mut builder = super::super::todo::CreateTodo::default();
            builder.stmt.set_scope(self);
            builder
        }
        pub fn query(
            self,
            filter: stmt::Expr<'a, super::super::todo::Todo, bool>,
        ) -> super::super::todo::Query<'a> {
            let query = self.into_select();
            super::super::todo::Query::from_stmt(query.and(filter))
        }
        pub fn find_by_id(
            self,
            id: impl toasty::stmt::IntoExpr<'a, super::super::todo::Todo, Id<super::super::todo::Todo>>,
        ) -> FindByCategoryAndId<'a> {
            FindByCategoryAndId {
                stmt: stmt::Select::from_expr(
                    super::super::todo::Todo::FIELDS
                        .category()
                        .in_query(self.scope)
                        .and(super::super::todo::Todo::FIELDS.id().eq(id.into_expr())),
                ),
            }
        }
    }
    impl<'a> stmt::IntoSelect<'a> for Todos<'a> {
        type Model = super::super::todo::Todo;
        fn into_select(self) -> stmt::Select<'a, super::super::todo::Todo> {
            super::super::todo::Todo::filter(
                super::super::todo::Todo::FIELDS
                    .category()
                    .in_query(self.scope),
            )
            .into_select()
        }
    }
    pub struct FindByCategoryAndId<'a> {
        stmt: stmt::Select<'a, super::super::todo::Todo>,
    }
    impl<'a> FindByCategoryAndId<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::super::todo::Todo>> {
            db.all(self).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::super::todo::Todo>> {
            db.first(self).await
        }
        pub async fn get(self, db: &Db) -> Result<super::super::todo::Todo> {
            db.get(self).await
        }
        pub fn update(self) -> super::super::todo::UpdateQuery<'a> {
            super::super::todo::UpdateQuery::from(self.stmt)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            db.exec(self.stmt.delete()).await?;
            Ok(())
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindByCategoryAndId<'a> {
        type Model = super::super::todo::Todo;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.stmt
        }
    }
}
pub mod queries {
    use super::*;
    impl super::Category {
        pub fn find_by_id<'a>(
            id: impl toasty::stmt::IntoExpr<'a, Category, Id<Category>>,
        ) -> FindById<'a> {
            FindById {
                query: Query::from_stmt(stmt::Select::from_expr(
                    Category::FIELDS.id().eq(id.into_expr()),
                )),
            }
        }
    }
    pub struct FindById<'a> {
        query: Query<'a>,
    }
    impl<'a> FindById<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Category>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Category>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Category> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn filter(self, filter: stmt::Expr<'a, Category, bool>) -> Query<'a> {
            let stmt = self.into_select();
            Query::from_stmt(stmt.and(filter))
        }
        pub async fn collect<A>(self, db: &'a Db) -> Result<A>
        where
            A: FromCursor<super::Category>,
        {
            self.all(db).await?.collect().await
        }
        pub fn todos(mut self) -> super::relation::Todos<'a> {
            super::relation::Todos { scope: self.query }
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindById<'a> {
        type Model = super::Category;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
}
