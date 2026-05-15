use super::{SparseRecord, Value, ValueRecord};

use hashbrown::{Equivalent, HashSet};
use std::hash::{Hash, Hasher};

/// A set of [`Value`]s.
///
/// Provides hash-based deduplication with well-defined semantics for every
/// `Value` variant, including future floating-point variants (which will use
/// bitwise comparison so `NaN == NaN` and `+0.0 != -0.0`). `Value` itself does
/// not implement `Hash`/`Eq` because the right float policy is
/// context-dependent; `ValueSet` picks the policy suitable for deduplication.
#[derive(Debug, Default, Clone)]
pub struct ValueSet {
    inner: HashSet<HashableValue>,
}

impl ValueSet {
    /// Creates an empty set.
    pub fn new() -> Self {
        Self {
            inner: HashSet::new(),
        }
    }

    /// Creates an empty set with capacity for at least `capacity` values.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: HashSet::with_capacity(capacity),
        }
    }

    /// Inserts a value into the set. Returns `true` if the value was not
    /// already present.
    pub fn insert(&mut self, value: Value) -> bool {
        self.inner.insert(HashableValue(value))
    }

    /// Returns the number of values in the set.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the set contains no values.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// A `Value` wrapped so it can be used as a hash-table key.
///
/// Hash/equality semantics are "literal bits": recursive, with bitwise
/// comparison for floats once they are added (so `NaN` hashes equal to itself
/// and `+0.0` is distinct from `-0.0`). This is appropriate for deduplication
/// and join-index lookup, not for SQL-semantic equality.
#[derive(Clone, Debug)]
pub(super) struct HashableValue(pub(super) Value);

impl PartialEq for HashableValue {
    fn eq(&self, other: &Self) -> bool {
        value_eq(&self.0, &other.0)
    }
}

impl Eq for HashableValue {}

impl Hash for HashableValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_value(&self.0, state);
    }
}

/// Borrowed view over a `&[Value]`, used to query hash tables keyed by
/// `Vec<HashableValue>` without per-lookup allocation. Implements
/// [`Equivalent`] against the owned key type.
///
/// The `Hash` impl here must produce a byte stream identical to
/// `<Vec<HashableValue> as Hash>` for equivalent contents.
pub(super) struct HashableValueSlice<'a>(pub(super) &'a [Value]);

impl Hash for HashableValueSlice<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Mirror `<[HashableValue] as Hash>::hash`: length prefix, then each
        // element. `usize::hash` and the default `write_length_prefix` both
        // resolve to `Hasher::write_usize`, so the two paths agree.
        self.0.len().hash(state);
        for v in self.0 {
            hash_value(v, state);
        }
    }
}

impl Equivalent<Vec<HashableValue>> for HashableValueSlice<'_> {
    fn equivalent(&self, key: &Vec<HashableValue>) -> bool {
        self.0.len() == key.len() && self.0.iter().zip(key).all(|(v, hv)| value_eq(v, &hv.0))
    }
}

// Each variant is spelled out rather than collapsed into a blanket `a == b`
// fallback for two reasons:
//
// 1. Some variants must diverge from `PartialEq`: containers (List, Record,
//    SparseRecord) recurse through `value_eq` so that future float variants
//    inside them use bitwise comparison instead of `PartialEq`'s NaN-never-
//    equal semantics. Future F32/F64 variants will themselves diverge, using
//    `to_bits()` equality.
// 2. Listing every variant forces a deliberate choice when a new one is
//    added. A tuple match can't be compiler-exhaustive without a catch-all,
//    so the real exhaustiveness check lives in `hash_value` below (single-
//    value match, no `_` arm) — adding a variant there errors until it's
//    handled, at which point the author is prompted to audit this function.
pub(super) fn value_eq(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Null, Null) => true,
        (Bool(a), Bool(b)) => a == b,
        (I8(a), I8(b)) => a == b,
        (I16(a), I16(b)) => a == b,
        (I32(a), I32(b)) => a == b,
        (I64(a), I64(b)) => a == b,
        (U8(a), U8(b)) => a == b,
        (U16(a), U16(b)) => a == b,
        (U32(a), U32(b)) => a == b,
        (U64(a), U64(b)) => a == b,
        (F32(a), F32(b)) => a.to_bits() == b.to_bits(),
        (F64(a), F64(b)) => a.to_bits() == b.to_bits(),
        (String(a), String(b)) => a == b,
        (Bytes(a), Bytes(b)) => a == b,
        (Uuid(a), Uuid(b)) => a == b,
        (List(a), List(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| value_eq(x, y)),
        (Record(a), Record(b)) => record_eq(a, b),
        (SparseRecord(a), SparseRecord(b)) => sparse_record_eq(a, b),
        #[cfg(feature = "rust_decimal")]
        (Decimal(a), Decimal(b)) => a == b,
        #[cfg(feature = "bigdecimal")]
        (BigDecimal(a), BigDecimal(b)) => a == b,
        #[cfg(feature = "jiff")]
        (Timestamp(a), Timestamp(b)) => a == b,
        #[cfg(feature = "jiff")]
        (Zoned(a), Zoned(b)) => a == b,
        #[cfg(feature = "jiff")]
        (Date(a), Date(b)) => a == b,
        #[cfg(feature = "jiff")]
        (Time(a), Time(b)) => a == b,
        #[cfg(feature = "jiff")]
        (DateTime(a), DateTime(b)) => a == b,
        _ => false,
    }
}

fn record_eq(a: &ValueRecord, b: &ValueRecord) -> bool {
    a.fields.len() == b.fields.len() && a.fields.iter().zip(&b.fields).all(|(x, y)| value_eq(x, y))
}

fn sparse_record_eq(a: &SparseRecord, b: &SparseRecord) -> bool {
    a.fields == b.fields
        && a.values.len() == b.values.len()
        && a.values.iter().zip(&b.values).all(|(x, y)| value_eq(x, y))
}

pub(super) fn hash_value<H: Hasher>(v: &Value, state: &mut H) {
    // Hash the discriminant so that two variants with equal payload bits
    // don't collide (e.g. `I32(0)` vs `U32(0)`).
    std::mem::discriminant(v).hash(state);
    match v {
        Value::Null => {}
        Value::Bool(x) => x.hash(state),
        Value::I8(x) => x.hash(state),
        Value::I16(x) => x.hash(state),
        Value::I32(x) => x.hash(state),
        Value::I64(x) => x.hash(state),
        Value::U8(x) => x.hash(state),
        Value::U16(x) => x.hash(state),
        Value::U32(x) => x.hash(state),
        Value::U64(x) => x.hash(state),
        Value::F32(x) => x.to_bits().hash(state),
        Value::F64(x) => x.to_bits().hash(state),
        Value::String(x) => x.hash(state),
        Value::Bytes(x) => x.hash(state),
        Value::Uuid(x) => x.hash(state),
        Value::List(items) => {
            items.len().hash(state);
            for it in items {
                hash_value(it, state);
            }
        }
        Value::Record(r) => {
            r.fields.len().hash(state);
            for v in &r.fields {
                hash_value(v, state);
            }
        }
        Value::Object(o) => {
            o.entries.len().hash(state);
            for (k, v) in &o.entries {
                k.hash(state);
                hash_value(v, state);
            }
        }
        Value::SparseRecord(r) => {
            r.fields.hash(state);
            r.values.len().hash(state);
            for v in &r.values {
                hash_value(v, state);
            }
        }
        #[cfg(feature = "rust_decimal")]
        Value::Decimal(x) => x.hash(state),
        #[cfg(feature = "bigdecimal")]
        Value::BigDecimal(x) => {
            // `bigdecimal::BigDecimal` implements `Hash`.
            x.hash(state);
        }
        #[cfg(feature = "jiff")]
        Value::Timestamp(x) => x.hash(state),
        #[cfg(feature = "jiff")]
        Value::Zoned(x) => x.hash(state),
        #[cfg(feature = "jiff")]
        Value::Date(x) => x.hash(state),
        #[cfg(feature = "jiff")]
        Value::Time(x) => x.hash(state),
        #[cfg(feature = "jiff")]
        Value::DateTime(x) => x.hash(state),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash<T: Hash + ?Sized>(v: &T) -> u64 {
        let mut h = DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    }

    #[test]
    fn slice_and_vec_hash_match() {
        // HashableValueSlice must produce the same hash as Vec<HashableValue>
        // for equivalent contents. If this fails, `HashIndex` lookups will
        // miss entries even when equal.
        let values = [Value::from(1_i64), Value::from("hello"), Value::from(true)];
        let owned: Vec<HashableValue> = values.iter().cloned().map(HashableValue).collect();

        assert_eq!(hash(&HashableValueSlice(&values)), hash(&owned));
    }

    #[test]
    fn slice_and_vec_hash_match_empty() {
        let values: [Value; 0] = [];
        let owned: Vec<HashableValue> = vec![];
        assert_eq!(hash(&HashableValueSlice(&values)), hash(&owned));
    }

    #[test]
    fn slice_equivalent_to_vec() {
        let values = [Value::from(1_i64), Value::from(2_i64)];
        let owned: Vec<HashableValue> = values.iter().cloned().map(HashableValue).collect();
        assert!(HashableValueSlice(&values).equivalent(&owned));
    }

    #[test]
    fn value_set_dedup() {
        let mut set = ValueSet::new();
        assert!(set.insert(Value::from(1_i64)));
        assert!(!set.insert(Value::from(1_i64)));
        assert!(set.insert(Value::from(2_i64)));
        assert_eq!(set.len(), 2);
    }
}
