use super::Simplify;
use toasty_core::stmt::{self, Expr, ExprReference, Source, Visit, VisitMut};

impl Simplify<'_> {
    /// Combine `IN` subqueries joined by `AND` when they test the same row.
    ///
    /// For example, if `users.id` is a primary key, this filter:
    ///
    /// ```sql
    /// user_id IN (SELECT id FROM users WHERE name = 'alice')
    /// AND user_id IN (SELECT id FROM users WHERE active)
    /// ```
    /// becomes:
    ///
    /// ```sql
    /// user_id IN (SELECT id FROM users WHERE name = 'alice' AND active)
    /// ```
    /// Both checks refer to the same user. That user must satisfy both
    /// filters. The subqueries can come from relation paths, explicit queries,
    /// or a mix of the two. Their operands need not be next to each other.
    ///
    /// The filters can contain `OR` or nested subqueries. For example, combining
    /// a filter for either Alice or Bob with a filter for active users gives:
    ///
    /// ```sql
    /// user_id IN (
    ///     SELECT id FROM users
    ///     WHERE (name = 'alice' OR name = 'bob') AND active
    /// )
    /// ```
    ///
    /// Both queries must test the same value and read the same columns from
    /// the same source. The returned columns must include a complete unique
    /// key, and none may be nullable. A secondary unique key works too. For a
    /// key made of `(tenant_id, id)`, returning only `tenant_id` is not enough.
    ///
    /// A query that returns a non-unique value stays separate. For example:
    ///
    /// ```sql
    /// parent_id IN (SELECT parent_id FROM children WHERE color = 'red')
    /// AND parent_id IN (SELECT parent_id FROM children WHERE color = 'blue')
    /// ```
    ///
    /// A parent can have one red child and another blue child. Combining the
    /// filters would require one child to be both red and blue. This is why
    /// separate `has_many` checks cannot usually be combined.
    ///
    /// Queries with limits also stay separate. For example, adding `active`
    /// to `WHERE name = 'alice' LIMIT 1` can select a different user from the
    /// one chosen before the merge. This rule also skips ordering, locks,
    /// CTEs, joins, set operations, and queries marked as returning one row.
    /// Those query restrictions also apply to nested subqueries. Queries that
    /// contain function calls or references to an outer query stay separate
    /// too. This rule combines only positive `IN` checks, not `NOT IN` checks.
    ///
    /// SQL nulls need care. If the left value is null, each `IN` check can be
    /// null even when the subqueries have no rows in common. The merged query
    /// then returns no rows, making `IN` false. Both results are rejected by a
    /// `WHERE` filter, including through `AND` or `OR`. They differ under `NOT` or when returned
    /// as a value, so those cases require a proven non-null left value. NoSQL
    /// membership always returns a boolean and does not need this restriction.
    ///
    /// This runs after relation lifting and before lowering extracts NoSQL
    /// subqueries into separate statements. Newly combined filters are
    /// simplified too, so matching subqueries inside them can also combine.
    pub(super) fn merge_in_subqueries(&self, operands: &mut Vec<Expr>) {
        for i in 0..operands.len() {
            // Earlier merges can shorten the operand list.
            if i >= operands.len() {
                break;
            }
            let Expr::InSubquery(first) = &operands[i] else {
                continue;
            };
            if !self.can_merge_membership(first) {
                continue;
            }

            let mut j = i + 1;
            while j < operands.len() {
                let Expr::InSubquery(first) = &operands[i] else {
                    unreachable!();
                };
                let compatible = match &operands[j] {
                    Expr::InSubquery(other) if self.can_merge_membership(other) => {
                        let a = first.query.body.as_select_unwrap();
                        let b = other.query.body.as_select_unwrap();
                        first.expr.is_equivalent_to(&other.expr)
                            && a.source == b.source
                            && a.returning == b.returning
                            && a.distinct == b.distinct
                    }
                    _ => false,
                };
                if !compatible {
                    j += 1;
                    continue;
                }

                let Expr::InSubquery(other) = operands.remove(j) else {
                    unreachable!();
                };
                let Expr::InSubquery(first) = &mut operands[i] else {
                    unreachable!();
                };
                let select = first.query.body.as_select_mut_unwrap();
                select.add_filter(other.query.body.as_select_unwrap().filter.clone());
                // Combining filters can expose further memberships at a deeper
                // relation hop. Simplify them before subquery extraction.
                self.scope(&select.source)
                    .visit_filter_mut(&mut select.filter);
            }
        }
    }

    fn can_merge_membership(&self, membership: &stmt::ExprInSubquery) -> bool {
        if membership.negated || !membership.expr.is_stable() {
            return false;
        }
        // In SQL, NULL IN two nonempty disjoint sets is NULL AND NULL, while
        // NULL IN their intersection is false. They select the same rows in
        // a positive filter, but are not interchangeable as values or under NOT.
        // NoSQL membership uses value equality and always returns a boolean.
        if self.capability.sql()
            && !self.positive_filter
            && !self.non_nullable_key(&membership.expr)
        {
            return false;
        }
        let mut safe = SafeQuery(true);
        safe.visit_stmt_query(&membership.query);
        if !safe.0 {
            return false;
        }
        let select = membership.query.body.as_select_unwrap();
        let stmt::Returning::Project(projection) = &select.returning else {
            return false;
        };
        let fields = projection
            .as_record()
            .map_or(std::slice::from_ref(projection), |record| &record.fields);

        match &select.source {
            Source::Model(source) => {
                let model = self.cx.schema().app.model(source.id).as_root_unwrap();
                let Some(indices) = fields
                    .iter()
                    .map(|expr| match expr {
                        Expr::Reference(ExprReference::Field { nesting: 0, index })
                            if !model.fields[*index].nullable =>
                        {
                            Some(*index)
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                model.indices.iter().any(|index| {
                    index.unique
                        && !index.fields.is_empty()
                        && index
                            .fields
                            .iter()
                            .all(|field| indices.contains(&field.field.index))
                })
            }
            Source::Table(source) => {
                let stmt::TableRef::Table(id) = source.tables[0] else {
                    unreachable!();
                };
                let table = self.cx.schema().db.table(id);
                let Some(indices) = fields
                    .iter()
                    .map(|expr| match expr {
                        Expr::Reference(ExprReference::Column(column))
                            if column.nesting == 0
                                && column.table == 0
                                && !table.columns[column.column].nullable =>
                        {
                            Some(column.column)
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                table.indices.iter().any(|index| {
                    index.unique
                        && !index.columns.is_empty()
                        && index
                            .columns
                            .iter()
                            .all(|column| indices.contains(&column.column.index))
                })
            }
        }
    }

    fn non_nullable_key(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Reference(reference @ ExprReference::Field { .. }) => !self
                .cx
                .resolve_expr_reference(reference)
                .as_field_unwrap()
                .nullable(),
            Expr::Record(record) => record
                .fields
                .iter()
                .all(|field| self.non_nullable_key(field)),
            Expr::Value(value) => !value.is_null() && !matches!(value, stmt::Value::Record(_)),
            // Columns may be nullable through an outer join even when their
            // schema declaration is not. Other expressions need a nullability proof.
            _ => false,
        }
    }
}

/// Restrict merging to plain reads. Walk nested subqueries too: their filters
/// may contain arbitrary boolean expressions, but must not introduce effects,
/// unstable functions, or references to an outer query scope.
struct SafeQuery(bool);

impl Visit for SafeQuery {
    fn visit_stmt_query(&mut self, query: &stmt::Query) {
        if query.with.is_some()
            || query.limit.is_some()
            || query.order_by.is_some()
            || !query.locks.is_empty()
            || query.single
        {
            self.0 = false;
            return;
        }
        let Some(select) = query.body.as_select() else {
            self.0 = false;
            return;
        };
        let plain_source = match &select.source {
            Source::Model(source) => source.via.is_none(),
            Source::Table(source) => {
                matches!(source.tables.as_slice(), [stmt::TableRef::Table(_)])
                    && matches!(source.from.as_slice(), [from]
                    if matches!(from.relation, stmt::TableFactor::Table(stmt::SourceTableId(0)))
                        && from.joins.is_empty())
            }
        };
        if !plain_source || !matches!(select.returning, stmt::Returning::Project(_)) {
            self.0 = false;
            return;
        }
        stmt::visit::visit_stmt_query(self, query);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        if matches!(
            expr,
            Expr::Func(_) | Expr::Stmt(_) | Expr::Default | Expr::Ident(_) | Expr::Error(_)
        ) {
            self.0 = false;
        } else {
            stmt::visit::visit_expr(self, expr);
        }
    }

    fn visit_expr_reference(&mut self, reference: &ExprReference) {
        let nesting = match reference {
            ExprReference::Field { nesting, .. } | ExprReference::Model { nesting } => *nesting,
            ExprReference::Column(column) => column.nesting,
        };
        self.0 &= nesting == 0;
    }
}
