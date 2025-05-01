use super::*;

/// A typed record, indicating the record represents a specific model (or a
/// subset of its fields).
#[derive(Debug, Clone, PartialEq)]
pub struct SparseRecord {
    /// Fields that are present
    pub fields: PathFieldSet,

    /// Values
    pub values: Vec<Value>,
}

impl Value {
    pub fn empty_sparse_record() -> Self {
        SparseRecord {
            fields: PathFieldSet::new(),
            values: vec![],
        }
        .into()
    }

    pub fn sparse_record(fields: PathFieldSet, record: ValueRecord) -> Self {
        let mut values = vec![];

        for (index, value) in fields.iter().zip(record.fields.into_iter()) {
            while index >= values.len() {
                values.push(Value::Null);
            }

            values[index] = value;
        }

        SparseRecord { fields, values }.into()
    }

    pub fn into_sparse_record(self) -> SparseRecord {
        match self {
            Self::SparseRecord(value) => value,
            _ => todo!(),
        }
    }
}

impl Type {
    pub fn sparse_record(fields: impl Into<PathFieldSet>) -> Self {
        Self::SparseRecord(fields.into())
    }

    pub fn empty_sparse_record() -> Self {
        Self::SparseRecord(PathFieldSet::default())
    }
}

impl IntoIterator for SparseRecord {
    type Item = (usize, Value);

    type IntoIter = Box<dyn Iterator<Item = (usize, Value)>>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(
            self.values
                .into_iter()
                .enumerate()
                .filter_map(move |(i, value)| {
                    if self.fields.contains(i) {
                        Some((i, value))
                    } else {
                        None
                    }
                }),
        )
    }
}

impl From<SparseRecord> for Value {
    fn from(value: SparseRecord) -> Self {
        Self::SparseRecord(value)
    }
}
