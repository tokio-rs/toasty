use toasty::codegen_support::*;
#[derive(Debug)]
pub struct Package {
    user: BelongsTo<super::user::User>,
    pub user_id: Id<super::user::User>,
    pub id: Id<Package>,
    pub name: String,
}
impl Package {
    pub const USER: <super::user::User as Relation>::OneField =
        <super::user::User as Relation>::OneField::from_path(Path::from_field_index::<Self>(0));
    pub const USER_ID: Path<Id<super::user::User>> = Path::from_field_index::<Self>(1);
    pub const ID: Path<Id<Package>> = Path::from_field_index::<Self>(2);
    pub const NAME: Path<String> = Path::from_field_index::<Self>(3);
    pub fn user(&self) -> <super::user::User as Relation>::One {
        <super::user::User as Relation>::One::from_stmt(
            super::user::User::filter(super::user::User::ID.eq(&self.user_id)).into_select(),
        )
    }
    pub async fn get_by_user_id_and_id(
        db: &Db,
        user_id: impl IntoExpr<Id<super::user::User>>,
        id: impl IntoExpr<Id<Package>>,
    ) -> Result<Package> {
        Self::filter_by_user_id_and_id(user_id, id).get(db).await
    }
    pub fn filter_by_user_id_and_id(
        user_id: impl IntoExpr<Id<super::user::User>>,
        id: impl IntoExpr<Id<Package>>,
    ) -> Query {
        Query::default().filter_by_user_id_and_id(user_id, id)
    }
    pub fn filter_by_user_id_and_id_batch(
        keys: impl IntoExpr<[(Id<super::user::User>, Id<Package>)]>,
    ) -> Query {
        Query::default().filter_by_user_id_and_id_batch(keys)
    }
    pub async fn get_by_user_id(
        db: &Db,
        user_id: impl IntoExpr<Id<super::user::User>>,
    ) -> Result<Package> {
        Self::filter_by_user_id(user_id).get(db).await
    }
    pub fn filter_by_user_id(user_id: impl IntoExpr<Id<super::user::User>>) -> Query {
        Query::default().filter_by_user_id(user_id)
    }
    pub fn create() -> builders::CreatePackage {
        builders::CreatePackage::default()
    }
    pub fn create_many() -> CreateMany<Package> {
        CreateMany::default()
    }
    pub fn update(&mut self) -> builders::UpdatePackage<'_> {
        let query = builders::UpdateQuery::from(self.into_select());
        builders::UpdatePackage { model: self, query }
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
impl Relation for Package {
    type Many = relations::Many;
    type ManyField = relations::ManyField;
    type One = relations::One;
    type OneField = relations::OneField;
    type OptionOne = relations::OptionOne;
}
impl stmt::IntoSelect for &Package {
    type Model = Package;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Query::default()
            .filter_by_user_id_and_id(&self.user_id, &self.id)
            .stmt
    }
}
impl stmt::IntoSelect for &mut Package {
    type Model = Package;
    fn into_select(self) -> stmt::Select<Self::Model> {
        (&*self).into_select()
    }
}
impl stmt::IntoSelect for Package {
    type Model = Package;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Query::default()
            .filter_by_user_id_and_id(self.user_id, self.id)
            .stmt
    }
}
impl stmt::IntoExpr<Package> for Package {
    fn into_expr(self) -> stmt::Expr<Package> {
        (self.user_id, self.id).into_expr().cast()
    }
    fn by_ref(&self) -> stmt::Expr<Package> {
        (&self.user_id, &self.id).into_expr().cast()
    }
}
impl stmt::IntoExpr<[Package]> for Package {
    fn into_expr(self) -> stmt::Expr<[Package]> {
        stmt::Expr::list([self])
    }
    fn by_ref(&self) -> stmt::Expr<[Package]> {
        stmt::Expr::list([self])
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
    pub async fn get_by_user_id_and_id(
        self,
        db: &Db,
        user_id: impl IntoExpr<Id<super::user::User>>,
        id: impl IntoExpr<Id<Package>>,
    ) -> Result<Package> {
        self.filter_by_user_id_and_id(user_id, id).get(db).await
    }
    pub fn filter_by_user_id_and_id(
        self,
        user_id: impl IntoExpr<Id<super::user::User>>,
        id: impl IntoExpr<Id<Package>>,
    ) -> Query {
        self.filter(stmt::Expr::and_all([
            Package::USER_ID.eq(user_id),
            Package::ID.eq(id),
        ]))
    }
    pub fn filter_by_user_id_and_id_batch(
        self,
        keys: impl IntoExpr<[(Id<super::user::User>, Id<Package>)]>,
    ) -> Query {
        self.filter(stmt::Expr::in_list((Package::USER_ID, Package::ID), keys))
    }
    pub async fn get_by_user_id(
        self,
        db: &Db,
        user_id: impl IntoExpr<Id<super::user::User>>,
    ) -> Result<Package> {
        self.filter_by_user_id(user_id).get(db).await
    }
    pub fn filter_by_user_id(self, user_id: impl IntoExpr<Id<super::user::User>>) -> Query {
        self.filter(Package::USER_ID.eq(user_id))
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
    pub fn update(self) -> builders::UpdateQuery {
        builders::UpdateQuery::from(self)
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
pub mod builders {
    use super::*;
    #[derive(Debug)]
    pub struct CreatePackage {
        pub(super) stmt: stmt::Insert<Package>,
    }
    impl CreatePackage {
        pub fn user(mut self, user: impl IntoExpr<super::super::user::User>) -> Self {
            self.stmt.set(0, user.into_expr());
            self
        }
        pub fn user_id(mut self, user_id: impl Into<Id<super::super::user::User>>) -> Self {
            self.stmt.set(1, user_id.into());
            self
        }
        pub fn id(mut self, id: impl Into<Id<Package>>) -> Self {
            self.stmt.set(2, id.into());
            self
        }
        pub fn name(mut self, name: impl Into<String>) -> Self {
            self.stmt.set(3, name.into());
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
        fn by_ref(&self) -> stmt::Expr<Package> {
            todo!()
        }
    }
    impl IntoExpr<[Package]> for CreatePackage {
        fn into_expr(self) -> stmt::Expr<[Package]> {
            self.stmt.into_list_expr()
        }
        fn by_ref(&self) -> stmt::Expr<[Package]> {
            todo!()
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
        pub(super) model: &'a mut Package,
        pub(super) query: UpdateQuery,
    }
    #[derive(Debug)]
    pub struct UpdateQuery {
        stmt: stmt::Update<Package>,
    }
    impl UpdatePackage<'_> {
        pub fn user(mut self, user: impl IntoExpr<super::super::user::User>) -> Self {
            self.query.set_user(user);
            self
        }
        pub fn user_id(mut self, user_id: impl Into<Id<super::super::user::User>>) -> Self {
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
                match field {
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
        pub fn user(mut self, user: impl IntoExpr<super::super::user::User>) -> Self {
            self.set_user(user);
            self
        }
        pub fn set_user(&mut self, user: impl IntoExpr<super::super::user::User>) -> &mut Self {
            self.stmt.set(0, user.into_expr());
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
        pub fn id(mut self, id: impl Into<Id<Package>>) -> Self {
            self.set_id(id);
            self
        }
        pub fn set_id(&mut self, id: impl Into<Id<Package>>) -> &mut Self {
            self.stmt.set(2, id.into());
            self
        }
        pub fn name(mut self, name: impl Into<String>) -> Self {
            self.set_name(name);
            self
        }
        pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
            self.stmt.set(3, name.into());
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
    impl From<stmt::Select<Package>> for UpdateQuery {
        fn from(src: stmt::Select<Package>) -> UpdateQuery {
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
        stmt: stmt::Association<[Package]>,
    }
    #[derive(Debug)]
    pub struct One {
        stmt: stmt::Select<Package>,
    }
    #[derive(Debug)]
    pub struct OptionOne {
        stmt: stmt::Select<Package>,
    }
    pub struct ManyField {
        pub(super) path: Path<[super::Package]>,
    }
    pub struct OneField {
        pub(super) path: Path<super::Package>,
    }
    impl Many {
        pub fn from_stmt(stmt: stmt::Association<[Package]>) -> Many {
            Many { stmt }
        }
        pub async fn get_by_user_id_and_id(
            self,
            db: &Db,
            user_id: impl IntoExpr<Id<super::super::user::User>>,
            id: impl IntoExpr<Id<Package>>,
        ) -> Result<Package> {
            self.filter_by_user_id_and_id(user_id, id).get(db).await
        }
        pub fn filter_by_user_id_and_id(
            self,
            user_id: impl IntoExpr<Id<super::super::user::User>>,
            id: impl IntoExpr<Id<Package>>,
        ) -> Query {
            Query::from_stmt(self.into_select()).filter(stmt::Expr::and_all([
                Package::USER_ID.eq(user_id),
                Package::ID.eq(id),
            ]))
        }
        pub fn filter_by_user_id_and_id_batch(
            self,
            keys: impl IntoExpr<[(Id<super::super::user::User>, Id<Package>)]>,
        ) -> Query {
            Query::from_stmt(self.into_select()).filter_by_user_id_and_id_batch(keys)
        }
        pub async fn get_by_id(self, db: &Db, id: impl IntoExpr<Id<Package>>) -> Result<Package> {
            self.filter_by_id(id).get(db).await
        }
        pub fn filter_by_id(self, id: impl IntoExpr<Id<Package>>) -> Query {
            Query::from_stmt(self.into_select()).filter(Package::ID.eq(id))
        }
        pub async fn get_by_user_id(
            self,
            db: &Db,
            user_id: impl IntoExpr<Id<super::super::user::User>>,
        ) -> Result<Package> {
            self.filter_by_user_id(user_id).get(db).await
        }
        pub fn filter_by_user_id(
            self,
            user_id: impl IntoExpr<Id<super::super::user::User>>,
        ) -> Query {
            Query::from_stmt(self.into_select()).filter(Package::USER_ID.eq(user_id))
        }
        #[doc = r" Iterate all entries in the relation"]
        pub async fn all(self, db: &Db) -> Result<Cursor<Package>> {
            db.all(self.stmt.into_select()).await
        }
        pub async fn collect<A>(self, db: &Db) -> Result<A>
        where
            A: FromCursor<Package>,
        {
            self.all(db).await?.collect().await
        }
        pub fn query(self, filter: stmt::Expr<bool>) -> super::Query {
            let query = self.into_select();
            super::Query::from_stmt(query.and(filter))
        }
        pub fn create(self) -> builders::CreatePackage {
            let mut builder = builders::CreatePackage::default();
            builder.stmt.set_scope(self.stmt.into_select());
            builder
        }
        #[doc = r" Remove items from the association"]
        pub async fn remove(self, db: &Db, item: impl IntoExpr<Package>) -> Result<()> {
            let stmt = self.stmt.remove(item);
            db.exec(stmt).await?;
            Ok(())
        }
    }
    impl stmt::IntoSelect for Many {
        type Model = Package;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.stmt.into_select()
        }
    }
    impl One {
        pub fn from_stmt(stmt: stmt::Select<Package>) -> One {
            One { stmt }
        }
        pub async fn get(self, db: &Db) -> Result<Package> {
            db.get(self.stmt.into_select()).await
        }
    }
    impl stmt::IntoSelect for One {
        type Model = Package;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.stmt.into_select()
        }
    }
    impl OptionOne {
        pub fn from_stmt(stmt: stmt::Select<Package>) -> OptionOne {
            OptionOne { stmt }
        }
        pub async fn get(self, db: &Db) -> Result<Option<Package>> {
            db.first(self.stmt.into_select()).await
        }
    }
    impl ManyField {
        pub const fn from_path(path: Path<[super::Package]>) -> ManyField {
            ManyField { path }
        }
    }
    impl Into<Path<[Package]>> for ManyField {
        fn into(self) -> Path<[Package]> {
            self.path
        }
    }
    impl OneField {
        pub const fn from_path(path: Path<super::Package>) -> OneField {
            OneField { path }
        }
        pub fn in_query<Q>(self, rhs: Q) -> toasty::stmt::Expr<bool>
        where
            Q: stmt::IntoSelect<Model = super::Package>,
        {
            self.path.in_query(rhs)
        }
    }
    impl Into<Path<Package>> for OneField {
        fn into(self) -> Path<Package> {
            self.path
        }
    }
}
