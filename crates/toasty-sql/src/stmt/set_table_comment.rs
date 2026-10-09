use super::{Name, Statement};

/// Sets or removes the native comment on a table.
#[derive(Debug, Clone)]
pub struct SetTableComment {
    /// Database table name.
    pub table: Name,

    /// New comment, or `None` to remove it.
    pub comment: Option<String>,
}

impl Statement {
    /// Sets or removes a table comment.
    pub fn set_table_comment(table: impl Into<Name>, comment: Option<&str>) -> Self {
        SetTableComment {
            table: table.into(),
            comment: comment.map(str::to_string),
        }
        .into()
    }
}

impl From<SetTableComment> for Statement {
    fn from(value: SetTableComment) -> Self {
        Self::SetTableComment(value)
    }
}
