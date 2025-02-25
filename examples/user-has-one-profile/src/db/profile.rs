use toasty::codegen_support::*;
#[derive(Debug)]
pub struct Profile {
    pub id: Id<Profile>,
    pub user: BelongsTo<super::user::User>,
    pub user_id: Option<Id<super::user::User>>,
}
impl Profile {
    pub const ID: Path<Id<Profile>> = Path::from_field_index::<Self>(0);
    pub const USER: <super::user::User as Relation>::OneField =
        <super::user::User as Relation>::OneField::from_path(Path::from_field_index::<Self>(1));
    pub const USER_ID: Path<Id<super::user::User>> = Path::from_field_index::<Self>(2);
    pub fn user(&self) -> <Option<super::user::User> as Relation>::One {
        <Option<super::user::User> as Relation>::One::from_stmt(
            super::user::User::filter(super::user::User::ID.eq(&self.user_id)).into_select(),
        )
    }
    pub async fn get_by_id(db: &Db, id: impl IntoExpr<Id<Profile>>) -> Result<Profile> {
        Self::filter_by_id(id).get(db).await
    }
    pub fn filter_by_id(id: impl IntoExpr<Id<Profile>>) -> Query {
        Query::default().filter_by_id(id)
    }
    pub fn filter_by_id_batch(keys: impl IntoExpr<[Id<Profile>]>) -> Query {
        Query::default().filter_by_id_batch(keys)
    }
    pub async fn get_by_user_id(
        db: &Db,
        user_id: impl IntoExpr<Id<super::user::User>>,
    ) -> Result<Profile> {
        Self::filter_by_user_id(user_id).get(db).await
    }
    pub fn filter_by_user_id(user_id: impl IntoExpr<Id<super::user::User>>) -> Query {
        Query::default().filter_by_user_id(user_id)
    }
    pub fn create() -> builders::CreateProfile {
        builders::CreateProfile::default()
    }
    pub fn create_many() -> CreateMany<Profile> {
        CreateMany::default()
    }
    pub fn update(&mut self) -> builders::UpdateProfile<'_> {
        let query = builders::UpdateQuery::from(self.into_select());
        builders::UpdateProfile { model: self, query }
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
impl Model for Profile {
    const ID: ModelId = ModelId(1);
    fn load(mut record: ValueRecord) -> Result<Self, Error> {
        Ok(Profile {
            id: Id::from_untyped(record[0].take().to_id()?),
            user: BelongsTo::load(record[1].take())?,
            user_id: record[2].take().to_option_id()?.map(Id::from_untyped),
        })
    }
}
impl Relation for Profile {
    type Query = Query;
    type Many = relations::Many;
    type ManyField = relations::ManyField;
    type One = relations::One;
    type OneField = relations::OneField;
    type OptionOne = relations::OptionOne;
}
impl stmt::IntoSelect for &Profile {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Query::default().filter_by_id(&self.id).stmt
    }
}
impl stmt::IntoSelect for &mut Profile {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<Self::Model> {
        (&*self).into_select()
    }
}
impl stmt::IntoSelect for Profile {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Query::default().filter_by_id(self.id).stmt
    }
}
impl stmt::IntoExpr<Profile> for Profile {
    fn into_expr(self) -> stmt::Expr<Profile> {
        let expr: stmt::Expr<Id<Profile>> = self.id.into_expr();
        expr.cast()
    }
    fn by_ref(&self) -> stmt::Expr<Profile> {
        let expr: stmt::Expr<Id<Profile>> = (&self.id).into_expr();
        expr.cast()
    }
}
impl stmt::IntoExpr<[Profile]> for Profile {
    fn into_expr(self) -> stmt::Expr<[Profile]> {
        stmt::Expr::list([self])
    }
    fn by_ref(&self) -> stmt::Expr<[Profile]> {
        stmt::Expr::list([self])
    }
}
#[derive(Debug)]
pub struct Query {
    stmt: stmt::Select<Profile>,
}
impl Query {
    pub const fn from_stmt(stmt: stmt::Select<Profile>) -> Query {
        Query { stmt }
    }
    pub async fn get_by_id(self, db: &Db, id: impl IntoExpr<Id<Profile>>) -> Result<Profile> {
        self.filter_by_id(id).get(db).await
    }
    pub fn filter_by_id(self, id: impl IntoExpr<Id<Profile>>) -> Query {
        self.filter(Profile::ID.eq(id))
    }
    pub fn filter_by_id_batch(self, keys: impl IntoExpr<[Id<Profile>]>) -> Query {
        self.filter(stmt::Expr::in_list(Profile::ID, keys))
    }
    pub async fn get_by_user_id(
        self,
        db: &Db,
        user_id: impl IntoExpr<Id<super::user::User>>,
    ) -> Result<Profile> {
        self.filter_by_user_id(user_id).get(db).await
    }
    pub fn filter_by_user_id(self, user_id: impl IntoExpr<Id<super::user::User>>) -> Query {
        self.filter(Profile::USER_ID.eq(user_id))
    }
    pub async fn all(self, db: &Db) -> Result<Cursor<Profile>> {
        db.all(self.stmt).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<Profile>> {
        db.first(self.stmt).await
    }
    pub async fn get(self, db: &Db) -> Result<Profile> {
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
        A: FromCursor<Profile>,
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
    pub fn user(mut self) -> <super::user::User as Relation>::Query {
        <super::user::User as Relation>::Query::from_stmt(
            stmt::Association::many_via_one(self.stmt, Profile::USER.into()).into_select(),
        )
    }
}
impl stmt::IntoSelect for Query {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<Profile> {
        self.stmt
    }
}
impl stmt::IntoSelect for &Query {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<Profile> {
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
    pub struct CreateProfile {
        pub(super) stmt: stmt::Insert<Profile>,
    }
    impl CreateProfile {
        pub fn id(mut self, id: impl Into<Id<Profile>>) -> Self {
            self.stmt.set(0, id.into());
            self
        }
        pub fn user(mut self, user: impl IntoExpr<super::super::user::User>) -> Self {
            self.stmt.set(1, user.into_expr());
            self
        }
        pub fn user_id(mut self, user_id: impl Into<Id<super::super::user::User>>) -> Self {
            self.stmt.set(2, user_id.into());
            self
        }
        pub async fn exec(self, db: &Db) -> Result<Profile> {
            db.exec_insert_one(self.stmt).await
        }
    }
    impl IntoInsert for CreateProfile {
        type Model = Profile;
        fn into_insert(self) -> stmt::Insert<Profile> {
            self.stmt
        }
    }
    impl IntoExpr<Profile> for CreateProfile {
        fn into_expr(self) -> stmt::Expr<Profile> {
            self.stmt.into()
        }
        fn by_ref(&self) -> stmt::Expr<Profile> {
            todo!()
        }
    }
    impl IntoExpr<[Profile]> for CreateProfile {
        fn into_expr(self) -> stmt::Expr<[Profile]> {
            self.stmt.into_list_expr()
        }
        fn by_ref(&self) -> stmt::Expr<[Profile]> {
            todo!()
        }
    }
    impl Default for CreateProfile {
        fn default() -> CreateProfile {
            CreateProfile {
                stmt: stmt::Insert::blank(),
            }
        }
    }
    #[derive(Debug)]
    pub struct UpdateProfile<'a> {
        pub(super) model: &'a mut Profile,
        pub(super) query: UpdateQuery,
    }
    #[derive(Debug)]
    pub struct UpdateQuery {
        stmt: stmt::Update<Profile>,
    }
    impl UpdateProfile<'_> {
        pub fn id(mut self, id: impl Into<Id<Profile>>) -> Self {
            self.query.set_id(id);
            self
        }
        pub fn user(mut self, user: impl IntoExpr<super::super::user::User>) -> Self {
            self.query.set_user(user);
            self
        }
        pub fn unset_user(&mut self) -> &mut Self {
            self.query.unset_user();
            self
        }
        pub fn user_id(mut self, user_id: impl Into<Id<super::super::user::User>>) -> Self {
            self.query.set_user_id(user_id);
            self
        }
        pub fn unset_user_id(&mut self) -> &mut Self {
            self.query.unset_user_id();
            self
        }
        pub async fn exec(self, db: &Db) -> Result<()> {
            let mut stmt = self.query.stmt;
            let mut result = db.exec_one(stmt.into()).await?;
            for (field, value) in result.into_sparse_record().into_iter() {
                match field {
                    0 => self.model.id = stmt::Id::from_untyped(value.to_id()?),
                    1 => todo!("should not be set; {} = {value:#?}", 1),
                    2 => self.model.user_id = value.to_option_id()?.map(stmt::Id::from_untyped),
                    _ => todo!("handle unknown field id in reload after update"),
                }
            }
            Ok(())
        }
    }
    impl UpdateQuery {
        pub fn id(mut self, id: impl Into<Id<Profile>>) -> Self {
            self.set_id(id);
            self
        }
        pub fn set_id(&mut self, id: impl Into<Id<Profile>>) -> &mut Self {
            self.stmt.set(0, id.into());
            self
        }
        pub fn user(mut self, user: impl IntoExpr<super::super::user::User>) -> Self {
            self.set_user(user);
            self
        }
        pub fn set_user(&mut self, user: impl IntoExpr<super::super::user::User>) -> &mut Self {
            self.stmt.set(1, user.into_expr());
            self
        }
        pub fn unset_user(&mut self) -> &mut Self {
            self.stmt.set(1, Value::Null);
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
            self.stmt.set(2, user_id.into());
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
    impl From<Query> for UpdateQuery {
        fn from(value: Query) -> UpdateQuery {
            UpdateQuery {
                stmt: stmt::Update::new(value.stmt),
            }
        }
    }
    impl From<stmt::Select<Profile>> for UpdateQuery {
        fn from(src: stmt::Select<Profile>) -> UpdateQuery {
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
        stmt: stmt::Association<[Profile]>,
    }
    #[derive(Debug)]
    pub struct One {
        stmt: stmt::Select<Profile>,
    }
    #[derive(Debug)]
    pub struct OptionOne {
        stmt: stmt::Select<Profile>,
    }
    pub struct ManyField {
        pub(super) path: Path<[super::Profile]>,
    }
    pub struct OneField {
        pub(super) path: Path<super::Profile>,
    }
    impl Many {
        pub fn from_stmt(stmt: stmt::Association<[Profile]>) -> Many {
            Many { stmt }
        }
        pub async fn get_by_id(self, db: &Db, id: impl IntoExpr<Id<Profile>>) -> Result<Profile> {
            self.filter_by_id(id).get(db).await
        }
        pub fn filter_by_id(self, id: impl IntoExpr<Id<Profile>>) -> Query {
            Query::from_stmt(self.into_select()).filter(Profile::ID.eq(id))
        }
        pub fn filter_by_id_batch(self, keys: impl IntoExpr<[Id<Profile>]>) -> Query {
            Query::from_stmt(self.into_select()).filter_by_id_batch(keys)
        }
        pub async fn get_by_user_id(
            self,
            db: &Db,
            user_id: impl IntoExpr<Id<super::super::user::User>>,
        ) -> Result<Profile> {
            self.filter_by_user_id(user_id).get(db).await
        }
        pub fn filter_by_user_id(
            self,
            user_id: impl IntoExpr<Id<super::super::user::User>>,
        ) -> Query {
            Query::from_stmt(self.into_select()).filter(Profile::USER_ID.eq(user_id))
        }
        #[doc = r" Iterate all entries in the relation"]
        pub async fn all(self, db: &Db) -> Result<Cursor<Profile>> {
            db.all(self.stmt.into_select()).await
        }
        pub async fn collect<A>(self, db: &Db) -> Result<A>
        where
            A: FromCursor<Profile>,
        {
            self.all(db).await?.collect().await
        }
        pub fn query(self, filter: stmt::Expr<bool>) -> super::Query {
            let query = self.into_select();
            super::Query::from_stmt(query.and(filter))
        }
        pub fn create(self) -> builders::CreateProfile {
            let mut builder = builders::CreateProfile::default();
            builder.stmt.set_scope(self.stmt.into_select());
            builder
        }
        #[doc = r" Add an item to the association"]
        pub async fn insert(self, db: &Db, item: impl IntoExpr<[Profile]>) -> Result<()> {
            let stmt = self.stmt.insert(item);
            db.exec(stmt).await?;
            Ok(())
        }
        #[doc = r" Remove items from the association"]
        pub async fn remove(self, db: &Db, item: impl IntoExpr<Profile>) -> Result<()> {
            let stmt = self.stmt.remove(item);
            db.exec(stmt).await?;
            Ok(())
        }
    }
    impl stmt::IntoSelect for Many {
        type Model = Profile;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.stmt.into_select()
        }
    }
    impl One {
        pub fn from_stmt(stmt: stmt::Select<Profile>) -> One {
            One { stmt }
        }
        #[doc = r" Create a new associated record"]
        pub fn create(self) -> builders::CreateProfile {
            let mut builder = builders::CreateProfile::default();
            builder.stmt.set_scope(self.stmt.into_select());
            builder
        }
        pub async fn get(self, db: &Db) -> Result<Profile> {
            db.get(self.stmt.into_select()).await
        }
    }
    impl stmt::IntoSelect for One {
        type Model = Profile;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.stmt.into_select()
        }
    }
    impl OptionOne {
        pub fn from_stmt(stmt: stmt::Select<Profile>) -> OptionOne {
            OptionOne { stmt }
        }
        #[doc = r" Create a new associated record"]
        pub fn create(self) -> builders::CreateProfile {
            let mut builder = builders::CreateProfile::default();
            builder.stmt.set_scope(self.stmt.into_select());
            builder
        }
        pub async fn get(self, db: &Db) -> Result<Option<Profile>> {
            db.first(self.stmt.into_select()).await
        }
    }
    impl ManyField {
        pub const fn from_path(path: Path<[super::Profile]>) -> ManyField {
            ManyField { path }
        }
    }
    impl Into<Path<[Profile]>> for ManyField {
        fn into(self) -> Path<[Profile]> {
            self.path
        }
    }
    impl OneField {
        pub const fn from_path(path: Path<super::Profile>) -> OneField {
            OneField { path }
        }
        pub fn eq<T>(self, rhs: T) -> stmt::Expr<bool>
        where
            T: IntoExpr<super::Profile>,
        {
            self.path.eq(rhs.into_expr())
        }
        pub fn in_query<Q>(self, rhs: Q) -> toasty::stmt::Expr<bool>
        where
            Q: stmt::IntoSelect<Model = super::Profile>,
        {
            self.path.in_query(rhs)
        }
    }
    impl Into<Path<Profile>> for OneField {
        fn into(self) -> Path<Profile> {
            self.path
        }
    }
}
