use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Insert<'stmt> {
    /// Where to insert the values
    pub target: InsertTarget<'stmt>,

    /// Source of values to insert
    pub source: Query<'stmt>,

    /// Optionally return data from the insertion
    pub returning: Option<Returning<'stmt>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertTarget<'stmt> {
    /// Inserting into a scope implies that the inserted value should be
    /// included by the query after insertion. This could be a combination of
    /// setting default field values or validating existing ones.
    Scope(Query<'stmt>),

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

impl<'stmt> Insert<'stmt> {
    pub fn merge(&mut self, other: Insert<'stmt>) {
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

impl<'stmt> From<Insert<'stmt>> for Statement<'stmt> {
    fn from(src: Insert<'stmt>) -> Statement<'stmt> {
        Statement::Insert(src)
    }
}

impl<'stmt> Node<'stmt> for Insert<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_stmt_insert(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_stmt_insert(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_stmt_insert_mut(self);
    }
}

impl<'stmt> InsertTarget<'stmt> {
    pub fn as_model_id(&self) -> ModelId {
        match self {
            InsertTarget::Scope(query) => query.body.as_select().source.as_model_id(),
            InsertTarget::Model(model_id) => *model_id,
            _ => todo!(),
        }
    }
}

impl<'stmt> From<InsertTable> for InsertTarget<'stmt> {
    fn from(value: InsertTable) -> Self {
        InsertTarget::Table(value)
    }
}

impl<'stmt> From<&InsertTable> for TableId {
    fn from(value: &InsertTable) -> Self {
        value.table
    }
}
