# Toasty Development Context

## Project Overview

Toasty is an ORM for Rust that supports both SQL and NoSQL databases. The project is in incubating status - not production-ready, with evolving APIs and limited documentation.

## How to Use This Documentation

When working on Toasty, follow this decision tree to efficiently navigate the codebase and make changes:

### 1. Starting a Task

**First, understand what type of change you're making:**
- Is it a new feature? → Check docs/CHANGE_GUIDE.md for similar patterns
- Is it a bug fix? → Read relevant component CONTEXT.md files
- Is it a refactor? → Review docs/ARCHITECTURE.md for design principles

### 2. Component Navigation

**Use the architecture layers to find where changes belong:**

```
User-facing API changes → crates/toasty/
Code generation changes → crates/toasty-codegen/
Core types/schemas → crates/toasty-core/
SQL generation → crates/toasty-sql/
Database-specific → crates/toasty-driver-*/
```

### 3. Making Changes - Follow the Flow

**Most changes cascade through layers in this order:**

1. **Define** in toasty-core (types, schemas, statements)
2. **Generate** in toasty-codegen (user API)
3. **Process** in toasty/engine (simplify → plan → execute)
4. **Serialize** in toasty-sql (if SQL databases)
5. **Execute** in drivers (database-specific)

### 4. Quick Reference for Common Tasks

#### Adding a Primitive Type
```
Start: docs/CHANGE_GUIDE.md § "Adding a New Primitive Type"
Read: toasty-core/CONTEXT.md § "Adding a New Primitive Type"
Follow: The 7-step process with example commits
```

#### Adding a Query Feature (ORDER BY, LIMIT, etc.)
```
Start: docs/CHANGE_GUIDE.md § "Adding a Query Feature"
Read: toasty/CONTEXT.md § "Query Execution Pipeline"
Check: toasty-sql/CONTEXT.md for SQL generation
```

#### Implementing a Database Driver
```
Start: toasty-driver-sqlite/CONTEXT.md (template for all drivers)
Read: docs/ARCHITECTURE.md § "Driver Layer"
Reference: Existing driver implementations
```

#### Optimizing Queries
```
Start: toasty/CONTEXT.md § "Simplification"
Read: Recent commits for simplification patterns
Focus: engine/simplify/ directory
```

### 5. Understanding Change Impact

**Before making changes, trace the impact:**

1. **Read the component's CONTEXT.md** to understand responsibilities
2. **Check docs/ARCHITECTURE.md** for layer dependencies
3. **Review docs/CHANGE_GUIDE.md** for similar past changes
4. **Look for Visit/VisitMut** traits that need updating (for AST changes)

### 6. Testing Strategy

**Testing Guidelines:**
- **No unit tests**: Avoid `#[test]` or `#[cfg(test)]` in source code unless explicitly requested
- **Test public APIs only**: Focus on testing the user-facing interface, not internal implementation
- **Full-stack database tests**: Go in the workspace `tests/` crate (uses actual databases)
- **Non-database tests**: Go in individual crate's `tests/` directory (e.g., testing stmt types)

**Based on change type:**
- **Core types**: Unit tests in module
- **Codegen**: UI tests for compile errors
- **Engine**: Integration tests across all drivers
- **Drivers**: Database-specific integration tests

**Test placement examples:**
- Database integration tests → `tests/tests/my_feature.rs`
- Statement parsing tests → `crates/toasty-core/tests/stmt_tests.rs`

### 7. Common Patterns to Follow

**From recent development:**
- Use specific imports, not glob imports
- Generate minimal code with fully qualified paths (#toasty::)
- Implement Visit/VisitMut for new AST nodes
- Handle NULL in value conversions
- Test with all database drivers

### 8. When You're Stuck

**Debugging approach:**
1. Check what layer the issue is in
2. Read that component's CONTEXT.md
3. Find similar code in recent commits (docs/CHANGE_GUIDE.md has examples)
4. Trace through the execution pipeline
5. Use debug assertions and dbg!() macros

### 9. Architecture Principles to Maintain

**Keep these in mind:**
- Zero-cost abstractions (no runtime overhead)
- Type safety (catch errors at compile time)
- Driver abstraction (work uniformly across databases)
- Stream processing (avoid loading everything into memory)
- Index awareness (use indexes when available)

### 10. Quick Component Summary

- **toasty-core**: Defines all types and interfaces (no implementation)
- **toasty-codegen**: Generates user-facing API from macros
- **toasty**: Runtime engine that executes queries
- **toasty-sql**: Converts statements to SQL strings
- **toasty-driver-***: Database-specific implementations

## Build and Test Commands

```bash
# Basic commands
cargo build
cargo test
cargo check

# Test specific database backends
cargo test --features sqlite
cargo test --features postgresql
cargo test --features mysql
cargo test --features dynamodb

# Run/check examples
./scripts/gen-examples run    # Run all examples
./scripts/gen-examples        # Check all examples

# Run a single test
cargo test test_name -- --nocapture
```

## Working Process for Engine Changes

When working on Toasty engine changes, follow this established pattern:

1. **Clarify Before Implementing**
   - Ask follow-up questions to understand the full scope
   - Identify edge cases and potential issues
   - Understand how the change fits into Toasty's architecture

2. **Design and Sketch First**
   - Present a design sketch or plan before writing code
   - Break complex changes into phases
   - Get feedback on the approach
   - Use TodoWrite to track multi-phase implementations

3. **Consult Documentation**
   - Review `docs/ARCHITECTURE.md` for system design
   - Check existing similar code for patterns

4. **Implement Incrementally**
   - Make changes in small, compilable increments
   - Test each phase before moving to the next
   - Document learnings as you go

## Implementation Strategy

- **Always create an implementation plan before starting work** - break changes into small, manageable steps
- **Bias toward small increments** - prefer multiple small changes over large ones
- **Report unexpected fallout immediately** - do not power through compilation errors or test failures
- **Unexpected issues indicate faulty assumptions** - stop and reassess implementation strategy
- **Validate each step** - ensure each increment compiles and existing tests pass before proceeding
- **PAUSE AND ASK when encountering architectural decisions** - If you need to:
  - Make private APIs public
  - Add new dependencies
  - Duplicate existing code
  - Work around architectural constraints
  
  Stop and ask: "I need to [describe need]. The current code [describe constraint]. Should I [option A] or [option B]?"

## Development Notes

- The codebase uses async/await throughout - all database operations are async
- Error handling uses `anyhow::Result<T>` - comprehensive error handling is still being developed
- When modifying drivers, ensure changes work across all database backends
- Schema changes require updates to both app and db schema representations
- New model features need implementation in toasty-macros and toasty-codegen
- **IMPORTANT**: Before modifying code in `crates/toasty/src/engine`, consult `docs/ARCHITECTURE.md` for detailed engine architecture documentation
- Test infrastructure: Multi-database test infrastructure in `tests/` crate
- Tests run against all enabled database backends via feature flags
- Test macros: `tests!()` and `models!()` for multi-database testing
- Connection strings: `"sqlite::memory:"`, `"postgres://..."`

## File Reference

- **docs/ARCHITECTURE.md**: System design and component relationships
- **docs/CHANGE_GUIDE.md**: How to make common changes with examples
- **docs/CONTEXT.md**: Writing style guidelines for documentation (load when editing docs/)
- **Component CONTEXT.md files**: Deep dive into each component
- **This file (CLAUDE.md)**: Navigation guide for using the documentation

## Context Files

When editing files in certain directories, load the corresponding CONTEXT.md:

- **docs/**: Load `docs/CONTEXT.md` for documentation writing guidelines

## Remember

The codebase follows predictable patterns. Most changes you'll make have been done before in similar ways. Use the commit history and documentation to find these patterns and follow them.