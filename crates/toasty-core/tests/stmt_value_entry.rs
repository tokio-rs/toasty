use toasty_core::stmt::{Project, Projection, Value};

// ---------------------------------------------------------------------------
// Identity path on scalar types — entry() returns the value itself
// ---------------------------------------------------------------------------

#[test]
fn entry_identity_bool() {
    let v = Value::Bool(true);
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::Bool(true)
    );
}

#[test]
fn entry_identity_i8() {
    let v = Value::I8(42);
    assert_eq!(v.entry(&Projection::identity()).as_value(), &Value::I8(42));
}

#[test]
fn entry_identity_i16() {
    let v = Value::I16(-100);
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::I16(-100)
    );
}

#[test]
fn entry_identity_i32() {
    let v = Value::I32(1_000);
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::I32(1_000)
    );
}

#[test]
fn entry_identity_i64() {
    let v = Value::I64(i64::MAX);
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::I64(i64::MAX)
    );
}

#[test]
fn entry_identity_u8() {
    let v = Value::U8(255);
    assert_eq!(v.entry(&Projection::identity()).as_value(), &Value::U8(255));
}

#[test]
fn entry_identity_u16() {
    let v = Value::U16(1000);
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::U16(1000)
    );
}

#[test]
fn entry_identity_u32() {
    let v = Value::U32(u32::MAX);
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::U32(u32::MAX)
    );
}

#[test]
fn entry_identity_u64() {
    let v = Value::U64(u64::MAX);
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::U64(u64::MAX)
    );
}

#[test]
fn entry_identity_string() {
    let v = Value::from("hello");
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::from("hello")
    );
}

#[test]
fn entry_identity_bytes() {
    let v = Value::Bytes(vec![1, 2, 3]);
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::Bytes(vec![1, 2, 3])
    );
}

#[test]
fn entry_identity_uuid() {
    let id = uuid::Uuid::nil();
    let v = Value::Uuid(id);
    assert_eq!(
        v.entry(&Projection::identity()).as_value(),
        &Value::Uuid(id)
    );
}

#[test]
fn entry_identity_null() {
    let v = Value::Null;
    assert_eq!(v.entry(&Projection::identity()).as_value(), &Value::Null);
}

// ---------------------------------------------------------------------------
// Single-step into a record — all scalar field types
// ---------------------------------------------------------------------------

#[test]
fn entry_record_field_bool() {
    let rec = Value::record_from_vec(vec![Value::Bool(false), Value::Bool(true)]);
    assert_eq!(
        rec.entry(&Projection::single(1)).as_value(),
        &Value::Bool(true)
    );
}

#[test]
fn entry_record_field_i8() {
    let rec = Value::record_from_vec(vec![Value::I8(10), Value::I8(20)]);
    assert_eq!(rec.entry(&Projection::single(0)).as_value(), &Value::I8(10));
}

#[test]
fn entry_record_field_i16() {
    let rec = Value::record_from_vec(vec![Value::I16(300), Value::I16(400)]);
    assert_eq!(
        rec.entry(&Projection::single(1)).as_value(),
        &Value::I16(400)
    );
}

#[test]
fn entry_record_field_i32() {
    let rec = Value::record_from_vec(vec![Value::I32(-1), Value::I32(2)]);
    assert_eq!(
        rec.entry(&Projection::single(0)).as_value(),
        &Value::I32(-1)
    );
}

#[test]
fn entry_record_field_i64() {
    let rec = Value::record_from_vec(vec![Value::I64(100), Value::I64(200)]);
    assert_eq!(
        rec.entry(&Projection::single(1)).as_value(),
        &Value::I64(200)
    );
}

#[test]
fn entry_record_field_u8() {
    let rec = Value::record_from_vec(vec![Value::U8(1), Value::U8(2)]);
    assert_eq!(rec.entry(&Projection::single(0)).as_value(), &Value::U8(1));
}

#[test]
fn entry_record_field_u16() {
    let rec = Value::record_from_vec(vec![Value::U16(500), Value::U16(600)]);
    assert_eq!(
        rec.entry(&Projection::single(1)).as_value(),
        &Value::U16(600)
    );
}

#[test]
fn entry_record_field_u32() {
    let rec = Value::record_from_vec(vec![Value::U32(999), Value::U32(0)]);
    assert_eq!(
        rec.entry(&Projection::single(0)).as_value(),
        &Value::U32(999)
    );
}

#[test]
fn entry_record_field_u64() {
    let rec = Value::record_from_vec(vec![Value::U64(u64::MAX), Value::U64(0)]);
    assert_eq!(
        rec.entry(&Projection::single(0)).as_value(),
        &Value::U64(u64::MAX)
    );
}

#[test]
fn entry_record_field_string() {
    let rec = Value::record_from_vec(vec![Value::from("first"), Value::from("second")]);
    assert_eq!(
        rec.entry(&Projection::single(0)).as_value(),
        &Value::from("first")
    );
}

#[test]
fn entry_record_field_bytes() {
    let rec = Value::record_from_vec(vec![Value::Bytes(vec![0xDE, 0xAD]), Value::I64(0)]);
    assert_eq!(
        rec.entry(&Projection::single(0)).as_value(),
        &Value::Bytes(vec![0xDE, 0xAD])
    );
}

#[test]
fn entry_record_field_uuid() {
    let id = uuid::Uuid::nil();
    let rec = Value::record_from_vec(vec![Value::Uuid(id), Value::I64(0)]);
    assert_eq!(
        rec.entry(&Projection::single(0)).as_value(),
        &Value::Uuid(id)
    );
}

#[test]
fn entry_record_field_null() {
    let rec = Value::record_from_vec(vec![Value::I64(1), Value::Null]);
    assert_eq!(rec.entry(&Projection::single(1)).as_value(), &Value::Null);
}

// ---------------------------------------------------------------------------
// List item projection
// ---------------------------------------------------------------------------

#[test]
fn entry_list_first_item() {
    let list = Value::List(vec![Value::I64(10), Value::I64(20), Value::I64(30)]);
    assert_eq!(
        list.entry(&Projection::single(0)).as_value(),
        &Value::I64(10)
    );
}

#[test]
fn entry_list_last_item() {
    let list = Value::List(vec![Value::I64(10), Value::I64(20), Value::I64(30)]);
    assert_eq!(
        list.entry(&Projection::single(2)).as_value(),
        &Value::I64(30)
    );
}

#[test]
fn entry_list_string_items() {
    let list = Value::List(vec![Value::from("a"), Value::from("b"), Value::from("c")]);
    assert_eq!(
        list.entry(&Projection::single(1)).as_value(),
        &Value::from("b")
    );
}

// ---------------------------------------------------------------------------
// Multi-step (nested) projections
// ---------------------------------------------------------------------------

#[test]
fn entry_record_of_records_two_steps() {
    // outer: [inner_record([10, 20]), 99]
    let inner = Value::record_from_vec(vec![Value::I64(10), Value::I64(20)]);
    let outer = Value::record_from_vec(vec![inner, Value::I64(99)]);
    assert_eq!(
        outer.entry(&Projection::from([0usize, 1])).as_value(),
        &Value::I64(20)
    );
}

#[test]
fn entry_deeply_nested_three_levels() {
    let lvl1 = Value::record_from_vec(vec![Value::I64(42)]);
    let lvl2 = Value::record_from_vec(vec![lvl1]);
    let lvl3 = Value::record_from_vec(vec![lvl2]);
    assert_eq!(
        lvl3.entry(&Projection::from([0usize, 0, 0])).as_value(),
        &Value::I64(42)
    );
}

#[test]
fn entry_list_of_records() {
    // list[1][0] → "second-first"
    let r0 = Value::record_from_vec(vec![
        Value::from("first-first"),
        Value::from("first-second"),
    ]);
    let r1 = Value::record_from_vec(vec![
        Value::from("second-first"),
        Value::from("second-second"),
    ]);
    let list = Value::List(vec![r0, r1]);
    assert_eq!(
        list.entry(&Projection::from([1usize, 0])).as_value(),
        &Value::from("second-first")
    );
}

#[test]
fn entry_record_of_lists() {
    // record[0][2] → Value::I64(30)
    let inner_list = Value::List(vec![Value::I64(10), Value::I64(20), Value::I64(30)]);
    let rec = Value::record_from_vec(vec![inner_list, Value::I64(0)]);
    assert_eq!(
        rec.entry(&Projection::from([0usize, 2])).as_value(),
        &Value::I64(30)
    );
}

// ---------------------------------------------------------------------------
// entry() → Entry methods: to_value(), is_const(), eval_const()
// ---------------------------------------------------------------------------

#[test]
fn entry_to_value() {
    let rec = Value::record_from_vec(vec![Value::I64(77), Value::I64(88)]);
    let val = rec.entry(&Projection::single(0)).to_value();
    assert_eq!(val, Value::I64(77));
}

#[test]
fn entry_is_const_always_true_for_value() {
    let v = Value::I64(5);
    assert!(v.entry(&Projection::identity()).is_const());
}

#[test]
fn entry_eval_const() {
    let v = Value::from("test");
    let result = v.entry(&Projection::identity()).eval_const().unwrap();
    assert_eq!(result, Value::from("test"));
}

// ---------------------------------------------------------------------------
// Project trait on Value and &Value
// ---------------------------------------------------------------------------

#[test]
fn project_trait_on_value_identity() {
    let v = Value::I64(55);
    let expr = v.project(&Projection::identity()).unwrap();
    assert_eq!(expr.eval_const().unwrap(), Value::I64(55));
}

#[test]
fn project_trait_on_value_single_step() {
    let rec = Value::record_from_vec(vec![Value::I64(1), Value::I64(2)]);
    let expr = rec.project(&Projection::single(1)).unwrap();
    assert_eq!(expr.eval_const().unwrap(), Value::I64(2));
}

#[test]
fn project_trait_on_ref_value() {
    let rec = Value::record_from_vec(vec![Value::from("x"), Value::from("y")]);
    let expr = (&rec).project(&Projection::single(0)).unwrap();
    assert_eq!(expr.eval_const().unwrap(), Value::from("x"));
}

#[test]
fn project_trait_on_value_multi_step() {
    let inner = Value::record_from_vec(vec![Value::I64(7), Value::I64(8)]);
    let outer = Value::record_from_vec(vec![inner, Value::I64(0)]);
    let expr = outer.project(&Projection::from([0usize, 1])).unwrap();
    assert_eq!(expr.eval_const().unwrap(), Value::I64(8));
}

// ---------------------------------------------------------------------------
// Projection::push builds correct paths for use with Value::entry()
// ---------------------------------------------------------------------------

#[test]
fn entry_via_pushed_projection() {
    let inner = Value::record_from_vec(vec![Value::I64(10), Value::I64(20)]);
    let outer = Value::record_from_vec(vec![inner, Value::I64(0)]);
    let mut proj = Projection::identity();
    proj.push(0);
    proj.push(1);
    assert_eq!(outer.entry(&proj).as_value(), &Value::I64(20));
}

// ---------------------------------------------------------------------------
// Error cases — Value::entry() panics on invalid paths
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn entry_panic_on_bool_with_step() {
    let v = Value::Bool(true);
    let _ = v.entry(&Projection::single(0));
}

#[test]
#[should_panic]
fn entry_panic_on_i64_with_step() {
    let v = Value::I64(42);
    let _ = v.entry(&Projection::single(0));
}

#[test]
#[should_panic]
fn entry_panic_on_string_with_step() {
    let v = Value::from("hello");
    let _ = v.entry(&Projection::single(0));
}

#[test]
#[should_panic]
fn entry_panic_on_null_with_step() {
    let v = Value::Null;
    let _ = v.entry(&Projection::single(0));
}

#[test]
#[should_panic]
fn entry_panic_on_bytes_with_step() {
    let v = Value::Bytes(vec![1, 2, 3]);
    let _ = v.entry(&Projection::single(0));
}

#[test]
#[should_panic]
fn entry_panic_on_record_out_of_bounds() {
    let rec = Value::record_from_vec(vec![Value::I64(1), Value::I64(2)]);
    let _ = rec.entry(&Projection::single(5));
}

#[test]
#[should_panic]
fn entry_panic_on_list_out_of_bounds() {
    let list = Value::List(vec![Value::I64(1), Value::I64(2)]);
    let _ = list.entry(&Projection::single(10));
}
