use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Update<'stmt> {
    /// Which records to update within the target
    pub selection: Query<'stmt>,

    /// Fields being updated
    pub fields: PathFieldSet,

    /// Expression to use to update the record
    pub expr: ExprRecord<'stmt>,

    /// A condition that must be satisfied in order for the update to apply.
    pub condition: Option<Expr<'stmt>>,

    /// If true, then Toasty should return a record for each instance of the
    /// model that was updated.
    pub returning: bool,
}

impl<'stmt> Update<'stmt> {
    pub fn ty(&self) -> Type {
        todo!()
    }

    pub fn set(&mut self, field: impl Into<PathStep>, expr: impl Into<Expr<'stmt>>) {
        let field = field.into().into_usize();
        self.fields.insert(field);
        self.expr[field] = expr.into();
    }
}

impl<'stmt> From<Update<'stmt>> for Statement<'stmt> {
    fn from(src: Update<'stmt>) -> Statement<'stmt> {
        Statement::Update(src)
    }
}

impl<'stmt> Node<'stmt> for Update<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_stmt_update(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_stmt_update(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_stmt_update_mut(self);
    }
}
