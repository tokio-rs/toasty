use toasty::codegen_support::*;
#[derive(Debug)]
pub struct User {
    pub id: Id<User>,
    pub name: String,
}
impl User {
    pub const ID: Path<Id<User>> = Path::from_field_index::<Self>(0);
    pub const NAME: Path<String> = Path::from_field_index::<Self>(1);
    pub const PROFILE: self::fields::Profile =
        self::fields::Profile::from_path(Path::from_field_index::<Self>(2));
    pub fn create() -> CreateUser {
        CreateUser::default()
    }
    pub fn create_many() -> CreateMany<User> {
        CreateMany::default()
    }
    pub fn filter(expr: stmt::Expr<bool>) -> Query {
        Query::from_stmt(stmt::Select::filter(expr))
    }
    pub fn update(&mut self) -> UpdateUser<'_> {
        let query = UpdateQuery::from(self.into_select());
        UpdateUser { model: self, query }
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
        })
    }
}
impl stmt::IntoSelect for &User {
    type Model = User;
    fn into_select(self) -> stmt::Select<Self::Model> {
        User::find_by_id(&self.id).into_select()
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
        User::find_by_id(self.id).into_select()
    }
}
impl stmt::IntoExpr<User> for User {
    fn into_expr(self) -> stmt::Expr<User> {
        todo!()
    }
}
impl stmt::IntoExpr<User> for &User {
    fn into_expr(self) -> stmt::Expr<User> {
        stmt::Key::from_expr(&self.id).into()
    }
}
impl stmt::IntoExpr<[User]> for &User {
    fn into_expr(self) -> stmt::Expr<[User]> {
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
    pub async fn all(self, db: &Db) -> Result<Cursor<User>> {
        db.all(self.stmt).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<User>> {
        db.first(self.stmt).await
    }
    pub async fn get(self, db: &Db) -> Result<User> {
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
        A: FromCursor<User>,
    {
        self.all(db).await?.collect().await
    }
    pub fn filter(self, expr: stmt::Expr<bool>) -> Query {
        Query {
            stmt: self.stmt.and(expr),
        }
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
    pub fn profile(mut self, profile: impl IntoExpr<super::profile::Profile>) -> Self {
        self.stmt.set(2, profile.into_expr());
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
}
impl IntoExpr<[User]> for CreateUser {
    fn into_expr(self) -> stmt::Expr<[User]> {
        self.stmt.into_list_expr()
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
    model: &'a mut User,
    query: UpdateQuery,
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
    pub fn profile(mut self, profile: impl IntoExpr<super::profile::Profile>) -> Self {
        self.query.set_profile(profile);
        self
    }
    pub fn unset_profile(&mut self) -> &mut Self {
        self.query.unset_profile();
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let mut stmt = self.query.stmt;
        let mut result = db.exec_one(stmt.into()).await?;
        for (field, value) in result.into_sparse_record().into_iter() {
            match field {
                0 => self.model.id = stmt::Id::from_untyped(value.to_id()?),
                1 => self.model.name = value.to_string()?,
                2 => todo!("should not be set; {} = {value:#?}", 2),
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
    pub fn profile(mut self, profile: impl IntoExpr<super::profile::Profile>) -> Self {
        self.set_profile(profile);
        self
    }
    pub fn set_profile(&mut self, profile: impl IntoExpr<super::profile::Profile>) -> &mut Self {
        self.stmt.set(2, profile.into_expr());
        self
    }
    pub fn unset_profile(&mut self) -> &mut Self {
        self.stmt.set(2, Value::Null);
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
pub mod fields {
    use super::*;
    pub struct Profile {
        pub(super) path: Path<super::super::profile::Profile>,
    }
    impl Profile {
        pub const fn from_path(path: Path<super::super::profile::Profile>) -> Profile {
            Profile { path }
        }
        pub fn id(mut self) -> Path<Id<super::super::profile::Profile>> {
            self.path.chain(super::super::profile::Profile::ID)
        }
        pub fn user(mut self) -> super::super::profile::fields::User {
            let path = self.path.chain(super::super::profile::Profile::USER);
            super::super::profile::fields::User::from_path(path)
        }
        pub fn user_id(mut self) -> Path<Id<User>> {
            self.path.chain(super::super::profile::Profile::USER_ID)
        }
    }
    impl From<Profile> for Path<super::super::profile::Profile> {
        fn from(val: Profile) -> Path<super::super::profile::Profile> {
            val.path
        }
    }
    impl<'a> stmt::IntoExpr<super::relation::profile::Profile<'a>> for Profile {
        fn into_expr(self) -> stmt::Expr<super::relation::profile::Profile<'a>> {
            todo!("into_expr for {} (field path struct)", stringify!(Profile));
        }
    }
}
pub mod relation {
    use super::*;
    use toasty::Cursor;
    pub mod profile {
        use super::*;
        #[derive(Debug)]
        pub struct Profile<'a> {
            scope: &'a User,
        }
        #[derive(Debug)]
        pub struct Query {
            pub(super) scope: super::Query,
        }
        impl super::User {
            pub fn profile(&self) -> Profile<'_> {
                Profile { scope: self }
            }
        }
        impl super::Query {
            pub fn profile(self) -> Query {
                Query::with_scope(self)
            }
        }
        impl Profile<'_> {
            #[doc = r" Get the relation"]
            pub async fn get(
                self,
                db: &Db,
            ) -> Result<Option<super::super::super::profile::Profile>> {
                db.first(self.into_select()).await
            }
            #[doc = r" Create a new associated record"]
            pub fn create(self) -> super::super::super::profile::CreateProfile {
                let mut builder = super::super::super::profile::CreateProfile::default();
                builder.stmt.set_scope(self);
                builder
            }
        }
        impl<'a> stmt::IntoSelect for Profile<'a> {
            type Model = super::super::super::profile::Profile;
            fn into_select(self) -> stmt::Select<super::super::super::profile::Profile> {
                super::super::super::profile::Profile::filter(
                    super::super::super::profile::Profile::USER.in_query(self.scope),
                )
                .into_select()
            }
        }
        impl Query {
            pub fn with_scope<S>(scope: S) -> Query
            where
                S: IntoSelect<Model = User>,
            {
                Query {
                    scope: super::Query::from_stmt(scope.into_select()),
                }
            }
        }
    }
    pub use profile::Profile;
}
pub mod queries {
    use super::*;
    impl super::User {
        pub fn find_by_id(id: impl stmt::IntoExpr<Id<User>>) -> FindById {
            FindById {
                query: Query::from_stmt(stmt::Select::filter(User::ID.eq(id))),
            }
        }
    }
    pub struct FindById {
        query: Query,
    }
    impl FindById {
        pub async fn all(self, db: &Db) -> Result<Cursor<super::User>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::User>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::User> {
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
            A: FromCursor<super::User>,
        {
            self.all(db).await?.collect().await
        }
        pub fn profile(mut self) -> self::relation::profile::Query {
            self::relation::profile::Query::with_scope(self)
        }
    }
    impl stmt::IntoSelect for FindById {
        type Model = super::User;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.query.into_select()
        }
    }
    impl super::User {
        pub fn find_many_by_id() -> FindManyById {
            FindManyById { items: vec![] }
        }
    }
    pub struct FindManyById {
        items: Vec<stmt::Expr<Id<User>>>,
    }
    impl FindManyById {
        pub fn item(mut self, id: impl stmt::IntoExpr<Id<User>>) -> Self {
            self.items.push(id.into_expr());
            self
        }
        pub async fn all(self, db: &Db) -> Result<Cursor<super::User>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::User>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::User> {
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
            A: FromCursor<super::User>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindManyById {
        type Model = super::User;
        fn into_select(self) -> stmt::Select<Self::Model> {
            stmt::Select::filter(stmt::in_set(User::ID, self.items))
        }
    }
}
