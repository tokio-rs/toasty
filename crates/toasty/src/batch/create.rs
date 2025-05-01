use crate::*;

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
        self.stmts.push(item.into_insert());
        self
    }

    pub async fn exec(self, db: &Db) -> Result<Vec<M>> {
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

impl<M: Model> Default for CreateMany<M> {
    fn default() -> Self {
        Self { stmts: vec![] }
    }
}
