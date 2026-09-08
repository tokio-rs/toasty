use toasty_core::stmt;

use crate::engine::mir;

/// A constant list of values.
///
/// Used to inject static data into the operation graph, such as literal values
/// from the query or pre-computed results.
#[derive(Debug)]
pub(crate) struct Const {
    /// The constant rows.
    pub(crate) value: stmt::Value,

    /// The type of this constant.
    pub(crate) ty: stmt::Type,
}

impl From<Const> for mir::Node {
    fn from(value: Const) -> Self {
        mir::Operation::Const(value).into()
    }
}
