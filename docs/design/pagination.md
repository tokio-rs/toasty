# Engine-Level Pagination Design

## Overview

This document describes the implementation of engine-level pagination in Toasty. The key principle is that pagination logic (limit+1 strategy, cursor extraction, etc.) should be handled by the engine, not in application-level code. This allows the engine to leverage database-specific capabilities (e.g., DynamoDB's native cursor support) while providing compatibility for databases that don't have native support (e.g., SQL databases).

## Architecture Context

### Statement System
- `toasty_core::stmt::Statement` represents a **superset of SQL** - "Toasty-flavored SQL"
- Contains both SQL concepts AND Toasty application-level concepts (models, paths, pagination)
- `Limit::PaginateForward` is a Toasty-level concept that must be transformed by the engine before reaching SQL generation
- By the time statements reach `toasty-sql`, they must contain ONLY valid SQL

### Engine Pipeline
1. **Planner**: Transforms Toasty statements into a pipeline of actions
2. **Actions**: Executed by the engine, store results in VarStore
3. **VarStore**: Stores intermediate results between pipeline steps
4. **ExecResponse**: Final result containing values and optional metadata

### Existing Patterns
- **eval::Func**: Pre-computed transformations that execute during pipeline execution
- **partition_returning**: Separates database-handled expressions from in-memory evaluations
- **Output::project**: Transforms raw database results before storing in VarStore

## Design

### Core Types

```rust
// In engine.rs
pub struct ExecResponse {
    pub values: ValueStream,
    pub metadata: Option<Metadata>,
}

pub struct Metadata {
    pub next_cursor: Option<Expr>,
    pub prev_cursor: Option<Expr>,
    pub query: Query,
}

// In engine/plan/exec_statement.rs
pub struct ExecStatement {
    pub input: Option<Input>,
    pub output: Option<Output>,
    pub stmt: stmt::Statement,
    pub conditional_update_with_no_returning: bool,
    
    /// Pagination configuration for this query
    pub pagination: Option<Pagination>,
}

pub struct Pagination {
    /// Original limit before +1 transformation
    pub limit: u64,
    
    /// Function to extract cursor from a row
    /// Takes row as arg[0], returns cursor value(s)
    pub extract_cursor: eval::Func,
}
```

### VarStore Changes

The VarStore needs to be updated to store `ExecResponse` instead of `ValueStream`:

```rust
pub(crate) struct VarStore {
    slots: Vec<Option<ExecResponse>>,
}
```

This allows pagination metadata to flow through the pipeline and be returned from `engine::exec`.

## Implementation Plan

### Phase 1: Update VarStore to ExecResponse [Mechanical Change]

This phase is a purely mechanical change to update the VarStore infrastructure. No pagination logic yet.

1. **Update VarStore** (`engine/exec/var_store.rs`):
   - Change storage type from `ValueStream` to `ExecResponse`
   - Update `load()` to return `ExecResponse`
   - Update `store()` to accept `ExecResponse`
   - Update `dup()` to clone entire `ExecResponse` (including metadata)

2. **Update all action executors** to wrap their results in `ExecResponse`:
   - For now, all actions will use `metadata: None`
   - Each action's result becomes: `ExecResponse { values, metadata: None }`
   - Actions to update:
     - `action_associate`
     - `action_batch_write`
     - `action_delete_by_key`
     - `action_exec_statement`
     - `action_find_pk_by_index`
     - `action_get_by_key`
     - `action_insert`
     - `action_query_pk`
     - `action_update_by_key`
     - `action_set_var`

3. **Update pipeline execution** (`engine/exec.rs`):
   - `exec_pipeline` returns `ExecResponse`
   - Handle `VarStore` returning `ExecResponse`

4. **Update main engine** (`engine.rs`):
   - `exec::exec` now returns `ExecResponse` directly
   - Remove the temporary wrapping logic

This phase establishes the infrastructure without any behavioral changes. All existing tests should continue to pass.

### Phase 2: Add Pagination to ExecStatement [Task 2]

1. Add `Pagination` struct to `engine/plan/exec_statement.rs`
2. Add `pagination: Option<Pagination>` field to `ExecStatement`
3. No execution changes yet - just the structure

### Phase 3: Planner Support for SQL Pagination [Task 3]

In `planner/select.rs`, add pagination planning logic:

```rust
impl Planner<'_> {
    fn plan_select_sql(...) {
        // ... existing logic ...
        
        // Handle pagination
        let pagination = if let Some(Limit::PaginateForward { limit, after }) = &stmt.limit {
            Some(self.plan_pagination(&mut stmt, &mut project, limit)?)
        } else {
            None
        };
        
        self.push_action(plan::ExecStatement {
            input,
            output: Some(plan::Output { var: output, project }),
            stmt: stmt.into(),
            conditional_update_with_no_returning: false,
            pagination,
        });
    }
    
    fn plan_pagination(
        &mut self,
        stmt: &mut stmt::Query,
        project: &mut eval::Func,
        limit_expr: &stmt::Expr,
    ) -> Result<Pagination> {
        let original_limit = self.extract_limit_value(limit_expr)?;
        
        // Get ORDER BY clause (required for pagination)
        let order_by = stmt.order_by.as_ref()
            .ok_or_else(|| anyhow!("Pagination requires ORDER BY"))?;
        
        // Check if ORDER BY is unique
        let is_unique = self.is_order_by_unique(order_by, stmt);
        
        // If not unique, append primary key as tie-breaker
        if !is_unique {
            self.append_pk_to_order_by(stmt)?;
        }
        
        // Ensure ORDER BY fields are in returning clause
        let (added_indices, original_field_count) = 
            self.ensure_order_by_in_returning(stmt)?;
        
        // Build cursor extraction function
        let extract_cursor = self.build_cursor_extraction_func(
            stmt,
            &added_indices,
        )?;
        
        // Modify project function if we added fields
        if !added_indices.is_empty() {
            self.adjust_project_for_pagination(
                project,
                original_field_count,
                added_indices.len(),
            );
        }
        
        // Transform limit to +1 for next page detection
        *stmt.limit.as_mut().unwrap() = Limit::Offset {
            limit: (original_limit + 1).into(),
            offset: None,
        };
        
        Ok(Pagination {
            limit: original_limit,
            extract_cursor,
        })
    }
}
```

Key helper methods:

1. **`is_order_by_unique`**: Checks if ORDER BY fields form a unique constraint
2. **`append_pk_to_order_by`**: Adds primary key as tie-breaker
3. **`ensure_order_by_in_returning`**: Adds ORDER BY fields to SELECT if missing
4. **`build_cursor_extraction_func`**: Creates `eval::Func` to extract cursor
5. **`adjust_project_for_pagination`**: Modifies project to filter out added fields

### Phase 4: Executor Implementation [Task 4]

In `engine/exec/exec_statement.rs`:

```rust
impl Exec<'_> {
    pub(super) async fn action_exec_statement(
        &mut self,
        action: &plan::ExecStatement,
    ) -> Result<()> {
        // ... existing logic to execute statement ...
        
        let res = if let Some(pagination) = &action.pagination {
            self.handle_paginated_query(res, pagination, &action.stmt).await?
        } else {
            ExecResponse {
                values: /* normal value stream */,
                metadata: None,
            }
        };
        
        self.vars.store(out.var, res);
        Ok(())
    }
    
    async fn handle_paginated_query(
        &mut self,
        rows: Rows,
        pagination: &Pagination,
        stmt: &Statement,
    ) -> Result<ExecResponse> {
        // Collect limit+1 rows
        let mut buffer = Vec::new();
        let mut count = 0;
        
        match rows {
            Rows::Values(stream) => {
                for await value in stream {
                    buffer.push(value?);
                    count += 1;
                    if count > pagination.limit {
                        break;
                    }
                }
            }
            _ => return Err(anyhow!("Pagination requires row results")),
        }
        
        // Check if there's a next page
        let has_next = buffer.len() > pagination.limit as usize;
        
        // Extract cursor if there's a next page
        let next_cursor = if has_next {
            // Get cursor from the LAST item we're keeping
            let last_kept = &buffer[pagination.limit as usize - 1];
            let cursor_value = pagination.extract_cursor.eval(&[last_kept.clone()])?;
            
            // Truncate buffer to requested limit
            buffer.truncate(pagination.limit as usize);
            
            Some(stmt::Expr::Value(cursor_value))
        } else {
            None
        };
        
        Ok(ExecResponse {
            values: ValueStream::from_vec(buffer),
            metadata: Some(Metadata {
                next_cursor,
                prev_cursor: None, // TODO: implement in future
                query: stmt.as_query().cloned().unwrap_or_default(),
            }),
        })
    }
}
```

### Phase 5: Clean Up Application Layer [Task 5]

Remove the limit+1 logic from `Paginate::collect`:

```rust
pub async fn collect(self, db: &Db) -> Result<Page<M>> {
    // Simply delegate to db.paginate - engine handles pagination
    db.paginate(self.query).await
}
```

Update `Db::paginate` to use the metadata from `ExecResponse`:

```rust
pub async fn paginate<M: Model>(&self, statement: stmt::Select<M>) -> Result<Page<M>> {
    let exec_response = engine::exec(self, statement.untyped.clone().into()).await?;
    
    // Convert value stream to models
    let mut cursor = Cursor::new(self.schema.clone(), exec_response.values);
    let mut items = Vec::new();
    while let Some(item) = cursor.next().await {
        items.push(item?);
    }
    
    // Extract pagination metadata
    let (next_cursor, prev_cursor) = match exec_response.metadata {
        Some(metadata) => (metadata.next_cursor, metadata.prev_cursor),
        None => (None, None),
    };
    
    Ok(Page::new(items, statement, next_cursor, prev_cursor))
}
```

## Key Design Decisions

1. **Single Source of Truth**: The `extract_cursor` function is the only place that knows how to extract cursors. No redundant `order_by_indices`.

2. **Type Safety**: Cursor extraction function uses actual inferred types from the schema, not `Type::Any`.

3. **Automatic Tie-Breaking**: The planner automatically appends primary key to ORDER BY when needed for uniqueness.

4. **Transparent Field Addition**: ORDER BY fields are added to returning clause transparently, and filtered out via the project function.

5. **Metadata Threading**: `ExecResponse` flows through VarStore, preserving metadata through the pipeline.

## Testing Strategy

1. **Unit Tests**: Test cursor extraction function generation
2. **Integration Tests**: Test pagination with various ORDER BY configurations
3. **Database Tests**: Ensure SQL generation is correct (no `PaginateForward` in SQL)
4. **End-to-End Tests**: Verify pagination works across different databases

## Future Enhancements

1. **Previous Page Support**: Implement `prev_cursor` extraction and `PaginateBackward`
2. **DynamoDB Native Pagination**: Leverage LastEvaluatedKey instead of limit+1
3. **Complex ORDER BY**: Support expressions beyond simple column references
4. **Optimization**: Cache cursor extraction functions for common patterns