use std::collections::HashMap;

use indexmap::IndexMap;
use toasty_core::stmt::{self, visit_mut, ExprReference, VisitMut};

use crate::engine::planner::Planner;

impl Planner<'_> {
    pub(crate) fn partition(&self, stmt: stmt::Query) -> stmt::Query {
        let mut stmt = stmt::Statement::Query(stmt);
        println!("stmt={stmt:#?}");

        let mut state = State {
            stmts: vec![StatementState::new()],
            edges: HashMap::new(),
            scopes: vec![ScopeState { stmt_id: StmtId(0) }],
        };

        // Map the statement
        Walker {
            state: &mut state,
            scope: 0,
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

    /// Directed edges of the dependency graph between statements. From
    /// statements that need data to the statements that provide the data.
    edges: HashMap<(StmtId, StmtId), Edge>,

    /// Scope state
    scopes: Vec<ScopeState>,
}

/// Per-statement state
#[derive(Debug)]
struct StatementState {
    /// Populated later
    stmt: Option<Box<stmt::Statement>>,

    /// Counts the number of inputs
    inputs: usize,

    /// Tracks if the node is visited in graph algorithms
    visited: bool,
}

#[derive(Debug)]
struct Edge {
    /// The statement's argument position for this input
    arg: usize,

    /// The expression on the target statement's relation representing the data
    /// that is neede.
    expr: stmt::Expr,

    /// Used when traversing the graph
    visited: bool,
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

                    let arg_id = self.curr_stmt().new_input();

                    let expr = std::mem::replace(i, stmt::Expr::arg(arg_id));

                    self.state
                        .edges
                        .insert((stmt_id, target_id), Edge::new(arg_id, expr));
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.

                // Create a `StatementState` to track the sub-statement
                let stmt_id = self.curr_stmt_id();
                let target_stmt_id = self.new_stmt();
                let mut scope = self.scope(target_stmt_id);
                visit_mut::visit_expr_stmt_mut(&mut scope, expr_stmt);
                self.state.scopes.pop();

                // Create a new input to receive the statement
                let arg_id = self.curr_stmt().new_input();
                let expr = match &mut *expr_stmt.stmt {
                    stmt::Statement::Query(query) => match &mut query.body {
                        stmt::ExprSet::Select(select) => match &mut select.returning {
                            stmt::Returning::Expr(expr) => expr.take(),
                            _ => todo!("expr_stmt={expr_stmt:#?}"),
                        },
                        _ => todo!("expr_stmt={expr_stmt:#?}"),
                    },
                    _ => todo!("expr_stmt={expr_stmt:#?}"),
                };

                self.state
                    .edges
                    .insert((stmt_id, target_stmt_id), Edge::new(arg_id, expr));
            }
            _ => {
                visit_mut::visit_expr_mut(self, i);
            }
        }
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
            inputs: 0,
            visited: false,
        }
    }

    fn new_input(&mut self) -> usize {
        let input_id = self.inputs;
        self.inputs += 1;
        input_id
    }
}

impl Edge {
    fn new(arg: usize, expr: stmt::Expr) -> Edge {
        Edge {
            arg,
            expr,
            visited: false,
        }
    }
}
