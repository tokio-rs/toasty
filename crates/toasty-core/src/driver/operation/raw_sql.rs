use super::{Operation, TypedValue};

use crate::stmt;

/// Executes user-authored SQL against a SQL-capable driver.
///
/// Unlike [`QuerySql`](super::QuerySql), this operation carries backend SQL
/// text directly. The driver does not serialize a Toasty statement AST before
/// executing it.
///
/// The SQL text uses the placeholder syntax reported by
/// [`Capability::sql_placeholder`](super::super::Capability::sql_placeholder).
/// Parameters carry database storage types. The public raw SQL builders infer
/// these from bound values and driver capabilities unless the caller supplies
/// an explicit type.
#[derive(Debug, Clone)]
pub struct RawSql {
    /// Backend SQL text to execute.
    pub sql: String,

    /// Typed bind parameters in placeholder order.
    pub params: Vec<TypedValue>,

    /// How the driver should execute and decode the SQL.
    pub ret: RawSqlRet,
}

/// Return mode for a [`RawSql`] operation.
#[derive(Debug, Clone)]
pub enum RawSqlRet {
    /// Execute as a statement and return an affected-row count.
    None,

    /// Execute as a query and infer result value types from driver metadata.
    ///
    /// Drivers should map backend-native result metadata to the closest
    /// [`stmt::Value`](crate::stmt::Value) variant. Ambiguous backend values
    /// may decode to their storage representation.
    Infer,

    /// Execute as a query using explicit result column type hints.
    ///
    /// Type hints affect decoding only; drivers must execute the SQL text
    /// unchanged.
    Types(Vec<stmt::Type>),
}

impl Operation {
    /// Returns `true` if this is a [`RawSql`](Operation::RawSql) operation.
    pub fn is_raw_sql(&self) -> bool {
        matches!(self, Operation::RawSql(_))
    }
}

impl From<RawSql> for Operation {
    fn from(value: RawSql) -> Self {
        Self::RawSql(value)
    }
}
