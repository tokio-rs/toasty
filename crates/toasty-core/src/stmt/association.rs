use super::*;

#[derive(Debug, Clone)]
pub struct Association {
    /// The association source
    pub source: Box<Query>,

    /// How to traverse fields from the source to get to the target
    pub path: Path,
}
