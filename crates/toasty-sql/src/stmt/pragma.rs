use super::Statement;

/// A SQLite PRAGMA statement.
#[derive(Debug, Clone)]
pub struct Pragma {
    /// The pragma name (e.g. "foreign_keys").
    pub name: String,

    /// The value to set, if any. When `None`, this is a query pragma.
    pub value: Option<String>,
}

impl Statement {
    /// Sets `PRAGMA foreign_keys = ON`.
    pub fn pragma_enable_foreign_keys() -> Self {
        Pragma {
            name: "foreign_keys".to_string(),
            value: Some("ON".to_string()),
        }
        .into()
    }

    /// Sets `PRAGMA foreign_keys = OFF`.
    pub fn pragma_disable_foreign_keys() -> Self {
        Pragma {
            name: "foreign_keys".to_string(),
            value: Some("OFF".to_string()),
        }
        .into()
    }

    /// Creates a PRAGMA statement with the given name and value.
    pub fn pragma(name: impl Into<String>, value: impl Into<String>) -> Self {
        Pragma {
            name: name.into(),
            value: Some(value.into()),
        }
        .into()
    }
}

impl From<Pragma> for Statement {
    fn from(value: Pragma) -> Self {
        Self::Pragma(value)
    }
}
