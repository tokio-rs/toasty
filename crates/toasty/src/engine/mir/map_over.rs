use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut};

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

/// Evaluates `body` once per row of `base`, splicing in values from
/// `attached`.
///
/// The body is a function of `1 + attached.len()` arguments: `arg(0)` is the
/// current row of `base`, and `arg(1 + i)` is the whole output of
/// `attached[i]`. The output has one element per `base` row, so pagination
/// metadata forwards from `base`. Zero `base` rows means zero body
/// evaluations, so the attached outputs are read only when `base` returned
/// rows — the guard pass reads this from
/// [`Operation::input_reads`](mir::Operation::input_reads).
#[derive(Debug)]
pub(crate) struct MapOver {
    /// The node whose rows are iterated.
    pub(crate) base: mir::NodeId,

    /// Nodes whose whole outputs the body reads while mapping.
    pub(crate) attached: IndexSet<mir::NodeId>,

    /// The per-row function: `arg(0)` = current row, `arg(1 + i)` =
    /// `attached[i]`.
    pub(crate) body: eval::Func,

    /// Output type: `List<body.ret>`.
    pub(crate) ty: stmt::Type,
}

impl MapOver {
    pub(crate) fn new(
        base: mir::NodeId,
        attached: IndexSet<mir::NodeId>,
        body: eval::Func,
    ) -> Self {
        debug_assert_eq!(body.args.len(), 1 + attached.len());
        debug_assert!(!attached.contains(&base));

        let ty = stmt::Type::list(body.ret.clone());
        MapOver {
            base,
            attached,
            body,
            ty,
        }
    }

    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::Eval {
        // The executor evaluates one function over whole input values, so the
        // per-row structure is erased here — at the last moment — into a
        // single `map` expression over input 0, with inputs ordered
        // `[base, attached...]`.
        let mut input_vars = vec![logical_plan[self.base].var.get().unwrap()];
        let mut arg_tys = vec![logical_plan[self.base].ty().clone()];
        for input in &self.attached {
            input_vars.push(logical_plan[input].var.get().unwrap());
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
        let eval = eval::Func::from_stmt_typed(expr, arg_tys, self.ty.clone());

        let output = var_table.register_var(self.ty.clone());
        node.var.set(Some(output));

        exec::Eval {
            inputs: input_vars,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            eval,
            // Metadata forwards from `base`, always input 0.
            metadata: Some(0),
        }
    }
}

impl From<MapOver> for mir::Node {
    fn from(value: MapOver) -> Self {
        mir::Operation::MapOver(value).into()
    }
}
