use toasty_core::schema::mapping::{Bijection, CoproductArm, StorageOp};
use toasty_core::stmt;

#[test]
fn identity_column_count() {
    assert_eq!(Bijection::Identity.column_count(), 1);
}

#[test]
fn cast_column_count() {
    let b = Bijection::Cast {
        from: stmt::Type::Uuid,
        to: stmt::Type::String,
    };
    assert_eq!(b.column_count(), 1);
}

#[test]
fn nullable_column_count() {
    let b = Bijection::Nullable(Box::new(Bijection::Identity));
    assert_eq!(b.column_count(), 1);
}

#[test]
fn product_column_count() {
    let b = Bijection::Product(vec![
        Bijection::Identity,
        Bijection::Identity,
        Bijection::Identity,
    ]);
    assert_eq!(b.column_count(), 3);
}

#[test]
fn coproduct_column_count() {
    // Enum with 2 variants, each with 1 data field
    let b = Bijection::Coproduct {
        discriminant: Box::new(Bijection::Identity),
        variants: vec![
            CoproductArm {
                discriminant_value: 0,
                body: Bijection::Identity,
            },
            CoproductArm {
                discriminant_value: 1,
                body: Bijection::Identity,
            },
        ],
    };
    // 1 (disc) + 1 + 1 = 3
    assert_eq!(b.column_count(), 3);
}

#[test]
fn identity_encode() {
    let v = stmt::Value::String("hello".to_string());
    assert_eq!(Bijection::Identity.encode(v.clone()), v);
}

#[test]
fn cast_encode_uuid_to_string() {
    let uuid = uuid::Uuid::new_v4();
    let b = Bijection::Cast {
        from: stmt::Type::Uuid,
        to: stmt::Type::String,
    };
    let result = b.encode(stmt::Value::Uuid(uuid));
    assert_eq!(result, stmt::Value::String(uuid.to_string()));
}

#[test]
fn nullable_encode_some() {
    let b = Bijection::Nullable(Box::new(Bijection::Cast {
        from: stmt::Type::Uuid,
        to: stmt::Type::String,
    }));
    let uuid = uuid::Uuid::new_v4();
    let result = b.encode(stmt::Value::Uuid(uuid));
    assert_eq!(result, stmt::Value::String(uuid.to_string()));
}

#[test]
fn nullable_encode_none() {
    let b = Bijection::Nullable(Box::new(Bijection::Identity));
    assert_eq!(b.encode(stmt::Value::Null), stmt::Value::Null);
}

#[test]
fn product_encode() {
    let b = Bijection::Product(vec![Bijection::Identity, Bijection::Identity]);
    let v = stmt::Value::Record(stmt::ValueRecord::from_vec(vec![
        stmt::Value::String("a".to_string()),
        stmt::Value::I64(42),
    ]));
    // Identity product should pass through
    assert_eq!(b.encode(v.clone()), v);
}

// can_distribute tests

#[test]
fn identity_distributes_all_ops() {
    let b = Bijection::Identity;
    assert_eq!(b.can_distribute(stmt::BinaryOp::Eq), Some(StorageOp::Eq));
    assert_eq!(b.can_distribute(stmt::BinaryOp::Lt), Some(StorageOp::Lt));
    assert_eq!(b.can_distribute(stmt::BinaryOp::Ge), Some(StorageOp::Ge));
}

#[test]
fn cast_uuid_string_distributes_eq_not_lt() {
    let b = Bijection::Cast {
        from: stmt::Type::Uuid,
        to: stmt::Type::String,
    };
    assert_eq!(b.can_distribute(stmt::BinaryOp::Eq), Some(StorageOp::Eq));
    assert_eq!(b.can_distribute(stmt::BinaryOp::Lt), None);
}

#[test]
fn cast_integer_widening_distributes_all() {
    let b = Bijection::Cast {
        from: stmt::Type::I32,
        to: stmt::Type::I64,
    };
    assert_eq!(b.can_distribute(stmt::BinaryOp::Eq), Some(StorageOp::Eq));
    assert_eq!(b.can_distribute(stmt::BinaryOp::Lt), Some(StorageOp::Lt));
}

#[test]
fn nullable_distributes_eq_as_null_safe() {
    let b = Bijection::Nullable(Box::new(Bijection::Identity));
    assert_eq!(
        b.can_distribute(stmt::BinaryOp::Eq),
        Some(StorageOp::IsNullSafe)
    );
}

#[test]
fn nullable_cast_distributes_eq_as_null_safe() {
    let b = Bijection::Nullable(Box::new(Bijection::Cast {
        from: stmt::Type::Uuid,
        to: stmt::Type::String,
    }));
    assert_eq!(
        b.can_distribute(stmt::BinaryOp::Eq),
        Some(StorageOp::IsNullSafe)
    );
}

#[test]
fn product_distributes_eq() {
    let b = Bijection::Product(vec![Bijection::Identity, Bijection::Identity]);
    assert_eq!(b.can_distribute(stmt::BinaryOp::Eq), Some(StorageOp::Eq));
}

#[test]
fn product_does_not_distribute_lt() {
    let b = Bijection::Product(vec![Bijection::Identity, Bijection::Identity]);
    assert_eq!(b.can_distribute(stmt::BinaryOp::Lt), None);
}

#[test]
fn coproduct_distributes_eq() {
    let b = Bijection::Coproduct {
        discriminant: Box::new(Bijection::Identity),
        variants: vec![
            CoproductArm {
                discriminant_value: 0,
                body: Bijection::Identity,
            },
            CoproductArm {
                discriminant_value: 1,
                body: Bijection::Product(vec![Bijection::Identity]),
            },
        ],
    };
    assert_eq!(b.can_distribute(stmt::BinaryOp::Eq), Some(StorageOp::Eq));
}

#[test]
fn coproduct_does_not_distribute_lt() {
    let b = Bijection::Coproduct {
        discriminant: Box::new(Bijection::Identity),
        variants: vec![CoproductArm {
            discriminant_value: 0,
            body: Bijection::Identity,
        }],
    };
    assert_eq!(b.can_distribute(stmt::BinaryOp::Lt), None);
}

#[cfg(feature = "jiff")]
#[test]
fn cast_timestamp_string_distributes_lt() {
    let b = Bijection::Cast {
        from: stmt::Type::Timestamp,
        to: stmt::Type::String,
    };
    assert_eq!(b.can_distribute(stmt::BinaryOp::Lt), Some(StorageOp::Lt));
}
