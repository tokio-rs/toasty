#[derive(Debug)]
pub(crate) struct Index {
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

    /// The scope of the index
    pub(crate) scope: IndexScope,
}

#[derive(Debug)]
pub(crate) enum IndexScope {
    /// The index column is used to partition rows across nodes of a distributed database.
    Partition,

    /// The index column is scoped to a physical node.
    Local,
}
