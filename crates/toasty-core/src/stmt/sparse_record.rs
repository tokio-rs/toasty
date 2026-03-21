use super::{PathFieldSet, Type, Value, ValueRecord};
use std::hash::Hash;

/// A record where only a subset of fields are populated.
///
/// Unlike [`ValueRecord`] (which stores values for every field by position),
/// `SparseRecord` tracks which field indices are present via a
/// [`PathFieldSet`] and stores corresponding values. Fields not in the set
/// are absent (not merely null).
///
/// Iterating over a `SparseRecord` yields `(usize, Value)` pairs of field
/// index and value.
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::{Value, PathFieldSet};
///
/// // Create a sparse record with field 0 populated
/// let v = Value::empty_sparse_record();
/// assert!(matches!(v, Value::SparseRecord(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SparseRecord {
    /// Bit set of field indices that are populated in this record.
    pub fields: PathFieldSet,

    /// Values indexed by field position. Indices not in `fields` contain
    /// placeholder [`Value::Null`] entries.
    pub values: Vec<Value>,
}

impl Value {
    /// Creates an empty [`Value::SparseRecord`] with no populated fields.
    pub fn empty_sparse_record() -> Self {
        SparseRecord {
            fields: PathFieldSet::new(),
            values: vec![],
        }
        .into()
    }

    /// Creates a [`Value::SparseRecord`] by distributing values from `record`
    /// into the positions specified by `fields`.
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

    /// Consumes this value and returns the contained [`SparseRecord`],
    /// panicking if this is not a [`Value::SparseRecord`].
    ///
    /// # Panics
    ///
    /// Panics if the value is not a `SparseRecord` variant.
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
