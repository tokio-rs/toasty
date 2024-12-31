use toasty::codegen_support::*;
#[derive(Debug)]
pub struct Person {
    pub id: Id<Person>,
    pub name: String,
    pub parent_id: Option<Id<Person>>,
    parent: Box<BelongsTo<Person>>,
}
impl Person {
    pub const ID: Path<Id<Person>> = Path::from_field_index::<Self>(0);
    pub const NAME: Path<String> = Path::from_field_index::<Self>(1);
    pub const PARENT_ID: Path<Id<Person>> = Path::from_field_index::<Self>(2);
    pub const PARENT: self::fields::Parent =
        self::fields::Parent::from_path(Path::from_field_index::<Self>(3));
    pub fn create() -> CreatePerson {
        CreatePerson::default()
    }
    pub fn create_many() -> CreateMany<Person> {
        CreateMany::default()
    }
    pub fn filter(expr: stmt::Expr<bool>) -> Query {
        Query::from_stmt(stmt::Select::filter(expr))
    }
    pub fn update(&mut self) -> UpdatePerson<'_> {
        let query = UpdateQuery::from(self.into_select());
        UpdatePerson { model: self, query }
    }
    pub async fn delete(self, db: &Db) -> Result<()> {
        let stmt = self.into_select().delete();
        db.exec(stmt).await?;
        Ok(())
    }
}
impl Model for Person {
    const ID: ModelId = ModelId(0);
    type Key = Id<Person>;
    fn load(mut record: ValueRecord) -> Result<Self, Error> {
        Ok(Person {
            id: Id::from_untyped(record[0].take().to_id()?),
            name: record[1].take().to_string()?,
            parent_id: record[2].take().to_option_id()?.map(Id::from_untyped),
            parent: Box::new(BelongsTo::load(record[3].take())?),
        })
    }
}
impl stmt::IntoSelect for &Person {
    type Model = Person;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Person::find_by_id(&self.id).into_select()
    }
}
impl stmt::IntoSelect for &mut Person {
    type Model = Person;
    fn into_select(self) -> stmt::Select<Self::Model> {
        (&*self).into_select()
    }
}
impl stmt::IntoSelect for Person {
    type Model = Person;
    fn into_select(self) -> stmt::Select<Self::Model> {
        Person::find_by_id(self.id).into_select()
    }
}
impl stmt::IntoExpr<Person> for Person {
    fn into_expr(self) -> stmt::Expr<Person> {
        todo!()
    }
}
impl stmt::IntoExpr<Person> for &Person {
    fn into_expr(self) -> stmt::Expr<Person> {
        stmt::Key::from_expr(&self.id).into()
    }
}
impl stmt::IntoExpr<[Person]> for &Person {
    fn into_expr(self) -> stmt::Expr<[Person]> {
        stmt::Expr::list([self])
    }
}
#[derive(Debug)]
pub struct Query {
    stmt: stmt::Select<Person>,
}
impl Query {
    pub const fn from_stmt(stmt: stmt::Select<Person>) -> Query {
        Query { stmt }
    }
    pub async fn all(self, db: &Db) -> Result<Cursor<Person>> {
        db.all(self.stmt).await
    }
    pub async fn first(self, db: &Db) -> Result<Option<Person>> {
        db.first(self.stmt).await
    }
    pub async fn get(self, db: &Db) -> Result<Person> {
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
        A: FromCursor<Person>,
    {
        self.all(db).await?.collect().await
    }
    pub fn filter(self, expr: stmt::Expr<bool>) -> Query {
        Query {
            stmt: self.stmt.and(expr),
        }
    }
    pub fn parent(mut self) -> self::Query {
        todo!()
    }
}
impl stmt::IntoSelect for Query {
    type Model = Person;
    fn into_select(self) -> stmt::Select<Person> {
        self.stmt
    }
}
impl stmt::IntoSelect for &Query {
    type Model = Person;
    fn into_select(self) -> stmt::Select<Person> {
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
pub struct CreatePerson {
    pub(super) stmt: stmt::Insert<Person>,
}
impl CreatePerson {
    pub fn id(mut self, id: impl Into<Id<Person>>) -> Self {
        self.stmt.set(0, id.into());
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.stmt.set(1, name.into());
        self
    }
    pub fn parent_id(mut self, parent_id: impl Into<Id<Person>>) -> Self {
        self.stmt.set(2, parent_id.into());
        self
    }
    pub fn parent<'b>(mut self, parent: impl IntoExpr<self::relation::Parent<'b>>) -> Self {
        self.stmt.set(3, parent.into_expr());
        self
    }
    pub async fn exec(self, db: &Db) -> Result<Person> {
        db.exec_insert_one(self.stmt).await
    }
}
impl IntoInsert for CreatePerson {
    type Model = Person;
    fn into_insert(self) -> stmt::Insert<Person> {
        self.stmt
    }
}
impl IntoExpr<Person> for CreatePerson {
    fn into_expr(self) -> stmt::Expr<Person> {
        self.stmt.into()
    }
}
impl IntoExpr<[Person]> for CreatePerson {
    fn into_expr(self) -> stmt::Expr<[Person]> {
        self.stmt.into_list_expr()
    }
}
impl Default for CreatePerson {
    fn default() -> CreatePerson {
        CreatePerson {
            stmt: stmt::Insert::blank(),
        }
    }
}
#[derive(Debug)]
pub struct UpdatePerson<'a> {
    model: &'a mut Person,
    query: UpdateQuery,
}
#[derive(Debug)]
pub struct UpdateQuery {
    stmt: stmt::Update<Person>,
}
impl UpdatePerson<'_> {
    pub fn id(mut self, id: impl Into<Id<Person>>) -> Self {
        self.query.set_id(id);
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.query.set_name(name);
        self
    }
    pub fn parent_id(mut self, parent_id: impl Into<Id<Person>>) -> Self {
        self.query.set_parent_id(parent_id);
        self
    }
    pub fn unset_parent_id(&mut self) -> &mut Self {
        self.query.unset_parent_id();
        self
    }
    pub fn parent<'b>(mut self, parent: impl IntoExpr<self::relation::Parent<'b>>) -> Self {
        self.query.set_parent(parent);
        self
    }
    pub fn unset_parent(&mut self) -> &mut Self {
        self.query.unset_parent();
        self
    }
    pub async fn exec(self, db: &Db) -> Result<()> {
        let mut stmt = self.query.stmt;
        let mut result = db.exec_one(stmt.into()).await?;
        for (field, value) in result.into_sparse_record().into_iter() {
            match field {
                0 => self.model.id = stmt::Id::from_untyped(value.to_id()?),
                1 => self.model.name = value.to_string()?,
                2 => self.model.parent_id = value.to_option_id()?.map(stmt::Id::from_untyped),
                3 => todo!("should not be set; {} = {value:#?}", 3),
                _ => todo!("handle unknown field id in reload after update"),
            }
        }
        Ok(())
    }
}
impl UpdateQuery {
    pub fn id(mut self, id: impl Into<Id<Person>>) -> Self {
        self.set_id(id);
        self
    }
    pub fn set_id(&mut self, id: impl Into<Id<Person>>) -> &mut Self {
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
    pub fn parent_id(mut self, parent_id: impl Into<Id<Person>>) -> Self {
        self.set_parent_id(parent_id);
        self
    }
    pub fn set_parent_id(&mut self, parent_id: impl Into<Id<Person>>) -> &mut Self {
        self.stmt.set(2, parent_id.into());
        self
    }
    pub fn unset_parent_id(&mut self) -> &mut Self {
        self.stmt.set(2, Value::Null);
        self
    }
    pub fn parent<'b>(mut self, parent: impl IntoExpr<self::relation::Parent<'b>>) -> Self {
        self.set_parent(parent);
        self
    }
    pub fn set_parent<'b>(
        &mut self,
        parent: impl IntoExpr<self::relation::Parent<'b>>,
    ) -> &mut Self {
        self.stmt.set(3, parent.into_expr());
        self
    }
    pub fn unset_parent(&mut self) -> &mut Self {
        self.stmt.set(3, Value::Null);
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
impl From<stmt::Select<Person>> for UpdateQuery {
    fn from(src: stmt::Select<Person>) -> UpdateQuery {
        UpdateQuery {
            stmt: stmt::Update::new(src),
        }
    }
}
pub mod fields {
    use super::*;
    pub struct Parent {
        pub(super) path: Path<Person>,
    }
    impl Parent {
        pub const fn from_path(path: Path<Person>) -> Parent {
            Parent { path }
        }
        pub fn id(mut self) -> Path<Id<Person>> {
            self.path.chain(Person::ID)
        }
        pub fn name(mut self) -> Path<String> {
            self.path.chain(Person::NAME)
        }
        pub fn parent_id(mut self) -> Path<Id<Person>> {
            self.path.chain(Person::PARENT_ID)
        }
        pub fn parent(mut self) -> Parent {
            let path = self.path.chain(Person::PARENT);
            Parent::from_path(path)
        }
        pub fn eq<'a, T>(self, rhs: T) -> stmt::Expr<bool>
        where
            T: toasty::stmt::IntoExpr<super::relation::parent::Parent<'a>>,
        {
            self.path.eq(rhs.into_expr().cast())
        }
        pub fn in_query<Q>(self, rhs: Q) -> toasty::stmt::Expr<bool>
        where
            Q: stmt::IntoSelect<Model = Person>,
        {
            self.path.in_query(rhs)
        }
    }
    impl From<Parent> for Path<Person> {
        fn from(val: Parent) -> Path<Person> {
            val.path
        }
    }
    impl<'a> stmt::IntoExpr<super::relation::parent::Parent<'a>> for Parent {
        fn into_expr(self) -> stmt::Expr<super::relation::parent::Parent<'a>> {
            todo!("into_expr for {} (field path struct)", stringify!(Parent));
        }
    }
}
pub mod relation {
    use super::*;
    use toasty::Cursor;
    pub mod parent {
        use super::*;
        #[derive(Debug)]
        pub struct Parent<'a> {
            scope: &'a Person,
        }
        impl super::Person {
            pub fn parent(&self) -> Parent<'_> {
                Parent { scope: self }
            }
        }
        impl Parent<'_> {
            pub fn get(&self) -> &Person {
                self.scope.parent.get()
            }
        }
        impl stmt::IntoSelect for &Parent<'_> {
            type Model = Person;
            fn into_select(self) -> stmt::Select<Self::Model> {
                Person::find_by_id(
                    self.scope
                        .parent_id
                        .as_ref()
                        .expect("TODO: handle null fk fields"),
                )
                .into_select()
            }
        }
        impl<'a> stmt::IntoExpr<Parent<'a>> for Parent<'a> {
            fn into_expr(self) -> stmt::Expr<Parent<'a>> {
                todo!(
                    "stmt::IntoExpr for {} (belongs_to Fk struct) - self = {:#?}",
                    stringify!(Parent),
                    self
                );
            }
        }
        impl<'a> stmt::IntoExpr<Parent<'a>> for &Parent<'a> {
            fn into_expr(self) -> stmt::Expr<Parent<'a>> {
                todo!(
                    "stmt::IntoExpr for &'a {} (belongs_to Fk struct) - self = {:#?}",
                    stringify!(Parent),
                    self
                );
            }
        }
        impl<'a> stmt::IntoExpr<Parent<'a>> for &Person {
            fn into_expr(self) -> stmt::Expr<Parent<'a>> {
                stmt::Expr::from_untyped(&self.id)
            }
        }
        impl<'a> stmt::IntoExpr<Parent<'a>> for CreatePerson {
            fn into_expr(self) -> stmt::Expr<Parent<'a>> {
                let expr: stmt::Expr<Person> = self.stmt.into();
                expr.cast()
            }
        }
        impl Parent<'_> {
            pub async fn find(&self, db: &Db) -> Result<Option<Person>> {
                db.first(self.into_select()).await
            }
        }
    }
    pub use parent::Parent;
}
pub mod queries {
    use super::*;
    impl super::Person {
        pub fn find_by_id(id: impl stmt::IntoExpr<Id<Person>>) -> FindById {
            FindById {
                query: Query::from_stmt(stmt::Select::filter(Person::ID.eq(id))),
            }
        }
    }
    pub struct FindById {
        query: Query,
    }
    impl FindById {
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Person>> {
            self.query.all(db).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Person>> {
            self.query.first(db).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Person> {
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
            A: FromCursor<super::Person>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindById {
        type Model = super::Person;
        fn into_select(self) -> stmt::Select<Self::Model> {
            self.query.into_select()
        }
    }
    impl super::Person {
        pub fn find_many_by_id() -> FindManyById {
            FindManyById { items: vec![] }
        }
    }
    pub struct FindManyById {
        items: Vec<stmt::Expr<Id<Person>>>,
    }
    impl FindManyById {
        pub fn item(mut self, id: impl stmt::IntoExpr<Id<Person>>) -> Self {
            self.items.push(id.into_expr());
            self
        }
        pub async fn all(self, db: &Db) -> Result<Cursor<super::Person>> {
            db.all(self.into_select()).await
        }
        pub async fn first(self, db: &Db) -> Result<Option<super::Person>> {
            db.first(self.into_select()).await
        }
        pub async fn get(self, db: &Db) -> Result<super::Person> {
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
            A: FromCursor<super::Person>,
        {
            self.all(db).await?.collect().await
        }
    }
    impl stmt::IntoSelect for FindManyById {
        type Model = super::Person;
        fn into_select(self) -> stmt::Select<Self::Model> {
            stmt::Select::filter(stmt::in_set(Person::ID, self.items))
        }
    }
}
