use crate::stmt::{Expr, ExprFunc};

/// The `LAST_INSERT_ID()` function expression (MySQL-specific).
///
/// Returns the first automatically generated value that was set for an AUTO_INCREMENT
/// column by the most recent INSERT statement. This is primarily used to retrieve
/// auto-increment IDs after insertion on MySQL, which doesn't support RETURNING clauses.
///
/// # Behavior
///
/// - Returns the first auto-increment ID from the most recent INSERT
/// - When multiple rows are inserted, returns the ID of the first row
/// - Subsequent row IDs can be calculated by adding row offsets (first_id + 1, first_id + 2, etc.)
/// - Returns 0 if no AUTO_INCREMENT value was generated
///
/// # MySQL Documentation
///
/// See: https://dev.mysql.com/doc/refman/8.0/en/information-functions.html#function_last-insert-id
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FuncLastInsertId;

impl Expr {
    pub fn last_insert_id() -> Self {
        FuncLastInsertId.into()
    }
}

impl From<FuncLastInsertId> for Expr {
    fn from(value: FuncLastInsertId) -> Self {
        Expr::Func(value.into())
    }
}

impl From<FuncLastInsertId> for ExprFunc {
    fn from(value: FuncLastInsertId) -> Self {
        ExprFunc::LastInsertId(value)
    }
}
