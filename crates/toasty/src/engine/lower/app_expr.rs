use super::LowerStatement;
use crate::engine::simplify;
use toasty_core::stmt::{self, BinaryOp, Expr, Value, VisitMut};

impl LowerStatement<'_, '_> {
    /// Translate application predicates before database simplification can
    /// discard the distinction between an absent value and an unknown boolean.
    pub(super) fn lower_app_expr(&mut self, expr: Expr) -> Expr {
        if expr.is_const()
            && expr.is_eval()
            && let Ok(Value::Bool(value)) = expr.clone().app().eval_const()
        {
            return value.into();
        }
        match expr {
            Expr::App(expr) => self.lower_app_expr(*expr),
            Expr::And(mut expr) => {
                expr.operands = expr
                    .operands
                    .into_iter()
                    .map(|e| self.lower_app_expr(e))
                    .collect();
                Expr::And(expr)
            }
            Expr::Or(mut expr) => {
                expr.operands = expr
                    .operands
                    .into_iter()
                    .map(|e| self.lower_app_expr(e))
                    .collect();
                Expr::Or(expr)
            }
            Expr::Not(expr) => Expr::not(self.lower_app_expr(*expr.expr)),
            Expr::BinaryOp(expr) if !expr.op.is_arithmetic() => {
                self.lower_app_comparison(*expr.lhs, expr.op, *expr.rhs)
            }
            Expr::Between(expr) => {
                if !self.capability().sql()
                    && self.app_is_none(&expr.expr).is_false()
                    && self.app_is_none(&expr.low).is_false()
                    && self.app_is_none(&expr.high).is_false()
                {
                    let mut native = Expr::Between(expr);
                    self.visit_expr_mut(&mut native);
                    return native;
                }
                let lower =
                    self.lower_app_comparison((*expr.expr).clone(), BinaryOp::Ge, *expr.low);
                let upper = self.lower_app_comparison(*expr.expr, BinaryOp::Le, *expr.high);
                Expr::and(lower, upper)
            }
            Expr::InList(expr) => {
                let items = match *expr.list {
                    Expr::List(list) => list.items,
                    Expr::Value(Value::List(items)) => items.into_iter().map(Expr::from).collect(),
                    list => {
                        let mut expr = Expr::in_list(*expr.expr, list);
                        self.visit_expr_mut(&mut expr);
                        return self.lower_bound_membership(expr);
                    }
                };
                if items.is_empty() {
                    return false.into();
                }
                // Preserve indexed IN and array binds when absence cannot
                // affect any comparison.
                if self.app_is_none(&expr.expr).is_false()
                    && items
                        .iter()
                        .all(|e| matches!(e, Expr::Value(v) if !v.is_null()) && !is_nan_literal(e))
                    && !(self.capability().binary_like_starts_with
                        && items
                            .iter()
                            .any(|e| matches!(e, Expr::Value(Value::String(_)))))
                {
                    let mut expr = Expr::in_list(*expr.expr, Expr::list(items));
                    self.visit_expr_mut(&mut expr);
                    return expr;
                }
                Expr::or_from_vec(
                    items
                        .into_iter()
                        .map(|item| {
                            self.lower_app_comparison((*expr.expr).clone(), BinaryOp::Eq, item)
                        })
                        .collect(),
                )
            }
            Expr::InSubquery(expr) => self.lower_app_in_subquery(expr),
            Expr::AnyOp(mut expr) if expr.op.is_eq() => {
                EncodeOptions.visit_expr_mut(&mut expr.lhs);
                if is_nan_literal(&expr.lhs) {
                    return false.into();
                }
                self.visit_expr_mut(&mut expr.lhs);
                self.visit_expr_mut(&mut expr.rhs);
                let ty = self.expr_cx.infer_expr_ty(&expr.lhs, &[]);
                if self.capability().native_float_nan
                    && matches!(ty, stmt::Type::F32 | stmt::Type::F64)
                    && !expr.lhs.is_value()
                {
                    Expr::and(Expr::not(Expr::IsNan(expr.lhs.clone())), Expr::AnyOp(expr))
                } else {
                    Expr::AnyOp(expr)
                }
            }
            Expr::IsNull(expr) => {
                let absent = self.app_is_none(&expr.expr);
                if expr.negated {
                    Expr::not(absent)
                } else {
                    absent
                }
            }
            Expr::Like(expr) => {
                let present = Expr::not(self.app_is_none(&expr.expr));
                let mut predicate = Expr::Like(expr);
                self.visit_expr_mut(&mut predicate);
                Expr::and(present, predicate)
            }
            Expr::StartsWith(expr) => {
                let present = Expr::not(self.app_is_none(&expr.expr));
                let mut predicate = Expr::StartsWith(expr);
                self.visit_expr_mut(&mut predicate);
                Expr::and(present, predicate)
            }
            mut expr => {
                self.visit_expr_mut(&mut expr);
                expr
            }
        }
    }

    pub(super) fn app_is_none(&mut self, expr: &Expr) -> Expr {
        match expr {
            Expr::OptionSome(_) | Expr::Value(Value::Option(Some(_))) => return false.into(),
            Expr::Value(Value::Option(None)) => return true.into(),
            Expr::Length(length) => return self.app_is_none(&length.expr),
            _ => {}
        }
        if super::relation_expr::is_optional_path(&self.expr_cx, expr) == Some(false) {
            return false.into();
        }
        if let Some(relation) = super::relation_expr::resolve(&self.expr_cx, expr)
            && relation.is_endpoint()
        {
            match &relation.field.ty {
                toasty_core::schema::app::FieldTy::BelongsTo(_) => {
                    let mut key = expr.clone();
                    self.rewrite_eq_operand(&mut key);
                    return self.app_is_none(&key);
                }
                _ => {
                    let query = stmt::Query::new_select(relation.target, true);
                    if let Some(membership) =
                        super::lift_in_subquery::lift_in_subquery(&self.expr_cx, expr, &query)
                    {
                        return Expr::not(self.lower_app_expr(membership));
                    }
                }
            }
        }
        let mut absent = Expr::is_null(expr.clone());
        simplify::simplify_expr(self.expr_cx, self.capability(), &mut absent);
        self.visit_expr_mut(&mut absent);
        simplify::simplify_expr(self.expr_cx, self.capability(), &mut absent);
        absent
    }

    fn lower_app_comparison(&mut self, mut lhs: Expr, op: BinaryOp, mut rhs: Expr) -> Expr {
        if let (Some(lhs), Some(rhs)) = (some_payload(&lhs), some_payload(&rhs)) {
            return self.lower_app_comparison(lhs, op, rhs);
        }
        let lhs_none = self.app_is_none(&lhs);
        let rhs_none = self.app_is_none(&rhs);
        if lhs_none.is_true() || rhs_none.is_true() {
            return option_comparison(lhs_none, op, rhs_none, false.into());
        }
        EncodeOptions.visit_expr_mut(&mut lhs);
        EncodeOptions.visit_expr_mut(&mut rhs);
        if op.is_eq()
            && let Some(present) = self.compare_relation_identities(&lhs, &rhs)
        {
            return option_comparison(lhs_none, op, rhs_none, present);
        }
        if op.is_eq() {
            for (relation_expr, value) in [(&lhs, &rhs), (&rhs, &lhs)] {
                if (value.is_value() || value.is_record())
                    && let Some(relation) =
                        super::relation_expr::resolve(&self.expr_cx, relation_expr)
                    && relation.is_endpoint()
                {
                    let model = self.schema().app.model(relation.target).as_root_unwrap();
                    let requires_lookup = match &relation.field.ty {
                        toasty_core::schema::app::FieldTy::BelongsTo(rel) => !rel
                            .foreign_key
                            .fields
                            .iter()
                            .map(|fk| fk.target)
                            .eq(model.primary_key.fields.iter().copied()),
                        _ => true,
                    };
                    if requires_lookup {
                        let key =
                            super::key_field_refs(0, model.primary_key.fields.iter().copied());
                        let query = stmt::Query::new_select(
                            relation.target,
                            Expr::eq(key, value.clone()).app(),
                        );
                        if let Some(membership) = super::lift_in_subquery::lift_in_subquery(
                            &self.expr_cx,
                            relation_expr,
                            &query,
                        ) {
                            return self.lower_app_expr(membership);
                        }
                    }
                }
            }
        }
        if op.is_eq() || op.is_ne() {
            self.rewrite_eq_operand(&mut lhs);
            self.rewrite_eq_operand(&mut rhs);
        }
        self.visit_expr_mut(&mut lhs);
        self.visit_expr_mut(&mut rhs);
        let comparison = self.compare_present(lhs, op, rhs);
        option_comparison(lhs_none, op, rhs_none, comparison)
    }

    fn compare_relation_identities(&mut self, lhs: &Expr, rhs: &Expr) -> Option<Expr> {
        use toasty_core::schema::app::FieldTy;

        if !self.capability().sql() {
            return None;
        }
        let left = super::relation_expr::resolve(&self.expr_cx, lhs)?;
        let right = super::relation_expr::resolve(&self.expr_cx, rhs)?;
        if !left.is_endpoint() || !right.is_endpoint() || left.target != right.target {
            return None;
        }
        if let (FieldTy::BelongsTo(left), FieldTy::BelongsTo(right)) =
            (&left.field.ty, &right.field.ty)
            && left.foreign_key.fields.iter().map(|f| f.target).eq(right
                .foreign_key
                .fields
                .iter()
                .map(|f| f.target))
        {
            // Matching reference keys already compare the same identity.
            return None;
        }
        let query = stmt::Query::new_select(left.target, true);
        let Expr::InSubquery(left) =
            super::lift_in_subquery::lift_in_subquery(&self.expr_cx, lhs, &query)?
        else {
            return None;
        };
        let Expr::InSubquery(right) =
            super::lift_in_subquery::lift_in_subquery(&self.expr_cx, rhs, &query)?
        else {
            return None;
        };
        if left.query.body.as_select_unwrap().source != right.query.body.as_select_unwrap().source {
            return None;
        }
        // Lower each lookup before adding correlation. Application field
        // references count statement scopes; the correlated SQL columns
        // below count query scopes instead.
        let mut lowered_left = Expr::InSubquery(left);
        let mut lowered_right = Expr::InSubquery(right);
        self.visit_expr_mut(&mut lowered_left);
        self.visit_expr_mut(&mut lowered_right);
        let Expr::InSubquery(mut left) = lowered_left else {
            return None;
        };
        let Expr::InSubquery(mut right) = lowered_right else {
            return None;
        };
        ShiftReferences { depth: 0 }.visit_expr_mut(&mut left.expr);
        ShiftReferences { depth: 0 }.visit_expr_mut(&mut right.expr);
        let key = |query: &stmt::Query| {
            let value = query.returning_unwrap().as_project_unwrap();
            super::scalar_or_record(
                record_fields(value)
                    .unwrap_or_else(|| vec![value.clone()])
                    .into_iter(),
            )
        };
        let left_key = key(&left.query);
        let right_key = key(&right.query);
        let select = left.query.body.as_select_mut_unwrap();
        // Native key equality deliberately excludes absent foreign keys.
        select.add_filter(Expr::eq(left_key, *left.expr));
        select.add_filter(Expr::eq(right_key, *right.expr));
        select.add_filter(right.query.body.as_select_unwrap().filter.clone());
        select.returning = stmt::Returning::Project(Expr::record([Expr::TRUE]));
        Some(Expr::exists(*left.query))
    }

    fn compare_present(&mut self, lhs: Expr, op: BinaryOp, rhs: Expr) -> Expr {
        let lhs = super::collapse_projections(lhs);
        let rhs = super::collapse_projections(rhs);
        if lhs.is_value_null() || rhs.is_value_null() {
            return false.into();
        }
        if is_nan_literal(&lhs) || is_nan_literal(&rhs) {
            return op.is_ne().into();
        }
        if let Expr::Match(decoded) = lhs {
            return self.compare_match(decoded, op, rhs, true);
        }
        if let Expr::Match(decoded) = rhs {
            return self.compare_match(decoded, op, lhs, false);
        }
        // A unit enum arm has a scalar discriminant, while a data arm has a
        // record payload. Different variant shapes cannot compare equal.
        if (op.is_eq() || op.is_ne())
            && record_fields(&lhs).is_some() != record_fields(&rhs).is_some()
            && !matches!(lhs, Expr::Project(_))
            && !matches!(rhs, Expr::Project(_))
        {
            return op.is_ne().into();
        }
        if (op.is_eq() || op.is_ne())
            && let (Some(lhs), Some(rhs)) = (record_fields(&lhs), record_fields(&rhs))
        {
            if lhs.len() != rhs.len() {
                return op.is_ne().into();
            }
            let comparisons = lhs
                .into_iter()
                .zip(rhs)
                .map(|(lhs, rhs)| {
                    let lhs_none = Expr::is_null(lhs.clone());
                    let rhs_none = Expr::is_null(rhs.clone());
                    let comparison = self.compare_present(lhs, op, rhs);
                    option_comparison(lhs_none, op, rhs_none, comparison)
                })
                .collect();
            return if op.is_eq() {
                Expr::and_from_vec(comparisons)
            } else {
                Expr::or_from_vec(comparisons)
            };
        }
        let mut lhs = lhs;
        let mut rhs = rhs;
        let string_payload = [&lhs, &rhs].iter().any(|expr| match expr {
            Expr::Value(Value::String(_)) => true,
            Expr::Cast(cast) => cast.ty.is_string(),
            Expr::Reference(reference) => {
                self.expr_cx.infer_expr_reference_ty(reference).is_string()
            }
            _ => false,
        });
        if let Some(comparison) = self.lower_expr_binary_op(op, &mut lhs, &mut rhs) {
            return comparison;
        }
        // MySQL's default string collation folds case and can ignore trailing
        // spaces. An explicit binary operand preserves Rust string equality.
        if self.capability().sql()
            && (self.capability().binary_like_starts_with || (!op.is_eq() && !op.is_ne()))
            && !(lhs.is_value() && rhs.is_value())
            && string_payload
        {
            lhs = Expr::BinaryString(Box::new(lhs));
            rhs = Expr::BinaryString(Box::new(rhs));
        }
        let mut guards = Vec::new();
        if self.capability().native_float_nan {
            for operand in [&lhs, &rhs] {
                let ty = match operand {
                    Expr::Reference(reference) => {
                        Some(self.expr_cx.infer_expr_reference_ty(reference))
                    }
                    Expr::Cast(cast) => Some(cast.ty.clone()),
                    _ => None,
                };
                if matches!(ty, Some(stmt::Type::F32 | stmt::Type::F64)) {
                    guards.push(Expr::not(Expr::IsNan(Box::new(operand.clone()))));
                }
            }
        }
        guards.push(Expr::binary_op(lhs, op, rhs));
        Expr::and_from_vec(guards)
    }

    fn compare_match(
        &mut self,
        decoded: stmt::ExprMatch,
        op: BinaryOp,
        other: Expr,
        left: bool,
    ) -> Expr {
        let mut unmatched = Expr::TRUE;
        let mut branches = Vec::new();
        for arm in decoded.arms {
            let pattern = Expr::from(arm.pattern);
            let present =
                self.compare_present((*decoded.subject).clone(), BinaryOp::Eq, pattern.clone());
            let matches = option_comparison(
                Expr::is_null((*decoded.subject).clone()),
                BinaryOp::Eq,
                Expr::is_null(pattern),
                present,
            );
            let guard = Expr::and(unmatched.clone(), matches.clone());
            unmatched = Expr::and(unmatched, Expr::not(matches));
            let comparison = if left {
                self.compare_present(arm.expr, op, other.clone())
            } else {
                self.compare_present(other.clone(), op, arm.expr)
            };
            branches.push(Expr::and(guard, comparison));
        }
        // Enum decoders can wrap the unreachable error in a cast or record.
        // That branch is not part of the application's value domain.
        if !contains_error(&decoded.else_expr) {
            let comparison = if left {
                self.compare_present(*decoded.else_expr, op, other)
            } else {
                self.compare_present(other, op, *decoded.else_expr)
            };
            branches.push(Expr::and(unmatched, comparison));
        }
        Expr::or_from_vec(branches)
    }

    fn lower_app_in_subquery(&mut self, expr: stmt::ExprInSubquery) -> Expr {
        let lhs_optional = record_fields(&expr.expr)
            .unwrap_or_else(|| vec![(*expr.expr).clone()])
            .iter()
            .any(|field| !self.app_is_none(field).is_false());
        let mut rhs_none = match expr.query.returning_unwrap() {
            stmt::Returning::Project(expr) => Expr::or_from_vec(
                record_fields(expr)
                    .unwrap_or_else(|| vec![expr.clone()])
                    .into_iter()
                    .map(Expr::is_null)
                    .collect(),
            ),
            _ => false.into(),
        };
        simplify::simplify_expr(
            self.expr_cx.scope(&*expr.query),
            self.capability(),
            &mut rhs_none,
        );
        let rhs_optional = !rhs_none.is_false();
        let mut lowered = Expr::InSubquery(expr);
        self.visit_expr_mut(&mut lowered);
        let payload_requires_comparison = match &lowered {
            Expr::InSubquery(membership) if self.capability().sql() => needs_payload_comparison(
                &self.expr_cx.infer_expr_ty(&membership.expr, &[]),
                self.capability(),
            ),
            _ => false,
        };
        if !lhs_optional && !rhs_optional && !payload_requires_comparison {
            return lowered;
        }
        let Expr::InSubquery(mut membership) = lowered else {
            // Key-value subqueries become lists bound before driver execution.
            return self.lower_bound_membership(lowered);
        };

        // Keep the original query, including LIMIT/OFFSET, as a derived table.
        // Filtering inside it would change membership in a limited result set.
        ShiftReferences { depth: 0 }.visit_expr_mut(&mut membership.expr);
        ShiftReferences { depth: 0 }.visit_stmt_query_mut(&mut membership.query);
        let rhs = match record_fields(&membership.expr) {
            Some(fields) => Expr::record((0..fields.len()).map(|column| {
                Expr::column(stmt::ExprColumn {
                    nesting: 0,
                    table: 0,
                    column,
                })
            })),
            None => Expr::column(stmt::ExprColumn {
                nesting: 0,
                table: 0,
                column: 0,
            }),
        };
        let lhs = *membership.expr;
        let source = stmt::Source::from(stmt::SourceTable::new(
            vec![stmt::TableRef::Derived(stmt::TableDerived {
                subquery: membership.query,
            })],
            stmt::TableWithJoins {
                relation: stmt::TableFactor::Table(stmt::SourceTableId(0)),
                joins: vec![],
            },
        ));
        let predicate = option_comparison(
            Expr::is_null(lhs.clone()),
            BinaryOp::Eq,
            Expr::is_null(rhs.clone()),
            self.scope_expr(&source)
                .compare_present(lhs, BinaryOp::Eq, rhs),
        );
        let mut select = stmt::Select::new(source, predicate);
        select.returning = stmt::Returning::Project(Expr::record([Expr::TRUE]));
        let exists = Expr::exists(stmt::Query::new(select));
        if membership.negated {
            Expr::not(exists)
        } else {
            exists
        }
    }

    fn lower_bound_membership(&mut self, expr: Expr) -> Expr {
        let Expr::InList(membership) = expr else {
            return expr;
        };
        let mut subject = *membership.expr;
        // The map introduces an argument scope, but no query-reference scope.
        stmt::visit_mut::walk_expr_scoped_mut(&mut subject, 0, |expr, depth| {
            if let Expr::Arg(arg) = expr
                && arg.nesting >= depth
            {
                arg.nesting += 1;
            }
            true
        });
        let candidate = Expr::arg(0);
        let predicate = option_comparison(
            Expr::is_null(subject.clone()),
            BinaryOp::Eq,
            Expr::is_null(candidate.clone()),
            self.compare_present(subject, BinaryOp::Eq, candidate),
        );
        Expr::any(Expr::map(*membership.list, predicate))
    }
}

/// Encode application option constructors at the storage boundary. Application
/// predicates retain their constructors until their presence tests are lowered.
pub(super) struct EncodeOptions;

impl VisitMut for EncodeOptions {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        match expr {
            Expr::App(_) => (),
            Expr::OptionSome(_) => {
                let Expr::OptionSome(inner) = expr.take() else {
                    unreachable!()
                };
                *expr = *inner;
                self.visit_expr_mut(expr);
            }
            _ => stmt::visit_mut::visit_expr_mut(self, expr),
        }
    }

    fn visit_value_mut(&mut self, value: &mut Value) {
        if let Value::Option(_) = value {
            let Value::Option(inner) = value.take() else {
                unreachable!()
            };
            *value = inner.map_or(Value::Null, |value| *value);
            self.visit_value_mut(value);
        } else {
            stmt::visit_mut::visit_value_mut(self, value);
        }
    }
}

fn contains_error(expr: &Expr) -> bool {
    use stmt::Visit;

    struct FindError(bool);
    impl Visit for FindError {
        fn visit_expr_error(&mut self, _: &stmt::ExprError) {
            self.0 = true;
        }
    }
    let mut visitor = FindError(false);
    visitor.visit_expr(expr);
    visitor.0
}

fn is_nan_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Value(Value::F32(value)) => value.is_nan(),
        Expr::Value(Value::F64(value)) => value.is_nan(),
        _ => false,
    }
}

fn some_payload(expr: &Expr) -> Option<Expr> {
    match expr {
        Expr::OptionSome(inner) => Some((**inner).clone()),
        Expr::Value(Value::Option(Some(inner))) => Some(Expr::Value((**inner).clone())),
        _ => None,
    }
}

fn needs_payload_comparison(ty: &stmt::Type, capability: &toasty_core::driver::Capability) -> bool {
    match ty {
        stmt::Type::String => capability.binary_like_starts_with,
        stmt::Type::F32 | stmt::Type::F64 => capability.native_float_nan,
        stmt::Type::Record(fields) => fields
            .iter()
            .any(|ty| needs_payload_comparison(ty, capability)),
        _ => false,
    }
}

struct ShiftReferences {
    depth: usize,
}

impl VisitMut for ShiftReferences {
    fn visit_expr_reference_mut(&mut self, reference: &mut stmt::ExprReference) {
        let nesting = match reference {
            stmt::ExprReference::Column(column) => &mut column.nesting,
            stmt::ExprReference::Field { nesting, .. } | stmt::ExprReference::Model { nesting } => {
                nesting
            }
        };
        if *nesting >= self.depth {
            *nesting += 1;
        }
    }

    fn visit_stmt_query_mut(&mut self, query: &mut stmt::Query) {
        self.depth += 1;
        stmt::visit_mut::visit_stmt_query_mut(self, query);
        self.depth -= 1;
    }
}

fn record_fields(expr: &Expr) -> Option<Vec<Expr>> {
    match expr {
        Expr::Record(record) => Some(record.fields.clone()),
        Expr::Value(Value::Record(record)) => {
            Some(record.fields.iter().cloned().map(Expr::from).collect())
        }
        _ => None,
    }
}

fn option_comparison(lhs_none: Expr, op: BinaryOp, rhs_none: Expr, present: Expr) -> Expr {
    let both_present = Expr::and(Expr::not(lhs_none.clone()), Expr::not(rhs_none.clone()));
    let present = Expr::and(both_present, present);
    let absent = match op {
        BinaryOp::Eq | BinaryOp::Ge | BinaryOp::Le => Expr::and(lhs_none.clone(), rhs_none.clone()),
        BinaryOp::Ne => Expr::or(
            Expr::and(lhs_none.clone(), Expr::not(rhs_none.clone())),
            Expr::and(Expr::not(lhs_none.clone()), rhs_none.clone()),
        ),
        _ => false.into(),
    };
    let absent = match op {
        BinaryOp::Lt | BinaryOp::Le => Expr::or(absent, Expr::and(lhs_none, Expr::not(rhs_none))),
        BinaryOp::Gt | BinaryOp::Ge => Expr::or(absent, Expr::and(Expr::not(lhs_none), rhs_none)),
        _ => absent,
    };
    Expr::or(absent, present)
}
