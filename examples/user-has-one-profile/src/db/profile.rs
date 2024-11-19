use toasty::codegen_support::*;
#[derive(Debug)]
pub struct Profile {
    pub id: Id<Profile>,
    user: BelongsTo<super::user::User>,
    pub user_id: Option<Id<super::user::User>>,
}
impl Profile {
    pub const ID: Path<Id<Profile>> = Path::from_field_index::<Self>(0);
    pub const USER: self::fields::User =
        self::fields::User::from_path(Path::from_field_index::<Self>(1));
    pub const USER_ID: Path<Id<super::user::User>> = Path::from_field_index::<Self>(2);
    pub fn create<'a>() -> CreateProfile<'a> {
        CreateProfile::default()
    }
    pub fn create_many<'a>() -> CreateMany<'a, Profile> {
        CreateMany::default()
    }
    pub fn filter<'a>(expr: stmt::Expr<'a, bool>) -> Query<'a> {
        Query::from_stmt(stmt::Select::from_expr(expr))
    }
    pub fn update<'a>(&'a mut self) -> UpdateProfile<'a> {
        UpdateProfile {
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
impl Model for Profile {
    const ID: ModelId = ModelId(1);
    const FIELD_COUNT: usize = 3;
    type Key = Id<Profile>;
    fn load(mut record: RecordOld<'_>) -> Result<Self, Error> {
        Ok(Profile {
            id: Id::from_untyped(record[0].take().to_id()?),
            user: BelongsTo::load(record[1].take())?,
            user_id: record[2].take().to_option_id()?.map(Id::from_untyped),
        })
    }
}
impl<'a> stmt::IntoSelect<'a> for &'a Profile {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<'a, Self::Model> {
        Profile::find_by_id(&self.id).into_select()
    }
}
impl stmt::AsSelect for Profile {
    type Model = Profile;
    fn as_select(&self) -> stmt::Select<'_, Self::Model> {
        Profile::find_by_id(&self.id).into_select()
    }
}
impl stmt::IntoSelect<'static> for Profile {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<'static, Self::Model> {
        Profile::find_by_id(self.id).into_select()
    }
}
impl<'a> stmt::IntoExpr<'a, Profile> for &'a Profile {
    fn into_expr(self) -> stmt::Expr<'a, Profile> {
        stmt::Key::from_expr(&self.id).into()
    }
}
impl<'a> stmt::IntoExpr<'a, [Profile]> for &'a Profile {
    fn into_expr(self) -> stmt::Expr<'a, [Profile]> {
        stmt::Key::from_expr(&self.id).into()
    }
}
#[derive(Debug)]
pub struct Query<'a> {
    stmt: stmt::Select<'a, Profile>,
}
impl<'a> Query<'a> {
    pub const fn from_stmt(stmt: stmt::Select<'a, Profile>) -> Query<'a> {
        Query { stmt }
    }
    pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, Profile>> {
        db.all(self.stmt).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<Profile>> {
        db.first(self.stmt).await
    }
    pub async fn get(self, db: &Db) -> Result<Profile> {
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
        A: FromCursor<Profile>,
    {
        self.all(db).await?.collect().await
    }
    pub fn filter(self, expr: stmt::Expr<'a, bool>) -> Query<'a> {
        Query {
            stmt: self.stmt.and(expr),
        }
    }
    pub fn user(mut self) -> super::user::Query<'a> {
        todo!()
    }
}
impl<'a> stmt::IntoSelect<'a> for Query<'a> {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<'a, Profile> {
        self.stmt
    }
}
impl<'a> stmt::IntoSelect<'a> for &Query<'a> {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<'a, Profile> {
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
pub struct CreateProfile<'a> {
    pub(super) stmt: stmt::Insert<'a, Profile>,
}
impl<'a> CreateProfile<'a> {
    pub fn id(mut self, id: impl Into<Id<Profile>>) -> Self {
        self.stmt.set_value(0, id.into());
        self
    }
    pub fn user<'b>(mut self, user: impl IntoExpr<'a, self::relation::User<'b>>) -> Self {
        self.stmt.set_expr(1, user.into_expr());
        self
    }
    pub fn user_id(mut self, user_id: impl Into<Id<super::user::User>>) -> Self {
        self.stmt.set_value(2, user_id.into());
        self
    }
    pub async fn exec(self, db: &'a Db) -> Result<Profile> {
        db.exec_insert_one::<Profile>(self.stmt).await
    }
}
impl<'a> IntoInsert<'a> for CreateProfile<'a> {
    type Model = Profile;
    fn into_insert(self) -> stmt::Insert<'a, Profile> {
        self.stmt
    }
}
impl<'a> IntoExpr<'a, Profile> for CreateProfile<'a> {
    fn into_expr(self) -> stmt::Expr<'a, Profile> {
        self.stmt.into()
    }
}
impl<'a> IntoExpr<'a, [Profile]> for CreateProfile<'a> {
    fn into_expr(self) -> stmt::Expr<'a, [Profile]> {
        self.stmt.into_list_expr()
    }
}
impl<'a> Default for CreateProfile<'a> {
    fn default() -> CreateProfile<'a> {
        CreateProfile {
            stmt: stmt::Insert::blank(),
        }
    }
}
#[derive(Debug)]
pub struct UpdateProfile<'a> {
    model: &'a mut Profile,
    query: UpdateQuery<'a>,
}
#[derive(Debug)]
pub struct UpdateQuery<'a> {
    stmt: stmt::Update<'a, Profile>,
}
impl<'a> UpdateProfile<'a> {
    pub fn id(mut self, id: impl Into<Id<Profile>>) -> Self {
        self.query.set_id(id);
        self
    }
    pub fn user<'b>(mut self, user: impl IntoExpr<'a, self::relation::User<'b>>) -> Self {
        self.query.set_user(user);
        self
    }
    pub fn unset_user(&mut self) -> &mut Self {
        self.query.unset_user();
        self
    }
    pub fn user_id(mut self, user_id: impl Into<Id<super::user::User>>) -> Self {
        self.query.set_user_id(user_id);
        self
    }
    pub fn unset_user_id(&mut self) -> &mut Self {
        self.query.unset_user_id();
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let fields;
        let mut into_iter;
        {
            let mut stmt = self.query.stmt;
            fields = stmt.fields().clone();
            stmt.set_selection(&*self.model);
            let mut records = db.exec::<Profile>(stmt.into()).await?;
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
                1 => {
                    self.model.user_id = into_iter
                        .next()
                        .unwrap()
                        .to_option_id()?
                        .map(stmt::Id::from_untyped)
                }
                2 => {
                    self.model.user_id = into_iter
                        .next()
                        .unwrap()
                        .to_option_id()?
                        .map(stmt::Id::from_untyped)
                }
                _ => todo!("handle unknown field id in reload after update"),
            }
        }
        Ok(())
    }
}
impl<'a> UpdateQuery<'a> {
    pub fn id(mut self, id: impl Into<Id<Profile>>) -> Self {
        self.set_id(id);
        self
    }
    pub fn set_id(&mut self, id: impl Into<Id<Profile>>) -> &mut Self {
        self.stmt.set_expr(0, id.into());
        self
    }
    pub fn user<'b>(mut self, user: impl IntoExpr<'a, self::relation::User<'b>>) -> Self {
        self.set_user(user);
        self
    }
    pub fn set_user<'b>(&mut self, user: impl IntoExpr<'a, self::relation::User<'b>>) -> &mut Self {
        self.stmt.set_expr(1, user.into_expr());
        self
    }
    pub fn unset_user(&mut self) -> &mut Self {
        self.stmt.set(1, Value::Null);
        self
    }
    pub fn user_id(mut self, user_id: impl Into<Id<super::user::User>>) -> Self {
        self.set_user_id(user_id);
        self
    }
    pub fn set_user_id(&mut self, user_id: impl Into<Id<super::user::User>>) -> &mut Self {
        self.stmt.set_expr(2, user_id.into());
        self
    }
    pub fn unset_user_id(&mut self) -> &mut Self {
        self.stmt.set(2, Value::Null);
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
impl<'a> From<stmt::Select<'a, Profile>> for UpdateQuery<'a> {
    fn from(src: stmt::Select<'a, Profile>) -> UpdateQuery<'a> {
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
        pub fn profile(mut self) -> super::super::user::fields::Profile {
            let path = self.path.chain(super::super::user::User::PROFILE);
            super::super::user::fields::Profile::from_path(path)
        }
        pub fn eq<'a, 'b, T>(self, rhs: T) -> stmt::Expr<'a, bool>
        where
            T: toasty::stmt::IntoExpr<'a, super::relation::user::User<'b>>,
        {
            self.path.eq(rhs.into_expr().cast())
        }
        pub fn in_query<'a, Q>(self, rhs: Q) -> toasty::stmt::Expr<'a, bool>
        where
            Q: stmt::IntoSelect<'a, Model = super::super::user::User>,
        {
            self.path.in_query(rhs)
        }
    }
    impl From<User> for Path<super::super::user::User> {
        fn from(val: User) -> Path<super::super::user::User> {
            val.path
        }
    }
    impl<'stmt> stmt::IntoExpr<'stmt, super::relation::user::User<'stmt>> for User {
        fn into_expr(self) -> stmt::Expr<'stmt, super::relation::user::User<'stmt>> {
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
            scope: &'a Profile,
        }
        impl super::Profile {
            pub fn user(&self) -> User<'_> {
                User { scope: self }
            }
        }
        impl<'a> User<'a> {
            pub fn get(&self) -> &super::super::super::user::User {
                self.scope.user.get()
            }
        }
        impl<'a> stmt::IntoSelect<'a> for &'a User<'_> {
            type Model = super::super::super::user::User;
            fn into_select(self) -> stmt::Select<'a, Self::Model> {
                super::super::super::user::User::find_by_id(
                    self.scope
                        .user_id
                        .as_ref()
                        .expect("TODO: handle null fk fields"),
                )
                .into_select()
            }
        }
        impl<'stmt, 'a> stmt::IntoExpr<'stmt, User<'a>> for User<'a> {
            fn into_expr(self) -> stmt::Expr<'stmt, User<'a>> {
                todo!(
                    "stmt::IntoExpr for {} (belongs_to Fk struct) - self = {:#?}",
                    stringify!(User),
                    self
                );
            }
        }
        impl<'stmt, 'a> stmt::IntoExpr<'stmt, User<'a>> for &'stmt User<'a> {
            fn into_expr(self) -> stmt::Expr<'stmt, User<'a>> {
                todo!(
                    "stmt::IntoExpr for &'a {} (belongs_to Fk struct) - self = {:#?}",
                    stringify!(User),
                    self
                );
            }
        }
        impl<'stmt, 'a> stmt::IntoExpr<'stmt, User<'a>> for &'stmt super::super::super::user::User {
            fn into_expr(self) -> stmt::Expr<'stmt, User<'a>> {
                stmt::Expr::from_untyped(&self.id)
            }
        }
        impl<'stmt, 'a> stmt::IntoExpr<'stmt, User<'a>> for super::super::super::user::CreateUser<'stmt> {
            fn into_expr(self) -> stmt::Expr<'stmt, User<'a>> {
                let expr: stmt::Expr<'stmt, super::super::super::user::User> = self.stmt.into();
                expr.cast()
            }
        }
        impl<'a> User<'a> {
            pub async fn find(&self, db: &Db) -> Result<Option<super::super::super::user::User>> {
                db.first(self.into_select()).await
            }
        }
    }
    pub use user::User;
}
pub mod queries {
    use super::*;
    impl super::Profile {
        pub fn find_by_id<'a>(id: impl stmt::IntoExpr<'a, Id<Profile>>) -> FindById<'a> {
            FindById {
                query: Query::from_stmt(stmt::Select::from_expr(Profile::ID.eq(id))),
            }
        }
    }
    pub struct FindById<'a> {
        query: Query<'a>,
    }
    impl<'a> FindById<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Profile>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Profile>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Profile> {
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
            A: FromCursor<super::Profile>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindById<'a> {
        type Model = super::Profile;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Profile {
        pub fn find_many_by_id<'a>() -> FindManyById<'a> {
            FindManyById { items: vec![] }
        }
    }
    pub struct FindManyById<'a> {
        items: Vec<stmt::Expr<'a, Id<Profile>>>,
    }
    impl<'a> FindManyById<'a> {
        pub fn item(mut self, id: impl stmt::IntoExpr<'a, Id<Profile>>) -> Self {
            self.items.push(id.into_expr());
            self
        }
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Profile>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Profile>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Profile> {
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
            A: FromCursor<super::Profile>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindManyById<'a> {
        type Model = super::Profile;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            stmt::Select::from_expr(stmt::in_set(Profile::ID, self.items))
        }
    }
    impl super::Profile {
        pub fn find_by_user_id<'a>(
            user_id: impl stmt::IntoExpr<'a, Id<super::super::user::User>>,
        ) -> FindByUserId<'a> {
            FindByUserId {
                query: Query::from_stmt(stmt::Select::from_expr(Profile::USER_ID.eq(user_id))),
            }
        }
    }
    pub struct FindByUserId<'a> {
        query: Query<'a>,
    }
    impl<'a> FindByUserId<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Profile>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Profile>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Profile> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> FindByUserId<'a> {
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
            A: FromCursor<super::Profile>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindByUserId<'a> {
        type Model = super::Profile;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Profile {
        pub fn find_many_by_user_id<'a>() -> FindManyByUserId<'a> {
            FindManyByUserId { items: vec![] }
        }
    }
    pub struct FindManyByUserId<'a> {
        items: Vec<stmt::Expr<'a, Id<super::super::user::User>>>,
    }
    impl<'a> FindManyByUserId<'a> {
        pub fn item(
            mut self,
            user_id: impl stmt::IntoExpr<'a, Id<super::super::user::User>>,
        ) -> Self {
            self.items.push(user_id.into_expr());
            self
        }
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Profile>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Profile>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Profile> {
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
            A: FromCursor<super::Profile>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindManyByUserId<'a> {
        type Model = super::Profile;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            stmt::Select::from_expr(stmt::in_set(Profile::USER_ID, self.items))
        }
    }
}
