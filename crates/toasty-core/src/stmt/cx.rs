use crate::{
    schema::app::{Field, Model, ModelId},
    stmt::{ExprReference, ExprSet, InsertTarget, Query, Select, Source, Update, UpdateTarget},
    Schema,
};

pub struct ExprContext<'a> {
    schema: &'a Schema,
    parent: Option<&'a ExprContext<'a>>,
    target: ExprTarget<'a>,
}

#[derive(Debug, Clone, Copy)]
pub enum ExprTarget<'a> {
    Const,
    Insert(&'a InsertTarget),
    Source(&'a Source),
    Update(&'a UpdateTarget),
}

impl<'a> ExprContext<'a> {
    pub fn new(schema: &'a Schema) -> ExprContext<'a> {
        ExprContext::new_with_target(schema, ExprTarget::Const)
    }

    pub fn new_with_target(
        schema: &'a Schema,
        target: impl Into<ExprTarget<'a>>,
    ) -> ExprContext<'a> {
        ExprContext {
            schema,
            parent: None,
            target: target.into(),
        }
    }

    pub fn schema(&self) -> &'a Schema {
        self.schema
    }

    pub fn target(&self) -> ExprTarget<'a> {
        self.target
    }

    pub fn scope<'child>(
        &'child self,
        target: impl Into<ExprTarget<'child>>,
    ) -> ExprContext<'child> {
        ExprContext {
            schema: self.schema,
            parent: Some(self),
            target: target.into(),
        }
    }

    pub fn target_as_model(&self) -> Option<&'a Model> {
        let model_id = self.target.as_model_id()?;
        Some(self.schema.app.model(model_id))
    }

    pub fn resolve_expr_reference(&self, expr_reference: &ExprReference) -> &'a Field {
        let ExprReference::Field { nesting, index } = expr_reference else {
            todo!();
        };

        let mut curr = self;

        // Walk up the stack
        for _ in 0..*nesting {
            let Some(parent) = self.parent else {
                todo!("bug");
            };

            curr = parent;
        }

        match curr.target {
            ExprTarget::Const => todo!("fail"),
            ExprTarget::Source(Source::Model(source_model)) => {
                &self.schema.app.model(source_model.model).fields[*index]
            }
            ExprTarget::Source(_) => todo!(),
            ExprTarget::Insert(InsertTarget::Model(model_id)) => {
                &self.schema.app.model(model_id).fields[*index]
            }
            ExprTarget::Insert(_) => todo!(),
            ExprTarget::Update(UpdateTarget::Query(query)) => {
                let model_id = query.body.as_select().source.as_model_id();
                &self.schema.app.model(model_id).fields[*index]
            }
            ExprTarget::Update(UpdateTarget::Model(model_id)) => {
                &self.schema.app.model(model_id).fields[*index]
            }
            ExprTarget::Update(UpdateTarget::Table(_)) => todo!(),
        }
    }
}

impl<'a> ExprTarget<'a> {
    pub fn as_model_id(self) -> Option<ModelId> {
        Some(match self {
            ExprTarget::Source(Source::Model(source_model)) => source_model.model,
            ExprTarget::Update(UpdateTarget::Model(model_id)) => *model_id,
            ExprTarget::Insert(InsertTarget::Model(model_id)) => *model_id,
            _ => return None,
        })
    }
}

impl<'a> From<&'a Query> for ExprTarget<'a> {
    fn from(value: &'a Query) -> Self {
        ExprTarget::from(&value.body)
    }
}

impl<'a> From<&'a ExprSet> for ExprTarget<'a> {
    fn from(value: &'a ExprSet) -> Self {
        match value {
            ExprSet::Select(select) => ExprTarget::from(&**select),
            ExprSet::SetOp(_) => todo!(),
            ExprSet::Update(update) => ExprTarget::from(&**update),
            ExprSet::Values(_) => ExprTarget::Const,
        }
    }
}

impl<'a> From<&'a Select> for ExprTarget<'a> {
    fn from(value: &'a Select) -> Self {
        ExprTarget::from(&value.source)
    }
}

impl<'a> From<&'a InsertTarget> for ExprTarget<'a> {
    fn from(value: &'a InsertTarget) -> Self {
        ExprTarget::Insert(value)
    }
}

impl<'a> From<&'a Source> for ExprTarget<'a> {
    fn from(value: &'a Source) -> Self {
        ExprTarget::Source(value)
    }
}

impl<'a> From<&'a Update> for ExprTarget<'a> {
    fn from(value: &'a Update) -> Self {
        ExprTarget::from(&value.target)
    }
}

impl<'a> From<&'a UpdateTarget> for ExprTarget<'a> {
    fn from(value: &'a UpdateTarget) -> Self {
        ExprTarget::Update(value)
    }
}
