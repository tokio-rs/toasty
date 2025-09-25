use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use toasty_core::stmt::{self, visit_mut, ExprReference, VisitMut};

use crate::engine::planner::Planner;

impl Planner<'_> {
    pub(crate) fn partition(&self, stmt: stmt::Query) -> stmt::Query {
        let mut stmt = stmt::Statement::Query(stmt);
        println!("stmt={stmt:#?}");

        let mut state = State {
            stmts: vec![StatementState::new()],
            scopes: vec![ScopeState { stmt_id: StmtId(0) }],
        };

        // Map the statement
        Walker {
            state: &mut state,
            scope: 0,
            returning: false,
        }
        .visit_stmt_mut(&mut stmt);

        if state.stmts.len() > 1 {
            state.stmts[0].stmt = Some(Box::new(stmt));
            // Build the execution plan...

            todo!("state={state:#?}");
        }

        let stmt::Statement::Query(stmt) = stmt else {
            todo!()
        };
        stmt
    }
}

#[derive(Debug)]
struct Plan {
    /// Statements to execute
    stmts: Vec<stmt::Statement>,

    /// Mapped statement
    state: State,
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
impl Plan {
    fn traverse(&mut self, root: StmtId) {
        let mut deps = vec![];
        self.traverse2(&mut deps, root);
    }

    fn traverse2(&mut self, deps: &mut Vec<StmtId>, curr: StmtId) {
        todo!()
    }
}

#[derive(Debug)]
struct State {
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
}

#[derive(Debug)]
struct ScopeState {
    /// Identifier of the statement in the partitioner state.
    stmt_id: StmtId,
}

struct Walker<'a> {
    /// Partitioning state
    state: &'a mut State,
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

impl StatementState {
    fn new() -> StatementState {
        StatementState {
            stmt: None,
            args: vec![],
            subs: vec![],
            back_refs: HashMap::new(),
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
