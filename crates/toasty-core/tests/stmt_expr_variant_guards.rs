use toasty_core::{
    schema::app::{ModelId, VariantId},
    stmt::{Expr, Query, Source},
};

fn variant(index: usize) -> VariantId {
    VariantId {
        model: ModelId(1),
        index,
    }
}

fn field(index: usize) -> Expr {
    Expr::ref_self_field(ModelId(0).field(index))
}

#[test]
fn predicate_without_selection_is_unchanged() {
    let predicate = Expr::eq(field(0), 1);
    assert_eq!(predicate.clone().with_variant_guards(), predicate);
}

#[test]
fn guards_are_collected_from_both_operands() {
    let lhs = Expr::project(Expr::variant(field(0), variant(0)), [1]);
    let rhs = Expr::project(Expr::variant(field(1), variant(0)), [1]);
    let predicate = Expr::ne(lhs, rhs);

    assert_eq!(
        predicate.clone().with_variant_guards(),
        Expr::and_from_vec(vec![
            Expr::is_variant(field(0), variant(0)),
            Expr::is_variant(field(1), variant(0)),
            predicate,
        ])
    );
}

#[test]
fn identical_selections_guard_once() {
    let selection = || Expr::project(Expr::variant(field(0), variant(0)), [1]);
    let predicate = Expr::eq(selection(), selection());

    assert_eq!(
        predicate.clone().with_variant_guards(),
        Expr::and_from_vec(vec![Expr::is_variant(field(0), variant(0)), predicate])
    );
}

#[test]
fn nested_selections_guard_every_enclosing_variant() {
    let inner = VariantId {
        model: ModelId(2),
        index: 1,
    };
    let outer_selection = Expr::project(Expr::variant(field(0), variant(2)), [0]);
    let predicate = Expr::is_null(Expr::project(
        Expr::variant(outer_selection.clone(), inner),
        [1],
    ));

    // Outermost variant first: the inner check is only meaningful once the
    // outer variant holds.
    assert_eq!(
        predicate.clone().with_variant_guards(),
        Expr::and_from_vec(vec![
            Expr::is_variant(field(0), variant(2)),
            Expr::is_variant(outer_selection, inner),
            predicate,
        ])
    );
}

#[test]
fn negation_wraps_the_guarded_predicate() {
    let predicate = Expr::eq(Expr::project(Expr::variant(field(0), variant(0)), [1]), 1);
    let guarded = predicate.clone().with_variant_guards();

    // Guards are fixed when the predicate is built, so a later negation
    // negates the guarded predicate as a whole rather than the comparison
    // alone.
    assert_eq!(
        Expr::not(guarded.clone()),
        Expr::not(Expr::and(Expr::is_variant(field(0), variant(0)), predicate))
    );
    // Re-guarding a negated predicate adds nothing: the guard already inside
    // is found again and deduplicated against itself only when hoisted, so
    // the walk descends through `Not` but stops at the inner `And`.
    assert_eq!(
        Expr::not(guarded.clone()).with_variant_guards(),
        Expr::not(guarded)
    );
}

#[test]
fn conjunction_operands_and_subqueries_keep_their_own_guards() {
    let selection = Expr::project(Expr::variant(field(0), variant(0)), [1]);
    let inner = Expr::eq(selection.clone(), 1).with_variant_guards();

    // Operands of `And` / `Or` are predicates that were guarded when built.
    assert_eq!(
        Expr::and(inner.clone(), true).with_variant_guards(),
        Expr::and(inner.clone(), true)
    );
    assert_eq!(
        Expr::or(inner.clone(), true).with_variant_guards(),
        Expr::or(inner, true)
    );

    // A selection inside a subquery belongs to that query's scope.
    let subquery = Query::new_select(Source::from(ModelId(0)), Expr::eq(selection, 1));
    let membership = Expr::in_subquery(field(2), subquery);
    assert_eq!(membership.clone().with_variant_guards(), membership);
}
