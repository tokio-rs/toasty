use std::borrow::Cow;

use lru::LruCache;
use tokio_postgres::Client;
use tokio_postgres::{Error, Statement};

#[derive(Debug, Clone)]
pub struct StatementCache {
    inner: LruCache<Key<'static>, Statement>,
}

impl StatementCache {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: LruCache::new(capacity.try_into().unwrap()),
        }
    }

    pub fn get(&mut self, query: &str) -> Option<Statement> {
        self.inner
            .get(&Key::new(query).into_owned())
            .map(ToOwned::to_owned)
    }

    pub fn insert(&mut self, query: &str, statement: Statement) {
        self.inner.put(Key::new(query).into_owned(), statement);
    }

    pub async fn prepare(&mut self, client: &mut Client, query: &str) -> Result<Statement, Error> {
        if let Some(statement) = self.get(query) {
            Ok(statement)
        } else {
            let stmt = client.prepare(query).await?;
            self.insert(query, stmt.clone());
            Ok(stmt)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key<'a> {
    query: Cow<'a, str>,
}

impl<'a> Key<'a> {
    #[must_use]
    pub fn new(query: &'a str) -> Self {
        Self {
            query: query.into(),
        }
    }

    pub fn into_owned(self) -> Key<'static> {
        Key::<'static> {
            query: self.query.into_owned().into(),
        }
    }
}
