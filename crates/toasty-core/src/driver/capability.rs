#[derive(Debug)]
pub struct Capability {
    /// When true, the database uses a SQL-based query language.
    pub sql: bool,

    /// SQL: supports update statements in CTE queries.
    pub cte_with_update: bool,

    /// SQL: Supports row-level locking. If false, then the driver is expected
    /// to serializable transaction-level isolation.
    pub select_for_update: bool,

    /// DynamoDB does not support != predicates on the primary key.
    pub primary_key_ne_predicate: bool,
}

impl Capability {
    /// SQLite capabilities.
    pub const SQLITE: Capability = Capability {
        sql: true,
        cte_with_update: false,
        select_for_update: false,
        primary_key_ne_predicate: true,
    };

    /// PostgreSQL capabilities
    pub const POSTGRESQL: Capability = Capability {
        cte_with_update: true,
        select_for_update: true,
        ..Capability::SQLITE
    };

    /// MySQL capabilities
    pub const MYSQL: Capability = Capability {
        cte_with_update: false,
        select_for_update: true,
        ..Capability::SQLITE
    };

    /// DynamoDB capabilities
    pub const DYNAMODB: Capability = Capability {
        sql: false,
        cte_with_update: false,
        select_for_update: false,
        primary_key_ne_predicate: false,
    };
}
