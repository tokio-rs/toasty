use toasty::codegen_support::*;
#[derive(Debug)]
pub struct Profile {
    pub id: Id<Profile>,
}
impl Profile {
    pub const ID: Path<Id<Profile>> = Path::from_field_index::<Self>(0);
    pub fn create() -> CreateProfile {
        CreateProfile::default()
    }
    pub fn create_many() -> CreateMany<Profile> {
        CreateMany::default()
    }
    pub fn filter(expr: stmt::Expr<bool>) -> Query {
        Query::from_stmt(stmt::Select::filter(expr))
    }
    pub fn update(&mut self) -> UpdateProfile<'_> {
        let query = UpdateQuery::from(self.into_select());
        UpdateProfile { model: self, query }
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        let stmt = self.into_select().delete();
        db.exec(stmt).await?;
        Ok(())
    }
}
impl Model for Profile {
    const ID: ModelId = ModelId(1);
    type Key = Id<Profile>;
    fn load(mut record: ValueRecord) -> Result<Self, Error> {
        Ok(Profile {
            id: Id::from_untyped(record[0].take().to_id()?),
        })
    }
}
impl stmt::IntoSelect for &Profile {
    type Model = Profile;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Profile::find_by_id(&self.id).into_select()
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
        Profile::find_by_id(self.id).into_select()
    }
}
impl stmt::IntoExpr<Profile> for Profile {
    fn into_expr(self) -> stmt::Expr<Profile> {
        todo!()
    }
}
impl stmt::IntoExpr<Profile> for &Profile {
    fn into_expr(self) -> stmt::Expr<Profile> {
        stmt::Key::from_expr(&self.id).into()
    }
}
impl stmt::IntoExpr<[Profile]> for &Profile {
    fn into_expr(self) -> stmt::Expr<[Profile]> {
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
    pub async fn all(self, db: &Db) -> Result<Cursor<Profile>> {
        db.all(self.stmt).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<Profile>> {
        db.first(self.stmt).await
    }
    pub async fn get(self, db: &Db) -> Result<Profile> {
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
        A: FromCursor<Profile>,
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
#[derive(Debug)]
pub struct CreateProfile {
    pub(super) stmt: stmt::Insert<Profile>,
}
impl CreateProfile {
    pub fn id(mut self, id: impl Into<Id<Profile>>) -> Self {
        self.stmt.set(0, id.into());
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
}
impl IntoExpr<[Profile]> for CreateProfile {
    fn into_expr(self) -> stmt::Expr<[Profile]> {
        self.stmt.into_list_expr()
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
    model: &'a mut Profile,
    query: UpdateQuery,
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
    pub async fn exec(self, db: &Db) -> Result<()> {
        let mut stmt = self.query.stmt;
        let mut result = db.exec_one(stmt.into()).await?;
        for (field, value) in result.into_sparse_record().into_iter() {
            match field {
                0 => self.model.id = stmt::Id::from_untyped(value.to_id()?),
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
pub mod fields {
    use super::*;
}
pub mod relation {
    use super::*;
    use toasty::Cursor;
}
pub mod queries {
    use super::*;
    impl super::Profile {
        pub fn find_by_id(id: impl stmt::IntoExpr<Id<Profile>>) -> FindById {
            FindById {
                query: Query::from_stmt(stmt::Select::filter(Profile::ID.eq(id))),
            }
        }
    }
    pub struct FindById {
        query: Query,
    }
    impl FindById {
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Profile>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Profile>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Profile> {
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
            A: FromCursor<super::Profile>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindById {
        type Model = super::Profile;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Profile {
        pub fn find_many_by_id() -> FindManyById {
            FindManyById { items: vec![] }
        }
    }
    pub struct FindManyById {
        items: Vec<stmt::Expr<Id<Profile>>>,
    }
    impl FindManyById {
        pub fn item(mut self, id: impl stmt::IntoExpr<Id<Profile>>) -> Self {
            self.items.push(id.into_expr());
            self
        }
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Profile>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Profile>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Profile> {
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
            A: FromCursor<super::Profile>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindManyById {
        type Model = super::Profile;
        fn into_select(self) -> stmt::Select<Self::Model> {
            stmt::Select::filter(stmt::in_set(Profile::ID, self.items))
        }
    }
}
