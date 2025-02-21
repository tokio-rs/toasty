use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Select {
    /// The `FROM` part of a SQL query. For model-level, this is the model being
    /// selected with any "includes". For table-level, this is the table with
    /// joins.
    pub source: Source,

    /// Query filter
    pub filter: Expr,

    /// The projection part of a SQL query.
    pub returning: Returning,
}

impl Select {
    pub fn new(source: impl Into<Source>, filter: impl Into<Expr>) -> Select {
        Select {
            source: source.into(),
            filter: filter.into(),
            returning: Returning::Star,
        }
    }

    pub(crate) fn include(&mut self, path: impl Into<Path>) {
        match &mut self.source {
            Source::Model(source) => source.include.push(path.into()),
            Source::Table(_) => panic!(),
        }
    }

    pub fn and(&mut self, expr: impl Into<Expr>) {
        if let Expr::And(expr_and) = &mut self.filter {
            expr_and.operands.push(expr.into());
        } else {
            self.filter = Expr::and(self.filter.take(), expr);
        }
    }

    pub fn or(&mut self, expr: impl Into<Expr>) {
        if let Expr::Or(expr_or) = &mut self.filter {
            expr_or.operands.push(expr.into());
        } else {
            self.filter = Expr::or(self.filter.take(), expr);
        }
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input) {
        self.filter.substitute_ref(input);
    }
}

impl Node for Select {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_select(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_select_mut(self);
    }
}
