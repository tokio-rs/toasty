use std::collections::{HashMap, HashSet};

use indexmap::IndexSet;
use toasty_core::stmt::{self, visit, visit_mut, VisitMut};
use toasty_core::Schema;

use crate::engine::eval;
use crate::engine::{plan, planner::Planner};
use crate::Result;

impl Planner<'_> {
    pub(crate) fn plan_v2_stmt_query(&mut self, stmt: stmt::Query) -> Result<plan::VarId> {
        let mut stmt = stmt::Statement::Query(stmt);

        let mut walker_state = WalkerState {
            stmts: vec![StatementState::new()],
            scopes: vec![ScopeState { stmt_id: StmtId(0) }],
        };

        // Map the statement
        Walker {
            state: &mut walker_state,
            scope: 0,
            returning: false,
        }
        .visit_stmt_mut(&mut stmt);

        walker_state.stmts[0].stmt = Some(Box::new(stmt));
        // Build the execution plan...

        let mut materialization = Materialization {
            stmts: walker_state.stmts,
            materializations: vec![],
        };
        materialization.plan_materialization(StmtId(0));

        // All the materializations have been found, but returns have not been
        // computed. Compute all the returns now.
        for materialization in &mut materialization.materializations {
            materialization.compute_query(self.schema);
        }

        // Now that materializations have been planed, we can plan the execution
        // of the statements.
        self.plan_v2_stmt_execution(&mut materialization, StmtId(0))
    }

    fn plan_v2_stmt_execution(
        &mut self,
        state: &mut Materialization,
        stmt_id: StmtId,
    ) -> Result<plan::VarId> {
        let stmt = &state.stmts[stmt_id.0];

        // For now, assume there is only one materialization
        assert_eq!(1, stmt.materializations.len(), "TODO");

        for materialization in &mut state.materializations {
            // TODO: don't clone
            let project_arg_ty = materialization.ret_ty.as_ref().unwrap();
            let mut output_targets = vec![];

            for output in &mut materialization.output {
                let project = eval::Func::from_stmt(output.expr.clone(), project_arg_ty.clone());
                let ty =
                    stmt::ExprContext::new_free().infer_expr_ty(&output.expr, &project.args[..]);
                let var = self.var_table.register_var(stmt::Type::list(ty));

                output_targets.push(plan::OutputTarget { var, project });
                output.var = Some(var);
            }

            self.push_action(plan::ExecStatement {
                input: None,
                output: Some(plan::Output {
                    ty: Some(project_arg_ty.clone()),
                    targets: output_targets,
                }),
                stmt: materialization.stmt.clone(),
                conditional_update_with_no_returning: false,
            });
        }

        let [mid] = &stmt.materializations[..] else {
            todo!()
        };
        let [output] = &state.materializations[*mid].output[..] else {
            todo!("materializations={:#?}", state.materializations)
        };
        Ok(output.var.unwrap())
    }
}

#[derive(Debug)]
struct Materialization {
    /// Mapped statement
    stmts: Vec<StatementState>,

    materializations: Vec<MaterializeStatement>,
}

#[derive(Debug)]
struct WalkerState {
    /// Statements to be executed by the database, though they may still be
    /// broken down into multiple sub-statements.
    stmts: Vec<StatementState>,

    /// Scope state
    scopes: Vec<ScopeState>,
}

/// Per-statement state
#[derive(Debug)]
struct StatementState {
    /// Populated later
    stmt: Option<Box<stmt::Statement>>,

    /// Statement arguments
    args: Vec<Arg>,

    /// Sub-statements are statements declared within the definition of the
    /// containing statement.
    subs: Vec<StmtId>,

    /// Back-refs are expressions within sub-statements that reference the
    /// current statemetn.
    back_refs: HashMap<StmtId, Vec<BackRef>>,

    /// Materialization
    materializations: Vec<usize>,
}

#[derive(Debug)]
struct ScopeState {
    /// Identifier of the statement in the partitioner state.
    stmt_id: StmtId,
}

#[derive(Debug)]
struct MaterializeStatement {
    /// Final query
    stmt: stmt::Statement,

    /// Expressions to return
    output: Vec<MaterializeOutput>,

    returnings: IndexSet<stmt::ExprReference>,

    /// Materializations it depends on
    deps: HashSet<usize>,

    /// Materialization return type. Because the materialization comes from the
    /// database, it is always a vec of values.
    ret_ty: Option<Vec<stmt::Type>>,
}

#[derive(Debug)]
struct MaterializeOutput {
    expr: stmt::Expr,
    var: Option<plan::VarId>,
}

struct Walker<'a> {
    /// Partitioning state
    state: &'a mut WalkerState,
    scope: usize,
    returning: bool,
}

#[derive(Debug)]
struct BackRef {
    /// The expression
    expr: stmt::Expr,
}

#[derive(Debug)]
enum Arg {
    /// A sub-statement
    Sub(StmtId),

    /// A back-reference
    Ref { stmt_id: StmtId, index: usize },
}

#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq)]
struct StmtId(usize);

impl<'a> visit_mut::VisitMut for Walker<'a> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        match i {
            stmt::Expr::Reference(expr_reference) => {
                // At this point, the query should have been fully lowered
                let stmt::ExprReference::Column {
                    nesting,
                    table,
                    column,
                } = expr_reference
                else {
                    panic!("unexpected state: statement not lowered")
                };

                if *nesting > 0 {
                    let stmt_id = self.curr_stmt_id();
                    let target_id = self.state.scopes[self.scope - *nesting].stmt_id;

                    // The reference is recreated assuming it is evaluated from the target context.
                    let expr = stmt::ExprReference::Column {
                        nesting: 0,
                        table: *table,
                        column: *column,
                    }
                    .into();

                    let index = self.stmt(target_id).new_back_ref(stmt_id, expr);
                    let arg_id = self.curr_stmt().new_ref_arg(target_id, index);

                    // Using ExprArg as a placeholder. It will be rewritten
                    // later.
                    *i = stmt::Expr::arg(arg_id);
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                assert!(self.returning);
                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.

                // Create a `StatementState` to track the sub-statement
                let target_id = self.new_stmt();
                let mut scope = self.scope(target_id);
                visit_mut::visit_expr_stmt_mut(&mut scope, expr_stmt);
                self.state.scopes.pop();

                // Create a new input to receive the statement
                let arg_id = self.curr_stmt().new_sub_stmt_arg(target_id);

                // Replace the sub-statement expression with a placeholder tracking the input
                let expr = std::mem::replace(i, stmt::Expr::arg(arg_id));
                let stmt::Expr::Stmt(expr_stmt) = expr else {
                    panic!()
                };
                self.stmt(target_id).stmt = Some(expr_stmt.stmt);
            }
            _ => {
                visit_mut::visit_expr_mut(self, i);
            }
        }
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        assert!(!self.returning);
        self.returning = true;
        visit_mut::visit_returning_mut(self, i);
        self.returning = false;
    }
}

impl<'a> Walker<'a> {
    fn new_stmt(&mut self) -> StmtId {
        let stmt_id = StmtId(self.state.stmts.len());
        self.state.stmts.push(StatementState::new());
        stmt_id
    }

    fn scope<'child>(&'child mut self, stmt_id: StmtId) -> Walker<'child> {
        let scope = self.state.scopes.len();
        self.state.scopes.push(ScopeState { stmt_id });

        Walker {
            state: self.state,
            scope,
            returning: false,
        }
    }

    fn curr_stmt_id(&self) -> StmtId {
        self.state.scopes[self.scope].stmt_id
    }

    fn curr_stmt(&mut self) -> &mut StatementState {
        &mut self.state.stmts[self.state.scopes[self.scope].stmt_id.0]
    }

    fn stmt(&mut self, stmt_id: StmtId) -> &mut StatementState {
        &mut self.state.stmts[stmt_id.0]
    }
}

/// What do I want to do here?
///
/// * Walk through the statement graph and identify cycles.
///     * Walk through *inputs*
///     * We need to flag **edges** as visited as well.
/// * For each node
///     * Flag node as visited
///     * Visit edge, mark edge as visited.
///     * If target is visited, we have a cycle break it.
///         * Split statement
///             * Keep all visited edges on old statement
///             * Move current edge + all unvisited edges to new statement
///     * If unvisited, recurse.
///
/// * We actually want to treat stmt returning as some sort of hash set so we
///   can easily modify it to add / remove items to satisfy the edges...
impl Materialization {
    fn plan_materialization(&mut self, stmt_id: StmtId) {
        let mut materialization_id = None;

        if !self.stmt_state(stmt_id).back_refs.is_empty() {
            // Essentially, we only handle one level for now
            assert!(
                self.stmt_state(stmt_id)
                    .args
                    .iter()
                    .all(|a| matches!(a, Arg::Sub(..))),
                "TODO"
            );

            let mid = self.new_materialization(stmt_id);
            let materialization = &mut self.materializations[mid];

            for back_refs in self.stmts[stmt_id.0].back_refs.values() {
                for back_ref in back_refs {
                    materialization.output.push(MaterializeOutput {
                        expr: back_ref.expr.clone(),
                        var: None,
                    });
                }
            }

            materialization_id = Some(mid);
        }

        // Plan materialization for all sub-statements.
        let subs = self.stmt_state(stmt_id).subs.clone();

        for sub in subs {
            self.plan_materialization(sub);
        }

        if materialization_id.is_none() {
            materialization_id = Some(self.new_materialization(stmt_id));
        }

        let stmt = &**self.stmts[stmt_id.0].stmt.as_ref().unwrap();
        let stmt::Statement::Query(query) = stmt else {
            panic!()
        };
        let stmt::ExprSet::Select(select) = &query.body else {
            panic!()
        };
        let stmt::Returning::Expr(returning) = &select.returning else {
            panic!();
        };

        let mid = materialization_id.unwrap();
        let materialization = &mut self.materializations[mid];

        materialization.output.push(MaterializeOutput {
            expr: returning.clone(),
            var: None,
        });
    }

    fn stmt_state(&mut self, stmt_id: StmtId) -> &mut StatementState {
        &mut self.stmts[stmt_id.0]
    }

    fn new_materialization(&mut self, stmt_id: StmtId) -> usize {
        let materialize_id = self.materializations.len();

        let stmt_state = &mut self.stmts[stmt_id.0];
        let mut stmt = stmt_state.stmt.as_deref().unwrap().clone();

        let stmt::Statement::Query(query) = &mut stmt else {
            panic!()
        };
        let stmt::ExprSet::Select(select) = &mut query.body else {
            panic!()
        };

        for (i, arg) in stmt_state.args.iter().enumerate() {
            let Arg::Ref { .. } = arg else {
                continue;
            };

            assert_eq!(1, stmt_state.args.len(), "TODO: handle more complex cases");

            // We rewrite the filter to batch load all possible records that
            // will be needed to materialize the original statement.
            /*
            -- Step 1: Store filtered users
            CREATE TEMP TABLE temp_users AS
            SELECT * FROM users WHERE users.active = true;

            -- Step 2: Fetch all potentially matching todos
            SELECT todos.*
            FROM todos
            WHERE EXISTS (
              SELECT 1 FROM temp_users u
              WHERE todos.user_id = u.id
              AND todos.created_at > u.created_at
              AND todos.priority > 3
            );
                 */

            visit_mut::for_each_expr_mut(&mut select.filter, |expr| {
                match expr {
                    stmt::Expr::Reference(stmt::ExprReference::Column { nesting, .. }) => {
                        // We need to up the nesting to reflect that the filter is moved
                        // one level deeper.
                        *nesting += 1;
                    }
                    stmt::Expr::Arg(expr_arg) if expr_arg.position == i => {
                        // Rewrite reference the new `FROM`.
                        // `column: 0` is hardcdoed for now
                        *expr = stmt::ExprReference::Column {
                            nesting: 0,
                            table: 0,
                            column: 0,
                        }
                        .into();
                    }
                    _ => {}
                }
            });

            let sub_select =
                stmt::Select::new(stmt::Values::from(stmt::Expr::arg(0)), select.filter.take());

            select.filter = stmt::Expr::exists(stmt::Query::builder(sub_select).returning(1));
        }

        // let stmt = self.stmt_state(stmt_id);
        stmt_state.materializations.push(materialize_id);

        self.materializations.push(MaterializeStatement {
            stmt,
            output: vec![],
            returnings: IndexSet::new(),
            deps: HashSet::new(),
            ret_ty: None,
        });

        materialize_id
    }
}

impl MaterializeStatement {
    fn compute_query(&mut self, schema: &Schema) {
        for output in &mut self.output {
            visit_mut::for_each_expr_mut(&mut output.expr, |expr| {
                match expr {
                    stmt::Expr::Reference(e) => {
                        // Track the needed reference and replace the expression with an argument that will pull from the position.
                        let (pos, _) = self.returnings.insert_full(e.clone());
                        *expr = stmt::Expr::arg(pos);
                    }
                    // Subqueries should have been removed at this point
                    stmt::Expr::Stmt(_) | stmt::Expr::InSubquery(_) => todo!(),
                    _ => {}
                }
            });
        }

        let stmt::Statement::Query(query) = &mut self.stmt else {
            todo!()
        };
        let stmt::ExprSet::Select(select) = &mut query.body else {
            todo!()
        };
        select.returning =
            stmt::Returning::from_expr_iter(self.returnings.iter().map(stmt::Expr::from));

        if let Some(expr) = stmt::ExprContext::new(schema).infer_stmt_ty(&self.stmt, &[]) {
            let stmt::Type::Record(fields) = expr else {
                todo!()
            };
            self.ret_ty = Some(fields);
        }
    }
}

impl StatementState {
    fn new() -> StatementState {
        StatementState {
            stmt: None,
            args: vec![],
            subs: vec![],
            back_refs: HashMap::new(),
            materializations: vec![],
        }
    }

    fn new_back_ref(&mut self, target_id: StmtId, expr: stmt::Expr) -> usize {
        let back_refs = self.back_refs.entry(target_id).or_default();
        let ret = back_refs.len();
        back_refs.push(BackRef { expr });
        ret
    }

    fn new_ref_arg(&mut self, stmt_id: StmtId, index: usize) -> usize {
        let arg_id = self.args.len();
        self.args.push(Arg::Ref { stmt_id, index });
        arg_id
    }

    fn new_sub_stmt_arg(&mut self, stmt_id: StmtId) -> usize {
        self.subs.push(stmt_id);
        let arg_id = self.args.len();
        self.args.push(Arg::Sub(stmt_id));
        arg_id
    }
}
