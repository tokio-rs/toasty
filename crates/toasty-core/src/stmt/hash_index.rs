use super::value_set::{HashableValue, HashableValueSlice};
use super::{Entry, Projection, Value};

use indexmap::IndexMap;

/// A unique hash index over a borrowed slice of [`Value`]s.
///
/// Keys are extracted from each value using a set of [`Projection`]s. The key is
/// the composite of the projected field values. Only equality lookup is supported.
///
/// Both construction and lookup are O(1) amortized (hash map operations).
///
/// # Uniqueness
///
/// The index assumes each extracted key is unique across the source slice. A
/// `debug_assert!` fires on duplicate keys at build time.
///
/// # Cloning
///
/// Key fields are cloned into owned [`Value`]s for use as map keys. Full records
/// are never cloned -- the map values are `&'a Value` references into the source slice.
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::{HashIndex, Projection, Value, ValueRecord};
///
/// let records = vec![
///     Value::record_from_vec(vec![Value::from(1_i64), Value::from("a")]),
///     Value::record_from_vec(vec![Value::from(2_i64), Value::from("b")]),
/// ];
/// let index = HashIndex::new(&records, &[Projection::single(0)]);
/// let found = index.find(&[Value::from(2_i64)]);
/// assert!(found.is_some());
/// ```
pub struct HashIndex<'a> {
    map: IndexMap<Vec<HashableValue>, &'a Value>,
}

impl<'a> HashIndex<'a> {
    /// Build an index over `values`, keyed by the fields selected by `projections`.
    ///
    /// Each projection navigates into a value to extract one key component. Multiple
    /// projections produce a composite key compared lexicographically.
    pub fn new(values: &'a [Value], projections: &[Projection]) -> Self {
        let mut map = IndexMap::with_capacity(values.len());

        for value in values {
            let key = extract_key(value, projections);
            let prev = map.insert(key, value);
            debug_assert!(prev.is_none(), "HashIndex: duplicate key detected");
        }

        Self { map }
    }

    /// Look up the value whose key equals `key`.
    ///
    /// `key` must be a slice of values with one entry per projection used at build time.
    /// Returns `None` if no value matches.
    pub fn find(&self, key: &[Value]) -> Option<&'a Value> {
        self.map.get(&HashableValueSlice(key)).copied()
    }
}

/// Extract the composite key from `value` using `projections`.
///
/// Each projection is applied to `value` in sequence, collecting the resulting
/// field references into an owned `Vec<HashableValue>`.
fn extract_key(value: &Value, projections: &[Projection]) -> Vec<HashableValue> {
    projections
        .iter()
        .map(|proj| match value.entry(proj) {
            Entry::Value(v) => HashableValue(v.clone()),
            Entry::Expr(_) => panic!("projection yielded an expression, not a value"),
        })
        .collect()
}
