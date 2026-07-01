use super::{Name, Statement};

use toasty_core::schema::db::{Column, Table};

/// A database-native table or column comment statement.
#[derive(Debug, Clone)]
pub struct CommentOn {
    /// The schema object receiving the comment.
    pub target: CommentTarget,
    /// The comment text, or `None` to clear the comment where supported.
    pub comment: Option<String>,
}

/// The schema object receiving a comment.
#[derive(Debug, Clone)]
pub enum CommentTarget {
    /// A table comment.
    Table(Name),
    /// A column comment.
    Column {
        /// The table containing the column.
        table: Name,
        /// The column receiving the comment.
        column: Name,
    },
}

impl Statement {
    /// Creates a table comment statement from a table schema entry.
    pub fn comment_on_table(table: &Table) -> Self {
        CommentOn {
            target: CommentTarget::Table(Name::from(table.name.as_str())),
            comment: table.comment.clone(),
        }
        .into()
    }

    /// Creates a column comment statement from table and column schema entries.
    pub fn comment_on_column(table: &Table, column: &Column) -> Self {
        CommentOn {
            target: CommentTarget::Column {
                table: Name::from(table.name.as_str()),
                column: Name::from(column.name.as_str()),
            },
            comment: column.comment.clone(),
        }
        .into()
    }
}

impl From<CommentOn> for Statement {
    fn from(value: CommentOn) -> Self {
        Self::CommentOn(value)
    }
}
