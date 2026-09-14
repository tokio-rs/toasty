use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut};

use crate::engine::{eval, mir};

/// Evaluates `body` once over whole input values: with no `row_input`,
/// `arg(i)` is `inputs[i]`'s complete output; with a `row_input`, `arg(0)` is
/// `row_input`'s complete output and `arg(1 + i)` is `inputs[i]`'s.
///
/// A `row_input` marks the operation as per-row: the body is a `map` over
/// `arg(0)` (built by [`Eval::map_over`]), so the output has one element per
/// input row and pagination metadata forwards from `row_input`. Zero input
/// rows means the map body never runs, so the other inputs are read only when
/// `row_input` returned rows — the guard pass reads this from
/// [`Operation::input_reads`](mir::Operation::input_reads).
#[derive(Debug)]
pub(crate) struct Eval {
    /// When set, this operation returns one result for each row from this input.
    pub(crate) row_input: Option<mir::NodeId>,

    /// Nodes whose whole outputs the body reads.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The function to evaluate, over whole input values ordered
    /// `[row_input?, inputs...]`. Its return type is the operation's output
    /// type.
    pub(crate) body: eval::Func,
}

impl Eval {
    /// Evaluates `body` once over the whole outputs of `inputs`:
    /// `arg(i)` = `inputs[i]`.
    pub(crate) fn compute(inputs: IndexSet<mir::NodeId>, body: eval::Func) -> Self {
        Eval {
            row_input: None,
            inputs,
            body,
        }
    }

    /// Evaluates the per-row `body` once per row of `row_input`: `arg(0)` =
    /// current row, `arg(1 + i)` = `inputs[i]`.
    ///
    /// The executor evaluates one function over whole input values, so the
    /// per-row structure is erased here into a single `map` expression over
    /// input 0, with inputs ordered `[row_input, inputs...]`.
    pub(crate) fn map_over(
        store: &mir::Store,
        row_input: mir::NodeId,
        inputs: IndexSet<mir::NodeId>,
        body: eval::Func,
    ) -> Self {
        debug_assert_eq!(body.args.len(), 1 + inputs.len());
        debug_assert!(!inputs.contains(&row_input));

        let mut arg_tys = vec![store[row_input].ty().clone()];
        for input in &inputs {
            arg_tys.push(store[input].ty().clone());
        }

        // Inside the map, the body's `arg(0)` (the row) resolves to the map's
        // element scope unchanged, while references to other inputs must
        // climb one extra scope — past the map — to reach the function
        // inputs. A body arg references a body parameter when its nesting
        // equals the number of scopes around it.
        let ty = stmt::Type::list(body.ret.clone());
        let mut map_body = body.into_expr();
        visit_mut::walk_expr_scoped_mut(&mut map_body, 0, |expr, scope_depth| {
            if let stmt::Expr::Arg(arg) = expr
                && arg.nesting == scope_depth
                && arg.position >= 1
            {
                arg.nesting += 1;
            }
            true
        });
        let expr = stmt::Expr::map(stmt::Expr::arg(0), map_body);

        Eval {
            row_input: Some(row_input),
            inputs,
            body: eval::Func::from_stmt_typed(expr, arg_tys, ty),
        }
    }
}

impl From<Eval> for mir::Node {
    fn from(value: Eval) -> Self {
        mir::Operation::Eval(value).into()
    }
}
