//! Bind a fully-resolved statement's inline values into typed parameters.
//!
//! Three phases:
//! 1. **Extract** ([`extract`]): replace scalar `Value` nodes with `Arg(n)`
//!    placeholders, initializing each param's type from the value itself.
//! 2. **Synthesize** (bottom-up): compute each node's inferred type from its
//!    children (column refs get their storage type from the schema, records get
//!    a tuple of field types, etc.).
//! 3. **Check** (top-down): push refined types into `Arg(n)` nodes, upgrading a
//!    param when context is more precise (e.g. `Enum` over `Text`).
//!
//! Synthesize and check ([`infer`]) run together in a single recursive walk.
//! Types carry **provenance** (`Column` vs `Inferred`) so schema-authoritative
//! column types win over value-inferred guesses when merging.
//!
//! `#[document]` values are named into their `Value::Object` form before this
//! runs: the mapping's lowering casts convert them during statement
//! lowering/simplification, and document *paths* are resolved by legalization
//! ([`super::legalize`]) — `Engine::prepare_for_driver` runs both in order.

use toasty_core::{
    driver::{Capability, operation::TypedValue},
    schema::db,
    stmt,
};

mod extract;
mod infer;

#[cfg(test)]
mod tests;

/// Bind a statement's inline values: replace scalar `Value` nodes with
/// `Expr::Arg(n)` placeholders and infer a precise `db::Type` for each. The
/// returned `Vec<TypedValue>` is indexed by the `n` in each placeholder.
///
/// The statement must already be legalized ([`super::legalize`]): document
/// values named, document paths resolved. Runs via
/// `Engine::prepare_for_driver`, as the final engine step before a driver
/// serializes the statement.
pub(crate) fn run(
    stmt: &mut stmt::Statement,
    db_schema: &db::Schema,
    capability: &Capability,
) -> Vec<TypedValue> {
    let mut params: Vec<Param> = Vec::new();

    // Phase 0: transpose a multi-row INSERT into per-column array params
    // where the driver can bind them as `unnest(...)`.
    transpose_insert_unnest(stmt, db_schema, capability, &mut params);

    // Phase 1: mechanical extraction — replace values with Arg(n).
    extract::extract_values(stmt, &mut params, capability);

    // Phases 2+3: bidirectional type inference — refine param types.
    infer::refine_param_types(stmt, db_schema, &mut params);

    // Materialize the final TypedValues. `finalize_ty` panics if any param is
    // still unresolved — synthesize/check is expected to type every param.
    params
        .into_iter()
        .map(|p| {
            let Param { value, ty } = p;
            TypedValue {
                ty: finalize_ty(&value, ty),
                value,
            }
        })
        .collect()
}

/// Rewrite a multi-row `INSERT ... VALUES` into a single row of per-column
/// array params, serialized as `SELECT * FROM unnest($1::t[], $2::t[])`.
/// No-op for single-row inserts, non-scalar target columns, non-value rows,
/// or when [`Capability::insert_values_unnest`] is off.
///
/// Each column array is bound as a param here, not left for extract: extract
/// refuses lists containing NULL, but a NULL cell must ride inside the array
/// bind. Must run after lowering (returning constantization assumes the
/// per-row `VALUES` shape) and before serialization.
fn transpose_insert_unnest(
    stmt: &mut stmt::Statement,
    db_schema: &db::Schema,
    capability: &Capability,
    params: &mut Vec<Param>,
) {
    if !capability.insert_values_unnest {
        return;
    }

    let stmt::Statement::Insert(insert) = stmt else {
        return;
    };
    let stmt::InsertTarget::Table(table) = &insert.target else {
        return;
    };
    let stmt::ExprSet::Values(values) = &mut insert.source.body else {
        return;
    };

    if values.rows.len() < 2 {
        return;
    }

    let num_cols = table.columns.len();
    if num_cols == 0 {
        return;
    }

    // A list/document column's per-row value is itself a collection, not a
    // single array element.
    let db_table = &db_schema.tables[table.table.0];
    let all_scalar = table
        .columns
        .iter()
        .all(|col_id| column_supports_unnest(&db_table.columns[col_id.index].storage_ty));
    if !all_scalar {
        tracing::debug!(
            table = %db_table.name,
            "multi-row INSERT not transposed to unnest: non-scalar target column"
        );
        return;
    }

    let rows_ok = values
        .rows
        .iter()
        .all(|row| row_is_transposable(row, num_cols));
    if !rows_ok {
        tracing::debug!(
            table = %db_table.name,
            "multi-row INSERT not transposed to unnest: non-value row shape"
        );
        return;
    }

    // Matched directly rather than via `Expr::into_record_items`: this is the
    // bulk-insert hot path and the generic iterator boxes per row.
    let mut columns: Vec<Vec<stmt::Value>> = (0..num_cols)
        .map(|_| Vec::with_capacity(values.rows.len()))
        .collect();
    for row in std::mem::take(&mut values.rows) {
        match row {
            stmt::Expr::Value(stmt::Value::Record(record)) => {
                for (col, value) in record.fields.into_iter().enumerate() {
                    columns[col].push(value);
                }
            }
            stmt::Expr::Record(record) => {
                for (col, field) in record.fields.into_iter().enumerate() {
                    let stmt::Expr::Value(value) = field else {
                        panic!("row validated as plain values above; field={field:?}");
                    };
                    columns[col].push(value);
                }
            }
            _ => unreachable!("row validated as a record above"),
        }
    }

    let args = columns
        .into_iter()
        .zip(table.columns.iter())
        .map(|(cells, column_id)| {
            let value = stmt::Value::List(cells);
            let ty = extract::infer_ty(&value);
            let position = params.len();
            params.push(Param { value, ty });
            stmt::FuncUnnestArg {
                expr: stmt::Expr::arg(position),
                elem_ty: db_table.columns[column_id.index].storage_ty.clone(),
            }
        })
        .collect::<Vec<_>>();

    let source = stmt::Source::table(stmt::TableRef::Func(stmt::FuncUnnest { args }.into()));
    insert.source.body = stmt::ExprSet::Select(Box::new(stmt::Select {
        returning: stmt::Returning::Star,
        source,
        filter: stmt::Filter::ALL,
        distinct: false,
    }));
}

/// A row is transposable if it is a record of exactly `num_cols` plain values.
fn row_is_transposable(row: &stmt::Expr, num_cols: usize) -> bool {
    if row.record_len() != Some(num_cols) {
        return false;
    }
    match row {
        stmt::Expr::Value(_) => true,
        stmt::Expr::Record(record) => record
            .fields
            .iter()
            .all(|field| matches!(field, stmt::Expr::Value(_))),
        _ => false,
    }
}

fn column_supports_unnest(ty: &db::Type) -> bool {
    !matches!(ty, db::Type::List(_) | db::Type::Document { .. })
}

/// A bind parameter being inferred. Once inference completes, the `Ty` is
/// converted to a concrete `db::Type` for the `TypedValue`.
struct Param {
    value: stmt::Value,
    ty: Ty,
}

/// Resolve a `Ty` to a concrete `db::Type`. Panics on `Unknown` / `Record` —
/// every param should be fully inferred by the synthesize/check pass; if a
/// statement reaches here with an unresolved param, that's a bug worth
/// surfacing so we can evaluate the specific case.
fn finalize_ty(value: &stmt::Value, ty: Ty) -> db::Type {
    match ty {
        Ty::Column(t) | Ty::Inferred(t) => t,
        Ty::List(elem) => db::Type::list(finalize_ty(value, *elem)),
        Ty::Unknown => panic!("bind left {value:?} with unresolved type"),
        Ty::Record(_) => panic!(
            "bind left {value:?} typed as a record; only scalars and lists are extracted as params"
        ),
    }
}

// ============================================================================
// Inferred type representation
// ============================================================================

/// The inferred database-level type of an expression node.
///
/// Each scalar type carries **provenance**: `Column` means the type came from
/// the schema (authoritative), `Inferred` means it was guessed from the value.
/// Column types always win when merging.
#[derive(Debug, Clone)]
enum Ty {
    /// Type from a column reference or schema (authoritative).
    Column(db::Type),
    /// Type inferred from a value (initial guess — may be less specific).
    Inferred(db::Type),
    /// A tuple of types (one per field).
    Record(Vec<Ty>),
    /// A homogeneous list where all elements share a type.
    List(Box<Ty>),
    /// Type could not be determined.
    Unknown,
}

impl Ty {
    /// Extract the `db::Type`, regardless of provenance.
    fn db_type(&self) -> Option<&db::Type> {
        match self {
            Ty::Column(ty) | Ty::Inferred(ty) => Some(ty),
            _ => None,
        }
    }

    /// Returns true if this type comes from the schema (authoritative).
    #[cfg(test)]
    fn is_column(&self) -> bool {
        matches!(self, Ty::Column(_))
    }
}
