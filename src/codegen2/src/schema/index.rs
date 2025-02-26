#[derive(Debug)]
pub(crate) struct Index {
    /// Uniquely identifies the index within the model.
    pub(crate) id: usize,

    /// Fields included in the index.
    pub(crate) fields: Vec<IndexField>,

    /// When `true`, indexed entries are unique
    pub(crate) unique: bool,

    /// True when the index is the primary key
    pub(crate) primary_key: bool,
}

#[derive(Debug)]
pub(crate) struct IndexField {
    /// The field being indexed
    pub(crate) field: usize,
}
