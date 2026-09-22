use crate as toasty;
use crate::{
    engine::{lower::relation_expr::resolve, test_util::test_schema_with},
    schema::{Embed, Model},
};
use toasty_core::{
    schema::app::VariantId,
    stmt::{Expr, ExprContext, Source},
};

#[derive(Debug, toasty::Embed)]
enum State {
    Active { value: i64 },
    Inactive,
}

#[derive(Debug, toasty::Model)]
struct Parent {
    #[key]
    id: i64,
    state: State,
}

#[derive(Debug, toasty::Embed)]
struct Link {
    parent_id: i64,
    #[belongs_to(key = parent_id, references = id)]
    parent: toasty::Deferred<Parent>,
}

#[derive(Debug, toasty::Embed)]
enum Owner {
    Linked { link: Link },
    Empty,
}

#[derive(Debug, toasty::Model)]
struct Object {
    #[key]
    id: i64,
    owner: Owner,
}

fn schema() -> toasty_core::Schema {
    test_schema_with(&[
        Object::schema(),
        Owner::schema(),
        Link::schema(),
        Parent::schema(),
        State::schema(),
    ])
}

fn owner() -> Expr {
    Expr::variant(
        Expr::ref_self_field(Object::id().field(1)),
        VariantId {
            model: Owner::id(),
            index: 0,
        },
    )
}

fn active(base: Expr) -> Expr {
    Expr::variant(
        base,
        VariantId {
            model: State::id(),
            index: 0,
        },
    )
}

#[test]
fn projection_segments_preserve_relation_and_target_variants() {
    let schema = schema();
    let source = Source::from(Object::id());
    let cx = ExprContext::new(&schema);
    let cx = cx.scope(&source);
    // Owner::Linked.link.parent.state, split at different projection boundaries.
    let paths = [
        Expr::project(owner(), [0, 1, 1]),
        Expr::project(Expr::project(owner(), [0]), [1, 1]),
        Expr::project(Expr::project(owner(), [0, 1]), [1]),
        Expr::project(Expr::project(Expr::project(owner(), [0]), [1]), [1]),
    ];
    let endpoint = Expr::project(owner(), [0, 1]);
    let relation = resolve(&cx, &endpoint).unwrap();
    assert!(relation.is_endpoint());
    for path in paths {
        let expr = Expr::project(active(path), [0]);
        let resolved = resolve(&cx, &expr).unwrap();
        assert_eq!(resolved.field.id, relation.field.id);
        assert_eq!(resolved.key_expr(), relation.key_expr());
        assert_eq!(resolved.target, Parent::id());
        assert_eq!(
            resolved.target_expr(),
            Some(Expr::project(
                active(Expr::ref_self_field(Parent::id().field(1))),
                [0]
            ))
        );
    }
}

#[test]
fn relation_endpoint_and_invalid_variant_selection() {
    let schema = schema();
    let source = Source::from(Object::id());
    let cx = ExprContext::new(&schema);
    let cx = cx.scope(&source);
    let relation = Expr::project(owner(), [0, 1]);
    let identity = Expr::project(relation.clone(), &[][..]);
    let resolved = resolve(&cx, &identity).unwrap();
    assert_eq!(
        resolved.key_expr(),
        Some(Expr::project(Expr::project(owner(), [0]), [0]))
    );
    assert!(resolved.is_endpoint());
    assert!(resolved.target_expr().is_none());
    // The relation targets a model, so selecting a variant before a field is invalid.
    assert!(resolve(&cx, &active(relation)).is_none());
    // An enum's fields require a variant selection before projection.
    assert!(
        resolve(
            &cx,
            &Expr::project(Expr::ref_self_field(Object::id().field(1)), [0, 1])
        )
        .is_none()
    );
}
