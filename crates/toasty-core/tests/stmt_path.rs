use toasty_core::{
    schema::app::{ModelId, VariantId},
    stmt::{Expr, Path, PathRoot},
};

#[test]
fn nested_variant_roots_convert_to_nested_selections() {
    let root = ModelId(0);
    let outer = VariantId {
        model: ModelId(1),
        index: 2,
    };
    let inner = VariantId {
        model: ModelId(2),
        index: 0,
    };
    let mut path = Path::from_variant(Path::field(root, 3), outer);
    path.chain(&Path::field(outer.model, 0));
    let mut path = Path::from_variant(path, inner);
    path.chain(&Path::field(inner.model, 1));

    // Each variant root becomes a selection over its parent's expression,
    // projected by the variant-local steps that follow it.
    let expected = Expr::project(
        Expr::variant(
            Expr::project(
                Expr::variant(Expr::ref_self_field(root.field(3)), outer),
                [0],
            ),
            inner,
        ),
        [1],
    );
    assert_eq!(path.into_stmt(), expected);
}

#[test]
fn variant_root_without_steps_converts_to_bare_selection() {
    let variant = VariantId {
        model: ModelId(1),
        index: 1,
    };
    let path = Path::from_variant(Path::field(ModelId(0), 2), variant);

    assert_eq!(
        path.into_stmt(),
        Expr::variant(Expr::ref_self_field(ModelId(0).field(2)), variant)
    );
}

#[test]
fn chaining_preserves_variant_root_of_chained_path() {
    let root = ModelId(0);
    let variant = VariantId {
        model: ModelId(1),
        index: 0,
    };
    let mut lhs = Path::field(root, 2);
    let mut rhs = Path::from_variant(Path::model(variant.model), variant);
    rhs.chain(&Path::field(variant.model, 0));
    lhs.chain(&rhs);

    // The chained path selects the variant at the point `lhs` reaches the
    // enum, then continues with `rhs`'s steps.
    let PathRoot::Variant { parent, variant_id } = &lhs.root else {
        panic!("expected a variant root, got {:?}", lhs.root);
    };
    assert_eq!(variant_id, &variant);
    assert_eq!(**parent, Path::field(root, 2));
    assert_eq!(lhs.projection.as_slice(), [0]);
    assert_eq!(
        lhs.into_stmt(),
        Expr::project(
            Expr::variant(Expr::ref_self_field(root.field(2)), variant),
            [0]
        )
    );
}

#[test]
fn chaining_onto_variant_root_appends_steps() {
    let variant = VariantId {
        model: ModelId(1),
        index: 0,
    };
    let mut path = Path::from_variant(Path::field(ModelId(0), 2), variant);
    path.chain(&Path::field(variant.model, 1));
    path.chain(&Path::field(ModelId(3), 4));

    assert_eq!(path.projection.as_slice(), [1, 4]);
}
