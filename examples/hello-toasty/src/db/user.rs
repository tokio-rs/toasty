use toasty::codegen_support::*;
#[derive(Debug)]
pub struct User {
    pub id: Id<User>,
    pub name: String,
    pub email: String,
    pub todos: HasMany<super::todo::Todo>,
    pub moto: Option<String>,
}
impl User {
    pub const ID: Path<Id<User>> = Path::from_field_index::<Self>(0);
    pub const NAME: Path<String> = Path::from_field_index::<Self>(1);
    pub const EMAIL: Path<String> = Path::from_field_index::<Self>(2);
    pub const TODOS: <super::todo::Todo as Relation>::ManyField =
        <super::todo::Todo as Relation>::ManyField::from_path(Path::from_field_index::<Self>(3));
    pub const MOTO: Path<String> = Path::from_field_index::<Self>(4);
    pub fn todos(&self) -> <super::todo::Todo as Relation>::Many {
        <super::todo::Todo as Relation>::Many::from_stmt(stmt::Association::many(
            self.into_select(),
            Self::TODOS.into(),
        ))
    }
    pub async fn get_by_email(db: &Db, email: impl IntoExpr<String>) -> Result<User> {
        Self::filter_by_email(email).get(db).await
    }
    pub fn filter_by_email(email: impl IntoExpr<String>) -> Query {
        Query::default().filter_by_email(email)
    }
    pub async fn get_by_id(db: &Db, id: impl IntoExpr<Id<User>>) -> Result<User> {
        Self::filter_by_id(id).get(db).await
    }
    pub fn filter_by_id(id: impl IntoExpr<Id<User>>) -> Query {
        Query::default().filter_by_id(id)
    }
    pub fn filter_by_id_batch(keys: impl IntoExpr<[Id<User>]>) -> Query {
        Query::default().filter_by_id_batch(keys)
    }
    pub fn create() -> builders::CreateUser {
        builders::CreateUser::default()
    }
    pub fn create_many() -> CreateMany<User> {
        CreateMany::default()
    }
    pub fn update(&mut self) -> builders::UpdateUser<'_> {
        let query = builders::UpdateQuery::from(self.into_select());
        builders::UpdateUser { model: self, query }
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
impl Model for User {
    const ID: ModelId = ModelId(0);
    type Key = Id<User>;
    fn load(mut record: ValueRecord) -> Result<Self, Error> {
        Ok(User {
            id: Id::from_untyped(record[0].take().to_id()?),
            name: record[1].take().to_string()?,
            email: record[2].take().to_string()?,
            todos: HasMany::load(record[3].take())?,
            moto: record[4].take().to_option_string()?,
        })
    }
}
impl Relation for User {
    type Query = Query;
    type Many = relations::Many;
    type ManyField = relations::ManyField;
    type One = relations::One;
    type OneField = relations::OneField;
    type OptionOne = relations::OptionOne;
}
impl stmt::IntoSelect for &User {
    type Model = User;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Query::default().filter_by_id(&self.id).stmt
    }
}
impl stmt::IntoSelect for &mut User {
    type Model = User;
    fn into_select(self) -> stmt::Select<Self::Model> {
        (&*self).into_select()
    }
}
impl stmt::IntoSelect for User {
    type Model = User;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Query::default().filter_by_id(self.id).stmt
    }
}
impl stmt::IntoExpr<User> for User {
    fn into_expr(self) -> stmt::Expr<User> {
        let expr: stmt::Expr<Id<User>> = self.id.into_expr();
        expr.cast()
    }
    fn by_ref(&self) -> stmt::Expr<User> {
        let expr: stmt::Expr<Id<User>> = (&self.id).into_expr();
        expr.cast()
    }
}
impl stmt::IntoExpr<[User]> for User {
    fn into_expr(self) -> stmt::Expr<[User]> {
        stmt::Expr::list([self])
    }
    fn by_ref(&self) -> stmt::Expr<[User]> {
        stmt::Expr::list([self])
    }
}
#[derive(Debug)]
pub struct Query {
    stmt: stmt::Select<User>,
}
impl Query {
    pub const fn from_stmt(stmt: stmt::Select<User>) -> Query {
        Query { stmt }
    }
    pub async fn get_by_email(self, db: &Db, email: impl IntoExpr<String>) -> Result<User> {
        self.filter_by_email(email).get(db).await
    }
    pub fn filter_by_email(self, email: impl IntoExpr<String>) -> Query {
        self.filter(User::EMAIL.eq(email))
    }
    pub async fn get_by_id(self, db: &Db, id: impl IntoExpr<Id<User>>) -> Result<User> {
        self.filter_by_id(id).get(db).await
    }
    pub fn filter_by_id(self, id: impl IntoExpr<Id<User>>) -> Query {
        self.filter(User::ID.eq(id))
    }
    pub fn filter_by_id_batch(self, keys: impl IntoExpr<[Id<User>]>) -> Query {
        self.filter(stmt::Expr::in_list(User::ID, keys))
    }
    pub async fn all(self, db: &Db) -> Result<Cursor<User>> {
        db.all(self.stmt).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<User>> {
        db.first(self.stmt).await
    }
    pub async fn get(self, db: &Db) -> Result<User> {
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
        A: FromCursor<User>,
    {
        self.all(db).await?.collect().await
    }
    pub fn filter(self, expr: stmt::Expr<bool>) -> Query {
        Query {
            stmt: self.stmt.and(expr),
        }
    }
    pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> Self {
        self.stmt.include(path.into());
        self
    }
    pub fn todos(mut self) -> <super::todo::Todo as Relation>::Query {
        <super::todo::Todo as Relation>::Query::from_stmt(
            stmt::Association::many(self.stmt, User::TODOS.into()).into_select(),
        )
    }
}
impl stmt::IntoSelect for Query {
    type Model = User;
    fn into_select(self) -> stmt::Select<User> {
        self.stmt
    }
}
impl stmt::IntoSelect for &Query {
    type Model = User;
    fn into_select(self) -> stmt::Select<User> {
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
    pub struct CreateUser {
        pub(super) stmt: stmt::Insert<User>,
    }
    impl CreateUser {
        pub fn id(mut self, id: impl Into<Id<User>>) -> Self {
            self.stmt.set(0, id.into());
            self
        }
        pub fn name(mut self, name: impl Into<String>) -> Self {
            self.stmt.set(1, name.into());
            self
        }
        pub fn email(mut self, email: impl Into<String>) -> Self {
            self.stmt.set(2, email.into());
            self
        }
        pub fn todo(mut self, todo: impl IntoExpr<super::super::todo::Todo>) -> Self {
            self.stmt.insert(3, todo.into_expr());
            self
        }
        pub fn moto(mut self, moto: impl Into<String>) -> Self {
            self.stmt.set(4, moto.into());
            self
        }
        pub async fn exec(self, db: &Db) -> Result<User> {
            db.exec_insert_one(self.stmt).await
        }
    }
    impl IntoInsert for CreateUser {
        type Model = User;
        fn into_insert(self) -> stmt::Insert<User> {
            self.stmt
        }
    }
    impl IntoExpr<User> for CreateUser {
        fn into_expr(self) -> stmt::Expr<User> {
            self.stmt.into()
        }
        fn by_ref(&self) -> stmt::Expr<User> {
            todo!()
        }
    }
    impl IntoExpr<[User]> for CreateUser {
        fn into_expr(self) -> stmt::Expr<[User]> {
            self.stmt.into_list_expr()
        }
        fn by_ref(&self) -> stmt::Expr<[User]> {
            todo!()
        }
    }
    impl Default for CreateUser {
        fn default() -> CreateUser {
            CreateUser {
                stmt: stmt::Insert::blank(),
            }
        }
    }
    #[derive(Debug)]
    pub struct UpdateUser<'a> {
        pub(super) model: &'a mut User,
        pub(super) query: UpdateQuery,
    }
    #[derive(Debug)]
    pub struct UpdateQuery {
        stmt: stmt::Update<User>,
    }
    impl UpdateUser<'_> {
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
        pub fn todo(mut self, todo: impl IntoExpr<super::super::todo::Todo>) -> Self {
            self.query.add_todo(todo);
            self
        }
        pub fn moto(mut self, moto: impl Into<String>) -> Self {
            self.query.set_moto(moto);
            self
        }
        pub fn unset_moto(&mut self) -> &mut Self {
            self.query.unset_moto();
            self
        }
        pub async fn exec(self, db: &Db) -> Result<()> {
            let mut stmt = self.query.stmt;
            let mut result = db.exec_one(stmt.into()).await?;
            for (field, value) in result.into_sparse_record().into_iter() {
                match field {
                    0 => self.model.id = stmt::Id::from_untyped(value.to_id()?),
                    1 => self.model.name = value.to_string()?,
                    2 => self.model.email = value.to_string()?,
                    3 => todo!("should not be set; {} = {value:#?}", 3),
                    4 => self.model.moto = value.to_option_string()?,
                    _ => todo!("handle unknown field id in reload after update"),
                }
            }
            Ok(())
        }
    }
    impl UpdateQuery {
        pub fn id(mut self, id: impl Into<Id<User>>) -> Self {
            self.set_id(id);
            self
        }
        pub fn set_id(&mut self, id: impl Into<Id<User>>) -> &mut Self {
            self.stmt.set(0, id.into());
            self
        }
        pub fn name(mut self, name: impl Into<String>) -> Self {
            self.set_name(name);
            self
        }
        pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
            self.stmt.set(1, name.into());
            self
        }
        pub fn email(mut self, email: impl Into<String>) -> Self {
            self.set_email(email);
            self
        }
        pub fn set_email(&mut self, email: impl Into<String>) -> &mut Self {
            self.stmt.set(2, email.into());
            self
        }
        pub fn todo(mut self, todo: impl IntoExpr<super::super::todo::Todo>) -> Self {
            self.add_todo(todo);
            self
        }
        pub fn add_todo(&mut self, todo: impl IntoExpr<super::super::todo::Todo>) -> &mut Self {
            self.stmt.insert(3, todo.into_expr());
            self
        }
        pub fn moto(mut self, moto: impl Into<String>) -> Self {
            self.set_moto(moto);
            self
        }
        pub fn set_moto(&mut self, moto: impl Into<String>) -> &mut Self {
            self.stmt.set(4, moto.into());
            self
        }
        pub fn unset_moto(&mut self) -> &mut Self {
            self.stmt.set(4, Value::Null);
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
    impl From<stmt::Select<User>> for UpdateQuery {
        fn from(src: stmt::Select<User>) -> UpdateQuery {
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
        stmt: stmt::Association<[User]>,
    }
    #[derive(Debug)]
    pub struct One {
        stmt: stmt::Select<User>,
    }
    #[derive(Debug)]
    pub struct OptionOne {
        stmt: stmt::Select<User>,
    }
    pub struct ManyField {
        pub(super) path: Path<[super::User]>,
    }
    pub struct OneField {
        pub(super) path: Path<super::User>,
    }
    impl Many {
        pub fn from_stmt(stmt: stmt::Association<[User]>) -> Many {
            Many { stmt }
        }
        pub async fn get_by_email(self, db: &Db, email: impl IntoExpr<String>) -> Result<User> {
            self.filter_by_email(email).get(db).await
        }
        pub fn filter_by_email(self, email: impl IntoExpr<String>) -> Query {
            Query::from_stmt(self.into_select()).filter(User::EMAIL.eq(email))
        }
        pub async fn get_by_id(self, db: &Db, id: impl IntoExpr<Id<User>>) -> Result<User> {
            self.filter_by_id(id).get(db).await
        }
        pub fn filter_by_id(self, id: impl IntoExpr<Id<User>>) -> Query {
            Query::from_stmt(self.into_select()).filter(User::ID.eq(id))
        }
        pub fn filter_by_id_batch(self, keys: impl IntoExpr<[Id<User>]>) -> Query {
            Query::from_stmt(self.into_select()).filter_by_id_batch(keys)
        }
        #[doc = r" Iterate all entries in the relation"]
        pub async fn all(self, db: &Db) -> Result<Cursor<User>> {
            db.all(self.stmt.into_select()).await
        }
        pub async fn collect<A>(self, db: &Db) -> Result<A>
        where
            A: FromCursor<User>,
        {
            self.all(db).await?.collect().await
        }
        pub fn query(self, filter: stmt::Expr<bool>) -> super::Query {
            let query = self.into_select();
            super::Query::from_stmt(query.and(filter))
        }
        pub fn create(self) -> builders::CreateUser {
            let mut builder = builders::CreateUser::default();
            builder.stmt.set_scope(self.stmt.into_select());
            builder
        }
        #[doc = r" Add an item to the association"]
        pub async fn insert(self, db: &Db, item: impl IntoExpr<[User]>) -> Result<()> {
            let stmt = self.stmt.insert(item);
            db.exec(stmt).await?;
            Ok(())
        }
        #[doc = r" Remove items from the association"]
        pub async fn remove(self, db: &Db, item: impl IntoExpr<User>) -> Result<()> {
            let stmt = self.stmt.remove(item);
            db.exec(stmt).await?;
            Ok(())
        }
    }
    impl stmt::IntoSelect for Many {
        type Model = User;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.stmt.into_select()
        }
    }
    impl One {
        pub fn from_stmt(stmt: stmt::Select<User>) -> One {
            One { stmt }
        }
        #[doc = r" Create a new associated record"]
        pub fn create(self) -> builders::CreateUser {
            let mut builder = builders::CreateUser::default();
            builder.stmt.set_scope(self.stmt.into_select());
            builder
        }
        pub async fn get(self, db: &Db) -> Result<User> {
            db.get(self.stmt.into_select()).await
        }
    }
    impl stmt::IntoSelect for One {
        type Model = User;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.stmt.into_select()
        }
    }
    impl OptionOne {
        pub fn from_stmt(stmt: stmt::Select<User>) -> OptionOne {
            OptionOne { stmt }
        }
        #[doc = r" Create a new associated record"]
        pub fn create(self) -> builders::CreateUser {
            let mut builder = builders::CreateUser::default();
            builder.stmt.set_scope(self.stmt.into_select());
            builder
        }
        pub async fn get(self, db: &Db) -> Result<Option<User>> {
            db.first(self.stmt.into_select()).await
        }
    }
    impl ManyField {
        pub const fn from_path(path: Path<[super::User]>) -> ManyField {
            ManyField { path }
        }
    }
    impl Into<Path<[User]>> for ManyField {
        fn into(self) -> Path<[User]> {
            self.path
        }
    }
    impl OneField {
        pub const fn from_path(path: Path<super::User>) -> OneField {
            OneField { path }
        }
        pub fn eq<T>(self, rhs: T) -> stmt::Expr<bool>
        where
            T: IntoExpr<super::User>,
        {
            self.path.eq(rhs.into_expr())
        }
        pub fn in_query<Q>(self, rhs: Q) -> toasty::stmt::Expr<bool>
        where
            Q: stmt::IntoSelect<Model = super::User>,
        {
            self.path.in_query(rhs)
        }
    }
    impl Into<Path<User>> for OneField {
        fn into(self) -> Path<User> {
            self.path
        }
    }
}
