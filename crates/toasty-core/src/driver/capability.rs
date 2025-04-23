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

    /// Supports row-level locking. If false, then the driver is expected to
    /// serializable transaction-level isolation.
    pub select_for_update: bool,
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

    pub fn select_for_update(&self) -> bool {
        if let Capability::Sql(cap) = self {
            cap.select_for_update
        } else {
            false
        }
    }
}
