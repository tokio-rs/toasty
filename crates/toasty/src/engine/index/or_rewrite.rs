use toasty_core::stmt;

/// Converts an index filter to canonical `ANY(MAP(...))` form for backends that do
/// not support OR in key conditions (e.g. DynamoDB).
///
/// Steps:
///   1. Flatten the expression to Disjunctive Normal Form (DNF) — a flat list of OR
///      branches, each branch being a single predicate or AND of predicates.
///   2. Group branches by their structural shape (predicate with literal values
///      replaced by `arg(i)`).
///   3. Unify each same-shape group into `ANY(MAP(Value::List([v1, v2, ...]), shape))`.
pub(super) fn index_filter_to_any_map(expr: stmt::Expr) -> stmt::Expr {
    let branches = flatten_to_dnf(expr);
    unify_dnf_branches(branches)
}

/// Flatten an expression to a list of OR branches in Disjunctive Normal Form
/// using an iterative work queue.
///
/// - `Or`: extend the queue with each operand.
/// - `And`: find the first `Or` operand and distribute over its branches,
///   re-queuing each resulting `And` for further processing. When no `Or`
///   operands remain the `And` is in final form.
/// - Leaf predicate: emit directly as a final branch.
fn flatten_to_dnf(expr: stmt::Expr) -> Vec<stmt::Expr> {
    let mut branches: Vec<stmt::Expr> = Vec::new();
    let mut queue: Vec<stmt::Expr> = vec![expr];

    while let Some(expr) = queue.pop() {
        match expr {
            stmt::Expr::Or(or) => queue.extend(or.operands.into_iter().rev()),
            stmt::Expr::And(and) => process_and(and, &mut queue, &mut branches),
            leaf => branches.push(leaf),
        }
    }

    // Validate that no branch contains an Or inside an Any(Map(...)). If it
    // does, the distribution logic above has a bug.
    for branch in &branches {
        assert_no_or_in_any(branch);
    }

    branches
}

/// Process one `And` expression from the DNF work queue.
///
/// Priority:
///   1. If any operand is `Or`, distribute AND over it and re-queue each branch.
///   2. If any operand is `Any(Map(...))`, distribute the remaining operands
///      into the map predicate and re-queue the resulting `Any`.
///   3. No `Or` or `Any` operands: emit as a final DNF conjunction.
fn process_and(
    and: stmt::ExprAnd,
    queue: &mut Vec<stmt::Expr>,
    branches: &mut Vec<stmt::Expr>,
) {
    if let Some(pos) = and.operands.iter().position(|op| matches!(op, stmt::Expr::Or(_))) {
        return distribute_over_or(and, pos, queue);
    }

    if let Some(pos) = and.operands.iter().position(|op| matches!(op, stmt::Expr::Any(_))) {
        return distribute_into_any(and, pos, queue);
    }

    branches.push(stmt::Expr::And(and));
}

/// Distribute AND over an `Or` operand at `pos`, re-queuing one `And` per branch.
///
/// `(p AND (a OR b) AND q)` → `(p AND a AND q)` and `(p AND b AND q)` on the queue.
fn distribute_over_or(and: stmt::ExprAnd, pos: usize, queue: &mut Vec<stmt::Expr>) {
    let mut operands = and.operands;
    let stmt::Expr::Or(or) = operands.remove(pos) else {
        unreachable!()
    };

    for branch in or.operands.into_iter().rev() {
        let mut new_operands = operands.clone();
        new_operands.insert(pos, branch);
        queue.push(stmt::ExprAnd { operands: new_operands }.into());
    }
}

/// Distribute the non-`Any` operands of `and` into an `Any(Map(...))` at `pos`.
///
/// `AND(p, ANY(MAP(base, pred)))` → `ANY(MAP(base, AND(pred, p)))`.
///
/// This is valid because the non-Any operands do not reference the map's arg variable.
fn distribute_into_any(and: stmt::ExprAnd, pos: usize, queue: &mut Vec<stmt::Expr>) {
    let mut operands = and.operands;
    let stmt::Expr::Any(any) = operands.remove(pos) else {
        unreachable!()
    };
    let stmt::Expr::Map(map) = *any.expr else {
        todo!("Any with non-Map expr in AND distribution");
    };

    // Keep the original map predicate first, then the distributed And operands.
    let mut inner_operands = vec![*map.map];
    inner_operands.extend(operands);
    let inner: stmt::Expr = if inner_operands.len() == 1 {
        inner_operands.into_iter().next().unwrap()
    } else {
        stmt::ExprAnd { operands: inner_operands }.into()
    };

    queue.push(
        stmt::ExprAny {
            expr: Box::new(stmt::Expr::Map(stmt::ExprMap {
                base: map.base,
                map: Box::new(inner),
            })),
        }
        .into(),
    );
}

/// Group DNF branches by shape; unify each group into `ANY(MAP(...))`.
/// If there is only a single branch (no OR), returns it unchanged.
fn unify_dnf_branches(branches: Vec<stmt::Expr>) -> stmt::Expr {
    if branches.len() == 1 {
        return branches.into_iter().next().unwrap();
    }

    // Each group: (shape, per-branch scalar-or-record values).
    let mut groups: Vec<(stmt::Expr, Vec<stmt::Value>)> = vec![];

    for branch in branches {
        let (shape, value) = extract_shape(branch);
        if let Some((_, values)) = groups.iter_mut().find(|(s, _)| *s == shape) {
            values.push(value);
        } else {
            groups.push((shape, vec![value]));
        }
    }

    if groups.len() > 1 {
        todo!(
            "OR index filter with multiple distinct branch shapes is not yet implemented; \
             shapes: {:#?}",
            groups.iter().map(|(s, _)| s).collect::<Vec<_>>()
        );
    }

    let (shape, values) = groups.into_iter().next().unwrap();

    stmt::Expr::any(stmt::Expr::map(
        stmt::Expr::Value(stmt::Value::List(values)),
        shape,
    ))
}

/// Extract the per-call predicate template (shape) and single value for one DNF branch.
///
/// - `col op literal` → shape `col op arg(0)`, value `literal`
/// - `col1 op1 v1 AND col2 op2 v2 AND ...` → shape with `arg(i)` per column,
///   value `Value::Record([v1, v2, ...])` — composite key fan-out (TODO)
fn extract_shape(branch: stmt::Expr) -> (stmt::Expr, stmt::Value) {
    match branch {
        stmt::Expr::BinaryOp(b) => {
            let stmt::Expr::Value(v) = *b.rhs else {
                todo!("non-literal value in OR branch rhs: {:#?}", b.rhs);
            };
            let shape: stmt::Expr = stmt::ExprBinaryOp {
                lhs: b.lhs,
                op: b.op,
                rhs: Box::new(stmt::Expr::arg(0)),
            }
            .into();
            (shape, v)
        }
        // Composite key: (col1 = t1 AND col2 >= s1) OR (col1 = t2 AND col2 >= s2)
        // → ANY(MAP([(t1,s1),(t2,s2)], col1=arg(0) AND col2>=arg(1)))
        stmt::Expr::And(_) => {
            todo!("composite-key AND branch in OR index filter fan-out");
        }
        _ => todo!("unsupported branch type in OR index filter: {branch:#?}"),
    }
}

/// Asserts that no `Any(Map(...))` in `expr` contains an `Or` anywhere in its
/// predicate sub-tree. This catches bugs where OR distribution was incomplete.
fn assert_no_or_in_any(expr: &stmt::Expr) {
    match expr {
        stmt::Expr::Any(any) => {
            assert!(
                !contains_or(&any.expr),
                "Any(Map(...)) contains an Or expression after DNF distribution; \
                 this is a bug in flatten_to_dnf: {:#?}",
                any.expr
            );
        }
        stmt::Expr::And(and) => and.operands.iter().for_each(assert_no_or_in_any),
        _ => {}
    }
}

/// Returns true if `expr` contains an `Expr::Or` anywhere in its sub-tree.
fn contains_or(expr: &stmt::Expr) -> bool {
    match expr {
        stmt::Expr::Or(_) => true,
        stmt::Expr::And(and) => and.operands.iter().any(contains_or),
        stmt::Expr::Any(a) => contains_or(&a.expr),
        stmt::Expr::Map(m) => contains_or(&m.base) || contains_or(&m.map),
        stmt::Expr::BinaryOp(b) => contains_or(&b.lhs) || contains_or(&b.rhs),
        stmt::Expr::Not(n) => contains_or(&n.expr),
        stmt::Expr::IsNull(n) => contains_or(&n.expr),
        // Leaf nodes (Arg, Reference, Value, Default, Type, etc.) contain no sub-expressions.
        _ => false,
    }
}
