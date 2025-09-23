use toasty_core::stmt::{self, visit_mut, VisitMut};

use crate::engine::planner::Planner;

impl Planner<'_> {
    pub(crate) fn partition(&self, mut stmt: stmt::Query) -> stmt::Query {
        println!("stmt={stmt:#?}");

        let mut state = State {
            stmts: vec![],
            scopes: vec![Scope {}],
        };
        Walker {
            state: &mut state,
            scope: 0,
        }
        .visit_stmt_query_mut(&mut stmt);

        stmt
    }
}

#[derive(Debug)]
struct State {
    /// Statements to be executed by the database
    stmts: Vec<stmt::Statement>,

    /// Scope state
    scopes: Vec<Scope>,
}

#[derive(Debug)]
struct Scope {}

struct Walker<'a> {
    /// Partitioning state
    state: &'a mut State,
    scope: usize,
}

impl<'a> visit_mut::VisitMut for Walker<'a> {
    fn visit_expr_reference_mut(&mut self, i: &mut stmt::ExprReference) {
        if i.nesting() > 0 {
            todo!("expr_reference={i:#?}; scope={:?}", self.scope());
        }
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut stmt::ExprStmt) {
        // For now, we assume nested sub-statements cannot be executed on the
        // target database. Eventually, we will need to make this smarter.

        // Create a new scope for walking the statement
        let scope = self.state.scopes.len();
        self.state.scopes.push(Scope {});

        let mut w = Walker {
            state: self.state,
            scope,
        };

        visit_mut::visit_expr_stmt_mut(&mut w, i);

        self.state.scopes.pop();
    }
}

impl<'a> Walker<'a> {
    fn scope(&mut self) -> &Scope {
        &self.state.scopes[self.scope]
    }
}
