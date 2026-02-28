use toasty_core::stmt::{self, Project};

use super::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_expr_project(
        &mut self,
        expr: &mut stmt::ExprProject,
    ) -> Option<stmt::Expr> {
        // Constant evaluation: if the base is an Expr::Value, we can evaluate
        // the projection at compile time.
        //
        // Examples:
        //   - `project(Value::I64(42), [])` → `Value::I64(42)`
        //   - `project(Value::Record([1, 2, 3]), [0])` → `Value::I64(1)`
        //
        // Note: This only handles Expr::Value, not Expr::Record or Expr::List.
        // Those variants represent expressions that will be evaluated later,
        // not constant values that can be folded now.
        if let stmt::Expr::Value(value) = &*expr.base {
            // Use the value's project method to follow the projection path
            if let Some(result) = value.project(&expr.projection) {
                return Some(result);
            }
        }

        // Handle projections through records (embedded fields lower to records of columns).
        // After lowering, embedded field references become records where each field is a column.
        // Uses `entry` to support arbitrary-depth projections (e.g., [1, 1] for nested embedded).
        // Examples:
        //   project([street_col, city_col, zip_col], [1]) → city_col
        //   project([name_col, record([street_col, city_col])], [1, 1]) → city_col
        if let stmt::Expr::Record(_) = &*expr.base {
            if let Some(entry) = expr.base.entry(&expr.projection) {
                return Some(entry.to_expr());
            }
        }

        // Variant+field projection through a Match: when the projection has 2+
        // steps [variant_disc, field_idx, ...], select the specific Match arm
        // whose guard matches variant_disc, then extract the field at position
        // field_idx + 1 from that arm's Record (offset by 1 because disc is at
        // position 0). Non-matching arms are irrelevant since the projection
        // explicitly targets one variant.
        //
        // Example: project(Match(d, [1 => Record([d, a])], else), [1, 0])
        //        → a   (field 0 of variant 1, at Record position 1)
        if let stmt::Expr::Match(match_expr) = &*expr.base {
            if expr.projection.len() >= 2 {
                let variant_disc = expr.projection[0];
                let field_idx = expr.projection[1];
                let further = &expr.projection.as_slice()[2..];

                if let Some(arm) = match_expr.arms.iter().find(|a| {
                    matches!(&a.pattern, stmt::Value::I64(d) if *d as usize == variant_disc)
                }) {
                    // The arm's Record has disc at [0] and fields starting at [1]
                    let record_position = field_idx + 1;

                    if let Some(entry) = arm.expr.entry(record_position) {
                        let field_expr = entry.to_expr();
                        if further.is_empty() {
                            return Some(field_expr);
                        } else {
                            return Some(stmt::Expr::project(field_expr, further));
                        }
                    }
                }
            }
        }

        // Project into Match: distribute the projection into each arm's expression.
        // Example: project(Match(d, [1 => Record([d, a]), 2 => Record([d, n])]), [0])
        //        → Match(d, [1 => project(Record([d, a]), [0]), 2 => project(Record([d, n]), [0])])
        //        → Match(d, [1 => d, 2 => d])   (after recursive simplification)
        if let stmt::Expr::Match(match_expr) = &mut *expr.base {
            for arm in &mut match_expr.arms {
                arm.expr = stmt::Expr::project(
                    std::mem::replace(&mut arm.expr, stmt::Expr::null()),
                    expr.projection.clone(),
                );
            }
            *match_expr.else_expr = stmt::Expr::project(
                std::mem::replace(&mut *match_expr.else_expr, stmt::Expr::null()),
                expr.projection.clone(),
            );
            return Some(std::mem::replace(&mut *expr.base, stmt::Expr::null()));
        }

        None
    }
}
