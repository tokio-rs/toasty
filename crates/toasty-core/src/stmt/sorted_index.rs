use super::{Entry, Projection, Value};

use std::cmp::Ordering;
use std::ops::Bound;

/// A sorted index over a borrowed slice of [`Value`]s.
///
/// Keys are extracted from each value using a set of [`Projection`]s. The index is
/// sorted using a private total ordering on [`Value`] that extends the existing
/// `PartialOrd` (which has SQL semantics and returns `None` for `Null` comparisons)
/// with a deterministic order for all cases.
///
/// Supports equality and range queries. Duplicate keys are allowed; queries
/// that would return multiple values (e.g. `find_range`) yield all of them.
///
/// Construction is O(n log n). All queries are O(log n + k) where k is the result count.
///
/// # Cloning
///
/// Key fields are cloned into owned [`Value`]s for storage. Full records are never
/// cloned — the stored values are `&'a Value` references into the source slice.
pub struct SortedIndex<'a> {
    /// Entries sorted by key using [`total_cmp`].
    entries: Vec<(Vec<Value>, &'a Value)>,
}

impl<'a> SortedIndex<'a> {
    /// Build a sorted index over `values`, keyed by the fields selected by `projections`.
    ///
    /// Each projection navigates into a value to extract one key component. Multiple
    /// projections produce a composite key compared lexicographically.
    pub fn new(values: &'a [Value], projections: &[Projection]) -> Self {
        let mut entries: Vec<(Vec<Value>, &'a Value)> = values
            .iter()
            .map(|value| (extract_key(value, projections), value))
            .collect();

        entries.sort_by(|(a, _), (b, _)| total_cmp(a, b));

        Self { entries }
    }

    /// Find the value whose key equals `key`.
    pub fn find_eq(&self, key: &[Value]) -> Option<&'a Value> {
        self.find_range(Bound::Included(key), Bound::Included(key))
            .next()
    }

    /// Iterate over all values whose key is strictly less than `key`.
    pub fn find_lt(&self, key: &[Value]) -> impl Iterator<Item = &'a Value> + '_ {
        self.find_range(Bound::Unbounded, Bound::Excluded(key))
    }

    /// Iterate over all values whose key is less than or equal to `key`.
    pub fn find_le(&self, key: &[Value]) -> impl Iterator<Item = &'a Value> + '_ {
        self.find_range(Bound::Unbounded, Bound::Included(key))
    }

    /// Iterate over all values whose key is strictly greater than `key`.
    pub fn find_gt(&self, key: &[Value]) -> impl Iterator<Item = &'a Value> + '_ {
        self.find_range(Bound::Excluded(key), Bound::Unbounded)
    }

    /// Iterate over all values whose key is greater than or equal to `key`.
    pub fn find_ge(&self, key: &[Value]) -> impl Iterator<Item = &'a Value> + '_ {
        self.find_range(Bound::Included(key), Bound::Unbounded)
    }

    /// Iterate over all values whose key falls within `[lower, upper]` using
    /// [`Bound`] to control inclusive/exclusive/unbounded endpoints.
    pub fn find_range(
        &self,
        lower: Bound<&[Value]>,
        upper: Bound<&[Value]>,
    ) -> impl Iterator<Item = &'a Value> + '_ {
        let start = match lower {
            Bound::Unbounded => 0,
            Bound::Included(lo) => {
                self.entries
                    .partition_point(|(k, _)| total_cmp(k, lo) == Ordering::Less)
            }
            Bound::Excluded(lo) => {
                self.entries
                    .partition_point(|(k, _)| total_cmp(k, lo) != Ordering::Greater)
            }
        };

        let end = match upper {
            Bound::Unbounded => self.entries.len(),
            Bound::Included(hi) => {
                self.entries
                    .partition_point(|(k, _)| total_cmp(k, hi) != Ordering::Greater)
            }
            Bound::Excluded(hi) => {
                self.entries
                    .partition_point(|(k, _)| total_cmp(k, hi) == Ordering::Less)
            }
        };

        let range = if start <= end { start..end } else { 0..0 };
        self.entries[range].iter().map(|(_, v)| *v)
    }
}

/// Extract the composite key from `value` using `projections`.
fn extract_key(value: &Value, projections: &[Projection]) -> Vec<Value> {
    projections
        .iter()
        .map(|proj| match value.entry(proj) {
            Entry::Value(v) => v.clone(),
            Entry::Expr(_) => panic!("projection yielded an expression, not a value"),
        })
        .collect()
}

/// Total ordering over composite keys (`&[Value]`).
///
/// Compares element-by-element using [`value_total_cmp`], then by length if all
/// elements are equal.
fn total_cmp(a: &[Value], b: &[Value]) -> Ordering {
    for (x, y) in a.iter().zip(b.iter()) {
        let ord = value_total_cmp(x, y);
        if ord != Ordering::Equal {
            return ord;
        }
    }
    a.len().cmp(&b.len())
}

/// Total ordering over individual [`Value`]s.
///
/// Extends [`Value::partial_cmp`] (which has SQL semantics and returns `None` for
/// `Null`) with a deterministic total ordering:
///
/// - `Null` sorts first (less than all other values)
/// - Same-type comparisons use the type's natural ordering
/// - Cross-type comparisons order by a fixed variant index
fn value_total_cmp(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,

        // For same-type pairs, partial_cmp always returns Some — unwrap is safe.
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::I8(a), Value::I8(b)) => a.cmp(b),
        (Value::I16(a), Value::I16(b)) => a.cmp(b),
        (Value::I32(a), Value::I32(b)) => a.cmp(b),
        (Value::I64(a), Value::I64(b)) => a.cmp(b),
        (Value::U8(a), Value::U8(b)) => a.cmp(b),
        (Value::U16(a), Value::U16(b)) => a.cmp(b),
        (Value::U32(a), Value::U32(b)) => a.cmp(b),
        (Value::U64(a), Value::U64(b)) => a.cmp(b),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::Bytes(a), Value::Bytes(b)) => a.cmp(b),
        (Value::Uuid(a), Value::Uuid(b)) => a.cmp(b),

        // Composite types: lexicographic total ordering.
        (Value::Record(a), Value::Record(b)) => {
            for (x, y) in a.iter().zip(b.iter()) {
                let ord = value_total_cmp(x, y);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            a.len().cmp(&b.len())
        }
        (Value::List(a), Value::List(b)) => {
            for (x, y) in a.iter().zip(b.iter()) {
                let ord = value_total_cmp(x, y);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            a.len().cmp(&b.len())
        }

        // Feature-gated types.
        #[cfg(feature = "rust_decimal")]
        (Value::Decimal(a), Value::Decimal(b)) => a.cmp(b),

        #[cfg(feature = "bigdecimal")]
        (Value::BigDecimal(a), Value::BigDecimal(b)) => {
            a.partial_cmp(b).unwrap_or(Ordering::Equal)
        }

        #[cfg(feature = "jiff")]
        (Value::Timestamp(a), Value::Timestamp(b)) => a.cmp(b),

        #[cfg(feature = "jiff")]
        (Value::Date(a), Value::Date(b)) => a.cmp(b),

        #[cfg(feature = "jiff")]
        (Value::Time(a), Value::Time(b)) => a.cmp(b),

        #[cfg(feature = "jiff")]
        (Value::DateTime(a), Value::DateTime(b)) => a.cmp(b),

        #[cfg(feature = "jiff")]
        (Value::Zoned(a), Value::Zoned(b)) => {
            a.partial_cmp(b).unwrap_or(Ordering::Equal)
        }

        // Cross-type: order by a fixed variant index.
        _ => variant_index(a).cmp(&variant_index(b)),
    }
}

/// Returns a fixed numeric index for each [`Value`] variant, used for cross-type ordering.
fn variant_index(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::I8(_) => 2,
        Value::I16(_) => 3,
        Value::I32(_) => 4,
        Value::I64(_) => 5,
        Value::U8(_) => 6,
        Value::U16(_) => 7,
        Value::U32(_) => 8,
        Value::U64(_) => 9,
        Value::String(_) => 10,
        Value::Bytes(_) => 11,
        Value::Uuid(_) => 12,
        Value::Record(_) => 13,
        Value::List(_) => 14,
        Value::SparseRecord(_) => 15,
        #[cfg(feature = "rust_decimal")]
        Value::Decimal(_) => 16,
        #[cfg(feature = "bigdecimal")]
        Value::BigDecimal(_) => 17,
        #[cfg(feature = "jiff")]
        Value::Timestamp(_) => 18,
        #[cfg(feature = "jiff")]
        Value::Zoned(_) => 19,
        #[cfg(feature = "jiff")]
        Value::Date(_) => 20,
        #[cfg(feature = "jiff")]
        Value::Time(_) => 21,
        #[cfg(feature = "jiff")]
        Value::DateTime(_) => 22,
    }
}
