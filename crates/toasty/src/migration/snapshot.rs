use crate::{Error, Result, schema::db::Schema};
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path, str::FromStr};
use toml_edit::{DocumentMut, Item};

const SNAPSHOT_VERSION: u32 = 1;

/// A TOML-serializable snapshot of a database schema.
///
/// Snapshots capture the schema after a generated migration. The next
/// generation compares the most recent snapshot against the current schema to
/// build a diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Snapshot format version.
    version: u32,

    /// The database schema captured by this snapshot.
    pub schema: Schema,
}

impl Snapshot {
    /// Create a new snapshot for `schema`.
    pub fn new(schema: Schema) -> Self {
        Self {
            version: SNAPSHOT_VERSION,
            schema,
        }
    }

    /// Load a snapshot from a TOML file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        contents.parse()
    }

    /// Serialize the snapshot as TOML.
    pub fn to_toml_string(&self) -> Result<String> {
        Ok(self.to_toml_document()?.to_string())
    }

    /// Save the snapshot to a TOML file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path.as_ref(), self.to_toml_string()?)?;
        Ok(())
    }

    fn to_toml_document(&self) -> Result<DocumentMut> {
        let mut doc = toml_edit::ser::to_document(self)
            .map_err(|err| Error::from_args(format_args!("{err}")))?;

        for (_key, item) in doc.as_table_mut().iter_mut() {
            if item.is_inline_table() {
                let mut placeholder = Item::None;
                std::mem::swap(item, &mut placeholder);
                let mut table = match placeholder.into_table() {
                    Ok(table) => table,
                    Err(original) => {
                        *item = original;
                        continue;
                    }
                };

                for (_key, item) in table.iter_mut() {
                    if item.is_array() {
                        let mut placeholder = Item::None;
                        std::mem::swap(item, &mut placeholder);
                        let mut array = match placeholder.into_array_of_tables() {
                            Ok(array) => array,
                            Err(original) => {
                                *item = original;
                                continue;
                            }
                        };

                        for table in array.iter_mut() {
                            for (_key, item) in table.iter_mut() {
                                if item.is_array() {
                                    let mut placeholder = Item::None;
                                    std::mem::swap(item, &mut placeholder);
                                    match placeholder.into_array_of_tables() {
                                        Ok(array) => *item = array.into(),
                                        Err(original) => *item = original,
                                    }
                                }
                            }
                        }

                        *item = array.into();
                    }
                }

                *item = table.into();
            }
        }

        Ok(doc)
    }
}

impl FromStr for Snapshot {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let snapshot: Snapshot =
            toml::from_str(s).map_err(|err| Error::from_args(format_args!("{err}")))?;

        if snapshot.version != SNAPSHOT_VERSION {
            return Err(Error::from_args(format_args!(
                "unsupported snapshot version: {}. Expected version {}",
                snapshot.version, SNAPSHOT_VERSION
            )));
        }

        Ok(snapshot)
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let doc = self.to_toml_document().map_err(|_| fmt::Error)?;
        write!(f, "{doc}")
    }
}
