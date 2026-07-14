use toasty_core::{Schema, stmt};

/// True when `expr` is an OR whose `IsVariant` operands cover every variant
/// of the same enum applied to the same anchor expression: the disjunction
/// is a tautology and the whole OR folds to `true`.
///
/// Pure on `(schema, expr)`; pulled out as a free function so the
/// lowering visitor and the unit tests can both call it without
/// constructing a full `LowerStatement`.
pub(super) fn is_variant_tautology_or(schema: &Schema, expr: &stmt::ExprOr) -> bool {
    // Find the first IsVariant to use as anchor.
    let Some(first) = expr.operands.iter().find_map(|op| match op {
        stmt::Expr::IsVariant(iv) => Some(iv),
        _ => None,
    }) else {
        return false;
    };

    let anchor_expr = &first.expr;
    let model_id = first.variant.model;
    let num_variants = schema
        .app
        .model(model_id)
        .as_embedded_enum_unwrap()
        .variants
        .len();

    let mut seen = bit_set::BitSet::with_capacity(num_variants);

    for operand in &expr.operands {
        let stmt::Expr::IsVariant(iv) = operand else {
            continue;
        };

        // Every `IsVariant` subject must be equivalent to the anchor:
        // two syntactically different (or non-deterministic) subjects
        // could disagree at runtime, so covering all variants of the
        // anchor tells us nothing about them.
        if !iv.expr.is_equivalent_to(anchor_expr) || iv.variant.model != model_id {
            return false;
        }

        seen.insert(iv.variant.index);
    }

    seen.count() == num_variants
}
