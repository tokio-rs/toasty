# toasty-sql Component Context

## Purpose
Serializes Toasty's statement AST into SQL strings for execution by SQL database drivers. Handles database-specific SQL dialects and parameter binding strategies.

## Architecture

### Core Components

#### Serializer (`src/serializer/`)
**Sophisticated SQL Generation System:**

**Core Infrastructure:**
- **Formatter**: Manages output string, parameters, and nesting depth
- **ToSql Trait**: Universal serialization interface
- **Delimited Types**: Comma, Period separators for clean SQL
- **Macro-based Generation**: `fmt!` macro for ergonomic SQL building

**Component Serializers:**
- **stmt.rs**: Complete SQL statement generation
  - CREATE TABLE with constraints
  - INSERT with RETURNING support
  - UPDATE with complex SET clauses
  - DELETE with filtering
  - SELECT with CTEs, JOINs, ORDER BY, LIMIT
- **expr.rs**: Rich expression serialization
  - Binary operators with precedence
  - Function calls and aggregates
  - Subqueries and correlated queries
  - CASE expressions
- **cte.rs**: Common Table Expression support
- **ty.rs**: Database-specific type mapping
- **value.rs**: Literal value formatting with escaping
- **params.rs**: Smart parameter binding
- **ident.rs**: Context-aware identifier quoting

#### SQL Statements (`src/stmt/`)
SQL-specific statement representations:
- **create_table.rs**: CREATE TABLE statements
- **create_index.rs**: CREATE INDEX statements
- **drop_table.rs**: DROP TABLE statements
- **column_def.rs**: Column definitions with constraints

## SQL Dialects (Flavors)

### Supported Flavors
- **PostgreSQL**: $1, $2 parameter style, RETURNING clause
- **MySQL**: ? parameter style, backtick quotes
- **SQLite**: ? parameter style, limited types

### Dialect Differences
```rust
pub enum Flavor {
    PostgreSQL,
    MySQL,
    SQLite,
}
```

Handled differences:
- Parameter placeholder format
- Identifier quoting (backticks vs quotes)
- Type names (SERIAL vs AUTOINCREMENT)
- Supported features (RETURNING clause)
- Function names (LAST_INSERT_ID vs lastval)

## Parameter Binding

### Strategies
1. **Positional** (?, ?): MySQL, SQLite
2. **Numbered** ($1, $2): PostgreSQL
3. **Inline**: For non-parameterizable values

The serializer tracks parameters and generates appropriate placeholders based on flavor.

## Common Change Patterns

### Adding SQL Feature Support
1. Add generation in `serializer/stmt.rs`
2. Handle flavor differences
3. Update parameter tracking
4. Test with all SQL drivers

### Supporting New Types
1. Add SQL type mapping in `serializer/ty.rs`
2. Handle value serialization in `serializer/value.rs`
3. Consider database differences
4. Update column definitions

### Adding SQL Statement Type
1. Create struct in `stmt/`
2. Implement serialization
3. Add to statement enum
4. Wire up in drivers

## Serialization Flow

1. **Input**: Toasty statement AST
2. **Initialize**: Create serializer with flavor and parameter strategy
3. **Walk AST**: Recursively serialize nodes
4. **Track Parameters**: Collect parameter values
5. **Output**: SQL string + parameter list

## Important Patterns

### Identifier Quoting
```rust
// PostgreSQL/SQLite
"user_name"

// MySQL  
`user_name`
```

### Parameter Placeholders
```rust
// PostgreSQL
WHERE id = $1 AND name = $2

// MySQL/SQLite
WHERE id = ? AND name = ?
```

### Type Mapping
```rust
// Toasty Type → SQL Type
Type::String → VARCHAR(255) or TEXT
Type::I64 → BIGINT
Type::Bool → BOOLEAN or TINYINT(1)
```

## Testing Approach

- Unit tests for individual serialization functions
- Integration tests via SQL drivers
- Comparison of generated SQL across flavors
- Edge cases: empty clauses, special characters

## Performance Considerations

- String building via fmt::Write
- Minimize allocations
- Reuse serializer instances
- Prepared statement compatibility

## Recent Changes Analysis

From recent commits:
- Added ORDER BY serialization
- LIMIT/OFFSET support
- Improved type mappings for new primitives
- MySQL-specific handling improvements
- Reduced glob imports
- Better NULL handling