#[derive(Debug)]
pub(crate) struct PrimaryKey {
    /// Index of fields in the primary key
    pub(crate) fields: Vec<usize>,
}
