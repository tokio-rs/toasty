use std::ops::Bound;
use toasty_core::stmt::{HashIndex, Projection, SortedIndex, Value};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn record(fields: impl IntoIterator<Item = Value>) -> Value {
    Value::record_from_vec(fields.into_iter().collect())
}

fn proj(field: usize) -> Projection {
    Projection::single(field)
}

// ---------------------------------------------------------------------------
// HashIndex — single field
// ---------------------------------------------------------------------------

#[test]
fn hash_index_find_single_field() {
    let values = vec![
        record([Value::I64(1), Value::from("alice")]),
        record([Value::I64(2), Value::from("bob")]),
        record([Value::I64(3), Value::from("carol")]),
    ];

    let index = HashIndex::new(&values, &[proj(0)]);

    assert_eq!(index.find(&[Value::I64(1)]), Some(&values[0]));
    assert_eq!(index.find(&[Value::I64(2)]), Some(&values[1]));
    assert_eq!(index.find(&[Value::I64(3)]), Some(&values[2]));
}

#[test]
fn hash_index_find_miss() {
    let values = vec![record([Value::I64(1)])];
    let index = HashIndex::new(&values, &[proj(0)]);
    assert_eq!(index.find(&[Value::I64(99)]), None);
}

#[test]
fn hash_index_empty() {
    let values: Vec<Value> = vec![];
    let index = HashIndex::new(&values, &[proj(0)]);
    assert_eq!(index.find(&[Value::I64(1)]), None);
}

#[test]
fn hash_index_composite_key() {
    let values = vec![
        record([Value::I64(1), Value::I64(10), Value::from("a")]),
        record([Value::I64(1), Value::I64(20), Value::from("b")]),
        record([Value::I64(2), Value::I64(10), Value::from("c")]),
    ];

    // Composite key: (field 0, field 1)
    let index = HashIndex::new(&values, &[proj(0), proj(1)]);

    assert_eq!(
        index.find(&[Value::I64(1), Value::I64(10)]),
        Some(&values[0])
    );
    assert_eq!(
        index.find(&[Value::I64(1), Value::I64(20)]),
        Some(&values[1])
    );
    assert_eq!(
        index.find(&[Value::I64(2), Value::I64(10)]),
        Some(&values[2])
    );
    assert_eq!(index.find(&[Value::I64(2), Value::I64(20)]), None);
}

#[test]
fn hash_index_null_key() {
    let values = vec![
        record([Value::Null, Value::from("a")]),
        record([Value::I64(1), Value::from("b")]),
    ];

    let index = HashIndex::new(&values, &[proj(0)]);

    assert_eq!(index.find(&[Value::Null]), Some(&values[0]));
    assert_eq!(index.find(&[Value::I64(1)]), Some(&values[1]));
}

#[test]
fn hash_index_identity_projection() {
    // Identity projection extracts the entire value as the key.
    let values = vec![Value::I64(10), Value::I64(20), Value::I64(30)];
    let index = HashIndex::new(&values, &[Projection::identity()]);

    assert_eq!(index.find(&[Value::I64(10)]), Some(&values[0]));
    assert_eq!(index.find(&[Value::I64(20)]), Some(&values[1]));
    assert_eq!(index.find(&[Value::I64(99)]), None);
}

// ---------------------------------------------------------------------------
// SortedIndex — equality
// ---------------------------------------------------------------------------

#[test]
fn sorted_index_find_eq() {
    let values = vec![
        record([Value::I64(3), Value::from("c")]),
        record([Value::I64(1), Value::from("a")]),
        record([Value::I64(2), Value::from("b")]),
    ];

    // Insert in unsorted order — new() must sort internally.
    let index = SortedIndex::new(&values, &[proj(0)]);

    assert_eq!(index.find_eq(&[Value::I64(1)]), Some(&values[1]));
    assert_eq!(index.find_eq(&[Value::I64(2)]), Some(&values[2]));
    assert_eq!(index.find_eq(&[Value::I64(3)]), Some(&values[0]));
    assert_eq!(index.find_eq(&[Value::I64(99)]), None);
}

// ---------------------------------------------------------------------------
// SortedIndex — range queries
// ---------------------------------------------------------------------------

fn sorted_i64_index() -> (Vec<Value>, SortedIndex<'static>) {
    // We return a 'static index by leaking the vec. Fine for tests.
    let values: Vec<Value> = (1i64..=5)
        .map(|i| record([Value::I64(i), Value::from(format!("v{i}"))]))
        .collect();
    let leaked: &'static [Value] = Box::leak(values.clone().into_boxed_slice());
    let index = SortedIndex::new(leaked, &[proj(0)]);
    (values, index)
}

#[test]
fn sorted_index_find_lt() {
    let (values, index) = sorted_i64_index();
    let result: Vec<_> = index.find_lt(&[Value::I64(3)]).collect();
    // values[0]=1, values[1]=2 (sorted)
    assert_eq!(result.len(), 2);
    assert!(result.contains(&&values[0]));
    assert!(result.contains(&&values[1]));
}

#[test]
fn sorted_index_find_le() {
    let (values, index) = sorted_i64_index();
    let result: Vec<_> = index.find_le(&[Value::I64(3)]).collect();
    assert_eq!(result.len(), 3);
    assert!(result.contains(&&values[0]));
    assert!(result.contains(&&values[1]));
    assert!(result.contains(&&values[2]));
}

#[test]
fn sorted_index_find_gt() {
    let (values, index) = sorted_i64_index();
    let result: Vec<_> = index.find_gt(&[Value::I64(3)]).collect();
    assert_eq!(result.len(), 2);
    assert!(result.contains(&&values[3]));
    assert!(result.contains(&&values[4]));
}

#[test]
fn sorted_index_find_ge() {
    let (values, index) = sorted_i64_index();
    let result: Vec<_> = index.find_ge(&[Value::I64(3)]).collect();
    assert_eq!(result.len(), 3);
    assert!(result.contains(&&values[2]));
    assert!(result.contains(&&values[3]));
    assert!(result.contains(&&values[4]));
}

#[test]
fn sorted_index_find_range_inclusive_inclusive() {
    let (values, index) = sorted_i64_index();
    let result: Vec<_> = index
        .find_range(
            Bound::Included(&[Value::I64(2)]),
            Bound::Included(&[Value::I64(4)]),
        )
        .collect();
    assert_eq!(result.len(), 3);
    assert!(result.contains(&&values[1]));
    assert!(result.contains(&&values[2]));
    assert!(result.contains(&&values[3]));
}

#[test]
fn sorted_index_find_range_exclusive_exclusive() {
    let (values, index) = sorted_i64_index();
    let result: Vec<_> = index
        .find_range(
            Bound::Excluded(&[Value::I64(1)]),
            Bound::Excluded(&[Value::I64(5)]),
        )
        .collect();
    assert_eq!(result.len(), 3);
    assert!(result.contains(&&values[1]));
    assert!(result.contains(&&values[2]));
    assert!(result.contains(&&values[3]));
}

#[test]
fn sorted_index_find_range_unbounded_unbounded() {
    let (values, index) = sorted_i64_index();
    let result: Vec<_> = index
        .find_range(Bound::Unbounded, Bound::Unbounded)
        .collect();
    assert_eq!(result.len(), 5);
    for v in &values {
        assert!(result.contains(&v));
    }
}

#[test]
fn sorted_index_find_range_empty() {
    let (_values, index) = sorted_i64_index();
    // Range where lower > upper — should be empty.
    let result: Vec<_> = index
        .find_range(
            Bound::Included(&[Value::I64(4)]),
            Bound::Included(&[Value::I64(2)]),
        )
        .collect();
    assert!(result.is_empty());
}

#[test]
fn sorted_index_find_lt_below_min() {
    let (_values, index) = sorted_i64_index();
    let result: Vec<_> = index.find_lt(&[Value::I64(1)]).collect();
    assert!(result.is_empty());
}

#[test]
fn sorted_index_find_gt_above_max() {
    let (_values, index) = sorted_i64_index();
    let result: Vec<_> = index.find_gt(&[Value::I64(5)]).collect();
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// SortedIndex — Null ordering (Null sorts first)
// ---------------------------------------------------------------------------

#[test]
fn sorted_index_null_sorts_first() {
    let values = vec![
        record([Value::I64(2)]),
        record([Value::Null]),
        record([Value::I64(1)]),
    ];
    let leaked: &'static [Value] = Box::leak(values.clone().into_boxed_slice());
    let index = SortedIndex::new(leaked, &[proj(0)]);

    // find_lt(I64(1)) should include Null (which sorts before 1)
    let result: Vec<_> = index.find_lt(&[Value::I64(1)]).collect();
    assert_eq!(result.len(), 1);
    assert!(result.contains(&&values[1])); // Null record
}

// ---------------------------------------------------------------------------
// SortedIndex — composite key
// ---------------------------------------------------------------------------

#[test]
fn sorted_index_composite_key_range() {
    let values = vec![
        record([Value::I64(1), Value::I64(10)]),
        record([Value::I64(1), Value::I64(20)]),
        record([Value::I64(2), Value::I64(10)]),
        record([Value::I64(2), Value::I64(20)]),
    ];
    let leaked: &'static [Value] = Box::leak(values.clone().into_boxed_slice());
    let index = SortedIndex::new(leaked, &[proj(0), proj(1)]);

    // find_ge((1, 20)) → (1,20), (2,10), (2,20)
    let result: Vec<_> = index
        .find_ge(&[Value::I64(1), Value::I64(20)])
        .collect();
    assert_eq!(result.len(), 3);
    assert!(result.contains(&&values[1]));
    assert!(result.contains(&&values[2]));
    assert!(result.contains(&&values[3]));
}
