use toasty_core::stmt::*;

// Tests for basic type inference functionality that doesn't require schema access
#[test]
fn test_type_unknown_exists() {
    // Just test that the Unknown variant exists and can be used
    let unknown = Type::Unknown;
    assert_eq!(unknown, Type::Unknown);

    // Test that it can be compared with other types
    assert_ne!(unknown, Type::Bool);
    assert_ne!(unknown, Type::I32);
    assert_ne!(unknown, Type::String);
}

#[test]
fn test_basic_type_variants() {
    // Test that all the basic types exist and work
    let types = vec![
        Type::Bool,
        Type::I8,
        Type::I16,
        Type::I32,
        Type::I64,
        Type::U8,
        Type::U16,
        Type::U32,
        Type::U64,
        Type::String,
        Type::Null,
        Type::Unknown,
    ];

    // Each type should equal itself
    for ty in &types {
        assert_eq!(ty, ty);
    }

    // Different types should not equal each other
    for (i, ty1) in types.iter().enumerate() {
        for (j, ty2) in types.iter().enumerate() {
            if i != j {
                assert_ne!(ty1, ty2);
            }
        }
    }
}

#[test]
fn test_complex_type_construction() {
    // Test List type construction
    let list_i32 = Type::List(Box::new(Type::I32));
    let list_string = Type::List(Box::new(Type::String));

    assert_ne!(list_i32, list_string);
    assert_eq!(list_i32, Type::List(Box::new(Type::I32)));

    // Test Record type construction
    let record1 = Type::Record(vec![Type::I32, Type::String]);
    let record2 = Type::Record(vec![Type::I32, Type::Bool]);
    let record3 = Type::Record(vec![Type::I32, Type::String]);

    assert_ne!(record1, record2);
    assert_eq!(record1, record3);

    // Test nested types
    let nested_list = Type::List(Box::new(Type::Record(vec![Type::I32, Type::String])));
    let nested_record = Type::Record(vec![Type::List(Box::new(Type::I32)), Type::String]);

    assert_ne!(nested_list, nested_record);
}
