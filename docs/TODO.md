# TODO Items for Toasty

## Pagination Implementation
- [ ] Phase 1: Update VarStore to ExecResponse (mechanical change)
- [ ] Phase 2: Add Pagination struct to ExecStatement
- [ ] Phase 3: Implement planner support for SQL pagination
- [ ] Phase 4: Implement executor pagination logic
- [ ] Phase 5: Clean up application layer

## Future Enhancements

### Clean up exec/exec_statement.rs
Code has gotten a bit spread out, we should take a look to see how it can be cleaned up.

### Metadata for Non-Query Operations
Consider passing count metadata through ExecResponse for INSERT/UPDATE/DELETE operations. This would allow:
- Returning affected row counts in metadata
- Providing additional execution statistics
- Supporting batch operation summaries

### Error Handling in Pipeline
Improve error handling when pagination metadata generation fails mid-pipeline:
- Ensure proper cleanup of partial metadata
- Consider transaction-like semantics for metadata generation
- Add detailed error context for debugging

### Previous Page Support
Implement `PaginateBackward` variant and `prev_cursor` extraction:
- Add `Limit::PaginateBackward` variant
- Implement reverse cursor extraction logic
- Handle ORDER BY direction reversal

### DynamoDB Native Pagination
Leverage DynamoDB's LastEvaluatedKey instead of limit+1 strategy:
- Detect DynamoDB driver via capabilities
- Pass through native pagination tokens
- Avoid unnecessary data fetching

### Complex ORDER BY Support
Support expressions beyond simple column references in ORDER BY:
- Handle computed fields (e.g., `ORDER BY price * quantity`)
- Support function calls in ORDER BY
- Manage type inference for complex expressions