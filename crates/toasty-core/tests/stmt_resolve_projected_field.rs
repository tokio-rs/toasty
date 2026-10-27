use toasty_core::{
    Schema,
    driver::Capability,
    schema::{Builder, Name, app::*, db},
    stmt::{Expr, ExprContext, ExprReference, ExprTarget, Path, Type},
};

const USER: ModelId = ModelId(0);
const POST: ModelId = ModelId(1);
const ADDRESS: ModelId = ModelId(2);
const CONTACT: ModelId = ModelId(3);
const POSTAL: VariantId = VariantId {
    model: CONTACT,
    index: 1,
};

fn primitive() -> FieldTy {
    FieldTy::Primitive(FieldPrimitive {
        ty: Type::String,
        storage_ty: None,
        serialize: None,
    })
}

fn embedded(target: ModelId) -> FieldTy {
    FieldTy::Embedded(Embedded {
        target,
        expr_ty: Type::Model(target),
        storage_ty: None,
    })
}

fn field(model: ModelId, index: usize, name: &str, ty: FieldTy) -> Field {
    Field {
        id: model.field(index),
        name: FieldName {
            app: Some(name.to_string()),
            storage: None,
        },
        ty,
        nullable: false,
        primary_key: false,
        auto: None,
        versionable: false,
        deferred: false,
        constraints: vec![],
        variant: None,
        shared: None,
    }
}

fn root(id: ModelId, name: &str) -> Model {
    let mut key = field(id, 0, "id", primitive());
    key.primary_key = true;
    let index = IndexId {
        model: id,
        index: 0,
    };
    Model::Root(ModelRoot {
        id,
        name: Name::new(name),
        fields: vec![
            key,
            field(id, 1, "address", embedded(ADDRESS)),
            field(id, 2, "contact", embedded(CONTACT)),
        ],
        primary_key: PrimaryKey {
            fields: vec![id.field(0)],
            index,
        },
        table_name: name.to_string(),
        indices: vec![Index {
            id: index,
            name: None,
            fields: vec![IndexField {
                field: id.field(0),
                op: db::IndexOp::Eq,
                scope: db::IndexScope::Partition,
            }],
            unique: true,
            primary_key: true,
        }],
        version_field: None,
    })
}

fn schema() -> Schema {
    let address = Model::EmbeddedStruct(EmbeddedStruct {
        id: ADDRESS,
        name: Name::new("Address"),
        fields: vec![field(ADDRESS, 0, "street", primitive())],
        indices: vec![],
    });
    let mut email = field(CONTACT, 0, "email", primitive());
    email.variant = Some(VariantId {
        model: CONTACT,
        index: 0,
    });
    let mut postal = field(CONTACT, 1, "address", embedded(ADDRESS));
    postal.variant = Some(POSTAL);
    let contact = Model::EmbeddedEnum(EmbeddedEnum {
        id: CONTACT,
        name: Name::new("Contact"),
        discriminant: FieldPrimitive {
            ty: Type::I64,
            storage_ty: None,
            serialize: None,
        },
        variants: vec![
            EnumVariant {
                name: Name::new("Email"),
                discriminant: 0i64.into(),
                field_range: 0..1,
            },
            EnumVariant {
                name: Name::new("Postal"),
                discriminant: 1i64.into(),
                field_range: 1..2,
            },
        ],
        fields: vec![email, postal],
        indices: vec![],
    });
    let app = toasty_core::schema::app::Schema::from_macro([
        root(USER, "User"),
        root(POST, "Post"),
        address,
        contact,
    ])
    .unwrap();
    Builder::new().build(app, &Capability::SQLITE).unwrap()
}

fn reference(nesting: usize, index: usize) -> Expr {
    Expr::Reference(ExprReference::Field { nesting, index })
}

#[test]
fn field_references_resolve_in_their_scope() {
    let schema = schema();
    let user = schema.app.model(USER).as_root_unwrap();
    let post = schema.app.model(POST).as_root_unwrap();
    let outer = ExprContext::new_with_target(&schema, user);
    let middle = outer.scope(post);
    let inner = middle.scope(user);

    for (nesting, model) in [(0, USER), (1, POST), (2, USER)] {
        let resolved = inner
            .resolve_projected_field(&reference(nesting, 0))
            .unwrap();
        assert_eq!(resolved.field.id, model.field(0));
        assert_eq!(resolved.path, Path::field(model, 0));
        assert_eq!(resolved.nesting, nesting);
        assert!(std::ptr::eq(
            resolved.mapping,
            &schema.mapping_for(model).fields[0]
        ));
    }
}

#[test]
fn embedded_fields_keep_the_root_mapping_and_scope() {
    let schema = schema();
    let user = schema.app.model(USER).as_root_unwrap();
    let post = schema.app.model(POST).as_root_unwrap();
    let outer = ExprContext::new_with_target(&schema, user);
    let middle = outer.scope(post);
    let inner = middle.scope(user);

    for (nesting, model) in [(0, USER), (1, POST), (2, USER)] {
        let expr = Expr::project(reference(nesting, 1), [0]);
        let resolved = inner.resolve_projected_field(&expr).unwrap();
        let mut path = Path::field(model, 1);
        path.projection.push(0);
        assert_eq!(resolved.field.id, ADDRESS.field(0));
        assert_eq!(resolved.path, path);
        assert_eq!(resolved.nesting, nesting);
        assert!(std::ptr::eq(
            resolved.mapping,
            &schema.mapping_for(model).fields[1]
                .as_struct()
                .unwrap()
                .fields[0]
        ));
    }
}

#[test]
fn outer_variant_fields_use_local_indexes() {
    let schema = schema();
    let user = schema.app.model(USER).as_root_unwrap();
    let post = schema.app.model(POST).as_root_unwrap();
    let outer = ExprContext::new_with_target(&schema, user);
    let middle = outer.scope(post);
    let inner = middle.scope(ExprTarget::Free);
    let variant = Expr::variant(reference(2, 2), POSTAL);
    let mapping = schema.mapping_for(USER).fields[2].as_enum().unwrap();
    let mut path = Path::from_variant(Path::field(USER, 2), POSTAL);
    path.projection.push(0);

    let resolved = inner
        .resolve_projected_field(&Expr::project(variant.clone(), [0]))
        .unwrap();
    assert_eq!(resolved.field.id, CONTACT.field(1));
    assert_eq!(resolved.path, path);
    assert_eq!(resolved.nesting, 2);
    assert!(std::ptr::eq(
        resolved.mapping,
        &mapping.variants[1].fields[0]
    ));

    path.projection.push(0);
    for expr in [
        Expr::project(variant.clone(), [0, 0]),
        Expr::project(Expr::project(variant, [0]), [0]),
    ] {
        let resolved = inner.resolve_projected_field(&expr).unwrap();
        assert_eq!(resolved.field.id, ADDRESS.field(0));
        assert_eq!(resolved.path, path);
        assert_eq!(resolved.nesting, 2);
        assert!(std::ptr::eq(
            resolved.mapping,
            &mapping.variants[1].fields[0].as_struct().unwrap().fields[0]
        ));
    }
}

#[test]
fn unsupported_expressions_and_invalid_projections_return_none() {
    let schema = schema();
    let cx = ExprContext::new_with_target(&schema, schema.app.model(USER).as_root_unwrap());
    for expr in [
        Expr::from(1i64),
        reference(0, 99),
        Expr::project(reference(0, 0), [0]),
        Expr::project(reference(0, 1), [99]),
        Expr::project(reference(0, 2), [0]),
        Expr::variant(reference(0, 2), POSTAL),
        Expr::project(Expr::variant(reference(0, 1), POSTAL), [0]),
        Expr::project(Expr::variant(reference(0, 2), POSTAL), [99]),
        Expr::project(
            Expr::variant(
                reference(0, 2),
                VariantId {
                    index: 99,
                    ..POSTAL
                },
            ),
            [0],
        ),
    ] {
        assert!(cx.resolve_projected_field(&expr).is_none(), "{expr:?}");
    }
    assert!(
        cx.scope(ExprTarget::Free)
            .resolve_projected_field(&reference(0, 0))
            .is_none()
    );
}
