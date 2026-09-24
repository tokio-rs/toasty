use super::LowerStatement;
use toasty_core::stmt::{self, VisitMut};

impl LowerStatement<'_, '_> {
    /// Lower derived inputs before their consumers so their columns carry
    /// storage types, with decoding casts applied where each column is used.
    pub(super) fn lower_derived_select_source(&mut self, select: &mut stmt::Select) -> bool {
        let stmt::Source::Table(source) = &mut select.source else {
            return false;
        };
        if !source
            .tables
            .iter()
            .any(|table| matches!(table, stmt::TableRef::Derived(_)))
        {
            return false;
        }

        if !self.capability().sql() && !Self::single_derived_source(source) {
            self.state
                .errors
                .push(toasty_core::Error::unsupported_feature(
                    "NoSQL derived queries require a single source without joins",
                ));
            return true;
        }

        for (table, source) in source.tables.iter_mut().enumerate() {
            let stmt::TableRef::Derived(derived) = source else {
                continue;
            };
            let casts = self.lower_derived_query(&mut derived.subquery);
            if let Some(filter) = &mut select.filter.expr {
                Self::decode_derived_columns(filter, table, &casts);
            }
            if let stmt::Returning::Project(returning) = &mut select.returning {
                Self::decode_derived_columns(returning, table, &casts);
            }
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

    fn lower_derived_query(&mut self, query: &mut stmt::Query) -> Vec<Option<stmt::ExprCast>> {
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

        // The inline query describes the derived columns; the HIR statement
        // owns execution and any dependencies of the derived query.
        let mut query: stmt::Statement = query.clone().into();
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
        let fields = match returning {
            stmt::Expr::Record(record) => record.fields.as_mut_slice(),
            expr => std::slice::from_mut(expr),
        };
        fields
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

    fn decode_derived_columns(
        expr: &mut stmt::Expr,
        table: usize,
        casts: &[Option<stmt::ExprCast>],
    ) {
        stmt::visit_mut::walk_expr_scoped_mut(expr, 0, |expr, depth| {
            let stmt::Expr::Reference(stmt::ExprReference::Column(column)) = expr else {
                return true;
            };
            if column.nesting != depth || column.table != table {
                return true;
            }
            if let Some(cast) = &casts[column.column] {
                let mut cast = cast.clone();
                cast.expr = Box::new(expr.take());
                *expr = cast.into();
                return false;
            }
            true
        });
    }
}
