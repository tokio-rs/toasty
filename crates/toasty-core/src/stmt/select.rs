use super::{Node, Path, Query, Returning, Source, SourceModel, Statement, Visit, VisitMut};
use crate::{
    schema::db::TableId,
    stmt::{ExprSet, Filter},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Select {
    /// The projection part of a SQL query.
    pub returning: Returning,

    /// The `FROM` part of a SQL query. For model-level, this is the model being
    /// selected with any "includes". For table-level, this is the table with
    /// joins.
    pub source: Source,

    /// Query filter
    pub filter: Filter,
}

impl Select {
    pub fn new(source: impl Into<Source>, filter: impl Into<Filter>) -> Self {
        Self {
            returning: Returning::Model { include: vec![] },
            source: source.into(),
            filter: filter.into(),
        }
    }

    pub(crate) fn include(&mut self, path: impl Into<Path>) {
        match &mut self.returning {
            Returning::Model { include } => include.push(path.into()),
            _ => panic!("Expected Returning::Model for include operation"),
        }
    }

    pub fn add_filter(&mut self, filter: impl Into<Filter>) {
        self.filter.add_filter(filter);
    }

    /*
    pub fn or(&mut self, expr: impl Into<Expr>) {
        if let Expr::Or(expr_or) = &mut self.filter {
            expr_or.operands.push(expr.into());
        } else {
            self.filter = Expr::or(self.filter.take(), expr);
        }
    }
    */
}

impl ExprSet {
    pub fn as_select(&self) -> Option<&Select> {
        match self {
            Self::Select(expr) => Some(expr),
            _ => None,
        }
    }

    #[track_caller]
    pub fn as_select_unwrap(&self) -> &Select {
        self.as_select()
            .unwrap_or_else(|| panic!("expected `Select`; actual={self:#?}"))
    }

    pub fn as_select_mut(&mut self) -> Option<&mut Select> {
        match self {
            Self::Select(expr) => Some(expr),
            _ => None,
        }
    }

    pub fn as_select_mut_unwrap(&mut self) -> &mut Select {
        match self {
            Self::Select(select) => select,
            _ => panic!("expected `Select`; actual={self:#?}"),
        }
    }

    #[track_caller]
    pub fn into_select(self) -> Select {
        match self {
            Self::Select(expr) => *expr,
            _ => todo!(),
        }
    }

    pub fn is_select(&self) -> bool {
        matches!(self, Self::Select(_))
    }
}

impl From<Select> for Statement {
    fn from(value: Select) -> Self {
        Self::Query(value.into())
    }
}

impl From<Select> for Query {
    fn from(value: Select) -> Self {
        Self::builder(value).build()
    }
}

impl From<TableId> for Select {
    fn from(value: TableId) -> Self {
        Self::new(Source::table(value), true)
    }
}

impl From<SourceModel> for Select {
    fn from(value: SourceModel) -> Self {
        Self::new(Source::Model(value), true)
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
