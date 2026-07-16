use toasty_core::{schema::db, stmt::ResolvedRef};

use super::{ColumnAlias, Comma, Delimited, Ident, ToSql};

use crate::{serializer::Flavor, stmt};

impl ToSql for &stmt::Expr {
    fn to_sql(self, f: &mut super::Formatter<'_>) {
        match self {
            stmt::Expr::And(expr) => {
                fmt!(f, Delimited(expr.operands.iter().map(AndOperand), " AND "));
            }
            stmt::Expr::Between(expr) => {
                fmt!(f, expr.expr " BETWEEN " expr.low " AND " expr.high);
            }
            stmt::Expr::BinaryOp(expr) => {
                assert!(!expr.lhs.is_value_null());
                assert!(!expr.rhs.is_value_null());

                fmt!(f, expr.lhs " " expr.op " " expr.rhs);
            }
            stmt::Expr::Exists(expr) => {
                f.depth += 1;
                fmt!(f, "EXISTS (" expr.subquery ")");
                f.depth -= 1;
            }
            stmt::Expr::Func(func) => func.to_sql(f),
            stmt::Expr::IsSuperset(e) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, e.lhs.as_ref() " @> " e.rhs.as_ref()),
                // The rhs Value::List is bound as one JSON string. MySQL's
                // `JSON_CONTAINS(target, candidate)` matches when every
                // element of `candidate` appears in `target`.
                Flavor::Mysql => {
                    fmt!(f, "JSON_CONTAINS(" e.lhs.as_ref() ", " e.rhs.as_ref() ")")
                }
                // SQLite has no direct superset operator; emulate via
                // `NOT EXISTS (rhs element with no match in lhs)`.
                Flavor::Sqlite => fmt!(
                    f,
                    "NOT EXISTS (SELECT 1 FROM json_each(" e.rhs.as_ref()
                    ") AS r WHERE r.value NOT IN (SELECT l.value FROM json_each("
                    e.lhs.as_ref() ") AS l))"
                ),
            },
            stmt::Expr::Intersects(e) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, e.lhs.as_ref() " && " e.rhs.as_ref()),
                Flavor::Mysql => {
                    fmt!(f, "JSON_OVERLAPS(" e.lhs.as_ref() ", " e.rhs.as_ref() ")")
                }
                Flavor::Sqlite => fmt!(
                    f,
                    "EXISTS (SELECT 1 FROM json_each(" e.rhs.as_ref()
                    ") AS r WHERE r.value IN (SELECT l.value FROM json_each("
                    e.lhs.as_ref() ") AS l))"
                ),
            },
            stmt::Expr::Length(e) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, "cardinality(" e.expr.as_ref() ")"),
                Flavor::Mysql => fmt!(f, "JSON_LENGTH(" e.expr.as_ref() ")"),
                Flavor::Sqlite => fmt!(f, "json_array_length(" e.expr.as_ref() ")"),
            },
            stmt::Expr::Ident(name) => {
                fmt!(f, Ident(name));
            }
            stmt::Expr::InList(expr) => {
                fmt!(f, expr.expr " IN " expr.list);
            }
            stmt::Expr::AnyOp(expr) => match f.serializer.flavor {
                // `value = ANY(col)` — PostgreSQL's array membership operator.
                // Drives `Path::contains` for native-array columns and the
                // IN-list rewrite.
                Flavor::Postgresql => {
                    fmt!(f, expr.lhs " " expr.op " ANY(" expr.rhs ")");
                }
                // MySQL's `value MEMBER OF (json_array)` (8.0.17+). Only the
                // equality form makes sense; `Path::contains` is the only
                // current emitter and the lowering pass never produces
                // ANY on MySQL since `predicate_match_any` is false.
                Flavor::Mysql if matches!(expr.op, stmt::BinaryOp::Eq) => {
                    fmt!(f, expr.lhs " MEMBER OF (" expr.rhs ")");
                }
                Flavor::Mysql => unreachable!("AnyOp with non-Eq operator on MySQL: {expr:?}"),
                // SQLite renders `value = ANY(col)` (i.e. `Path::contains`)
                // as `value IN (SELECT value FROM json_each(col))`.
                Flavor::Sqlite if matches!(expr.op, stmt::BinaryOp::Eq) => {
                    fmt!(
                        f,
                        expr.lhs " IN (SELECT value FROM json_each(" expr.rhs "))"
                    );
                }
                Flavor::Sqlite => unreachable!("AnyOp with non-Eq operator on SQLite: {expr:?}"),
            },
            stmt::Expr::AllOp(expr) => {
                fmt!(f, expr.lhs " " expr.op " ALL(" expr.rhs ")");
            }
            stmt::Expr::InSubquery(expr) => {
                fmt!(f, expr.expr " IN (" expr.query ")");
            }
            stmt::Expr::IsNull(expr) => {
                fmt!(f, expr.expr " IS NULL");
            }
            stmt::Expr::Like(expr) => {
                let op =
                    if expr.case_insensitive && matches!(f.serializer.flavor, Flavor::Postgresql) {
                        " ILIKE "
                    } else {
                        " LIKE "
                    };
                fmt!(f, expr.expr op expr.pattern);
                if let Some(escape) = expr.escape {
                    let escape = if f.serializer.is_mysql() && escape == '\\' {
                        stmt::Value::String("\\\\".to_string())
                    } else {
                        stmt::Value::String(escape.to_string())
                    };
                    let escape = &escape;
                    fmt!(f, " ESCAPE " escape);
                }
            }
            stmt::Expr::StartsWith(expr) => {
                match f.serializer.flavor {
                    // PostgreSQL's `^@` prefix-match operator; prefix is bound
                    // as a plain string parameter.
                    Flavor::Postgresql => {
                        fmt!(f, expr.expr " ^@ " expr.prefix);
                    }
                    // SQLite GLOB is case-sensitive.  extract_params has already
                    // escaped GLOB metacharacters and appended `*` to the prefix
                    // parameter, so we only need to emit the right operator.
                    Flavor::Sqlite => {
                        fmt!(f, expr.expr " GLOB " expr.prefix);
                    }
                    // MySQL LIKE is case-insensitive by default; casting the
                    // column side to BINARY forces a case-sensitive byte
                    // comparison.  extract_params has escaped `%`/`_`/`!` and
                    // appended `%` to the prefix parameter.
                    Flavor::Mysql => {
                        fmt!(f, "BINARY " expr.expr " LIKE " expr.prefix " ESCAPE '!'");
                    }
                }
            }
            stmt::Expr::Not(expr) => {
                fmt!(f, "NOT (" expr.expr ")");
            }
            stmt::Expr::Or(expr) => {
                fmt!(f, Delimited(&expr.operands, " OR "));
            }
            stmt::Expr::Record(expr) => {
                let fields = Comma(expr.fields.iter());
                fmt!(f, "(" fields ")");
            }
            stmt::Expr::Reference(expr_reference @ stmt::ExprReference::Column(expr_column)) => {
                if f.alias {
                    let depth = f.depth - expr_column.nesting;

                    match f.cx.resolve_expr_reference(expr_reference) {
                        ResolvedRef::Column(column) => {
                            let name = Ident(&column.name);
                            fmt!(f, "tbl_" depth "_" expr_column.table "." name)
                        }
                        ResolvedRef::Cte { .. } | ResolvedRef::Derived(_) => {
                            fmt!(f, "tbl_" depth "_" expr_column.table "." ColumnAlias(expr_column.column))
                        }
                        ResolvedRef::Model(model) => {
                            panic!("Model references cannot be serialized to SQL; model={model:?}")
                        }
                        ResolvedRef::Field(field) => {
                            panic!("Field references cannot be serialized to SQL; field={field:?}")
                        }
                    }
                } else {
                    let column =
                        f.cx.resolve_expr_reference(expr_reference)
                            .as_column_unwrap();
                    fmt!(f, Ident(&column.name))
                }
            }
            stmt::Expr::Stmt(expr) => {
                let stmt = &*expr.stmt;
                fmt!(f, "(" stmt ")");
            }
            stmt::Expr::List(expr) => {
                let items = Comma(expr.items.iter());
                fmt!(f, "(" items ")");
            }
            stmt::Expr::Value(expr) => expr.to_sql(f),
            stmt::Expr::Arg(arg) => {
                // Pre-extracted bind parameter placeholder — render as a
                // positional parameter. The arg position is 0-based; the
                // placeholder is 1-based.
                f.arg_positions.push(arg.position);
                let placeholder = super::Placeholder(arg.position + 1);
                fmt!(f, placeholder);
            }
            stmt::Expr::Default => match f.serializer.flavor {
                Flavor::Postgresql | Flavor::Mysql => fmt!(f, "DEFAULT"),
                // SQLite does not support the DEFAULT keyword but NULL acts similarly.
                Flavor::Sqlite => fmt!(f, "NULL"),
            },
            _ => todo!("expr={:#?}", self),
        }
    }
}

impl ToSql for &stmt::ExprFunc {
    fn to_sql(self, f: &mut super::Formatter<'_>) {
        match self {
            stmt::ExprFunc::Count(func) => match (&func.arg, &func.filter) {
                (None, None) => fmt!(f, "COUNT(*)"),
                // Mysql does not support filters, so translate it to an expression
                (None, Some(expr)) if f.serializer.is_mysql() => {
                    fmt!(f, "COUNT(CASE WHEN " expr " THEN 1 END)")
                }
                (None, Some(expr)) => fmt!(f, "COUNT(*) FILTER (WHERE " expr ")"),
                _ => todo!("func={func:#?}"),
            },
            stmt::ExprFunc::LastInsertId(_) => fmt!(f, "LAST_INSERT_ID()"),
            stmt::ExprFunc::JsonExtract(func) => serialize_json_extract(f, func),
            stmt::ExprFunc::Unnest(func) => {
                if !matches!(f.serializer.flavor, Flavor::Postgresql) {
                    panic!("unnest is only supported on PostgreSQL");
                }

                fmt!(f, "unnest(");
                for (i, arg) in func.args.iter().enumerate() {
                    if i > 0 {
                        f.dst.push_str(", ");
                    }
                    let expr = &arg.expr;
                    let ty = &db::Type::list(arg.elem_ty.clone());
                    fmt!(f, expr "::" ty);
                }
                f.dst.push(')');
            }
        }
    }
}

/// A single operand of an `AND` chain.
///
/// `OR` binds looser than `AND` in SQL, so an `Or` operand must be
/// parenthesized: `a AND (b OR c)` would otherwise serialize as
/// `a AND b OR c`, which parses as `(a AND b) OR c` and silently changes the
/// query's meaning. Operands of other kinds bind at least as tightly as `AND`
/// (comparisons, `IS NULL`, `NOT (..)`, nested `AND`), so they need no parens.
struct AndOperand<'a>(&'a stmt::Expr);

impl ToSql for AndOperand<'_> {
    fn to_sql(self, f: &mut super::Formatter<'_>) {
        if matches!(self.0, stmt::Expr::Or(_)) {
            fmt!(f, "(" self.0 ")");
        } else {
            fmt!(f, self.0);
        }
    }
}

/// Serializes a document path extraction per dialect: `json_extract(col,
/// '$.a.b')` on SQLite, `CAST(JSON_UNQUOTE(JSON_EXTRACT(col, '$.a.b')) AS ...)`
/// on MySQL, and `(col->'a'->>'b')::cast` on PostgreSQL — the latter two unwrap
/// the leaf to text and cast it to match the bound parameter's type.
fn serialize_json_extract(f: &mut super::Formatter<'_>, func: &stmt::FuncJsonExtract) {
    match f.serializer.flavor {
        Flavor::Sqlite => {
            // SQLite's `json_extract` returns SQL-native scalars (unquoted text,
            // integers, reals), so a path read compares directly against a bound
            // parameter with no cast. The path is a single-quoted JSONPath like
            // `$.a.b`.
            fmt!(
                f,
                "json_extract(" func.base.as_ref() ", '$"
                Delimited(func.path.iter().map(|key| (".", key.as_str())), "")
                "')"
            );
        }
        Flavor::Mysql => serialize_mysql_json_extract(f, func),
        Flavor::Postgresql => {
            // Descend with `->`, take the leaf as text with `->>`, then cast the
            // text to the leaf type so it compares against a bound parameter.
            let (leaf, parents) = func
                .path
                .split_last()
                .expect("json extract path has at least one key");
            fmt!(
                f,
                "(" func.base.as_ref()
                Delimited(parents.iter().map(|key| ("->'", key.as_str(), "'")), "")
                "->>'" leaf.as_str() "')"
                pg_json_cast(&func.ty).map(|cast| ("::", cast))
            );
        }
    }
}

/// Serializes a MySQL document path read. `JSON_EXTRACT` yields a *JSON-typed*
/// value, which compares against a bound SQL parameter only by luck — a JSON
/// string (e.g. an ISO timestamp) never equals a native `DATETIME`, and JSON
/// string comparison is `utf8mb4_bin` (case-sensitive), unlike a `VARCHAR`
/// column. So unwrap the leaf to text with `JSON_UNQUOTE` and `CAST` it to the
/// leaf's SQL type (`CHAR` for strings, to recover a `VARCHAR`-matching
/// collation — see [`mysql_json_cast`]), mirroring the `->>`-plus-cast
/// PostgreSQL path. Booleans are the exception: the unquoted text
/// `'true'`/`'false'` casts to `0`, so cast the *bare* JSON boolean to
/// `UNSIGNED` instead (`true` -> 1, `false` -> 0), matching a bound bool param.
fn serialize_mysql_json_extract(f: &mut super::Formatter<'_>, func: &stmt::FuncJsonExtract) {
    if matches!(func.ty, stmt::Type::Bool) {
        fmt!(f, "CAST(");
        mysql_json_extract(f, func);
        fmt!(f, " AS UNSIGNED)");
    } else if let Some(cast) = mysql_json_cast(&func.ty) {
        fmt!(f, "CAST(JSON_UNQUOTE(");
        mysql_json_extract(f, func);
        fmt!(f, ") AS " cast ")");
    } else {
        fmt!(f, "JSON_UNQUOTE(");
        mysql_json_extract(f, func);
        fmt!(f, ")");
    }
}

/// Emits the bare `JSON_EXTRACT(col, '$.a.b')` every MySQL path read is built
/// on, with the path as a single-quoted JSONPath argument.
fn mysql_json_extract(f: &mut super::Formatter<'_>, func: &stmt::FuncJsonExtract) {
    fmt!(
        f,
        "JSON_EXTRACT(" func.base.as_ref() ", '$"
        Delimited(func.path.iter().map(|key| (".", key.as_str())), "")
        "')"
    );
}

/// The MySQL `CAST(... AS <type>)` target wrapped around a
/// `JSON_UNQUOTE(JSON_EXTRACT(...))` text extraction so it compares against a
/// bound parameter of the leaf type. Mirrors [`pg_json_cast`]; `None` leaves the
/// extraction as bare unquoted text (only floats, which compare via numeric
/// coercion). `Bool` is absent because it is cast separately — see
/// [`serialize_mysql_json_extract`].
///
/// `String`/`Uuid` cast to `CHAR` not for the type but for the *collation*:
/// `JSON_UNQUOTE` yields `utf8mb4_bin` (case-sensitive), while `CAST(... AS
/// CHAR)` adopts the connection's default collation — the same one a bound
/// literal and a `VARCHAR` column use — so a string filter on a document leaf
/// matches the case sensitivity of a plain column. `AS CHAR` (no length) does
/// not truncate, and inheriting the server default keeps it portable across
/// server collation configs rather than hardcoding a collation name.
///
/// The temporal targets carry `(6)` precision: a bare `CAST(... AS DATETIME)`
/// truncates to whole seconds, which would drop the microseconds the JSON codec
/// writes and break an equality filter on a sub-second value.
fn mysql_json_cast(ty: &stmt::Type) -> Option<&'static str> {
    use crate::stmt::Type;
    Some(match ty {
        Type::String | Type::Uuid => "CHAR",
        Type::I8 | Type::I16 | Type::I32 | Type::I64 => "SIGNED",
        Type::U8 | Type::U16 | Type::U32 | Type::U64 => "UNSIGNED",
        #[cfg(feature = "rust_decimal")]
        Type::Decimal => "DECIMAL(65, 30)",
        #[cfg(feature = "bigdecimal")]
        Type::BigDecimal => "DECIMAL(65, 30)",
        #[cfg(feature = "jiff")]
        Type::Timestamp => "DATETIME(6)",
        #[cfg(feature = "jiff")]
        Type::Date => "DATE",
        #[cfg(feature = "jiff")]
        Type::Time => "TIME(6)",
        #[cfg(feature = "jiff")]
        Type::DateTime => "DATETIME(6)",
        _ => return None,
    })
}

/// The PostgreSQL cast applied to a `->>'` text extraction so it compares
/// against a bound parameter of the leaf type. `String` (and any non-scalar
/// leaf) needs no cast — `->>` already yields text.
///
/// Every scalar a `#[document]` leaf can hold must appear here: an unlisted
/// scalar falls through to `None` and renders as an *uncast* text extraction,
/// which PostgreSQL then refuses to compare against a typed parameter
/// (`operator does not exist: text = ...`). The temporal casts pair with the
/// microsecond-truncated text the JSON codec writes (see `toasty_sql::json`),
/// so the extracted value parses cleanly into the SQL temporal type. `Zoned` is
/// intentionally absent: it is rejected at schema-build because jiff renders it
/// with an RFC 9557 `[IANA]` annotation that no PostgreSQL cast can parse.
fn pg_json_cast(ty: &stmt::Type) -> Option<&'static str> {
    use crate::stmt::Type;
    Some(match ty {
        Type::Bool => "boolean",
        Type::I8 | Type::I16 | Type::I32 | Type::I64 => "bigint",
        Type::U8 | Type::U16 | Type::U32 | Type::U64 => "bigint",
        Type::F32 | Type::F64 => "double precision",
        Type::Uuid => "uuid",
        #[cfg(feature = "rust_decimal")]
        Type::Decimal => "numeric",
        #[cfg(feature = "bigdecimal")]
        Type::BigDecimal => "numeric",
        #[cfg(feature = "jiff")]
        Type::Timestamp => "timestamptz",
        #[cfg(feature = "jiff")]
        Type::Date => "date",
        #[cfg(feature = "jiff")]
        Type::Time => "time",
        #[cfg(feature = "jiff")]
        Type::DateTime => "timestamp",
        _ => return None,
    })
}

impl ToSql for &stmt::BinaryOp {
    fn to_sql(self, f: &mut super::Formatter<'_>) {
        f.dst.push_str(match self {
            stmt::BinaryOp::Eq => "=",
            stmt::BinaryOp::Gt => ">",
            stmt::BinaryOp::Ge => ">=",
            stmt::BinaryOp::Lt => "<",
            stmt::BinaryOp::Le => "<=",
            stmt::BinaryOp::Ne => "<>",
            stmt::BinaryOp::Add => "+",
            stmt::BinaryOp::Sub => "-",
        })
    }
}
