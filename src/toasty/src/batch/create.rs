use crate::*;

pub struct CreateMany<'stmt, M: Model> {
    /// The builder holds an `Insert` statement which can create multiple
    /// records for the same model.
    stmts: Vec<stmt::Insert<'stmt, M>>,
}

impl<'stmt, M: Model> CreateMany<'stmt, M> {
    pub fn new() -> CreateMany<'stmt, M> {
        CreateMany::default()
    }

    pub fn item(mut self, item: impl stmt::IntoInsert<'stmt, Model = M>) -> Self {
        self.stmts.push(item.into_insert());
        self
    }

    pub async fn exec(self, db: &'stmt Db) -> Result<Vec<M>> {
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

        let records = db.exec(merged.into()).await?;
        let cursor = Cursor::new(db.schema.clone(), records);
        cursor.collect().await
    }
}

impl<'a, M: Model> Default for CreateMany<'a, M> {
    fn default() -> Self {
        CreateMany { stmts: vec![] }
    }
}
