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
    pub fn empty_sparse_record() -> Value {
        SparseRecord {
            fields: PathFieldSet::new(),
            values: vec![],
        }
        .into()
    }

    pub fn sparse_record(fields: PathFieldSet, record: ValueRecord) -> Value {
        let mut values = vec![];

        for (index, value) in fields.iter().zip(record.fields.into_iter()) {
            assert!(index >= values.len());

            while index > values.len() {
                values.push(Value::Null);
            }

            values.push(value);
        }

        SparseRecord { fields, values }.into()
    }

    pub fn into_sparse_record(self) -> SparseRecord {
        match self {
            Value::SparseRecord(value) => value,
            _ => todo!(),
        }
    }
}

impl Type {
    pub fn sparse_record(fields: impl Into<PathFieldSet>) -> Type {
        Type::SparseRecord(fields.into())
    }

    pub fn empty_sparse_record() -> Type {
        Type::SparseRecord(PathFieldSet::default())
    }
}

impl SparseRecord {
    pub fn into_iter(self) -> impl Iterator<Item = (usize, Value)> {
        self.values
            .into_iter()
            .enumerate()
            .flat_map(move |(i, value)| {
                if self.fields.contains(i) {
                    Some((i.into(), value))
                } else {
                    None
                }
            })
    }
}

impl From<SparseRecord> for Value {
    fn from(value: SparseRecord) -> Self {
        Value::SparseRecord(value)
    }
}
