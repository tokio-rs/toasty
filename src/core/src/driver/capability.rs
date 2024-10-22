#[derive(Debug)]
pub enum Capability {
    Sql,
    KeyValue(KeyValue),
}

#[derive(Debug)]
pub struct KeyValue {
    /// DynamoDB does not support != predicates on the primary key.
    pub primary_key_ne_predicate: bool,
}

impl Capability {
    pub fn is_sql(&self) -> bool {
        matches!(self, Capability::Sql)
    }
}
