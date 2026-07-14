/// Helper function to look up TableId by table name (handles database-specific prefixes)
pub fn table_id(db: &toasty::Db, table_name: &str) -> toasty_core::schema::db::TableId {
    let schema = db.schema();

    // First try exact match
    if let Some(position) = schema.db.tables.iter().position(|t| t.name == table_name) {
        return toasty_core::schema::db::TableId(position);
    }

    // If not found, try to find a table that ends with the given name (for database prefixes)
    if let Some(position) = schema
        .db
        .tables
        .iter()
        .position(|t| t.name.ends_with(table_name))
    {
        return toasty_core::schema::db::TableId(position);
    }

    // If still not found, show available tables for debugging
    let available_tables: Vec<_> = schema.db.tables.iter().map(|t| &t.name).collect();
    panic!(
        "Table '{}' not found. Available tables: {:?}",
        table_name, available_tables
    );
}

/// Helper function to get a single ColumnId for specified table and column
pub fn column(
    db: &toasty::Db,
    table_name: &str,
    column_name: &str,
) -> toasty_core::schema::db::ColumnId {
    columns(db, table_name, &[column_name])[0]
}

/// Helper function to generate a `Vec<ColumnId>` for specified table and columns
pub fn columns(
    db: &toasty::Db,
    table_name: &str,
    column_names: &[&str],
) -> Vec<toasty_core::schema::db::ColumnId> {
    let schema = db.schema();

    // Find the table using the same logic as table_id (handles prefixes)
    let table = schema
        .db
        .tables
        .iter()
        .find(|t| t.name == table_name || t.name.ends_with(table_name))
        .unwrap_or_else(|| panic!("Table '{}' not found", table_name));

    let table_id = table_id(db, table_name);

    column_names
        .iter()
        .map(|col_name| {
            let index = table
                .columns
                .iter()
                .position(|c| c.name == *col_name)
                .unwrap_or_else(|| {
                    panic!("Column '{}' not found in table '{}'", col_name, table_name)
                });

            toasty_core::schema::db::ColumnId {
                table: table_id,
                index,
            }
        })
        .collect()
}

use std::collections::BTreeMap;

use toasty_core::{
    driver::{Operation, operation::TypedValue},
    stmt::{Assignment, Expr, ExprSet, Statement, Value},
};

use crate::Test;

/// Resolve a value expression: a literal `Value`, or a bound param looked up in
/// `params` (SQL drivers replace scalars with `Expr::Arg` placeholders).
fn expr_value(expr: &Expr, params: &[Value]) -> Value {
    match expr {
        Expr::Value(value) => value.clone(),
        Expr::Arg(arg) => params[arg.position].clone(),
        other => panic!("expected a value expression, got {other:#?}"),
    }
}

fn params_of(params: Vec<TypedValue>) -> Vec<Value> {
    params.into_iter().map(|tv| tv.value).collect()
}

/// Pop the next logged op (a create) and return `column index -> inserted
/// value`, normalizing SQL (`QuerySql`) vs key-value (`Insert`) ops and inlining
/// bound params. Useful for asserting exactly what an `INSERT` writes.
pub fn pop_insert(test: &mut Test) -> BTreeMap<usize, Value> {
    let (op, _) = test.log().pop();
    let (stmt, params) = match op {
        Operation::QuerySql(q) => (q.stmt, params_of(q.params)),
        Operation::Insert(i) => (i.stmt, params_of(i.params)),
        other => panic!("expected an insert op, got {other:#?}"),
    };
    let Statement::Insert(insert) = stmt else {
        panic!("expected an Insert statement");
    };
    let toasty_core::stmt::InsertTarget::Table(target) = &insert.target else {
        panic!("expected a table insert target");
    };
    let ExprSet::Values(values) = &insert.source.body else {
        panic!("expected a VALUES source");
    };
    // The single row is an `Expr::Record` of value/param exprs (SQL) or an
    // already-evaluated `Value::Record` (key-value drivers).
    let row: Vec<Value> = match &values.rows[0] {
        Expr::Record(record) => record
            .fields
            .iter()
            .map(|expr| expr_value(expr, &params))
            .collect(),
        Expr::Value(Value::Record(record)) => record.fields.clone(),
        other => panic!("expected a record row, got {other:#?}"),
    };
    target
        .columns
        .iter()
        .zip(row)
        .map(|(col, value)| (col.index, value))
        .collect()
}

/// Pop the next logged op (an update) and return `column index -> assigned
/// value`, normalizing SQL (`QuerySql`) vs key-value (`UpdateByKey`) ops and
/// inlining bound params.
pub fn pop_update(test: &mut Test) -> BTreeMap<usize, Value> {
    let (op, _) = test.log().pop();
    let (assignments, params) = match op {
        Operation::QuerySql(q) => match q.stmt {
            Statement::Update(update) => (update.assignments, params_of(q.params)),
            other => panic!("expected an Update statement, got {other:#?}"),
        },
        Operation::UpdateByKey(update) => (update.assignments, vec![]),
        other => panic!("expected an update op, got {other:#?}"),
    };
    assignments
        .iter()
        .map(|(projection, assignment)| {
            let Assignment::Set(expr) = assignment else {
                panic!("expected a Set assignment, got {assignment:#?}");
            };
            (projection.as_slice()[0], expr_value(expr, &params))
        })
        .collect()
}

/// Pop the next logged op (a read) and return its filter predicate, normalizing
/// SQL (`QuerySql` select) vs key-value (`Scan`) ops.
pub fn pop_filter(test: &mut Test) -> Expr {
    let (op, _) = test.log().pop();
    match op {
        Operation::QuerySql(q) => {
            let Statement::Query(query) = q.stmt else {
                panic!("expected a Query statement");
            };
            let ExprSet::Select(select) = query.body else {
                panic!("expected a Select body");
            };
            select.filter.expr.expect("filter predicate present")
        }
        Operation::Scan(scan) => scan.filter.expect("filter predicate present"),
        other => panic!("expected a query/scan op, got {other:#?}"),
    }
}
