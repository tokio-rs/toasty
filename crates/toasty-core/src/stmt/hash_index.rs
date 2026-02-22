use super::{Entry, Projection, Value};

use std::collections::HashMap;

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
/// are never cloned — the map values are `&'a Value` references into the source slice.
pub struct HashIndex<'a> {
    map: HashMap<Vec<Value>, &'a Value>,
}

impl<'a> HashIndex<'a> {
    /// Build an index over `values`, keyed by the fields selected by `projections`.
    ///
    /// Each projection navigates into a value to extract one key component. Multiple
    /// projections produce a composite key compared lexicographically.
    pub fn new(values: &'a [Value], projections: &[Projection]) -> Self {
        let mut map = HashMap::with_capacity(values.len());

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
        self.map.get(key).copied()
    }
}

/// Extract the composite key from `value` using `projections`.
///
/// Each projection is applied to `value` in sequence, collecting the resulting
/// field references into an owned `Vec<Value>`.
fn extract_key(value: &Value, projections: &[Projection]) -> Vec<Value> {
    projections
        .iter()
        .map(|proj| match value.entry(proj) {
            Entry::Value(v) => v.clone(),
            Entry::Expr(_) => panic!("projection yielded an expression, not a value"),
        })
        .collect()
}
