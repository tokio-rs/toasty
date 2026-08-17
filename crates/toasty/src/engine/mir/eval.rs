use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut};

use crate::engine::{eval, mir};

/// Evaluates `body` once over whole input values: with no `base`, `arg(i)` is
/// `attached[i]`'s complete output; with a `base`, `arg(0)` is `base`'s
/// complete output and `arg(1 + i)` is `attached[i]`'s.
///
/// A `base` marks the operation as per-row: the body is a `map` over `arg(0)`
/// (built by [`Eval::map_over`]), so the output has one element per `base`
/// row and pagination metadata forwards from `base`. Zero `base` rows means
/// the map body never runs, so the attached outputs are read only when `base`
/// returned rows — the guard pass reads this from
/// [`Operation::input_reads`](mir::Operation::input_reads).
#[derive(Debug)]
pub(crate) struct Eval {
    /// When set, the node whose rows the body maps over.
    pub(crate) base: Option<mir::NodeId>,

    /// Nodes whose whole outputs the body reads.
    pub(crate) attached: IndexSet<mir::NodeId>,

    /// The function to evaluate, over whole input values ordered
    /// `[base?, attached...]`. Its return type is the operation's output type.
    pub(crate) body: eval::Func,
}

impl Eval {
    /// Evaluates `body` once over the whole outputs of `inputs`:
    /// `arg(i)` = `inputs[i]`.
    pub(crate) fn compute(inputs: IndexSet<mir::NodeId>, body: eval::Func) -> Self {
        Eval {
            base: None,
            attached: inputs,
            body,
        }
    }

    /// Evaluates the per-row `body` once per row of `base`: `arg(0)` =
    /// current row, `arg(1 + i)` = `attached[i]`.
    ///
    /// The executor evaluates one function over whole input values, so the
    /// per-row structure is erased here into a single `map` expression over
    /// input 0, with inputs ordered `[base, attached...]`.
    pub(crate) fn map_over(
        store: &mir::Store,
        base: mir::NodeId,
        attached: IndexSet<mir::NodeId>,
        body: eval::Func,
    ) -> Self {
        debug_assert_eq!(body.args.len(), 1 + attached.len());
        debug_assert!(!attached.contains(&base));

        let mut arg_tys = vec![store[base].ty().clone()];
        for input in &attached {
            arg_tys.push(store[input].ty().clone());
        }

        // Inside the map, the body's `arg(0)` (the row) resolves to the map's
        // element scope unchanged, while references to attached values must
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
            base: Some(base),
            attached,
            body: eval::Func::from_stmt_typed(expr, arg_tys, ty),
        }
    }
}

impl From<Eval> for mir::Node {
    fn from(value: Eval) -> Self {
        mir::Operation::Eval(value).into()
    }
}
