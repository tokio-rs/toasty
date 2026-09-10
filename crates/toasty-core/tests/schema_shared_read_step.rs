use toasty_core::schema::Name;
use toasty_core::schema::app::*;
use toasty_core::stmt;

const ENUM: ModelId = ModelId(0);

fn prim_field(index: usize, name: &str, variant_index: usize, shared: Option<Name>) -> Field {
    Field {
        id: ENUM.field(index),
        name: FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty: FieldTy::Primitive(FieldPrimitive {
            ty: stmt::Type::String,
            storage_ty: None,
            serialize: None,
        }),
        nullable: false,
        primary_key: false,
        auto: None,
        versionable: false,
        deferred: false,
        constraints: vec![],
        variant: Some(VariantId {
            model: ENUM,
            index: variant_index,
        }),
        shared,
    }
}

/// `Alpha { label(shared), kind }, Beta` — one data variant spans every
/// flattened field, so its record positions reach `fields.len()`. This is the
/// shape where a shared step offset of `fields.len() + index` collided with a
/// variant-gated read's record position.
fn degenerate_enum() -> EmbeddedEnum {
    EmbeddedEnum {
        id: ENUM,
        name: Name::new("Probe"),
        discriminant: FieldPrimitive {
            ty: stmt::Type::I64,
            storage_ty: None,
            serialize: None,
        },
        variants: vec![
            EnumVariant {
                name: Name::new("Alpha"),
                discriminant: stmt::Value::I64(0),
            },
            EnumVariant {
                name: Name::new("Beta"),
                discriminant: stmt::Value::I64(1),
            },
        ],
        fields: vec![
            prim_field(0, "label", 0, Some(Name::new("label"))),
            prim_field(1, "kind", 0, None),
        ],
        indices: vec![],
    }
}

/// `Alpha { label(shared), kind }, Beta { extra }` — every variant's record
/// stays strictly below `fields.len()`.
fn balanced_enum() -> EmbeddedEnum {
    EmbeddedEnum {
        id: ENUM,
        name: Name::new("Probe"),
        discriminant: FieldPrimitive {
            ty: stmt::Type::I64,
            storage_ty: None,
            serialize: None,
        },
        variants: vec![
            EnumVariant {
                name: Name::new("Alpha"),
                discriminant: stmt::Value::I64(0),
            },
            EnumVariant {
                name: Name::new("Beta"),
                discriminant: stmt::Value::I64(1),
            },
        ],
        fields: vec![
            prim_field(0, "label", 0, Some(Name::new("label"))),
            prim_field(1, "kind", 0, None),
            prim_field(2, "extra", 1, None),
        ],
        indices: vec![],
    }
}

/// `Alpha { label(shared) }, B, C, D` — many unit siblings push the variant
/// index range past `fields.len()`. This is the shape where a shared step
/// offset of `fields.len() + 1 + index` collided with a discriminant access.
fn unit_heavy_enum() -> EmbeddedEnum {
    EmbeddedEnum {
        id: ENUM,
        name: Name::new("Probe"),
        discriminant: FieldPrimitive {
            ty: stmt::Type::I64,
            storage_ty: None,
            serialize: None,
        },
        variants: vec![
            EnumVariant {
                name: Name::new("Alpha"),
                discriminant: stmt::Value::I64(0),
            },
            EnumVariant {
                name: Name::new("Beta"),
                discriminant: stmt::Value::I64(1),
            },
            EnumVariant {
                name: Name::new("Gamma"),
                discriminant: stmt::Value::I64(2),
            },
            EnumVariant {
                name: Name::new("Delta"),
                discriminant: stmt::Value::I64(3),
            },
        ],
        fields: vec![prim_field(0, "label", 0, Some(Name::new("label")))],
        indices: vec![],
    }
}

/// A record position — the trailing step a variant-gated read projects — must
/// never decode as the gateless shared read, in any variant shape. The shared
/// accessor offsets its step past both `fields.len()` (the largest reachable
/// record position) and the variant index range; `shared_read_at_step` must
/// not fire below that.
#[test]
fn shared_read_step_never_claims_record_positions() {
    for e in [degenerate_enum(), balanced_enum(), unit_heavy_enum()] {
        let base = EmbeddedEnum::shared_step_base(e.fields.len(), e.variants.len());
        for position in 0..base {
            assert!(
                e.shared_read_at_step(position).is_none(),
                "record position {position} decoded as a shared read"
            );
        }
    }
}

/// A discriminant access — a single step naming a variant index — must never
/// decode as the gateless shared read. The shared base sits at or above the
/// variant count, so the two encodings stay disjoint even when unit siblings
/// outnumber the flattened fields.
#[test]
fn shared_read_step_never_claims_variant_indices() {
    for e in [degenerate_enum(), balanced_enum(), unit_heavy_enum()] {
        for variant_index in 0..e.variants.len() {
            // A bare variant index only resolves as shared when it coincides
            // with an actual shared step; with the offset above the variant
            // range that never happens.
            if let Some((_, field)) = e.shared_read_at_step(variant_index) {
                panic!(
                    "variant index {variant_index} decoded as shared field {}",
                    field.name.app.as_deref().unwrap_or("<unnamed>")
                );
            }
        }
    }
}

/// The generated accessor's step — `shared_step_base(len, variant_count) +
/// flattened index` — resolves to the shared field it names, and only to
/// shared fields.
#[test]
fn shared_read_step_resolves_shared_fields() {
    for e in [degenerate_enum(), balanced_enum(), unit_heavy_enum()] {
        let base = EmbeddedEnum::shared_step_base(e.fields.len(), e.variants.len());

        let (index, field) = e.shared_read_at_step(base).unwrap();
        assert_eq!(index, 0);
        assert_eq!(field.name.app.as_deref(), Some("label"));
        assert!(field.shared.is_some());

        // The flattened field at index 1 is not shared: no step may decode to
        // it through the shared read.
        assert!(e.shared_read_at_step(base + 1).is_none());
    }
}
