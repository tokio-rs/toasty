use toasty_core::{
    schema::app::{ModelId, VariantId},
    stmt::{Expr, Path, PathStep},
};

#[test]
fn nested_variant_steps_survive_expression_construction() {
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

    let steps = [
        PathStep::Field(3),
        PathStep::Variant(outer),
        PathStep::Field(0),
        PathStep::Variant(inner),
        PathStep::Field(1),
    ];
    assert_eq!(path.steps(), steps);
    let Expr::Path(expr) = path.into_stmt() else {
        panic!("expected application path");
    };
    assert_eq!(*expr.base, Expr::ref_self_field(root.field(3)));
    assert_eq!(expr.steps, steps[1..]);
}

#[test]
fn chaining_preserves_variant_steps_from_both_paths() {
    let root = ModelId(0);
    let variant = VariantId {
        model: ModelId(1),
        index: 0,
    };
    let mut lhs = Path::field(root, 2);
    let mut rhs = Path::from_variant(Path::model(variant.model), variant);
    rhs.chain(&Path::field(variant.model, 0));
    lhs.chain(&rhs);

    assert_eq!(
        lhs.steps(),
        [
            PathStep::Field(2),
            PathStep::Variant(variant),
            PathStep::Field(0)
        ]
    );
    assert!(lhs.field_projection().is_none());
}
