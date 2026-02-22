use super::{Name, Statement};

use toasty_core::schema::db::Index;

/// A statement to drop a SQL index.
#[derive(Debug, Clone)]
pub struct DropIndex {
    /// Name of the index.
    pub name: Name,

    /// Whether or not to add an `IF EXISTS` clause.
    pub if_exists: bool,
}

impl Statement {
    /// Drops an index.
    ///
    /// This function _does not_ add an `IF EXISTS` clause.
    pub fn drop_index(index: &Index) -> Self {
        DropIndex {
            name: Name::from(&index.name[..]),
            if_exists: false,
        }
        .into()
    }

    /// Drops a index if it exists.
    ///
    /// This function _does_ add an `IF EXISTS` clause.
    pub fn drop_index_if_exists(index: &Index) -> Self {
        DropIndex {
            name: Name::from(&index.name[..]),
            if_exists: true,
        }
        .into()
    }
}

impl From<DropIndex> for Statement {
    fn from(value: DropIndex) -> Self {
        Self::DropIndex(value)
    }
}
