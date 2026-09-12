use super::Expr;
use crate::schema::app::VariantId;

/// One application-schema traversal step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStep {
    /// Select a field, using a local index after a variant selection.
    Field(usize),
    /// Select an enum variant without projecting a record slot.
    Variant(VariantId),
}

/// An application-level path whose steps require schema-aware lowering.
///
/// Predicate construction attaches guards for the selected variants. The steps
/// remain explicit until schema-aware lowering resolves them to ordinary field
/// projections; record offsets are not part of this representation.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprPath {
    /// The expression to traverse.
    pub base: Box<Expr>,
    /// Field and variant selections in traversal order.
    pub steps: Vec<PathStep>,
}

impl Expr {
    /// Traverse application fields and enum variants from `base`.
    pub fn path(base: impl Into<Self>, steps: impl Into<Vec<PathStep>>) -> Self {
        let base = base.into();
        let steps = steps.into();
        if steps.is_empty() {
            return base;
        }
        if steps.iter().all(|step| matches!(step, PathStep::Field(_))) {
            let mut projection = super::Projection::identity();
            for step in steps {
                let PathStep::Field(index) = step else {
                    unreachable!()
                };
                projection.push(index);
            }
            return Self::project(base, projection);
        }
        Self::Path(ExprPath {
            base: Box::new(base),
            steps,
        })
    }

    /// Require every variant selected by this predicate's value operands.
    ///
    /// Call this when constructing a predicate, before applying boolean
    /// combinators. Existing AND/OR predicates and subqueries own their guards.
    /// Field selections remain application paths until schema-aware lowering.
    pub fn with_path_guards(self) -> Self {
        use super::Visit;

        #[derive(Default)]
        struct Guards(Vec<Expr>);
        impl Visit for Guards {
            fn visit_expr_path(&mut self, path: &ExprPath) {
                self.visit_expr(&path.base);
                for (offset, step) in path.steps.iter().enumerate() {
                    if let PathStep::Variant(variant) = step {
                        let parent = Expr::path(*path.base.clone(), path.steps[..offset].to_vec());
                        let guard = Expr::is_variant(parent, *variant);
                        if !self.0.contains(&guard) {
                            self.0.push(guard);
                        }
                    }
                }
            }
            fn visit_expr_stmt(&mut self, _: &super::ExprStmt) {}
            fn visit_stmt_query(&mut self, _: &super::Query) {}
            fn visit_expr_and(&mut self, _: &super::ExprAnd) {}
            fn visit_expr_or(&mut self, _: &super::ExprOr) {}
        }

        let mut guards = Guards::default();
        guards.visit_expr(&self);
        if guards.0.is_empty() {
            self
        } else {
            guards.0.push(self);
            Expr::and_from_vec(guards.0)
        }
    }
}
