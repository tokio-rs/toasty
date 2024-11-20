use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Insert {
    /// Where to insert the values
    pub target: InsertTarget,

    /// Source of values to insert
    pub source: Query,

    /// Optionally return data from the insertion
    pub returning: Option<Returning>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertTarget {
    /// Inserting into a scope implies that the inserted value should be
    /// included by the query after insertion. This could be a combination of
    /// setting default field values or validating existing ones.
    Scope(Query),

    /// Insert a model
    Model(ModelId),

    /// Insert into a table
    Table(InsertTable),
}

#[derive(Debug, Clone, PartialEq)]
pub struct InsertTable {
    /// Table identifier to insert into
    pub table: TableId,

    /// Columns to insert into
    pub columns: Vec<ColumnId>,
}

impl Insert {
    pub fn merge(&mut self, other: Insert) {
        /*
        if self.target != other.target {
            todo!("handle this case");
        }

        match (&mut self.values, other.values) {
            (Expr::Record(expr_record), Expr::Record(other)) => {
                for expr in other {
                    expr_record.push(expr);
                }
            }
            (self_values, other) => todo!("self={:#?}; other={:#?}", self_values, other),
        }
        */
        todo!("self={self:#?} / other={other:#?}");
    }
}

impl From<Insert> for Statement {
    fn from(src: Insert) -> Statement {
        Statement::Insert(src)
    }
}

impl Node for Insert {
    fn map<V: Map>(&self, visit: &mut V) -> Self {
        visit.map_stmt_insert(self)
    }

    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_insert(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_insert_mut(self);
    }
}

impl InsertTarget {
    pub fn as_model_id(&self) -> ModelId {
        match self {
            InsertTarget::Scope(query) => query.body.as_select().source.as_model_id(),
            InsertTarget::Model(model_id) => *model_id,
            _ => todo!(),
        }
    }
}

impl From<Query> for InsertTarget {
    fn from(value: Query) -> Self {
        InsertTarget::Scope(value)
    }
}

impl From<InsertTable> for InsertTarget {
    fn from(value: InsertTable) -> Self {
        InsertTarget::Table(value)
    }
}

impl From<&InsertTable> for TableId {
    fn from(value: &InsertTable) -> Self {
        value.table
    }
}
