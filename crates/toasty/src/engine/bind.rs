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
    // Phase 1: mechanical extraction — replace values with Arg(n).
    let mut params: Vec<Param> = Vec::new();
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
