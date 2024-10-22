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
    pub fn create<'a>() -> CreatePackage<'a> {
        CreatePackage::default()
    }
    pub fn create_many<'a>() -> CreateMany<'a, Package> {
        CreateMany::default()
    }
    pub fn filter<'a>(expr: stmt::Expr<'a, bool>) -> Query<'a> {
        Query::from_stmt(stmt::Select::from_expr(expr))
    }
    pub fn update<'a>(&'a mut self) -> UpdatePackage<'a> {
        UpdatePackage {
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
impl Model for Package {
    const ID: ModelId = ModelId(1);
    const FIELD_COUNT: usize = 4;
    type Key = (Id<super::user::User>, Id<Package>);
    fn load(mut record: Record<'_>) -> Result<Self, Error> {
        Ok(Package {
            user: BelongsTo::load(record[0].take())?,
            user_id: Id::from_untyped(record[1].take().to_id()?),
            id: Id::from_untyped(record[2].take().to_id()?),
            name: record[3].take().to_string()?,
        })
    }
}
impl<'a> stmt::IntoSelect<'a> for &'a Package {
    type Model = Package;
    fn into_select(self) -> stmt::Select<'a, Self::Model> {
        Package::find_by_user_id_and_id(&self.user_id, &self.id).into_select()
    }
}
impl stmt::AsSelect for Package {
    type Model = Package;
    fn as_select(&self) -> stmt::Select<'_, Self::Model> {
        Package::find_by_user_id_and_id(&self.user_id, &self.id).into_select()
    }
}
impl stmt::IntoSelect<'static> for Package {
    type Model = Package;
    fn into_select(self) -> stmt::Select<'static, Self::Model> {
        Package::find_by_user_id_and_id(self.user_id, self.id).into_select()
    }
}
impl<'a> stmt::IntoExpr<'a, Package> for &'a Package {
    fn into_expr(self) -> stmt::Expr<'a, Package> {
        stmt::Key::from_expr((&self.user_id, &self.id)).into()
    }
}
impl<'a> stmt::IntoExpr<'a, [Package]> for &'a Package {
    fn into_expr(self) -> stmt::Expr<'a, [Package]> {
        stmt::Key::from_expr((&self.user_id, &self.id)).into()
    }
}
#[derive(Debug)]
pub struct Query<'a> {
    stmt: stmt::Select<'a, Package>,
}
impl<'a> Query<'a> {
    pub const fn from_stmt(stmt: stmt::Select<'a, Package>) -> Query<'a> {
        Query { stmt }
    }
    pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, Package>> {
        db.all(self).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<Package>> {
        db.first(self).await
    }
    pub async fn get(self, db: &Db) -> Result<Package> {
        db.get(self).await
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
        A: FromCursor<Package>,
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
    type Model = Package;
    fn into_select(self) -> stmt::Select<'a, Package> {
        self.stmt
    }
}
impl<'a> stmt::IntoSelect<'a> for &Query<'a> {
    type Model = Package;
    fn into_select(self) -> stmt::Select<'a, Package> {
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
pub struct CreatePackage<'a> {
    pub(super) stmt: stmt::Insert<'a, Package>,
}
impl<'a> CreatePackage<'a> {
    pub fn user<'b>(mut self, user: impl IntoExpr<'a, self::relation::User<'b>>) -> Self {
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
    pub async fn exec(self, db: &'a Db) -> Result<Package> {
        db.exec_insert_one::<Package>(self.stmt).await
    }
}
impl<'a> IntoInsert<'a> for CreatePackage<'a> {
    type Model = Package;
    fn into_insert(self) -> stmt::Insert<'a, Package> {
        self.stmt
    }
}
impl<'a> IntoExpr<'a, Package> for CreatePackage<'a> {
    fn into_expr(self) -> stmt::Expr<'a, Package> {
        self.stmt.into()
    }
}
impl<'a> IntoExpr<'a, [Package]> for CreatePackage<'a> {
    fn into_expr(self) -> stmt::Expr<'a, [Package]> {
        self.stmt.into_list_expr()
    }
}
impl<'a> Default for CreatePackage<'a> {
    fn default() -> CreatePackage<'a> {
        CreatePackage {
            stmt: stmt::Insert::blank(),
        }
    }
}
#[derive(Debug)]
pub struct UpdatePackage<'a> {
    model: &'a mut Package,
    query: UpdateQuery<'a>,
}
#[derive(Debug)]
pub struct UpdateQuery<'a> {
    stmt: stmt::Update<'a, Package>,
}
impl<'a> UpdatePackage<'a> {
    pub fn user<'b>(mut self, user: impl IntoExpr<'a, self::relation::User<'b>>) -> Self {
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
        let fields;
        let mut into_iter;
        {
            let mut stmt = self.query.stmt;
            fields = stmt.fields().clone();
            stmt.set_selection(&*self.model);
            let mut records = db.exec::<Package>(stmt.into()).await?;
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
                0 => {
                    self.model.user_id = stmt::Id::from_untyped(into_iter.next().unwrap().to_id()?)
                }
                1 => {
                    self.model.user_id = stmt::Id::from_untyped(into_iter.next().unwrap().to_id()?)
                }
                2 => self.model.id = stmt::Id::from_untyped(into_iter.next().unwrap().to_id()?),
                3 => self.model.name = into_iter.next().unwrap().to_string()?,
                _ => todo!("handle unknown field id in reload after update"),
            }
        }
        Ok(())
    }
}
impl<'a> UpdateQuery<'a> {
    pub fn user<'b>(mut self, user: impl IntoExpr<'a, self::relation::User<'b>>) -> Self {
        self.set_user(user);
        self
    }
    pub fn set_user<'b>(&mut self, user: impl IntoExpr<'a, self::relation::User<'b>>) -> &mut Self {
        self.stmt.set_expr(0, user.into_expr());
        self
    }
    pub fn user_id(mut self, user_id: impl Into<Id<super::user::User>>) -> Self {
        self.set_user_id(user_id);
        self
    }
    pub fn set_user_id(&mut self, user_id: impl Into<Id<super::user::User>>) -> &mut Self {
        self.stmt.set_expr(1, user_id.into().into_expr());
        self
    }
    pub fn id(mut self, id: impl Into<Id<Package>>) -> Self {
        self.set_id(id);
        self
    }
    pub fn set_id(&mut self, id: impl Into<Id<Package>>) -> &mut Self {
        self.stmt.set_expr(2, id.into().into_expr());
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.set_name(name);
        self
    }
    pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.stmt.set_expr(3, name.into().into_expr());
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
impl<'a> From<stmt::Select<'a, Package>> for UpdateQuery<'a> {
    fn from(src: stmt::Select<'a, Package>) -> UpdateQuery<'a> {
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
            scope: &'a Package,
        }
        impl super::Package {
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
                super::super::super::user::User::find_by_id(&self.scope.user_id).into_select()
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
            pub async fn find<'db>(&self, db: &'db Db) -> Result<super::super::super::user::User> {
                db.get(self).await
            }
        }
    }
    pub use user::User;
}
pub mod queries {
    use super::*;
    impl super::Package {
        pub fn find_by_user_id<'a>(
            user_id: impl stmt::IntoExpr<'a, Id<super::super::user::User>>,
        ) -> FindByUserId<'a> {
            FindByUserId {
                query: Query::from_stmt(stmt::Select::from_expr(Package::USER_ID.eq(user_id))),
            }
        }
    }
    pub struct FindByUserId<'a> {
        query: Query<'a>,
    }
    impl<'a> FindByUserId<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Package>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Package>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Package> {
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
            A: FromCursor<super::Package>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindByUserId<'a> {
        type Model = super::Package;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Package {
        pub fn find_by_user_id_and_id<'a>(
            user_id: impl stmt::IntoExpr<'a, Id<super::super::user::User>>,
            id: impl stmt::IntoExpr<'a, Id<Package>>,
        ) -> FindByUserIdAndId<'a> {
            FindByUserIdAndId {
                query: Query::from_stmt(stmt::Select::from_expr(
                    Package::USER_ID.eq(user_id).and(Package::ID.eq(id)),
                )),
            }
        }
    }
    pub struct FindByUserIdAndId<'a> {
        query: Query<'a>,
    }
    impl<'a> FindByUserIdAndId<'a> {
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Package>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Package>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Package> {
            self.query.get(db).await
        }
        pub fn update(self) -> super::UpdateQuery<'a> {
            super::UpdateQuery::from(self.query)
        }
        pub async fn delete(self, db: &Db) -> Result<()> {
            self.query.delete(db).await
        }
        pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> FindByUserIdAndId<'a> {
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
            A: FromCursor<super::Package>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindByUserIdAndId<'a> {
        type Model = super::Package;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Package {
        pub fn find_many_by_user_id_and_id<'a>() -> FindManyByUserIdAndId<'a> {
            FindManyByUserIdAndId { items: vec![] }
        }
    }
    pub struct FindManyByUserIdAndId<'a> {
        items: Vec<stmt::Expr<'a, (Id<super::super::user::User>, Id<Package>)>>,
    }
    impl<'a> FindManyByUserIdAndId<'a> {
        pub fn item(
            mut self,
            user_id: impl stmt::IntoExpr<'a, Id<super::super::user::User>>,
            id: impl stmt::IntoExpr<'a, Id<Package>>,
        ) -> Self {
            self.items
                .push((user_id.into_expr(), id.into_expr()).into_expr());
            self
        }
        pub async fn all(self, db: &'a Db) -> Result<Cursor<'a, super::Package>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Package>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Package> {
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
            A: FromCursor<super::Package>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl<'a> stmt::IntoSelect<'a> for FindManyByUserIdAndId<'a> {
        type Model = super::Package;
        fn into_select(self) -> stmt::Select<'a, Self::Model> {
            stmt::Select::from_expr(stmt::in_set((Package::USER_ID, Package::ID), self.items))
        }
    }
}
