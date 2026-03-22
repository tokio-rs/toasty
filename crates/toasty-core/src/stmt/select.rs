use super::{Node, Path, Query, Returning, Source, SourceModel, Statement, Visit, VisitMut};
use crate::{
    schema::db::TableId,
    stmt::{ExprSet, Filter},
};

/// A `SELECT` expression within a query body.
///
/// Represents the combination of a data source, a filter (WHERE clause), and a
/// projection (RETURNING/SELECT list). This is the most common query body type.
///
/// At the model level, the source is a model with optional association includes.
/// After lowering, the source becomes a table with joins.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Select, Source, Filter};
/// use toasty_core::schema::app::ModelId;
///
/// let select = Select::new(Source::from(ModelId(0)), Filter::ALL);
/// assert!(select.source.is_model());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Select {
    /// The projection (what columns/fields to return).
    pub returning: Returning,

    /// The data source (`FROM` clause). At the model level this is a model
    /// reference; at the table level this is a table with joins.
    pub source: Source,

    /// The filter (`WHERE` clause).
    pub filter: Filter,
}

impl Select {
    /// Creates a new `Select` with the given source and filter, defaulting to
    /// a model-level returning clause with no includes.
    pub fn new(source: impl Into<Source>, filter: impl Into<Filter>) -> Self {
        Self {
            returning: Returning::Model { include: vec![] },
            source: source.into(),
            filter: filter.into(),
        }
    }

    /// Adds an association include path to the returning clause.
    ///
    /// # Panics
    ///
    /// Panics if the returning clause is not `Returning::Model`.
    pub(crate) fn include(&mut self, path: impl Into<Path>) {
        match &mut self.returning {
            Returning::Model { include } => include.push(path.into()),
            _ => panic!("Expected Returning::Model for include operation"),
        }
    }

    /// Adds an additional filter, AND-ing it with any existing filter.
    pub fn add_filter(&mut self, filter: impl Into<Filter>) {
        self.filter.add_filter(filter);
    }
}

impl Statement {
    /// If this is a query with a `SELECT` body, returns a reference to that
    /// [`Select`]. Returns `None` otherwise.
    pub fn query_select(&self) -> Option<&Select> {
        self.as_query().and_then(|query| query.body.as_select())
    }

    /// Returns a reference to this statement's inner [`Select`].
    ///
    /// # Panics
    ///
    /// Panics if this is not a query statement with a `SELECT` body.
    #[track_caller]
    pub fn query_select_unwrap(&self) -> &Select {
        match self {
            Statement::Query(query) => match &query.body {
                ExprSet::Select(select) => select,
                _ => panic!("expected `Select`; actual={self:#?}"),
            },
            _ => panic!("expected `Select`; actual={self:#?}"),
        }
    }
}

impl Query {
    /// Consumes this query and returns the inner [`Select`].
    ///
    /// # Panics
    ///
    /// Panics if the query body is not a `SELECT`.
    pub fn into_select(self) -> Select {
        self.body.into_select()
    }
}

impl ExprSet {
    /// Returns a reference to the inner [`Select`] if this is a `Select` variant.
    pub fn as_select(&self) -> Option<&Select> {
        match self {
            Self::Select(expr) => Some(expr),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`Select`], panicking if this is not a
    /// `Select` variant.
    #[track_caller]
    pub fn as_select_unwrap(&self) -> &Select {
        self.as_select()
            .unwrap_or_else(|| panic!("expected `Select`; actual={self:#?}"))
    }

    /// Returns a mutable reference to the inner [`Select`] if this is a
    /// `Select` variant.
    pub fn as_select_mut(&mut self) -> Option<&mut Select> {
        match self {
            Self::Select(expr) => Some(expr),
            _ => None,
        }
    }

    /// Returns a mutable reference to the inner [`Select`], panicking if this
    /// is not a `Select` variant.
    #[track_caller]
    pub fn as_select_mut_unwrap(&mut self) -> &mut Select {
        match self {
            Self::Select(select) => select,
            _ => panic!("expected `Select`; actual={self:#?}"),
        }
    }

    /// Consumes this `ExprSet` and returns the inner [`Select`].
    ///
    /// # Panics
    ///
    /// Panics if this is not a `Select` variant.
    #[track_caller]
    pub fn into_select(self) -> Select {
        match self {
            Self::Select(expr) => *expr,
            _ => todo!(),
        }
    }

    /// Returns `true` if this is a `Select` variant.
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
