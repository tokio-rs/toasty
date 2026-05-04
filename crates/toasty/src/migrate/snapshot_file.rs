use super::{err, err_ctx};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use toasty_core::schema::db::Schema;
use toml_edit::{DocumentMut, Item};

const SNAPSHOT_FILE_VERSION: u32 = 1;

/// Serializable snapshot of the database schema at the moment a migration was
/// generated.
///
/// Each call to [`generate`](super::generate) writes a snapshot alongside the
/// SQL migration. The next call loads the most recent snapshot to compute the
/// diff against the current schema, producing the next migration.
///
/// The file carries a version number; loading rejects files written with an
/// incompatible version.
///
/// # Examples
///
/// ```
/// use toasty::migrate::SnapshotFile;
/// use toasty::schema::db::Schema;
///
/// let snapshot = SnapshotFile::new(Schema::default());
/// assert!(snapshot.schema.tables.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFile {
    /// Snapshot file format version.
    version: u32,

    /// Captured database schema.
    pub schema: Schema,
}

impl SnapshotFile {
    /// Creates a new snapshot wrapping the given schema.
    pub fn new(schema: Schema) -> Self {
        Self {
            version: SNAPSHOT_FILE_VERSION,
            schema,
        }
    }

    /// Loads a snapshot file from `path`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|e| err_ctx(format!("reading {}", path.display()), e))?;
        contents.parse()
    }

    /// Saves the snapshot to `path`, overwriting any existing file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        std::fs::write(path, self.to_string())
            .map_err(|e| err_ctx(format!("writing {}", path.display()), e))?;
        Ok(())
    }
}

impl FromStr for SnapshotFile {
    type Err = toasty_core::Error;

    fn from_str(s: &str) -> Result<Self> {
        let file: SnapshotFile =
            toml::from_str(s).map_err(|e| err_ctx("parsing schema snapshot", e))?;

        if file.version != SNAPSHOT_FILE_VERSION {
            return Err(err(format!(
                "unsupported snapshot file version: {}. Expected version {}",
                file.version, SNAPSHOT_FILE_VERSION
            )));
        }

        Ok(file)
    }
}

impl fmt::Display for SnapshotFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let doc = self.to_toml_document().map_err(|_| fmt::Error)?;
        write!(f, "{}", doc)
    }
}

impl SnapshotFile {
    /// Renders the snapshot as a `toml_edit::DocumentMut`, flattening
    /// nested arrays of tables into the multi-line TOML form so the file is
    /// easier to read and review.
    fn to_toml_document(&self) -> Result<DocumentMut> {
        let mut doc = toml_edit::ser::to_document(self)
            .map_err(|e| err_ctx("serializing schema snapshot", e))?;
        for (_key, item) in doc.as_table_mut().iter_mut() {
            if item.is_inline_table() {
                let mut placeholder = Item::None;
                std::mem::swap(item, &mut placeholder);
                let mut table = placeholder.into_table().unwrap();

                for (_key, item) in table.iter_mut() {
                    if item.is_array() {
                        let mut placeholder = Item::None;
                        std::mem::swap(item, &mut placeholder);
                        let mut array = placeholder.into_array_of_tables().unwrap();

                        for table in array.iter_mut() {
                            for (_key, item) in table.iter_mut() {
                                if item.is_array() {
                                    let mut placeholder = Item::None;
                                    std::mem::swap(item, &mut placeholder);
                                    let array = placeholder.into_array_of_tables().unwrap();
                                    *item = array.into();
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
