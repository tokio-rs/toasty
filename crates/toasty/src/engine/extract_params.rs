//! Extract typed bind parameters from a fully-resolved statement.
//!
//! Walks the statement AST to infer the `db::Type` for each value node from
//! its column context, then extracts scalar values into `Vec<TypedValue>` and
//! replaces them with `Expr::Arg(n)` placeholders.

use std::collections::HashMap;

use toasty_core::{
    driver::operation::TypedValue,
    schema::{Schema, db},
    stmt::{self, ExprContext},
};

/// Infer database storage types, extract scalar values as typed bind
/// parameters, and replace them with `Expr::Arg(n)` placeholders.
pub(crate) fn extract_params(stmt: &mut stmt::Statement, schema: &Schema) -> Vec<TypedValue> {
    // Phase 1: Infer db::Type for expression nodes using ExprContext for
    // column resolution.
    let db_types = infer_db_types(stmt, schema);

    // Phase 2+3: Extract values and replace with Arg placeholders.
    let mut params = Vec::new();
    extract_and_replace(stmt, &db_types, &schema.db, &mut params);

    params
}

// ---------------------------------------------------------------------------
// Phase 1: Bottom-up type inference
// ---------------------------------------------------------------------------

/// Maps expression nodes (by pointer address) to their inferred `db::Type`.
struct DbTypes {
    types: HashMap<usize, db::Type>,
}

impl DbTypes {
    fn new() -> Self {
        Self {
            types: HashMap::new(),
        }
    }

    fn insert(&mut self, expr: &stmt::Expr, ty: db::Type) {
        self.types.insert(expr as *const _ as usize, ty);
    }

    fn get(&self, expr: &stmt::Expr) -> Option<&db::Type> {
        self.types.get(&(expr as *const _ as usize))
    }
}

fn infer_db_types(stmt: &stmt::Statement, schema: &Schema) -> DbTypes {
    let mut db_types = DbTypes::new();
    let cx = ExprContext::new(&schema.db);

    match stmt {
        stmt::Statement::Insert(insert) => {
            let _cx = cx.scope(insert);
            infer_insert(&mut db_types, insert, &schema.db);
            // INSERT doesn't have a filter to infer
        }
        stmt::Statement::Update(update) => {
            let cx = cx.scope(update);
            infer_update_assignments(&mut db_types, update, &schema.db);
            infer_filter_expr(&mut db_types, &update.filter, &cx, &schema.db);
        }
        stmt::Statement::Delete(delete) => {
            let cx = cx.scope(delete);
            infer_filter_expr(&mut db_types, &delete.filter, &cx, &schema.db);
        }
        stmt::Statement::Query(query) => {
            let cx = cx.scope(query);
            if let stmt::ExprSet::Select(select) = &query.body {
                let cx = cx.scope(&**select);
                infer_filter_expr(&mut db_types, &select.filter, &cx, &schema.db);
            }
        }
    }

    db_types
}

fn infer_insert(db_types: &mut DbTypes, insert: &stmt::Insert, db_schema: &db::Schema) {
    // Get column storage types from the INSERT target
    let col_types: Vec<db::Type> = match &insert.target {
        stmt::InsertTarget::Table(table) => {
            let db_table = &db_schema.tables[table.table.0];
            table
                .columns
                .iter()
                .map(|col_id| db_table.columns[col_id.index].storage_ty.clone())
                .collect()
        }
        _ => return,
    };

    // Infer types for VALUES rows
    if let stmt::ExprSet::Values(values) = &insert.source.body {
        for row in &values.rows {
            infer_record_with_col_types(db_types, row, &col_types);
        }
    }
}

fn infer_update_assignments(db_types: &mut DbTypes, update: &stmt::Update, db_schema: &db::Schema) {
    let table_id = match &update.target {
        stmt::UpdateTarget::Table(table_id) => *table_id,
        _ => return,
    };

    let db_table = &db_schema.tables[table_id.0];

    for (projection, assignment) in update.assignments.iter() {
        if let stmt::Assignment::Set(expr) = assignment {
            // The projection's first step is the column index
            let steps = projection.as_slice();
            if let Some(&col_idx) = steps.first() {
                if let Some(col) = db_table.columns.get(col_idx) {
                    assign_type_to_value(db_types, expr, &col.storage_ty);
                }
            }
        }
    }
}

/// Walk a filter expression and propagate column storage types to adjacent values.
fn infer_filter_expr<T: std::fmt::Debug>(
    db_types: &mut DbTypes,
    filter: &stmt::Filter,
    cx: &ExprContext<'_, T>,
    db_schema: &db::Schema,
) {
    if let Some(expr) = &filter.expr {
        infer_expr(db_types, expr, cx, db_schema);
    }
}

fn infer_expr<T: std::fmt::Debug>(
    db_types: &mut DbTypes,
    expr: &stmt::Expr,
    cx: &ExprContext<'_, T>,
    db_schema: &db::Schema,
) {
    match expr {
        stmt::Expr::BinaryOp(binary) => {
            infer_expr(db_types, &binary.lhs, cx, db_schema);
            infer_expr(db_types, &binary.rhs, cx, db_schema);

            // Propagate: if one side is a column ref, type the other side
            let lhs_ty = resolve_column_storage_ty(&binary.lhs, cx, db_schema);
            let rhs_ty = resolve_column_storage_ty(&binary.rhs, cx, db_schema);

            if let Some(ty) = lhs_ty {
                assign_type_to_value(db_types, &binary.rhs, ty);
            }
            if let Some(ty) = rhs_ty {
                assign_type_to_value(db_types, &binary.lhs, ty);
            }
        }
        stmt::Expr::And(and) => {
            for op in &and.operands {
                infer_expr(db_types, op, cx, db_schema);
            }
        }
        stmt::Expr::Or(or) => {
            for op in &or.operands {
                infer_expr(db_types, op, cx, db_schema);
            }
        }
        stmt::Expr::Not(not) => {
            infer_expr(db_types, &not.expr, cx, db_schema);
        }
        stmt::Expr::InList(in_list) => {
            infer_expr(db_types, &in_list.expr, cx, db_schema);

            if let Some(ty) = resolve_column_storage_ty(&in_list.expr, cx, db_schema) {
                assign_type_to_list(db_types, &in_list.list, ty);
            }
        }
        stmt::Expr::InSubquery(in_sub) => {
            infer_expr(db_types, &in_sub.expr, cx, db_schema);
        }
        stmt::Expr::IsNull(is_null) => {
            infer_expr(db_types, &is_null.expr, cx, db_schema);
        }
        stmt::Expr::Exists(_) => {
            // Subqueries have their own scope — skip for now
        }
        stmt::Expr::Stmt(_) => {
            // Nested statement — recurse would need its own scope
        }
        _ => {}
    }
}

/// If `expr` is a column reference, resolve its storage type.
fn resolve_column_storage_ty<'a, T: std::fmt::Debug>(
    expr: &stmt::Expr,
    _cx: &ExprContext<'_, T>,
    db_schema: &'a db::Schema,
) -> Option<&'a db::Type> {
    // For now, handle the common case: ExprReference::Column at nesting=0.
    // The ExprColumn.table is a SourceTableId index, but for simple statements
    // (single-table SELECT/UPDATE/DELETE), table=0 maps to the statement's target table.
    //
    // TODO: For complex statements with joins, use ExprContext to resolve the
    // actual table. For now this covers the common case.
    let stmt::Expr::Reference(stmt::ExprReference::Column(col_ref)) = expr else {
        return None;
    };

    // We need to figure out which db table this column belongs to.
    // For simple statements, ExprColumn.table=0 refers to the statement's target.
    // The ExprContext knows this mapping, but we can't easily extract a db::TableId from it
    // without going through the full resolution path.
    //
    // Workaround: iterate over all tables and find the column by index.
    // This works because column indices are unique per table, and for single-table
    // statements there's only one table to check.
    for table in &db_schema.tables {
        if let Some(col) = table.columns.get(col_ref.column) {
            return Some(&col.storage_ty);
        }
    }
    None
}

/// Assign a db type to an expression if it's a value.
fn assign_type_to_value(db_types: &mut DbTypes, expr: &stmt::Expr, ty: &db::Type) {
    if matches!(expr, stmt::Expr::Value(_)) {
        db_types.insert(expr, ty.clone());
    }
}

/// Assign a db type to list items.
fn assign_type_to_list(db_types: &mut DbTypes, list_expr: &stmt::Expr, ty: &db::Type) {
    match list_expr {
        stmt::Expr::List(list) => {
            for item in &list.items {
                assign_type_to_value(db_types, item, ty);
            }
        }
        stmt::Expr::Value(stmt::Value::List(_)) => {
            // The whole list expression gets the type; extraction will apply it per-item
            db_types.insert(list_expr, ty.clone());
        }
        _ => {
            assign_type_to_value(db_types, list_expr, ty);
        }
    }
}

/// Infer types for a record expression given column types.
fn infer_record_with_col_types(db_types: &mut DbTypes, expr: &stmt::Expr, col_types: &[db::Type]) {
    match expr {
        stmt::Expr::Record(record) => {
            for (i, field) in record.fields.iter().enumerate() {
                if let Some(ty) = col_types.get(i) {
                    assign_type_to_value(db_types, field, ty);
                }
            }
        }
        stmt::Expr::Value(stmt::Value::Record(record)) => {
            // Value::Record — tag the containing Expr with each field's type.
            // During extraction this gets converted to Expr::Record.
            // We tag the overall expr so each nested Value gets typed.
            for (i, _field) in record.fields.iter().enumerate() {
                if i < col_types.len() {
                    // We can't tag individual Value fields directly (they're not Exprs).
                    // Instead, we'll rely on the extraction pass to convert Value::Record
                    // to Expr::Record and the field types will be inferred from the
                    // Expr::Value wrappers created during conversion.
                }
            }
            // Tag the overall expr for fallback
            if col_types.len() == 1 {
                db_types.insert(expr, col_types[0].clone());
            }
        }
        _ => {
            // Single-column insert or non-record expression
            if col_types.len() == 1 {
                assign_type_to_value(db_types, expr, &col_types[0]);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2+3: Extract values and replace with Arg placeholders
// ---------------------------------------------------------------------------

fn extract_and_replace(
    stmt: &mut stmt::Statement,
    db_types: &DbTypes,
    schema: &db::Schema,
    params: &mut Vec<TypedValue>,
) {
    // Walk ALL expressions in the statement using the visitor infrastructure.
    // This catches values everywhere: INSERT rows, filters, subqueries, CTEs, etc.
    stmt::visit_mut::for_each_expr_mut(stmt, |expr| {
        let inferred_ty = db_types.get(expr).cloned();

        match expr {
            stmt::Expr::Value(value) if is_extractable_scalar(value) => {
                let ty = inferred_ty.unwrap_or_else(|| infer_db_type_from_value(value));
                let position = params.len();
                let value = std::mem::replace(value, stmt::Value::Null);
                params.push(TypedValue { value, ty });
                *expr = stmt::Expr::arg(position);
            }
            stmt::Expr::Value(stmt::Value::List(values)) => {
                *expr = value_to_expr_with_extraction(&stmt::Value::List(values.clone()), params);
            }
            stmt::Expr::Value(stmt::Value::Record(record)) => {
                *expr = value_to_expr_with_extraction(&stmt::Value::Record(record.clone()), params);
            }
            _ => {}
        }
    });
}

/// Extract from all variants of a query body.
fn extract_from_query_body(
    body: &mut stmt::ExprSet,
    db_types: &DbTypes,
    schema: &db::Schema,
    params: &mut Vec<TypedValue>,
) {
    match body {
        stmt::ExprSet::Select(select) => {
            if let Some(expr) = &mut select.filter.expr {
                extract_expr(expr, db_types, schema, params);
            }
        }
        stmt::ExprSet::Values(values) => {
            for row in &mut values.rows {
                extract_expr(row, db_types, schema, params);
            }
        }
        _ => {}
    }
}

/// Extract values from an INSERT row, with column type context.
fn extract_insert_row(
    expr: &mut stmt::Expr,
    col_types: &[db::Type],
    db_types: &DbTypes,
    schema: &db::Schema,
    params: &mut Vec<TypedValue>,
) {
    match expr {
        stmt::Expr::Record(record) => {
            for (i, field) in record.fields.iter_mut().enumerate() {
                extract_with_col_type(field, col_types.get(i), db_types, schema, params);
            }
        }
        stmt::Expr::Value(stmt::Value::Record(record)) => {
            let mut fields: Vec<stmt::Expr> = record
                .fields
                .iter()
                .map(|v| stmt::Expr::Value(v.clone()))
                .collect();
            for (i, field) in fields.iter_mut().enumerate() {
                extract_with_col_type(field, col_types.get(i), db_types, schema, params);
            }
            *expr = stmt::Expr::Record(stmt::ExprRecord::from_vec(fields));
        }
        _ => {
            extract_with_col_type(expr, col_types.first(), db_types, schema, params);
        }
    }
}

/// Extract a value with an explicit column type override.
fn extract_with_col_type(
    expr: &mut stmt::Expr,
    col_type: Option<&db::Type>,
    db_types: &DbTypes,
    schema: &db::Schema,
    params: &mut Vec<TypedValue>,
) {
    let inferred = db_types.get(expr).cloned();

    // For scalar values with a known column type, use it directly
    if let stmt::Expr::Value(value) = expr {
        if is_extractable_scalar(value) {
            let ty = col_type
                .cloned()
                .or(inferred)
                .unwrap_or_else(|| infer_db_type_from_value(value));
            let position = params.len();
            let value = std::mem::replace(value, stmt::Value::Null);
            params.push(TypedValue { value, ty });
            *expr = stmt::Expr::arg(position);
            return;
        }
    }
    // Fall back to general extraction
    extract_expr(expr, db_types, schema, params);
}

/// Extract scalar values from an expression, replacing them with `Expr::Arg(n)`.
fn extract_expr(
    expr: &mut stmt::Expr,
    db_types: &DbTypes,
    schema: &db::Schema,
    params: &mut Vec<TypedValue>,
) {
    // Look up the inferred type before we borrow expr mutably
    let inferred_ty = db_types.get(expr).cloned();

    match expr {
        // Scalar value — extract as bind parameter
        stmt::Expr::Value(value) if is_extractable_scalar(value) => {
            let ty = inferred_ty.unwrap_or_else(|| infer_db_type_from_value(value));
            let position = params.len();
            let value = std::mem::replace(value, stmt::Value::Null);
            params.push(TypedValue { value, ty });
            *expr = stmt::Expr::arg(position);
        }

        // Value::List — extract each scalar item
        stmt::Expr::Value(stmt::Value::List(values)) => {
            let mut args = Vec::new();

            for value in values.iter() {
                if is_extractable_scalar(value) {
                    let ty = inferred_ty
                        .clone()
                        .unwrap_or_else(|| infer_db_type_from_value(value));
                    let position = params.len();
                    params.push(TypedValue {
                        value: value.clone(),
                        ty,
                    });
                    args.push(stmt::Expr::arg(position));
                } else {
                    args.push(stmt::Expr::Value(value.clone()));
                }
            }
            *expr = stmt::Expr::List(stmt::ExprList { items: args });
        }

        // NULL — leave as literal (not a bind param)
        stmt::Expr::Value(stmt::Value::Null) => {}

        // Default — leave as-is
        stmt::Expr::Default => {}

        // Record expression — recurse into fields
        stmt::Expr::Record(record) => {
            for field in &mut record.fields {
                extract_expr(field, db_types, schema, params);
            }
        }

        // List expression — recurse into items
        stmt::Expr::List(list) => {
            for item in &mut list.items {
                extract_expr(item, db_types, schema, params);
            }
        }

        // Binary op
        stmt::Expr::BinaryOp(binary) => {
            extract_expr(&mut binary.lhs, db_types, schema, params);
            extract_expr(&mut binary.rhs, db_types, schema, params);
        }

        // Logical operators
        stmt::Expr::And(and) => {
            for op in &mut and.operands {
                extract_expr(op, db_types, schema, params);
            }
        }
        stmt::Expr::Or(or) => {
            for op in &mut or.operands {
                extract_expr(op, db_types, schema, params);
            }
        }
        stmt::Expr::Not(not) => {
            extract_expr(&mut not.expr, db_types, schema, params);
        }

        // IN list
        stmt::Expr::InList(in_list) => {
            extract_expr(&mut in_list.expr, db_types, schema, params);
            extract_expr(&mut in_list.list, db_types, schema, params);
        }

        // Subquery
        stmt::Expr::InSubquery(in_sub) => {
            extract_expr(&mut in_sub.expr, db_types, schema, params);
            let mut query_stmt = stmt::Statement::Query(*in_sub.query.clone());
            extract_and_replace(&mut query_stmt, db_types, schema, params);
            if let stmt::Statement::Query(q) = query_stmt {
                in_sub.query = Box::new(q);
            }
        }

        stmt::Expr::IsNull(is_null) => {
            extract_expr(&mut is_null.expr, db_types, schema, params);
        }

        stmt::Expr::Exists(exists) => {
            let mut query_stmt = stmt::Statement::Query(*exists.subquery.clone());
            extract_and_replace(&mut query_stmt, db_types, schema, params);
            if let stmt::Statement::Query(q) = query_stmt {
                exists.subquery = Box::new(q);
            }
        }

        stmt::Expr::Stmt(expr_stmt) => {
            extract_and_replace(&mut expr_stmt.stmt, db_types, schema, params);
        }

        // Leaf nodes that don't contain extractable values
        stmt::Expr::Reference(_) | stmt::Expr::Arg(_) | stmt::Expr::Ident(_) => {}

        // Value::Record — convert to Expr::Record and extract each field
        stmt::Expr::Value(stmt::Value::Record(record)) => {
            let mut fields: Vec<stmt::Expr> = record
                .fields
                .iter()
                .map(|v| stmt::Expr::Value(v.clone()))
                .collect();
            for field in &mut fields {
                extract_expr(field, db_types, schema, params);
            }
            *expr = stmt::Expr::Record(stmt::ExprRecord::from_vec(fields));
        }

        // Catch-all
        _ => {}
    }
}

/// Recursively convert a `Value` into an `Expr`, extracting scalar values
/// as bind parameters. Handles nested Record and List values.
fn value_to_expr_with_extraction(value: &stmt::Value, params: &mut Vec<TypedValue>) -> stmt::Expr {
    match value {
        stmt::Value::Null => stmt::Expr::Value(stmt::Value::Null),
        stmt::Value::Record(record) => {
            let fields = record
                .fields
                .iter()
                .map(|f| value_to_expr_with_extraction(f, params))
                .collect();
            stmt::Expr::Record(stmt::ExprRecord::from_vec(fields))
        }
        stmt::Value::List(values) => {
            let items = values
                .iter()
                .map(|v| value_to_expr_with_extraction(v, params))
                .collect();
            stmt::Expr::List(stmt::ExprList { items })
        }
        scalar => {
            let ty = infer_db_type_from_value(scalar);
            let position = params.len();
            params.push(TypedValue {
                value: scalar.clone(),
                ty,
            });
            stmt::Expr::arg(position)
        }
    }
}

fn is_extractable_scalar(value: &stmt::Value) -> bool {
    !matches!(
        value,
        stmt::Value::Null | stmt::Value::Record(_) | stmt::Value::List(_)
    )
}

fn infer_db_type_from_value(value: &stmt::Value) -> db::Type {
    match value {
        stmt::Value::Bool(_) => db::Type::Boolean,
        stmt::Value::I8(_) => db::Type::Integer(1),
        stmt::Value::I16(_) => db::Type::Integer(2),
        stmt::Value::I32(_) => db::Type::Integer(4),
        stmt::Value::I64(_) => db::Type::Integer(8),
        stmt::Value::U8(_) => db::Type::UnsignedInteger(1),
        stmt::Value::U16(_) => db::Type::UnsignedInteger(2),
        stmt::Value::U32(_) => db::Type::UnsignedInteger(4),
        stmt::Value::U64(_) => db::Type::UnsignedInteger(8),
        stmt::Value::String(_) => db::Type::Text,
        stmt::Value::Uuid(_) => db::Type::Uuid,
        stmt::Value::Bytes(_) => db::Type::Blob,
        #[cfg(feature = "rust_decimal")]
        stmt::Value::Decimal(_) => db::Type::Numeric(None),
        #[cfg(feature = "jiff")]
        stmt::Value::Timestamp(_) => db::Type::Timestamp(6),
        #[cfg(feature = "jiff")]
        stmt::Value::Date(_) => db::Type::Date,
        #[cfg(feature = "jiff")]
        stmt::Value::Time(_) => db::Type::Time(6),
        #[cfg(feature = "jiff")]
        stmt::Value::DateTime(_) => db::Type::DateTime(6),
        _ => db::Type::Text,
    }
}
