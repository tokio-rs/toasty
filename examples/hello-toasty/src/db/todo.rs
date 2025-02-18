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
    pub const USER: <super::user::User as Relation>::OneField =
        <super::user::User as Relation>::OneField::from_path(Path::from_field_index::<Self>(2));
    pub const TITLE: Path<String> = Path::from_field_index::<Self>(3);
    pub fn user(&self) -> <super::user::User as Relation>::One {
        <super::user::User as Relation>::One::from_stmt(
            super::user::User::filter(super::user::User::ID.eq(&self.user_id)).into_select(),
        )
    }
    pub async fn get_by_id(db: &Db, id: impl IntoExpr<Id<Todo>>) -> Result<Todo> {
        Self::filter_by_id(id).get(db).await
    }
    pub fn filter_by_id(id: impl IntoExpr<Id<Todo>>) -> Query {
        Query::default().filter_by_id(id)
    }
    pub fn filter_by_id_batch(keys: impl IntoExpr<[Id<Todo>]>) -> Query {
        Query::default().filter_by_id_batch(keys)
    }
    pub async fn get_by_user_id(
        db: &Db,
        user_id: impl IntoExpr<Id<super::user::User>>,
    ) -> Result<Todo> {
        Self::filter_by_user_id(user_id).get(db).await
    }
    pub fn filter_by_user_id(user_id: impl IntoExpr<Id<super::user::User>>) -> Query {
        Query::default().filter_by_user_id(user_id)
    }
    pub fn create() -> builders::CreateTodo {
        builders::CreateTodo::default()
    }
    pub fn create_many() -> CreateMany<Todo> {
        CreateMany::default()
    }
    pub fn update(&mut self) -> builders::UpdateTodo<'_> {
        let query = builders::UpdateQuery::from(self.into_select());
        builders::UpdateTodo { model: self, query }
    }
    pub fn filter(expr: stmt::Expr<bool>) -> Query {
        Query::from_stmt(stmt::Select::filter(expr))
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
    fn load(mut record: ValueRecord) -> Result<Self, Error> {
        Ok(Todo {
            id: Id::from_untyped(record[0].take().to_id()?),
            user_id: Id::from_untyped(record[1].take().to_id()?),
            user: BelongsTo::load(record[2].take())?,
            title: record[3].take().to_string()?,
        })
    }
}
impl Relation for Todo {
    type Many = relations::Many;
    type ManyField = relations::ManyField;
    type One = relations::One;
    type OneField = relations::OneField;
    type OptionOne = relations::OptionOne;
}
impl stmt::IntoSelect for &Todo {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Query::default().filter_by_id(&self.id).stmt
    }
}
impl stmt::IntoSelect for &mut Todo {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<Self::Model> {
        (&*self).into_select()
    }
}
impl stmt::IntoSelect for Todo {
    type Model = Todo;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Query::default().filter_by_id(self.id).stmt
    }
}
impl stmt::IntoExpr<Todo> for Todo {
    fn into_expr(self) -> stmt::Expr<Todo> {
        self.id.into_expr().cast()
    }
    fn by_ref(&self) -> stmt::Expr<Todo> {
        (&self.id).into_expr().cast()
    }
}
impl stmt::IntoExpr<[Todo]> for Todo {
    fn into_expr(self) -> stmt::Expr<[Todo]> {
        stmt::Expr::list([self])
    }
    fn by_ref(&self) -> stmt::Expr<[Todo]> {
        stmt::Expr::list([self])
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
    pub fn filter_by_id(self, id: impl IntoExpr<Id<Todo>>) -> Query {
        self.filter(Todo::ID.eq(id))
    }
    pub fn filter_by_id_batch(self, keys: impl IntoExpr<[Id<Todo>]>) -> Query {
        self.filter(stmt::Expr::in_list(Todo::ID, keys))
    }
    pub fn filter_by_user_id(self, user_id: impl IntoExpr<Id<super::user::User>>) -> Query {
        self.filter(Todo::USER_ID.eq(user_id))
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
    pub fn update(self) -> builders::UpdateQuery {
        builders::UpdateQuery::from(self)
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
pub mod builders {
    use super::*;
    #[derive(Debug)]
    pub struct CreateTodo {
        pub(super) stmt: stmt::Insert<Todo>,
    }
    impl CreateTodo {
        pub fn id(mut self, id: impl Into<Id<Todo>>) -> Self {
            self.stmt.set(0, id.into());
            self
        }
        pub fn user_id(mut self, user_id: impl Into<Id<super::super::user::User>>) -> Self {
            self.stmt.set(1, user_id.into());
            self
        }
        pub fn user(mut self, user: impl IntoExpr<super::super::user::User>) -> Self {
            self.stmt.set(2, user.into_expr());
            self
        }
        pub fn title(mut self, title: impl Into<String>) -> Self {
            self.stmt.set(3, title.into());
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
        fn by_ref(&self) -> stmt::Expr<Todo> {
            todo!()
        }
    }
    impl IntoExpr<[Todo]> for CreateTodo {
        fn into_expr(self) -> stmt::Expr<[Todo]> {
            self.stmt.into_list_expr()
        }
        fn by_ref(&self) -> stmt::Expr<[Todo]> {
            todo!()
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
        pub(super) model: &'a mut Todo,
        pub(super) query: UpdateQuery,
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
        pub fn user_id(mut self, user_id: impl Into<Id<super::super::user::User>>) -> Self {
            self.query.set_user_id(user_id);
            self
        }
        pub fn user(mut self, user: impl IntoExpr<super::super::user::User>) -> Self {
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
                match field {
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
            self.stmt.set(0, id.into());
            self
        }
        pub fn user_id(mut self, user_id: impl Into<Id<super::super::user::User>>) -> Self {
            self.set_user_id(user_id);
            self
        }
        pub fn set_user_id(
            &mut self,
            user_id: impl Into<Id<super::super::user::User>>,
        ) -> &mut Self {
            self.stmt.set(1, user_id.into());
            self
        }
        pub fn user(mut self, user: impl IntoExpr<super::super::user::User>) -> Self {
            self.set_user(user);
            self
        }
        pub fn set_user(&mut self, user: impl IntoExpr<super::super::user::User>) -> &mut Self {
            self.stmt.set(2, user.into_expr());
            self
        }
        pub fn title(mut self, title: impl Into<String>) -> Self {
            self.set_title(title);
            self
        }
        pub fn set_title(&mut self, title: impl Into<String>) -> &mut Self {
            self.stmt.set(3, title.into());
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
                stmt: stmt::Update::new(value.stmt),
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
}
pub mod relations {
    use super::*;
    #[derive(Debug)]
    pub struct Many {
        stmt: stmt::Association<[Todo]>,
    }
    #[derive(Debug)]
    pub struct One {
        stmt: stmt::Select<Todo>,
    }
    #[derive(Debug)]
    pub struct OptionOne {
        stmt: stmt::Select<Todo>,
    }
    pub struct ManyField {
        pub(super) path: Path<[super::Todo]>,
    }
    pub struct OneField {
        pub(super) path: Path<super::Todo>,
    }
    impl Many {
        pub fn from_stmt(stmt: stmt::Association<[Todo]>) -> Many {
            Many { stmt }
        }
        pub async fn get_by_id(self, db: &Db, id: impl IntoExpr<Id<Todo>>) -> Result<Todo> {
            self.filter_by_id(id).get(db).await
        }
        pub fn filter_by_id(self, id: impl IntoExpr<Id<Todo>>) -> Query {
            Query::from_stmt(self.into_select()).filter_by_id(id)
        }
        pub fn filter_by_id_batch(self, keys: impl IntoExpr<[Id<Todo>]>) -> Query {
            Query::from_stmt(self.into_select()).filter_by_id_batch(keys)
        }
        pub async fn get_by_user_id(
            self,
            db: &Db,
            user_id: impl IntoExpr<Id<super::super::user::User>>,
        ) -> Result<Todo> {
            self.filter_by_user_id(user_id).get(db).await
        }
        pub fn filter_by_user_id(
            self,
            user_id: impl IntoExpr<Id<super::super::user::User>>,
        ) -> Query {
            Query::from_stmt(self.into_select()).filter_by_user_id(user_id)
        }
        #[doc = r" Iterate all entries in the relation"]
        pub async fn all(self, db: &Db) -> Result<Cursor<Todo>> {
            db.all(self.stmt.into_select()).await
        }
        pub async fn collect<A>(self, db: &Db) -> Result<A>
        where
            A: FromCursor<Todo>,
        {
            self.all(db).await?.collect().await
        }
        pub fn query(self, filter: stmt::Expr<bool>) -> super::Query {
            let query = self.into_select();
            super::Query::from_stmt(query.and(filter))
        }
        pub fn create(self) -> builders::CreateTodo {
            let mut builder = builders::CreateTodo::default();
            builder.stmt.set_scope(self.stmt.into_select());
            builder
        }
        #[doc = r" Remove items from the association"]
        pub async fn remove(self, db: &Db, item: impl IntoExpr<Todo>) -> Result<()> {
            let stmt = self.stmt.remove(item);
            db.exec(stmt).await?;
            Ok(())
        }
    }
    impl stmt::IntoSelect for Many {
        type Model = Todo;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.stmt.into_select()
        }
    }
    impl One {
        pub fn from_stmt(stmt: stmt::Select<Todo>) -> One {
            One { stmt }
        }
        pub async fn get(self, db: &Db) -> Result<Todo> {
            db.get(self.stmt.into_select()).await
        }
    }
    impl stmt::IntoSelect for One {
        type Model = Todo;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.stmt.into_select()
        }
    }
    impl OptionOne {
        pub fn from_stmt(stmt: stmt::Select<Todo>) -> OptionOne {
            OptionOne { stmt }
        }
        pub async fn get(self, db: &Db) -> Result<Option<Todo>> {
            db.first(self.stmt.into_select()).await
        }
    }
    impl ManyField {
        pub const fn from_path(path: Path<[super::Todo]>) -> ManyField {
            ManyField { path }
        }
    }
    impl Into<Path<[Todo]>> for ManyField {
        fn into(self) -> Path<[Todo]> {
            self.path
        }
    }
    impl OneField {
        pub const fn from_path(path: Path<super::Todo>) -> OneField {
            OneField { path }
        }
        pub fn in_query<Q>(self, rhs: Q) -> toasty::stmt::Expr<bool>
        where
            Q: stmt::IntoSelect<Model = super::Todo>,
        {
            self.path.in_query(rhs)
        }
    }
    impl Into<Path<Todo>> for OneField {
        fn into(self) -> Path<Todo> {
            self.path
        }
    }
}
