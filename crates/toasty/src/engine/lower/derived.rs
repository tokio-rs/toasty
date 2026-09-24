use super::LowerStatement;
use toasty_core::stmt::{self, VisitMut};

impl LowerStatement<'_, '_> {
    /// Lower a single derived input before its consumers so its columns carry
    /// storage types, with decoding casts applied where each column is used.
    pub(super) fn lower_derived_select_source(&mut self, select: &mut stmt::Select) -> bool {
        let stmt::Source::Table(source) = &mut select.source else {
            return false;
        };
        if !source.tables.iter().any(stmt::TableRef::is_derived) {
            return false;
        }

        if !Self::single_derived_source(source) {
            self.state
                .errors
                .push(toasty_core::Error::unsupported_feature(
                    "derived query lowering requires a single source without joins",
                ));
            return true;
        }

        let casts = self.lower_derived_query(&mut source.tables[0]);
        if let Some(filter) = &mut select.filter.expr {
            Self::decode_derived_columns(filter, &casts);
        }
        if let stmt::Returning::Project(returning) = &mut select.returning {
            Self::decode_derived_columns(returning, &casts);
        }
        true
    }

    fn single_derived_source(source: &stmt::SourceTable) -> bool {
        let single_source = matches!(
            source.from.as_slice(),
            [from] if from.joins.is_empty()
                && from.relation == stmt::TableFactor::Table(stmt::SourceTableId(0))
        );
        source.tables.len() == 1 && single_source
    }

    fn lower_derived_query(&mut self, source: &mut stmt::TableRef) -> Vec<Option<stmt::ExprCast>> {
        let stmt::TableRef::Derived(derived) = source else {
            unreachable!()
        };
        let query = &mut *derived.subquery;
        if self.capability().sql() {
            self.visit_stmt_query_mut(query);
            return Self::take_derived_output_casts(query);
        }

        let target_id = self.scope_statement(|child| child.visit_stmt_query_mut(query));
        let casts = Self::take_derived_output_casts(query);
        if !self.state.hir[target_id].independent {
            self.state
                .errors
                .push(toasty_core::Error::unsupported_feature(
                    "correlated derived queries are not supported on NoSQL",
                ));
            return casts;
        }

        let returning = query.returning_unwrap().as_project_unwrap();
        let ty = self
            .state
            .engine
            .expr_cx_for(&*query)
            .infer_expr_ty(returning, &[]);
        let stmt::Type::Record(columns) = ty else {
            unreachable!("derived queries return records")
        };
        let stmt::TableRef::Derived(derived) =
            std::mem::replace(source, stmt::TableRef::Input(columns))
        else {
            unreachable!()
        };
        let mut query: stmt::Statement = (*derived.subquery).into();
        self.state.engine.simplify_stmt(&mut query);
        self.state.hir[target_id].stmt = Some(Box::new(query));
        self.curr_stmt_info().derived_source = Some(target_id);
        self.track_dependency(target_id);
        casts
    }

    /// A derived table exposes stored values. Move decoding casts to the
    /// outer column references, just as model-field lowering inserts them.
    fn take_derived_output_casts(query: &mut stmt::Query) -> Vec<Option<stmt::ExprCast>> {
        let returning = query.returning_mut_unwrap().as_project_mut_unwrap();
        returning
            .as_record_mut_unwrap()
            .fields
            .iter_mut()
            .map(|field| {
                if !field.is_cast() {
                    return None;
                }
                let stmt::Expr::Cast(mut cast) = field.take() else {
                    unreachable!()
                };
                *field = cast.expr.take();
                Some(cast)
            })
            .collect()
    }

    fn decode_derived_columns(expr: &mut stmt::Expr, casts: &[Option<stmt::ExprCast>]) {
        stmt::visit_mut::walk_expr_scoped_mut(expr, 0, |expr, depth| {
            let stmt::Expr::Reference(stmt::ExprReference::Column(column)) = expr else {
                return true;
            };
            if column.nesting != depth || column.table != 0 {
                return true;
            }
            if let Some(cast) = &casts[column.column] {
                *expr = stmt::ExprCast {
                    expr: Box::new(expr.take()),
                    from: cast.from.clone(),
                    ty: cast.ty.clone(),
                }
                .into();
                return false;
            }
            true
        });
    }
}
