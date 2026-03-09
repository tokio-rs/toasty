use toasty_core::stmt::{Type, TypeUnion};

fn make_union(types: Vec<Type>) -> Type {
    let mut union = TypeUnion::new();
    for ty in types {
        union.insert(ty);
    }
    union.simplify()
}

// ---------------------------------------------------------------------------
// Null is a universal subtype
// ---------------------------------------------------------------------------

#[test]
fn null_is_subtype_of_anything() {
    assert!(Type::Null.is_subtype_of(&Type::String));
    assert!(Type::Null.is_subtype_of(&Type::I64));
    assert!(Type::Null.is_subtype_of(&Type::Record(vec![Type::I32])));
}

#[test]
fn anything_is_subtype_of_null() {
    assert!(Type::String.is_subtype_of(&Type::Null));
    assert!(Type::I64.is_subtype_of(&Type::Null));
    assert!(Type::Record(vec![Type::I32]).is_subtype_of(&Type::Null));
}

// ---------------------------------------------------------------------------
// Same type identity
// ---------------------------------------------------------------------------

#[test]
fn same_type_is_subtype() {
    assert!(Type::String.is_subtype_of(&Type::String));
    assert!(Type::I64.is_subtype_of(&Type::I64));
    assert!(Type::Bool.is_subtype_of(&Type::Bool));
    assert!(Type::Uuid.is_subtype_of(&Type::Uuid));
}

#[test]
fn different_types_are_not_subtypes() {
    assert!(!Type::String.is_subtype_of(&Type::I64));
    assert!(!Type::Bool.is_subtype_of(&Type::Bytes));
}

// ---------------------------------------------------------------------------
// Concrete type vs Union
// ---------------------------------------------------------------------------

#[test]
fn record_is_subtype_of_union_containing_record() {
    let record_ty = Type::Record(vec![Type::U64, Type::String, Type::U64, Type::Null]);
    let union_ty = make_union(vec![Type::I64, record_ty.clone()]);
    assert!(record_ty.is_subtype_of(&union_ty));
}

#[test]
fn i64_is_subtype_of_union_containing_i64() {
    let union_ty = make_union(vec![Type::I64, Type::Record(vec![Type::U64])]);
    assert!(Type::I64.is_subtype_of(&union_ty));
}

#[test]
fn string_is_not_subtype_of_union_without_string() {
    let union_ty = make_union(vec![Type::I64, Type::Record(vec![Type::U64])]);
    assert!(!Type::String.is_subtype_of(&union_ty));
}

// ---------------------------------------------------------------------------
// Union vs Union (subset check)
// ---------------------------------------------------------------------------

#[test]
fn union_subset_is_subtype() {
    let small = make_union(vec![Type::I64]);
    let big = make_union(vec![Type::I64, Type::Record(vec![Type::U64])]);
    assert!(small.is_subtype_of(&big));
}

#[test]
fn union_equal_is_subtype() {
    let a = make_union(vec![Type::I64, Type::Record(vec![Type::U64])]);
    let b = make_union(vec![Type::I64, Type::Record(vec![Type::U64])]);
    assert!(a.is_subtype_of(&b));
}

#[test]
fn union_superset_is_not_subtype() {
    let big = make_union(vec![Type::I64, Type::String]);
    let small = make_union(vec![Type::I64]);
    assert!(!big.is_subtype_of(&small));
}

// ---------------------------------------------------------------------------
// List recursion
// ---------------------------------------------------------------------------

#[test]
fn list_with_subtype_element() {
    let actual = Type::list(Type::I64);
    let expected = Type::list(make_union(vec![Type::I64, Type::String]));
    assert!(actual.is_subtype_of(&expected));
}

#[test]
fn list_element_mismatch_not_subtype() {
    let actual = Type::list(Type::Bool);
    let expected = Type::list(make_union(vec![Type::I64, Type::String]));
    assert!(!actual.is_subtype_of(&expected));
}

// ---------------------------------------------------------------------------
// Record recursion
// ---------------------------------------------------------------------------

#[test]
fn record_with_subtype_field() {
    let actual = Type::Record(vec![Type::U64, Type::I64]);
    let expected = Type::Record(vec![Type::U64, make_union(vec![Type::I64, Type::String])]);
    assert!(actual.is_subtype_of(&expected));
}

#[test]
fn record_field_mismatch_not_subtype() {
    let actual = Type::Record(vec![Type::U64, Type::Bool]);
    let expected = Type::Record(vec![Type::U64, make_union(vec![Type::I64, Type::String])]);
    assert!(!actual.is_subtype_of(&expected));
}

#[test]
fn record_length_mismatch_not_subtype() {
    let actual = Type::Record(vec![Type::U64]);
    let expected = Type::Record(vec![Type::U64, Type::I64]);
    assert!(!actual.is_subtype_of(&expected));
}
