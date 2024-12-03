use toasty::codegen_support::*;
#[derive(Debug)]
pub struct Package {
    user: BelongsTo<super::user::User>,
    pub user_id: Id<super::user::User>,
    pub id: Id<Package>,
    pub name: String,
}
impl Package {
    pub const USER: self::fields::User =
        self::fields::User::from_path(Path::from_field_index::<Self>(0));
    pub const USER_ID: Path<Id<super::user::User>> = Path::from_field_index::<Self>(1);
    pub const ID: Path<Id<Package>> = Path::from_field_index::<Self>(2);
    pub const NAME: Path<String> = Path::from_field_index::<Self>(3);
    pub fn create() -> CreatePackage {
        CreatePackage::default()
    }
    pub fn create_many() -> CreateMany<Package> {
        CreateMany::default()
    }
    pub fn filter(expr: stmt::Expr<bool>) -> Query {
        Query::from_stmt(stmt::Select::from_expr(expr))
    }
    pub fn update(&mut self) -> UpdatePackage<'_> {
        let query = UpdateQuery::from(self.into_select());
        UpdatePackage { model: self, query }
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        let stmt = self.into_select().delete();
        db.exec(stmt).await?;
        Ok(())
    }
}
impl Model for Package {
    const ID: ModelId = ModelId(1);
    type Key = (Id<super::user::User>, Id<Package>);
    fn load(mut record: ValueRecord) -> Result<Self, Error> {
        Ok(Package {
            user: BelongsTo::load(record[0].take())?,
            user_id: Id::from_untyped(record[1].take().to_id()?),
            id: Id::from_untyped(record[2].take().to_id()?),
            name: record[3].take().to_string()?,
        })
    }
}
impl stmt::IntoSelect for &Package {
    type Model = Package;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Package::find_by_user_id_and_id(&self.user_id, &self.id).into_select()
    }
}
impl stmt::IntoSelect for &mut Package {
    type Model = Package;
    fn into_select(self) -> stmt::Select<Self::Model> {
        (&*self).into_select()
    }
}
impl stmt::AsSelect for Package {
    type Model = Package;
    fn as_select(&self) -> stmt::Select<Self::Model> {
        Package::find_by_user_id_and_id(&self.user_id, &self.id).into_select()
    }
}
impl stmt::IntoSelect for Package {
    type Model = Package;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Package::find_by_user_id_and_id(self.user_id, self.id).into_select()
    }
}
impl stmt::IntoExpr<Package> for &Package {
    fn into_expr(self) -> stmt::Expr<Package> {
        stmt::Key::from_expr((&self.user_id, &self.id)).into()
    }
}
impl stmt::IntoExpr<[Package]> for &Package {
    fn into_expr(self) -> stmt::Expr<[Package]> {
        stmt::Key::from_expr((&self.user_id, &self.id)).into()
    }
}
#[derive(Debug)]
pub struct Query {
    stmt: stmt::Select<Package>,
}
impl Query {
    pub const fn from_stmt(stmt: stmt::Select<Package>) -> Query {
        Query { stmt }
    }
    pub async fn all(self, db: &Db) -> Result<Cursor<Package>> {
        db.all(self.stmt).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<Package>> {
        db.first(self.stmt).await
    }
    pub async fn get(self, db: &Db) -> Result<Package> {
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
        A: FromCursor<Package>,
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
    type Model = Package;
    fn into_select(self) -> stmt::Select<Package> {
        self.stmt
    }
}
impl stmt::IntoSelect for &Query {
    type Model = Package;
    fn into_select(self) -> stmt::Select<Package> {
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
pub struct CreatePackage {
    pub(super) stmt: stmt::Insert<Package>,
}
impl CreatePackage {
    pub fn user<'b>(mut self, user: impl IntoExpr<self::relation::User<'b>>) -> Self {
        self.stmt.set_expr(0, user.into_expr());
        self
    }
    pub fn user_id(mut self, user_id: impl Into<Id<super::user::User>>) -> Self {
        self.stmt.set_value(1, user_id.into());
        self
    }
    pub fn id(mut self, id: impl Into<Id<Package>>) -> Self {
        self.stmt.set_value(2, id.into());
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.stmt.set_value(3, name.into());
        self
    }
    pub async fn exec(self, db: &Db) -> Result<Package> {
        db.exec_insert_one(self.stmt).await
    }
}
impl IntoInsert for CreatePackage {
    type Model = Package;
    fn into_insert(self) -> stmt::Insert<Package> {
        self.stmt
    }
}
impl IntoExpr<Package> for CreatePackage {
    fn into_expr(self) -> stmt::Expr<Package> {
        self.stmt.into()
    }
}
impl IntoExpr<[Package]> for CreatePackage {
    fn into_expr(self) -> stmt::Expr<[Package]> {
        self.stmt.into_list_expr()
    }
}
impl Default for CreatePackage {
    fn default() -> CreatePackage {
        CreatePackage {
            stmt: stmt::Insert::blank(),
        }
    }
}
#[derive(Debug)]
pub struct UpdatePackage<'a> {
    model: &'a mut Package,
    query: UpdateQuery,
}
#[derive(Debug)]
pub struct UpdateQuery {
    stmt: stmt::Update<Package>,
}
impl UpdatePackage<'_> {
    pub fn user<'b>(mut self, user: impl IntoExpr<self::relation::User<'b>>) -> Self {
        self.query.set_user(user);
        self
    }
    pub fn user_id(mut self, user_id: impl Into<Id<super::user::User>>) -> Self {
        self.query.set_user_id(user_id);
        self
    }
    pub fn id(mut self, id: impl Into<Id<Package>>) -> Self {
        self.query.set_id(id);
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.query.set_name(name);
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let mut stmt = self.query.stmt;
        let mut result = db.exec_one(stmt.into()).await?;
        for (field, value) in result.into_sparse_record().into_iter() {
            match field.into_usize() {
                0 => todo!("should not be set; {} = {value:#?}", 0),
                1 => self.model.user_id = stmt::Id::from_untyped(value.to_id()?),
                2 => self.model.id = stmt::Id::from_untyped(value.to_id()?),
                3 => self.model.name = value.to_string()?,
                _ => todo!("handle unknown field id in reload after update"),
            }
        }
        Ok(())
    }
}
impl UpdateQuery {
    pub fn user<'b>(mut self, user: impl IntoExpr<self::relation::User<'b>>) -> Self {
        self.set_user(user);
        self
    }
    pub fn set_user<'b>(&mut self, user: impl IntoExpr<self::relation::User<'b>>) -> &mut Self {
        self.stmt.set_expr(0, user.into_expr());
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
    pub fn id(mut self, id: impl Into<Id<Package>>) -> Self {
        self.set_id(id);
        self
    }
    pub fn set_id(&mut self, id: impl Into<Id<Package>>) -> &mut Self {
        self.stmt.set_expr(2, id.into());
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.set_name(name);
        self
    }
    pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.stmt.set_expr(3, name.into());
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
impl From<stmt::Select<Package>> for UpdateQuery {
    fn from(src: stmt::Select<Package>) -> UpdateQuery {
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
        pub fn packages(mut self) -> super::super::user::fields::Packages {
            let path = self.path.chain(super::super::user::User::PACKAGES);
            super::super::user::fields::Packages::from_path(path)
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
            scope: &'a Package,
        }
        impl super::Package {
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
    impl super::Package {
        pub fn find_by_user_id(
            user_id: impl stmt::IntoExpr<Id<super::super::user::User>>,
        ) -> FindByUserId {
            FindByUserId {
                query: Query::from_stmt(stmt::Select::from_expr(Package::USER_ID.eq(user_id))),
            }
        }
    }
    pub struct FindByUserId {
        query: Query,
    }
    impl FindByUserId {
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Package>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Package>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Package> {
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
            A: FromCursor<super::Package>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindByUserId {
        type Model = super::Package;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Package {
        pub fn find_by_user_id_and_id(
            user_id: impl stmt::IntoExpr<Id<super::super::user::User>>,
            id: impl stmt::IntoExpr<Id<Package>>,
        ) -> FindByUserIdAndId {
            FindByUserIdAndId {
                query: Query::from_stmt(stmt::Select::from_expr(
                    Package::USER_ID.eq(user_id).and(Package::ID.eq(id)),
                )),
            }
        }
    }
    pub struct FindByUserIdAndId {
        query: Query,
    }
    impl FindByUserIdAndId {
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Package>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Package>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Package> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> FindByUserIdAndId {
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
            A: FromCursor<super::Package>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindByUserIdAndId {
        type Model = super::Package;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Package {
        pub fn find_many_by_user_id_and_id() -> FindManyByUserIdAndId {
            FindManyByUserIdAndId { items: vec![] }
        }
    }
    pub struct FindManyByUserIdAndId {
        items: Vec<stmt::Expr<(Id<super::super::user::User>, Id<Package>)>>,
    }
    impl FindManyByUserIdAndId {
        pub fn item(
            mut self,
            user_id: impl stmt::IntoExpr<Id<super::super::user::User>>,
            id: impl stmt::IntoExpr<Id<Package>>,
        ) -> Self {
            self.items
                .push((user_id.into_expr(), id.into_expr()).into_expr());
            self
        }
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Package>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Package>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Package> {
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
            A: FromCursor<super::Package>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindManyByUserIdAndId {
        type Model = super::Package;
        fn into_select(self) -> stmt::Select<Self::Model> {
            stmt::Select::from_expr(stmt::in_set((Package::USER_ID, Package::ID), self.items))
        }
    }
}
