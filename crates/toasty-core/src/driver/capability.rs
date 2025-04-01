#[derive(Debug)]
pub enum Capability {
    KeyValue(CapabilityKeyValue),
    Sql(CapabilitySql),
}

#[derive(Debug)]
pub struct CapabilityKeyValue {
    /// DynamoDB does not support != predicates on the primary key.
    pub primary_key_ne_predicate: bool,
}

#[derive(Debug)]
pub struct CapabilitySql {
    /// Supports update statements in CTE queries.
    pub cte_with_update: bool,
}

impl Capability {
    pub fn is_sql(&self) -> bool {
        matches!(self, Capability::Sql(..))
    }

    pub fn cte_with_update(&self) -> bool {
        if let Capability::Sql(cap) = self {
            cap.cte_with_update
        } else {
            false
        }
    }
}
