// Acyclic Flow Analysis and CTE Rewriting
//
// This module transforms statements with cyclic ExprStmt dependencies to acyclic form using CTEs.
//
// The key insight is that cyclic dependencies arise when ExprStmt contains ExprReference::Column
// with nesting > 0, meaning the subquery needs values from its parent query to execute.

use anyhow::Result;
use toasty_core::stmt::{self, visit};

use super::Planner;

impl Planner<'_> {
    /// Transform statements with cyclic ExprStmt dependencies to acyclic form using CTEs
    pub(super) fn rewrite_to_acyclic_flow(&mut self, stmt: &mut stmt::Query) -> Result<()> {
        // Debug assertion: Verify ExprStmt nodes are lowered (have table sources)
        debug_assert!(
            self.verify_expr_stmt_lowered(stmt),
            "ExprStmt nodes should be lowered before acyclic rewriting"
        );

        // Analyze ExprStmt dependencies in returning clause
        if self.has_cyclic_dependencies(stmt) {
            self.rewrite_with_cte(stmt)?;

            // Debug assertion: Verify no more cycles after CTE rewriting
            debug_assert!(
                !self.has_cyclic_dependencies(stmt),
                "CTE rewriting should eliminate all cyclic dependencies"
            );
        }
        Ok(())
    }

    /// Check if the query has cyclic dependencies in its ExprStmt nodes
    fn has_cyclic_dependencies(&self, stmt: &stmt::Query) -> bool {
        // Check only immediate ExprStmt in returning, don't traverse into nested queries
        let mut has_cycles = false;

        // Get the select body to access the returning field
        let select = stmt.body.as_select();

        // Only check if returning is an expression
        if let stmt::Returning::Expr(expr) = &select.returning {
            visit::for_each_expr_curr_stmt(expr, |expr| {
                if let stmt::Expr::Stmt(expr_stmt) = expr {
                    if self.has_parent_references(&expr_stmt.stmt) {
                        has_cycles = true;
                    }
                }
            });
        }

        has_cycles
    }

    /// Check if a specific query has parent references (ExprReference::Column with nesting > 0)
    fn has_parent_references(&self, statement: &stmt::Statement) -> bool {
        let stmt::Statement::Query(query) = statement else {
            return false;
        };

        // Check if this specific query has ExprReference::Column with nesting > 0
        let mut has_parent_ref = false;
        visit::for_each_expr(query, |expr| {
            if let stmt::Expr::Reference(stmt::ExprReference::Column { nesting, .. }) = expr {
                if *nesting > 0 {
                    has_parent_ref = true;
                }
            }
        });
        has_parent_ref
    }

    /// Transform cyclic dependencies to CTEs (requires careful human implementation)
    fn rewrite_with_cte(&mut self, _stmt: &mut stmt::Query) -> Result<()> {
        // ⚠️  IMPLEMENTATION STOP POINT ⚠️
        //
        // CTE rewriting is the most complex part of this design and requires careful human-driven implementation.
        //
        // ALGORITHM OVERVIEW (for human implementer):
        //
        // PROBLEM: Query with nested subqueries that reference parent query fields creates cycles:
        //   - Outer query needs subquery results
        //   - Subquery needs outer query field values
        //   - Cannot execute in either order without the other
        //
        // SOLUTION: Break cycles by converting to CTEs (Common Table Expressions):
        //
        // Step 1: EXTRACT OUTER QUERY → Create CTE with fields needed by subqueries
        // Step 2: EXTRACT SUBQUERIES → Convert to independent CTEs with batched IN clauses
        // Step 3: REWRITE MAIN QUERY → Reference CTEs instead of direct tables/subqueries
        //
        // EXAMPLE TRANSFORMATION:
        // Before: SELECT user_name, (SELECT todo_title FROM todos WHERE todos.user_id = users.id) FROM users
        // After:  WITH cte_users AS (SELECT user_name, id FROM users),
        //              cte_todos AS (SELECT todo_title FROM todos WHERE todos.user_id IN (SELECT id FROM cte_users))
        //         SELECT user_name, cte_todos.todo_title FROM cte_users
        //
        // IMPLEMENTATION NOTES:
        // - Reuse patterns from planner/output.rs (Partitioner) for field analysis
        // - Handle parent reference rewriting (nesting > 0 → CTE references)
        // - Convert equality comparisons to IN clauses for batching efficiency
        // - Ensure proper CTE indexing and field mapping
        // - Add comprehensive debug assertions to verify transformations
        //
        // TODO: Human implementer should:
        // 1. Start with simple cases (single subquery, no nesting)
        // 2. Add thorough testing at each step
        // 3. Gradually handle more complex scenarios
        // 4. Focus on correctness over optimization initially

        todo!("CTE rewriting requires human-driven implementation - see algorithm overview above")
    }

    // Debug assertion helper functions
    fn verify_expr_stmt_references_ctes(&self, stmt: &stmt::Query) -> bool {
        let mut all_reference_ctes = true;

        // Get the select body to access the returning field
        let select = stmt.body.as_select();

        if let stmt::Returning::Expr(expr) = &select.returning {
            visit::for_each_expr_curr_stmt(expr, |expr| {
                if let stmt::Expr::Stmt(expr_stmt) = expr {
                    // After CTE rewriting, ExprStmt should reference CTE tables, not regular tables
                    let stmt::Statement::Query(query) = expr_stmt.stmt.as_ref() else {
                        all_reference_ctes = false;
                        return;
                    };

                    if !query.body.as_select().source.is_cte() {
                        all_reference_ctes = false;
                    }
                }
            });
        }

        all_reference_ctes
    }

    fn verify_expr_stmt_lowered(&self, stmt: &stmt::Query) -> bool {
        let mut all_lowered = true;

        // Get the select body to access the returning field
        let select = stmt.body.as_select();

        if let stmt::Returning::Expr(expr) = &select.returning {
            visit::for_each_expr_curr_stmt(expr, |expr| {
                if let stmt::Expr::Stmt(expr_stmt) = expr {
                    // ExprStmt should have table sources after lowering
                    let stmt::Statement::Query(query) = expr_stmt.stmt.as_ref() else {
                        all_lowered = false;
                        return;
                    };

                    if !query.body.as_select().source.is_table() {
                        all_lowered = false;
                    }
                }
            });
        }

        all_lowered
    }
}
