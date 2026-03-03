use crate::{
    stmt::{self, IntoInsert},
    Cursor, Db, Model, Result,
};
use toasty_core::stmt as core_stmt;

pub struct CreateMany<M: Model> {
    /// The builder holds an `Insert` statement which can create multiple
    /// records for the same model.
    stmts: Vec<stmt::Insert<M>>,
}

impl<M: Model> CreateMany<M> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn item(mut self, item: impl stmt::IntoInsert<Model = M>) -> Self {
        let stmt = item.into_insert();
        assert!(
            stmt.untyped.source.single,
            "BUG: insert statement should have `single` flag set"
        );
        self.stmts.push(stmt);
        self
    }

    /// Closure-based variant of `item`: builds a single record using the model's
    /// create builder. `f` receives a default create builder and must return it
    /// after setting the desired fields.
    pub fn with_item(mut self, f: impl FnOnce(M::Create) -> M::Create) -> Self
    where
        M::Create: stmt::IntoInsert<Model = M>,
    {
        let create = f(M::Create::default());
        let stmt = create.into_insert();
        assert!(
            stmt.untyped.source.single,
            "BUG: insert statement should have `single` flag set"
        );
        self.stmts.push(stmt);
        self
    }

    /// Convert the collected inserts into a list expression suitable for
    /// embedding in a parent insert statement (e.g., as a nested HasMany value).
    ///
    /// Unlike `exec`, this does not run any database query.
    pub fn into_expr(self) -> stmt::Expr<[M]> {
        if self.stmts.is_empty() {
            return stmt::Expr::from_untyped(core_stmt::Expr::list(
                std::iter::empty::<core_stmt::Expr>(),
            ));
        }
        let mut stmts = self.stmts.into_iter();
        let mut merged = stmts.next().unwrap();
        for stmt in stmts {
            merged.merge(stmt);
        }
        // Clear the single flag so the engine handles multi-row inserts correctly.
        merged.untyped.source.single = false;
        merged.into_list_expr()
    }

    pub async fn exec(self, db: &mut Db) -> Result<Vec<M>> {
        // If there are no records to create, then return an empty vec
        if self.stmts.is_empty() {
            return Ok(vec![]);
        }

        // TODO: improve
        let mut stmts = self.stmts.into_iter();
        let mut merged = stmts.next().unwrap();

        for stmt in stmts {
            merged.merge(stmt);
        }

        merged.untyped.source.single = false;

        let records = db.exec(merged.into()).await?;
        let cursor = Cursor::new(db.schema().clone(), records);
        cursor.collect().await
    }
}

impl<M: Model> Default for CreateMany<M> {
    fn default() -> Self {
        Self { stmts: vec![] }
    }
}
