#[derive(Debug)]
pub(crate) struct PrimaryKey {
    /// Index of fields in the primary key
    pub(crate) fields: Vec<usize>,

    /// Model index that represents the primary key
    pub(crate) index: usize,
}
