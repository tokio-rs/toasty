//! Phase 1: mechanical value extraction — replace scalar `Value` nodes with
//! `Arg(n)` placeholders, initializing each param's type from the value itself.

use toasty_core::{
    driver::Capability,
    schema::db,
    stmt::{self, VisitMut},
};

use super::{Param, Ty};

/// Replace all scalar `Value` nodes with `Arg(n)` placeholders, initializing
/// each param's `ty` from the value itself.
pub(super) fn extract_values(
    stmt: &mut stmt::Statement,
    params: &mut Vec<Param>,
    capability: &Capability,
) {
    struct Extract<'a> {
        params: &'a mut Vec<Param>,
        bind_list_param: bool,
        glob_starts_with: bool,
        binary_like_starts_with: bool,
    }

    impl stmt::VisitMut for Extract<'_> {
        fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
            // Intercept ANY/ALL: bind their array operand as one Value::List
            // param rather than visiting the rhs and extracting each element
            // separately. The element type is refined to the column type by
            // the synthesize/check pass.
            match expr {
                stmt::Expr::AnyOp(e) => {
                    self.visit_expr_mut(&mut e.lhs);
                    if let Some(arg) = extract_array_operand(&mut e.rhs, self.params) {
                        *e.rhs = arg;
                    } else {
                        self.visit_expr_mut(&mut e.rhs);
                    }
                    return;
                }
                stmt::Expr::AllOp(e) => {
                    self.visit_expr_mut(&mut e.lhs);
                    if let Some(arg) = extract_array_operand(&mut e.rhs, self.params) {
                        *e.rhs = arg;
                    } else {
                        self.visit_expr_mut(&mut e.rhs);
                    }
                    return;
                }
                // `IN (...)` always renders as N separate placeholders on
                // backends without `predicate_match_any` (the ones that support
                // ANY were rewritten to it during lowering). Force per-element
                // expansion of the rhs list regardless of `bind_list_param`, so
                // `id IN (1, 2, 3)` stays N placeholders rather than collapsing
                // to `id IN ?` on backends that bind a `Vec<scalar>` column as
                // one param.
                stmt::Expr::InList(e) => {
                    self.visit_expr_mut(&mut e.expr);
                    if let stmt::Expr::Value(stmt::Value::List(_)) = e.list.as_ref() {
                        let stmt::Expr::Value(stmt::Value::List(items)) =
                            std::mem::replace(e.list.as_mut(), stmt::Expr::null())
                        else {
                            unreachable!()
                        };
                        let items = items
                            .into_iter()
                            .map(|v| value_to_extracted_expr(v, self.params, false))
                            .collect();
                        *e.list = stmt::Expr::List(stmt::ExprList { items });
                    } else {
                        self.visit_expr_mut(&mut e.list);
                    }
                    return;
                }
                // For SQLite/MySQL, transform the prefix into the final search
                // pattern before binding it: GLOB needs `*`/`?`/`[` escaped and
                // a `*` appended; BINARY LIKE needs `%`/`_`/`!` escaped and a
                // `%` appended.  The column expression is visited normally.
                stmt::Expr::StartsWith(e)
                    if self.glob_starts_with || self.binary_like_starts_with =>
                {
                    self.visit_expr_mut(&mut e.expr);
                    let stmt::Expr::Value(stmt::Value::String(prefix)) = e.prefix.as_ref() else {
                        panic!("starts_with prefix must be a string literal");
                    };
                    let pattern = if self.glob_starts_with {
                        glob_prefix_pattern(prefix)
                    } else {
                        binary_like_prefix_pattern(prefix)
                    };
                    let position = self.params.len();
                    self.params.push(Param {
                        value: stmt::Value::String(pattern),
                        ty: Ty::Inferred(db::Type::Text),
                    });
                    *e.prefix = stmt::Expr::arg(position);
                    return;
                }
                _ => {}
            }

            // On backends that bind arrays as a single protocol parameter
            // (PostgreSQL, see `Capability::bind_list_param`), a literal
            // list of scalar values is the value of a `Vec<scalar>` model
            // field — extract as one `Value::List` arg so it round-trips
            // through the driver as a `text[]` / `int8[]` bind. Without
            // this, recursion would expand the list to one arg per item
            // and render it as a SQL record literal.
            if self.bind_list_param
                && is_scalar_list(expr)
                && let Some(arg) = extract_array_operand(expr, self.params)
            {
                *expr = arg;
                return;
            }

            // Default post-order: recurse first, then maybe extract this node.
            stmt::visit_mut::visit_expr_mut(self, expr);

            match expr {
                stmt::Expr::Value(value) if is_extractable_scalar(value) => {
                    let ty = infer_ty(value);
                    let position = self.params.len();
                    let value = std::mem::replace(value, stmt::Value::Null);
                    self.params.push(Param { value, ty });
                    *expr = stmt::Expr::arg(position);
                }
                // A bare `#[document]` embed value (named by the mapping's
                // lowering cast) binds as one param with an unknown type; the
                // synthesize/check pass resolves it to the document column
                // type.
                stmt::Expr::Value(value @ stmt::Value::Object(_)) => {
                    let owned = std::mem::replace(value, stmt::Value::Null);
                    let position = self.params.len();
                    self.params.push(Param {
                        value: owned,
                        ty: Ty::Unknown,
                    });
                    *expr = stmt::Expr::arg(position);
                }
                stmt::Expr::Value(value @ (stmt::Value::Record(_) | stmt::Value::List(_))) => {
                    let owned = std::mem::replace(value, stmt::Value::Null);
                    *expr = value_to_extracted_expr(owned, self.params, self.bind_list_param);
                }
                _ => {}
            }
        }
    }

    Extract {
        params,
        bind_list_param: capability.bind_list_param,
        glob_starts_with: capability.glob_starts_with,
        binary_like_starts_with: capability.binary_like_starts_with,
    }
    .visit_mut(stmt);
}

/// Whether `expr` is an `Expr::Value` carrying an extractable scalar.
fn is_extractable_scalar_expr(expr: &stmt::Expr) -> bool {
    matches!(expr, stmt::Expr::Value(v) if is_extractable_scalar(v))
}

/// Whether `expr` is a literal list of scalar values — either an
/// `Expr::List` of `Expr::Value(...)` items, or an already-collapsed
/// `Expr::Value(Value::List(...))`. The canonicalizer (`fold::expr_list`)
/// produces the latter shape, but lowering can still emit the former, so
/// we cover both.
fn is_scalar_list(expr: &stmt::Expr) -> bool {
    match expr {
        stmt::Expr::List(list) => list.items.iter().all(is_extractable_scalar_expr),
        stmt::Expr::Value(stmt::Value::List(items)) => items.iter().all(is_extractable_scalar),
        _ => false,
    }
}

/// If `expr` is a list literal of values, take it out, push one
/// `Param { value: Value::List(items), ty: Ty::List(<elem>) }` onto `params`,
/// and return an `Expr::Arg(n)` to put back in its place. Used for both the
/// `ANY/ALL` rhs operand and `Vec<scalar>` field literals on backends that
/// bind arrays as a single protocol parameter.
///
/// The element type starts as the value-inferred type of the first non-null
/// item — or `Ty::Unknown` for empty / all-null lists. The synthesize/check
/// pass refines it to the column type when one is known.
fn extract_array_operand(expr: &mut stmt::Expr, params: &mut Vec<Param>) -> Option<stmt::Expr> {
    let items: Vec<stmt::Value> = match expr {
        stmt::Expr::Value(stmt::Value::List(_)) => {
            let stmt::Expr::Value(stmt::Value::List(items)) =
                std::mem::replace(expr, stmt::Expr::null())
            else {
                unreachable!()
            };
            items
        }
        stmt::Expr::List(list) if list.items.iter().all(|i| matches!(i, stmt::Expr::Value(_))) => {
            let stmt::Expr::List(list) = std::mem::replace(expr, stmt::Expr::null()) else {
                unreachable!()
            };
            list.items
                .into_iter()
                .map(|e| match e {
                    stmt::Expr::Value(v) => v,
                    _ => unreachable!(),
                })
                .collect()
        }
        _ => return None,
    };

    let value = stmt::Value::List(items);
    let ty = infer_ty(&value);

    let position = params.len();
    params.push(Param { value, ty });
    Some(stmt::Expr::arg(position))
}

/// Recursively convert a `Value` into an `Expr`, extracting scalar values.
/// Takes ownership to avoid cloning.
///
/// On backends that bind arrays as a single protocol parameter (`bind_list_param`),
/// a `Value::List` of all extractable scalars is captured as a single param of
/// `Value::List` shape so it round-trips through the driver as one array bind.
/// Other lists fall through to per-element expansion to preserve the existing
/// record/tuple semantics on backends without native array binds.
fn value_to_extracted_expr(
    value: stmt::Value,
    params: &mut Vec<Param>,
    bind_list_param: bool,
) -> stmt::Expr {
    match value {
        stmt::Value::Null => stmt::Expr::Value(stmt::Value::Null),
        stmt::Value::Record(record) => {
            let fields = record
                .fields
                .into_iter()
                .map(|f| value_to_extracted_expr(f, params, bind_list_param))
                .collect();
            stmt::Expr::Record(stmt::ExprRecord::from_vec(fields))
        }
        stmt::Value::List(values)
            if bind_list_param
                && values.iter().all(|v| {
                    // A `Vec<scalar>` collection, or a `#[document]` collection
                    // of embedded structs (named `Value::Object`s by the
                    // mapping's lowering cast). Either way the whole list binds
                    // as one parameter; the synthesize/check pass resolves its
                    // type.
                    is_extractable_scalar(v) || matches!(v, stmt::Value::Object(_))
                }) =>
        {
            let value = stmt::Value::List(values);
            let ty = infer_ty(&value);
            let position = params.len();
            params.push(Param { value, ty });
            stmt::Expr::arg(position)
        }
        stmt::Value::List(values) => {
            let items = values
                .into_iter()
                .map(|v| value_to_extracted_expr(v, params, bind_list_param))
                .collect();
            stmt::Expr::List(stmt::ExprList { items })
        }
        scalar => {
            let ty = infer_ty(&scalar);
            let position = params.len();
            params.push(Param { value: scalar, ty });
            stmt::Expr::arg(position)
        }
    }
}

fn is_extractable_scalar(value: &stmt::Value) -> bool {
    // A `Value::Object` is a named document blob, not a scalar: it binds as one
    // param via the dedicated `Expr::Value(Object)` arm / `value_to_extracted_expr`,
    // never through scalar extraction.
    !matches!(
        value,
        stmt::Value::Null | stmt::Value::Record(_) | stmt::Value::List(_) | stmt::Value::Object(_)
    )
}

/// Initial type guess for a value, used as the starting point for inference.
///
/// Returns the most precise `Ty` derivable from the value alone:
/// - Scalars become `Ty::Inferred(<db::Type>)`.
/// - Lists become `Ty::List(<elem>)`, recursing into the first non-null item.
///   Empty / all-null lists yield `Ty::List(Ty::Unknown)`; the element type is
///   refined by synthesize/check when a column context is available.
/// - Anything we can't classify (`Null`, `Record`, `F32`/`F64`, `Zoned`,
///   `BigDecimal`, `SparseRecord`) becomes `Ty::Unknown`.
pub(super) fn infer_ty(value: &stmt::Value) -> Ty {
    use stmt::Value;
    match value {
        Value::Bool(_) => Ty::Inferred(db::Type::Boolean),
        Value::I8(_) => Ty::Inferred(db::Type::Integer(1)),
        Value::I16(_) => Ty::Inferred(db::Type::Integer(2)),
        Value::I32(_) => Ty::Inferred(db::Type::Integer(4)),
        Value::I64(_) => Ty::Inferred(db::Type::Integer(8)),
        Value::U8(_) => Ty::Inferred(db::Type::UnsignedInteger(1)),
        Value::U16(_) => Ty::Inferred(db::Type::UnsignedInteger(2)),
        Value::U32(_) => Ty::Inferred(db::Type::UnsignedInteger(4)),
        Value::U64(_) => Ty::Inferred(db::Type::UnsignedInteger(8)),
        Value::String(_) => Ty::Inferred(db::Type::Text),
        Value::Uuid(_) => Ty::Inferred(db::Type::Uuid),
        Value::Bytes(_) => Ty::Inferred(db::Type::Blob),
        #[cfg(feature = "rust_decimal")]
        Value::Decimal(_) => Ty::Inferred(db::Type::Numeric(None)),
        #[cfg(feature = "jiff")]
        Value::Timestamp(_) => Ty::Inferred(db::Type::Timestamp(6)),
        #[cfg(feature = "jiff")]
        Value::Date(_) => Ty::Inferred(db::Type::Date),
        #[cfg(feature = "jiff")]
        Value::Time(_) => Ty::Inferred(db::Type::Time(6)),
        #[cfg(feature = "jiff")]
        Value::DateTime(_) => Ty::Inferred(db::Type::DateTime(6)),
        Value::List(items) => {
            let elem = items
                .iter()
                .find(|v| !v.is_null())
                .map(infer_ty)
                .unwrap_or(Ty::Unknown);
            Ty::List(Box::new(elem))
        }
        _ => Ty::Unknown,
    }
}

// ============================================================================
// `starts_with` pattern builders (used when a backend lowers it to GLOB / LIKE)
// ============================================================================

/// Build a SQLite GLOB pattern for a `starts_with(prefix)` expression.
///
/// GLOB metacharacters (`*`, `?`, `[`) are escaped by wrapping them in a
/// bracket class: `*` → `[*]`, `?` → `[?]`, `[` → `[[]`. A trailing `*`
/// wildcard is appended so the pattern matches any string starting with
/// `prefix`. GLOB has no ESCAPE clause, so bracket-class escaping is the only
/// available mechanism.
pub(super) fn glob_prefix_pattern(prefix: &str) -> String {
    // Each metachar expands to 3 chars; over-allocate slightly rather than
    // under-allocate and trigger a realloc on prefixes with wildcards.
    let mut pattern = String::with_capacity(prefix.len() * 3 + 1);
    for c in prefix.chars() {
        match c {
            '*' => pattern.push_str("[*]"),
            '?' => pattern.push_str("[?]"),
            '[' => pattern.push_str("[[]"),
            c => pattern.push(c),
        }
    }
    pattern.push('*');
    pattern
}

/// Build a MySQL `BINARY col LIKE ? ESCAPE '!'` pattern for `starts_with(prefix)`.
///
/// `!` is the hardcoded escape character. In a single pass, `!`, `%`, and `_`
/// are all prefixed with `!` (so `!` → `!!`, `%` → `!%`, `_` → `!_`). A
/// trailing `%` wildcard is appended.
pub(super) fn binary_like_prefix_pattern(prefix: &str) -> String {
    let mut pattern = String::with_capacity(prefix.len() + 1);
    for c in prefix.chars() {
        match c {
            '!' | '%' | '_' => {
                pattern.push('!');
                pattern.push(c);
            }
            c => pattern.push(c),
        }
    }
    pattern.push('%');
    pattern
}
