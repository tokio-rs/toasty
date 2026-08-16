use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut};

use crate::engine::{
    eval,
    mir::{self, LogicalPlan},
};

/// Evaluates `body` over node outputs.
///
/// Without a `base`, the body evaluates once over whole values: `arg(i)` is
/// `attached[i]`'s complete output.
///
/// With a `base`, the body evaluates once per row of `base`: `arg(0)` is the
/// current row, and `arg(1 + i)` is the whole output of `attached[i]`. The
/// output has one element per `base` row, so pagination metadata forwards
/// from `base`. Zero `base` rows means zero body evaluations, so the
/// attached outputs are read only when `base` returned rows — the guard pass
/// reads this from [`Operation::input_reads`](mir::Operation::input_reads).
#[derive(Debug)]
pub(crate) struct Eval {
    /// When set, the node whose rows are iterated.
    pub(crate) base: Option<mir::NodeId>,

    /// Nodes whose whole outputs the body reads.
    pub(crate) attached: IndexSet<mir::NodeId>,

    /// The function to evaluate.
    pub(crate) body: eval::Func,

    /// Output type: `body.ret`, or `List<body.ret>` when mapping over `base`.
    pub(crate) ty: stmt::Type,
}

impl Eval {
    /// Evaluates `body` once over the whole outputs of `inputs`:
    /// `arg(i)` = `inputs[i]`.
    pub(crate) fn compute(inputs: IndexSet<mir::NodeId>, body: eval::Func) -> Self {
        let ty = body.ret.clone();
        Eval {
            base: None,
            attached: inputs,
            body,
            ty,
        }
    }

    /// Evaluates `body` once per row of `base`: `arg(0)` = current row,
    /// `arg(1 + i)` = `attached[i]`.
    pub(crate) fn map_over(
        base: mir::NodeId,
        attached: IndexSet<mir::NodeId>,
        body: eval::Func,
    ) -> Self {
        debug_assert_eq!(body.args.len(), 1 + attached.len());
        debug_assert!(!attached.contains(&base));

        let ty = stmt::Type::list(body.ret.clone());
        Eval {
            base: Some(base),
            attached,
            body,
            ty,
        }
    }

    /// Builds the executable per-row function for an `Eval` with a `base`.
    /// The executor evaluates one function over whole input values, so the
    /// per-row structure is erased here — at the last moment — into a single
    /// `map` expression over input 0, with inputs ordered `[base, attached...]`.
    pub(crate) fn map_func(&self, logical_plan: &LogicalPlan) -> eval::Func {
        let base = self.base.expect("map_func on an Eval without a base");

        let mut arg_tys = vec![logical_plan[base].ty().clone()];
        for input in &self.attached {
            arg_tys.push(logical_plan[input].ty().clone());
        }

        // Inside the map, the body's `arg(0)` (the row) resolves to the map's
        // element scope unchanged, while references to attached values must
        // climb one extra scope — past the map — to reach the function
        // inputs. A body arg references a body parameter when its nesting
        // equals the number of scopes around it.
        let mut body = self.body.expr().clone();
        visit_mut::walk_expr_scoped_mut(&mut body, 0, |expr, scope_depth| {
            if let stmt::Expr::Arg(arg) = expr
                && arg.nesting == scope_depth
                && arg.position >= 1
            {
                arg.nesting += 1;
            }
            true
        });
        let expr = stmt::Expr::map(stmt::Expr::arg(0), body);
        eval::Func::from_stmt_typed(expr, arg_tys, self.ty.clone())
    }
}

impl From<Eval> for mir::Node {
    fn from(value: Eval) -> Self {
        mir::Operation::Eval(value).into()
    }
}
