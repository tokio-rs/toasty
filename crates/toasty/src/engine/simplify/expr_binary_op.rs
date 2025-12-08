use std::cmp::PartialOrd;

use super::Simplify;
use toasty_core::{
    schema::app::FieldTy,
    stmt::{self, Expr},
};

impl Simplify<'_> {
    pub(super) fn simplify_expr_eq_operand(&mut self, operand: &mut stmt::Expr) {
        if let stmt::Expr::Reference(expr_reference) = operand {
            match &*expr_reference {
                stmt::ExprReference::Model { nesting } => {
                    let model = self
                        .cx
                        .resolve_expr_reference(expr_reference)
                        .expect_model();

                    let [pk_field] = &model.primary_key.fields[..] else {
                        todo!("handle composite keys");
                    };

                    *operand = stmt::Expr::ref_field(*nesting, pk_field);
                }
                stmt::ExprReference::Field { .. } => {
                    let field = self
                        .cx
                        .resolve_expr_reference(expr_reference)
                        .expect_field();

                    match &field.ty {
                        FieldTy::Primitive(_) => {}
                        FieldTy::HasMany(_) | FieldTy::HasOne(_) => todo!(),
                        FieldTy::BelongsTo(rel) => {
                            let [fk_field] = &rel.foreign_key.fields[..] else {
                                todo!("handle composite keys");
                            };

                            let stmt::ExprReference::Field { index, .. } = expr_reference else {
                                panic!()
                            };
                            *index = fk_field.source.index;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Recursively walk a binary expression in parallel
    pub(super) fn simplify_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        if op.is_eq() || op.is_ne() {
            self.simplify_expr_eq_operand(lhs);
            self.simplify_expr_eq_operand(rhs);
        }

        match (&mut *lhs, &mut *rhs) {
            // Self-comparison, e.g.,
            //
            //  - `x = x` → `true`
            //  - `x != x` → `false`
            //
            // Only applied for non-nullable field references.
            (Expr::Reference(lhs), Expr::Reference(rhs))
                if lhs == rhs && (op.is_eq() || op.is_ne()) =>
            {
                if lhs.is_field() {
                    let field = self.cx.resolve_expr_reference(lhs).expect_field();
                    if !field.nullable() {
                        return Some(op.is_eq().into());
                    }
                }
                None
            }
            // Constant folding and null propagation,
            //
            //   - `5 = 5` → `true`
            //   - `1 < 5` → `true`
            //   - `"a" >= "b"` → `false`
            //   - `2 + 3` → `5`
            //   - `null <op> x` → `null`
            //   - `x <op> null` → `null`
            (Expr::Value(lhs_val), Expr::Value(rhs_val)) => {
                if lhs_val.is_null() || rhs_val.is_null() {
                    return Some(Expr::null());
                }

                match op {
                    stmt::BinaryOp::Eq => Some((*lhs_val == *rhs_val).into()),
                    stmt::BinaryOp::Ne => Some((*lhs_val != *rhs_val).into()),
                    stmt::BinaryOp::Lt => {
                        PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_lt().into())
                    }
                    stmt::BinaryOp::Le => {
                        PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_le().into())
                    }
                    stmt::BinaryOp::Gt => {
                        PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_gt().into())
                    }
                    stmt::BinaryOp::Ge => {
                        PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_ge().into())
                    }
                    stmt::BinaryOp::Add => fold_add(lhs_val, rhs_val),
                    stmt::BinaryOp::Sub => fold_sub(lhs_val, rhs_val),
                    stmt::BinaryOp::Mul => fold_mul(lhs_val, rhs_val),
                    stmt::BinaryOp::Div => fold_div(lhs_val, rhs_val),
                    stmt::BinaryOp::Mod => fold_mod(lhs_val, rhs_val),
                    _ => None,
                }
            }
            // Arithmetic identity and annihilator rules
            _ if op.is_arithmetic() => self.simplify_arithmetic_identity(op, lhs, rhs),
            // Boolean constant comparisons:
            //
            //  - `x = true` → `x`
            //  - `x = false` → `not(x)`
            //  - `x != true` → `not(x)`
            //  - `x != false` → `x`
            (expr, Expr::Value(stmt::Value::Bool(b)))
            | (Expr::Value(stmt::Value::Bool(b)), expr)
                if op.is_eq() || op.is_ne() =>
            {
                let is_eq_true = (op.is_eq() && *b) || (op.is_ne() && !*b);
                if is_eq_true {
                    Some(expr.take())
                } else {
                    Some(Expr::not(expr.take()))
                }
            }
            // Tuple decomposition,
            //
            //  - `(a, b) = (x, y)` → `a = x and b = y`
            //  - `(a, b) != (x, y)` → `a != x or b != y`
            (Expr::Record(lhs_rec), Expr::Record(rhs_rec))
                if (op.is_eq() || op.is_ne()) && lhs_rec.len() == rhs_rec.len() =>
            {
                let comparisons: Vec<_> = std::mem::take(&mut lhs_rec.fields)
                    .into_iter()
                    .zip(std::mem::take(&mut rhs_rec.fields))
                    .map(|(l, r)| Expr::binary_op(l, op, r))
                    .collect();

                if op.is_eq() {
                    Some(Expr::and_from_vec(comparisons))
                } else {
                    Some(Expr::or_from_vec(comparisons))
                }
            }
            (Expr::Cast(cast), Expr::Value(val)) if cast.ty.is_id() => {
                *lhs = cast.expr.take();
                self.uncast_value_id(val);
                None
            }
            (Expr::Value(val), Expr::Cast(cast)) if cast.ty.is_id() => {
                *rhs = cast.expr.take();
                self.uncast_value_id(val);
                None
            }
            (stmt::Expr::Key(_), other) | (other, stmt::Expr::Key(_)) => {
                assert!(op.is_eq());

                // At this point, we must be in a model context, otherwise key
                // expressions don't make sense.
                let Some(model) = self.cx.target_as_model() else {
                    todo!();
                };

                Some(self.rewrite_root_path_expr(model, other.take()))
            }
            // Canonicalization, `literal <op> col` → `col <op_commuted> literal`
            (Expr::Value(_), rhs) if !rhs.is_value() => {
                if let Some(commuted_op) = op.commute() {
                    std::mem::swap(lhs, rhs);
                    Some(Expr::binary_op(lhs.take(), commuted_op, rhs.take()))
                } else {
                    None
                }
            }
            _ => {
                // For now, just make sure there are no relations in the expression
                stmt::visit::for_each_expr(lhs, |expr| {
                    if let stmt::Expr::Project(_) = expr {
                        todo!()
                    }
                });

                stmt::visit::for_each_expr(rhs, |expr| {
                    if let stmt::Expr::Project(_) = expr {
                        todo!()
                    }
                });

                None
            }
        }
    }

    /// Simplifies arithmetic identity and annihilator rules.
    fn simplify_arithmetic_identity(
        &self,
        op: stmt::BinaryOp,
        lhs: &mut Expr,
        rhs: &mut Expr,
    ) -> Option<Expr> {
        match op {
            stmt::BinaryOp::Add => {
                // `x + 0` → `x`, `0 + x` → `x`
                if is_zero(rhs) {
                    return Some(lhs.take());
                }
                if is_zero(lhs) {
                    return Some(rhs.take());
                }
                None
            }
            stmt::BinaryOp::Sub => {
                // `x - 0` → `x`
                if is_zero(rhs) {
                    return Some(lhs.take());
                }
                None
            }
            stmt::BinaryOp::Mul => {
                // `x * 1` → `x`, `1 * x` → `x`
                if is_one(rhs) {
                    return Some(lhs.take());
                }
                if is_one(lhs) {
                    return Some(rhs.take());
                }
                // `x * 0` → `0`, `0 * x` → `0`
                if is_zero(rhs) {
                    return Some(rhs.take());
                }
                if is_zero(lhs) {
                    return Some(lhs.take());
                }
                None
            }
            stmt::BinaryOp::Div => {
                // `x / 1` → `x`
                if is_one(rhs) {
                    return Some(lhs.take());
                }
                // `0 / x` → `0`
                if is_zero(lhs) {
                    return Some(lhs.take());
                }
                None
            }
            stmt::BinaryOp::Mod => {
                // `x % 1` → `0`
                if is_one(rhs) {
                    if let Expr::Value(val) = lhs {
                        let val = match val {
                            stmt::Value::I8(_) => stmt::Value::I8(0),
                            stmt::Value::I16(_) => stmt::Value::I16(0),
                            stmt::Value::I32(_) => stmt::Value::I32(0),
                            stmt::Value::I64(_) => stmt::Value::I64(0),
                            stmt::Value::U8(_) => stmt::Value::U8(0),
                            stmt::Value::U16(_) => stmt::Value::U16(0),
                            stmt::Value::U32(_) => stmt::Value::U32(0),
                            stmt::Value::U64(_) => stmt::Value::U64(0),
                            _ => todo!(),
                        };

                        return Some(Expr::Value(val));
                    }
                }
                None
            }
            _ => None,
        }
    }
}

/// Checks if an expression is the provided value.
macro_rules! is_integer_value {
    ($expr:expr, $val:literal) => {
        matches!(
            $expr,
            Expr::Value(stmt::Value::I8($val))
                | Expr::Value(stmt::Value::I16($val))
                | Expr::Value(stmt::Value::I32($val))
                | Expr::Value(stmt::Value::I64($val))
                | Expr::Value(stmt::Value::U8($val))
                | Expr::Value(stmt::Value::U16($val))
                | Expr::Value(stmt::Value::U32($val))
                | Expr::Value(stmt::Value::U64($val))
        )
    };
}

/// Checks whether the expression is a zero.
fn is_zero(expr: &Expr) -> bool {
    is_integer_value!(expr, 0)
}

/// Checks whether the expression is a one.
fn is_one(expr: &Expr) -> bool {
    is_integer_value!(expr, 1)
}

/// Generates constant folding functions for arithmetic operations using checked
/// arithmetic.
///
/// Returns `None` if the operation would overflow or is otherwise invalid.
macro_rules! fold_arithmetic {
    ($name:ident, $method:ident) => {
        fn $name(lhs: &stmt::Value, rhs: &stmt::Value) -> Option<Expr> {
            let result = match (lhs, rhs) {
                (stmt::Value::I8(a), stmt::Value::I8(b)) => stmt::Value::I8(a.$method(*b)?),
                (stmt::Value::I16(a), stmt::Value::I16(b)) => stmt::Value::I16(a.$method(*b)?),
                (stmt::Value::I32(a), stmt::Value::I32(b)) => stmt::Value::I32(a.$method(*b)?),
                (stmt::Value::I64(a), stmt::Value::I64(b)) => stmt::Value::I64(a.$method(*b)?),
                (stmt::Value::U8(a), stmt::Value::U8(b)) => stmt::Value::U8(a.$method(*b)?),
                (stmt::Value::U16(a), stmt::Value::U16(b)) => stmt::Value::U16(a.$method(*b)?),
                (stmt::Value::U32(a), stmt::Value::U32(b)) => stmt::Value::U32(a.$method(*b)?),
                (stmt::Value::U64(a), stmt::Value::U64(b)) => stmt::Value::U64(a.$method(*b)?),
                _ => return None,
            };
            Some(Expr::Value(result))
        }
    };
}

fold_arithmetic!(fold_add, checked_add);
fold_arithmetic!(fold_sub, checked_sub);
fold_arithmetic!(fold_mul, checked_mul);
fold_arithmetic!(fold_div, checked_div);
fold_arithmetic!(fold_mod, checked_rem);

#[cfg(test)]
mod tests {
    use super::*;
    use crate as toasty;
    use crate::Model as _;
    use toasty_core::{
        driver::Capability,
        schema::{app, Builder},
        stmt::{BinaryOp, ExprCast, ExprReference, Id, Type, Value},
    };

    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,

        #[allow(dead_code)]
        name: Option<String>,
    }

    fn test_schema() -> toasty_core::Schema {
        let app_schema =
            app::Schema::from_macro(&[User::schema()]).expect("schema should build from macro");

        Builder::new()
            .build(app_schema, &Capability::SQLITE)
            .expect("schema should build")
    }

    #[test]
    fn cast_id_on_lhs_unwrapped() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(cast(arg(0), Id), Id("abc")) → eq(arg(0), "abc")`
        let mut lhs = Expr::Cast(ExprCast {
            expr: Box::new(Expr::arg(0)),
            ty: Type::Id(User::id()),
        });
        let mut rhs = Expr::Value(Value::Id(Id::from_string(User::id(), "abc".to_string())));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(result.is_none());
        assert!(matches!(lhs, Expr::Arg(_)));
        assert!(matches!(rhs, Expr::Value(Value::String(s)) if s == "abc"));
    }

    #[test]
    fn cast_id_on_rhs_unwrapped() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(Id("xyz"), cast(arg(0), Id)) → eq("xyz", arg(0))`
        let mut lhs = Expr::Value(Value::Id(Id::from_string(User::id(), "xyz".to_string())));
        let mut rhs = Expr::Cast(ExprCast {
            expr: Box::new(Expr::arg(0)),
            ty: Type::Id(User::id()),
        });

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(result.is_none());
        assert!(matches!(lhs, Expr::Value(Value::String(s)) if s == "xyz"));
        assert!(matches!(rhs, Expr::Arg(_)));
    }

    #[test]
    fn non_id_cast_not_unwrapped() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(cast(arg(0), String), "test")`, non-Id cast is not unwrapped
        let mut lhs = Expr::Cast(ExprCast {
            expr: Box::new(Expr::arg(0)),
            ty: Type::String,
        });
        let mut rhs = Expr::Value(Value::from("test"));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(result.is_none());
        assert!(matches!(lhs, Expr::Cast(_)));
    }

    #[test]
    fn constant_eq_same_values_becomes_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(5, 5)` → `true`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
    }

    #[test]
    fn constant_eq_different_values_becomes_false() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(1, 2)` → `false`
        let mut lhs = Expr::Value(Value::from(1i64));
        let mut rhs = Expr::Value(Value::from(2i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
    }

    #[test]
    fn constant_ne_same_values_becomes_false() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `ne(5, 5)` → `false`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
    }

    #[test]
    fn constant_ne_different_values_becomes_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `ne("abc", "def")` → `true`
        let mut lhs = Expr::Value(Value::from("abc"));
        let mut rhs = Expr::Value(Value::from("def"));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
    }

    #[test]
    fn constant_eq_with_null_becomes_null() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(null, 5)` → `null`
        let mut lhs = Expr::Value(Value::Null);
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Null))));
    }

    #[test]
    fn constant_eq_null_with_null_becomes_null() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(null, null)` → `null`
        let mut lhs = Expr::Value(Value::Null);
        let mut rhs = Expr::Value(Value::Null);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Null))));
    }

    #[test]
    fn constant_lt_becomes_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `1 < 5` → `true`
        let mut lhs = Expr::Value(Value::from(1i64));
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
    }

    #[test]
    fn constant_lt_becomes_false() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `5 < 1` → `false`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::Value(Value::from(1i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
    }

    #[test]
    fn constant_le_equal_becomes_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `5 <= 5` → `true`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Le, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
    }

    #[test]
    fn constant_le_becomes_false() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `10 <= 5` → `false`
        let mut lhs = Expr::Value(Value::from(10i64));
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Le, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
    }

    #[test]
    fn constant_gt_becomes_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `10 > 5` → `true`
        let mut lhs = Expr::Value(Value::from(10i64));
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Gt, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
    }

    #[test]
    fn constant_gt_becomes_false() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `1 > 5` → `false`
        let mut lhs = Expr::Value(Value::from(1i64));
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Gt, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
    }

    #[test]
    fn constant_ge_equal_becomes_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `5 >= 5` → `true`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Ge, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
    }

    #[test]
    fn constant_ge_becomes_false() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `1 >= 5` → `false`
        let mut lhs = Expr::Value(Value::from(1i64));
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Ge, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
    }

    #[test]
    fn constant_lt_string_lexicographic() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `"abc" < "def"` → `true` (lexicographic)
        let mut lhs = Expr::Value(Value::from("abc"));
        let mut rhs = Expr::Value(Value::from("def"));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
    }

    #[test]
    fn constant_lt_different_types_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `5 < "abc"` is not simplified (incompatible types)
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::Value(Value::from("abc"));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

        assert!(result.is_none());
    }

    #[test]
    fn lt_with_non_constant_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `arg(0) < 5` is not simplified (non-constant lhs)
        let mut lhs = Expr::arg(0);
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

        assert!(result.is_none());
    }

    #[test]
    fn self_comparison_eq_non_nullable_becomes_true() {
        let schema = test_schema();
        let model = schema.app.model(User::id());
        let simplify = Simplify::new(&schema);
        let mut simplify = simplify.scope(model);

        // `id = id` → `true` (non-nullable field)
        let mut lhs = Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 0,
        });
        let mut rhs = Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 0,
        });

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
    }

    #[test]
    fn self_comparison_ne_non_nullable_becomes_false() {
        let schema = test_schema();
        let model = schema.app.model(User::id());
        let simplify = Simplify::new(&schema);
        let mut simplify = simplify.scope(model);

        // `id != id` → `false` (non-nullable field)
        let mut lhs = Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 0,
        });
        let mut rhs = Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 0,
        });

        let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
    }

    #[test]
    fn self_comparison_nullable_not_simplified() {
        let schema = test_schema();
        let model = schema.app.model(User::id());
        let simplify = Simplify::new(&schema);
        let mut simplify = simplify.scope(model);

        // `name = name` is not simplified (nullable field)
        let mut lhs = Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 1,
        });
        let mut rhs = Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 1,
        });

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(result.is_none());
    }

    #[test]
    fn different_fields_not_simplified() {
        let schema = test_schema();
        let model = schema.app.model(User::id());
        let simplify = Simplify::new(&schema);
        let mut simplify = simplify.scope(model);

        // `id = name` is not simplified (different fields)
        let mut lhs = Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 0,
        });
        let mut rhs = Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 1,
        });

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(result.is_none());
    }

    #[test]
    fn x_eq_true_becomes_x() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `x = true` → `x`
        let mut lhs = Expr::arg(0);
        let mut rhs = Expr::Value(Value::Bool(true));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Arg(_))));
    }

    #[test]
    fn true_eq_x_becomes_x() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `true = x` → `x`
        let mut lhs = Expr::Value(Value::Bool(true));
        let mut rhs = Expr::arg(0);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Arg(_))));
    }

    #[test]
    fn x_eq_false_becomes_not_x() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `x = false` → `not(x)`
        let mut lhs = Expr::arg(0);
        let mut rhs = Expr::Value(Value::Bool(false));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Not(_))));
    }

    #[test]
    fn x_ne_true_becomes_not_x() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `x != true` → `not(x)`
        let mut lhs = Expr::arg(0);
        let mut rhs = Expr::Value(Value::Bool(true));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Not(_))));
    }

    #[test]
    fn x_ne_false_becomes_x() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `x != false` → `x`
        let mut lhs = Expr::arg(0);
        let mut rhs = Expr::Value(Value::Bool(false));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

        assert!(matches!(result, Some(Expr::Arg(_))));
    }

    #[test]
    fn canonicalize_eq_literal_on_left() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `5 = x` → `x = 5`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::arg(0);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        let Some(Expr::BinaryOp(binary_op)) = result else {
            panic!("expected BinaryOp");
        };
        assert_eq!(binary_op.op, BinaryOp::Eq);
        assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
        assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
    }

    #[test]
    fn canonicalize_lt_literal_on_left() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `5 < x` → `x > 5`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::arg(0);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

        let Some(Expr::BinaryOp(binary_op)) = result else {
            panic!("expected BinaryOp");
        };
        assert_eq!(binary_op.op, BinaryOp::Gt);
        assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
        assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
    }

    #[test]
    fn canonicalize_gt_literal_on_left() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `5 > x` → `x < 5`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::arg(0);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Gt, &mut lhs, &mut rhs);

        let Some(Expr::BinaryOp(binary_op)) = result else {
            panic!("expected BinaryOp");
        };
        assert_eq!(binary_op.op, BinaryOp::Lt);
        assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
        assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
    }

    #[test]
    fn canonicalize_le_literal_on_left() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `5 <= x` → `x >= 5`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::arg(0);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Le, &mut lhs, &mut rhs);

        let Some(Expr::BinaryOp(binary_op)) = result else {
            panic!("expected BinaryOp");
        };
        assert_eq!(binary_op.op, BinaryOp::Ge);
        assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
        assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
    }

    #[test]
    fn canonicalize_ge_literal_on_left() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `5 >= x` → `x <= 5`
        let mut lhs = Expr::Value(Value::from(5i64));
        let mut rhs = Expr::arg(0);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Ge, &mut lhs, &mut rhs);

        let Some(Expr::BinaryOp(binary_op)) = result else {
            panic!("expected BinaryOp");
        };
        assert_eq!(binary_op.op, BinaryOp::Le);
        assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
        assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
    }

    #[test]
    fn no_canonicalize_when_literal_on_right() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `x < 5` is already canonical, no change
        let mut lhs = Expr::arg(0);
        let mut rhs = Expr::Value(Value::from(5i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

        assert!(result.is_none());
    }

    #[test]
    fn tuple_eq_decomposition_two_elements() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `(a, b) = (x, y)` → `a = x and b = y`
        let mut lhs = Expr::record([Expr::arg(0), Expr::arg(1)]);
        let mut rhs = Expr::record([Expr::arg(2), Expr::arg(3)]);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        let Some(Expr::And(and_expr)) = result else {
            panic!("expected And expression");
        };
        assert_eq!(and_expr.len(), 2);
        assert!(matches!(&and_expr[0], Expr::BinaryOp(op) if op.op.is_eq()));
        assert!(matches!(&and_expr[1], Expr::BinaryOp(op) if op.op.is_eq()));
    }

    #[test]
    fn tuple_eq_decomposition_three_elements() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `(a, b, c) = (x, y, z)` → `a = x and b = y and c = z`
        let mut lhs = Expr::record([Expr::arg(0), Expr::arg(1), Expr::arg(2)]);
        let mut rhs = Expr::record([Expr::arg(3), Expr::arg(4), Expr::arg(5)]);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        let Some(Expr::And(and_expr)) = result else {
            panic!("expected And expression");
        };
        assert_eq!(and_expr.len(), 3);
    }

    #[test]
    fn tuple_ne_decomposition() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `(a, b) != (x, y)` → `a != x or b != y`
        let mut lhs = Expr::record([Expr::arg(0), Expr::arg(1)]);
        let mut rhs = Expr::record([Expr::arg(2), Expr::arg(3)]);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

        let Some(Expr::Or(or_expr)) = result else {
            panic!("expected Or expression");
        };
        assert_eq!(or_expr.len(), 2);
        assert!(matches!(&or_expr[0], Expr::BinaryOp(op) if op.op.is_ne()));
        assert!(matches!(&or_expr[1], Expr::BinaryOp(op) if op.op.is_ne()));
    }

    #[test]
    fn single_element_tuple_eq() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `(a) = (x)` → `a = x`
        let mut lhs = Expr::record([Expr::arg(0)]);
        let mut rhs = Expr::record([Expr::arg(1)]);

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);
        assert!(matches!(result, Some(Expr::BinaryOp(op)) if op.op.is_eq()));
    }

    macro_rules! test_constant_fold {
        ($name:ident, $op:ident, $ty:ident, $lhs:expr, $rhs:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let schema = test_schema();
                let mut simplify = Simplify::new(&schema);
                let mut lhs = Expr::Value(Value::$ty($lhs));
                let mut rhs = Expr::Value(Value::$ty($rhs));
                let result = simplify.simplify_expr_binary_op(BinaryOp::$op, &mut lhs, &mut rhs);
                assert!(matches!(result, Some(Expr::Value(Value::$ty(v))) if v == $expected));
            }
        };
    }

    test_constant_fold!(add_i64_constant_folding, Add, I64, 2, 3, 5);
    test_constant_fold!(add_i32_constant_folding, Add, I32, 10, 20, 30);
    test_constant_fold!(add_i16_constant_folding, Add, I16, 100, 200, 300);
    test_constant_fold!(add_i8_constant_folding, Add, I8, 10, 20, 30);
    test_constant_fold!(add_u64_constant_folding, Add, U64, 100, 200, 300);
    test_constant_fold!(add_u32_constant_folding, Add, U32, 50, 75, 125);
    test_constant_fold!(add_u16_constant_folding, Add, U16, 1000, 2000, 3000);
    test_constant_fold!(add_u8_constant_folding, Add, U8, 10, 20, 30);
    test_constant_fold!(sub_i64_constant_folding, Sub, I64, 10, 3, 7);
    test_constant_fold!(sub_negative_result, Sub, I64, 3, 10, -7);
    test_constant_fold!(mul_i64_constant_folding, Mul, I64, 6, 7, 42);
    test_constant_fold!(mul_negative_values, Mul, I64, -3, 4, -12);
    test_constant_fold!(mul_two_negatives, Mul, I64, -3, -4, 12);
    test_constant_fold!(div_i64_constant_folding, Div, I64, 20, 4, 5);
    test_constant_fold!(div_truncates_toward_zero, Div, I64, 7, 3, 2);
    test_constant_fold!(mod_i64_constant_folding, Mod, I64, 17, 5, 2);
    test_constant_fold!(mod_exact_divisor, Mod, I64, 20, 5, 0);
    test_constant_fold!(mod_negative_by_one, Mod, I64, -17, 1, 0);

    macro_rules! test_not_simplified {
        ($name:ident, $op:ident, $lhs:expr, $rhs:expr) => {
            #[test]
            fn $name() {
                let schema = test_schema();
                let mut simplify = Simplify::new(&schema);
                let mut lhs = $lhs;
                let mut rhs = $rhs;
                let result = simplify.simplify_expr_binary_op(BinaryOp::$op, &mut lhs, &mut rhs);
                assert!(result.is_none());
            }
        };
    }

    test_not_simplified!(
        div_by_zero_not_simplified,
        Div,
        Expr::Value(Value::I64(10)),
        Expr::Value(Value::I64(0))
    );
    test_not_simplified!(
        mod_by_zero_not_simplified,
        Mod,
        Expr::Value(Value::I64(10)),
        Expr::Value(Value::I64(0))
    );
    test_not_simplified!(
        add_mismatched_types_not_simplified,
        Add,
        Expr::Value(Value::I32(10)),
        Expr::Value(Value::I64(20))
    );
    test_not_simplified!(
        add_string_not_simplified,
        Add,
        Expr::Value(Value::from("a")),
        Expr::Value(Value::from("b"))
    );
    test_not_simplified!(
        add_overflow_not_simplified,
        Add,
        Expr::Value(Value::I64(i64::MAX)),
        Expr::Value(Value::I64(1))
    );
    test_not_simplified!(
        add_i8_overflow_not_simplified,
        Add,
        Expr::Value(Value::I8(127)),
        Expr::Value(Value::I8(1))
    );
    test_not_simplified!(
        sub_underflow_not_simplified,
        Sub,
        Expr::Value(Value::I64(i64::MIN)),
        Expr::Value(Value::I64(1))
    );
    test_not_simplified!(
        sub_u64_underflow_not_simplified,
        Sub,
        Expr::Value(Value::U64(0)),
        Expr::Value(Value::U64(1))
    );
    test_not_simplified!(
        mul_overflow_not_simplified,
        Mul,
        Expr::Value(Value::I64(i64::MAX)),
        Expr::Value(Value::I64(2))
    );
    test_not_simplified!(
        div_min_by_neg_one_not_simplified,
        Div,
        Expr::Value(Value::I64(i64::MIN)),
        Expr::Value(Value::I64(-1))
    );
    test_not_simplified!(
        sub_literal_left_not_canonicalized,
        Sub,
        Expr::Value(Value::I64(5)),
        Expr::arg(0)
    );
    test_not_simplified!(
        div_literal_left_not_canonicalized,
        Div,
        Expr::Value(Value::I64(10)),
        Expr::arg(0)
    );
    test_not_simplified!(
        mod_literal_left_not_canonicalized,
        Mod,
        Expr::Value(Value::I64(10)),
        Expr::arg(0)
    );
    test_not_simplified!(
        sub_zero_left_not_identity,
        Sub,
        Expr::Value(Value::I64(0)),
        Expr::arg(0)
    );
    test_not_simplified!(
        div_one_left_not_identity,
        Div,
        Expr::Value(Value::I64(1)),
        Expr::arg(0)
    );

    macro_rules! test_identity_to_arg {
        ($name:ident, $op:ident, $val:expr) => {
            #[test]
            fn $name() {
                let schema = test_schema();
                let mut simplify = Simplify::new(&schema);
                let mut lhs = Expr::arg(0);
                let mut rhs = $val;
                let result = simplify.simplify_expr_binary_op(BinaryOp::$op, &mut lhs, &mut rhs);
                assert!(matches!(result, Some(Expr::Arg(_))));
            }
        };
        ($name:ident, $op:ident, $val:expr, left) => {
            #[test]
            fn $name() {
                let schema = test_schema();
                let mut simplify = Simplify::new(&schema);
                let mut lhs = $val;
                let mut rhs = Expr::arg(0);
                let result = simplify.simplify_expr_binary_op(BinaryOp::$op, &mut lhs, &mut rhs);
                assert!(matches!(result, Some(Expr::Arg(_))));
            }
        };
    }

    test_identity_to_arg!(add_zero_right_identity, Add, Expr::Value(Value::I64(0)));
    test_identity_to_arg!(
        add_zero_left_identity,
        Add,
        Expr::Value(Value::I64(0)),
        left
    );
    test_identity_to_arg!(add_zero_i32_identity, Add, Expr::Value(Value::I32(0)));
    test_identity_to_arg!(add_zero_u64_identity, Add, Expr::Value(Value::U64(0)));
    test_identity_to_arg!(sub_zero_identity, Sub, Expr::Value(Value::I64(0)));
    test_identity_to_arg!(mul_one_right_identity, Mul, Expr::Value(Value::I64(1)));
    test_identity_to_arg!(mul_one_left_identity, Mul, Expr::Value(Value::I64(1)), left);
    test_identity_to_arg!(mul_one_i8_identity, Mul, Expr::Value(Value::I8(1)));
    test_identity_to_arg!(div_one_identity, Div, Expr::Value(Value::I64(1)));

    macro_rules! test_annihilator {
        ($name:ident, $op:ident, $ty:ident, $zero_pos:ident) => {
            #[test]
            fn $name() {
                let schema = test_schema();
                let mut simplify = Simplify::new(&schema);
                let (mut lhs, mut rhs) = match stringify!($zero_pos) {
                    "right" => (Expr::arg(0), Expr::Value(Value::$ty(0))),
                    "left" => (Expr::Value(Value::$ty(0)), Expr::arg(0)),
                    _ => panic!("invalid zero_pos"),
                };
                let result = simplify.simplify_expr_binary_op(BinaryOp::$op, &mut lhs, &mut rhs);
                assert!(matches!(result, Some(Expr::Value(Value::$ty(0)))));
            }
        };
    }

    test_annihilator!(mul_zero_right_annihilator, Mul, I64, right);
    test_annihilator!(mul_zero_left_annihilator, Mul, I64, left);
    test_annihilator!(mul_zero_u32_annihilator, Mul, U32, right);
    test_annihilator!(div_zero_numerator_annihilator, Div, I64, left);
    test_annihilator!(div_zero_numerator_u16_annihilator, Div, U16, left);

    macro_rules! test_null_propagation {
        ($name:ident, $op:ident, $null_pos:ident) => {
            #[test]
            fn $name() {
                let schema = test_schema();
                let mut simplify = Simplify::new(&schema);
                let (mut lhs, mut rhs) = match stringify!($null_pos) {
                    "left" => (Expr::Value(Value::Null), Expr::Value(Value::I64(5))),
                    "right" => (Expr::Value(Value::I64(5)), Expr::Value(Value::Null)),
                    "both" => (Expr::Value(Value::Null), Expr::Value(Value::Null)),
                    _ => panic!("invalid null_pos"),
                };
                let result = simplify.simplify_expr_binary_op(BinaryOp::$op, &mut lhs, &mut rhs);
                assert!(matches!(result, Some(Expr::Value(Value::Null))));
            }
        };
    }

    test_null_propagation!(add_null_left_becomes_null, Add, left);
    test_null_propagation!(add_null_right_becomes_null, Add, right);
    test_null_propagation!(sub_null_becomes_null, Sub, left);
    test_null_propagation!(mul_null_becomes_null, Mul, left);
    test_null_propagation!(div_null_becomes_null, Div, left);
    test_null_propagation!(mod_null_becomes_null, Mod, left);
    test_null_propagation!(null_null_arithmetic_becomes_null, Add, both);

    #[test]
    fn mod_by_one_constant_becomes_zero() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);
        let mut lhs = Expr::Value(Value::I64(17));
        let mut rhs = Expr::Value(Value::I64(1));
        let result = simplify.simplify_expr_binary_op(BinaryOp::Mod, &mut lhs, &mut rhs);
        assert!(matches!(result, Some(Expr::Value(Value::I64(0)))));
    }
}
