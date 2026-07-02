#![warn(missing_docs)]

use tokio::sync::Mutex;
use turso::sync::Database;

use crate::{TursoCommon, TursoConfig, TursoPath};

pub use turso::sync::Builder;

///
pub struct TursoSync {
    config: TursoConfig,
    database: Mutex<Option<Database>>,
}

impl TursoSync {}
