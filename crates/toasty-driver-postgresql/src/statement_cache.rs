use std::{borrow::Cow, collections::HashMap};

use postgres::{Error, Statement};
use postgres_types::Type;
use tokio_postgres::Client;

#[derive(Debug, Clone)]
pub struct StatementCache {
    map: HashMap<Key<'static>, Statement>,
}

impl StatementCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get(&self, query: &str, types: &[Type]) -> Option<Statement> {
        self.map.get(&Key::new(query, types)).map(ToOwned::to_owned)
    }

    pub fn insert(&mut self, query: &str, types: &[Type], statement: Statement) {
        self.map
            .insert(Key::new(query, types).into_owned(), statement);
    }

    pub async fn prepare_typed(
        &mut self,
        client: &mut Client,
        query: &str,
        types: &[Type],
    ) -> Result<Statement, Error> {
        if let Some(statement) = self.get(query, types) {
            Ok(statement)
        } else {
            let stmt = client.prepare_typed(query, types).await?;
            self.insert(query, types, stmt.clone());
            Ok(stmt)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key<'a> {
    query: Cow<'a, str>,
    types: Cow<'a, [Type]>,
}

impl<'a> Key<'a> {
    #[must_use]
    pub fn new(query: &'a str, types: &'a [Type]) -> Self {
        Self {
            query: query.into(),
            types: types.into(),
        }
    }

    pub fn into_owned(self) -> Key<'static> {
        Key::<'static> {
            query: self.query.into_owned().into(),
            types: self.types.into_owned().into(),
        }
    }
}
