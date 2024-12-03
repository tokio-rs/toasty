use super::*;

/// The "root" an expression is targetting. This can be a model, table, ...
pub(crate) enum ExprTarget<'a> {
    /// Expressions are executed in a constant context (no references to models
    /// or fields).
    Const,

    /// The expression is in context of a model before the expression has been
    /// lowered.
    Model(&'a Model),

    /// The expression has already been lowered and is in context of a table
    Table(&'a Table),

    /// A lowered insert specifies the columns to insert into
    TableWithColumns(&'a Table, Vec<ColumnId>),
}

impl<'a> ExprTarget<'a> {
    pub(crate) fn from_source(schema: &'a Schema, source: &stmt::Source) -> ExprTarget<'a> {
        match source {
            stmt::Source::Model(source_model) => {
                let model = schema.model(source_model.model);
                ExprTarget::from(model)
            }
            stmt::Source::Table(tables_with_joins) => {
                let [table_with_joins] = &tables_with_joins[..] else {
                    todo!("source={source:#?}")
                };

                let table = schema.table(table_with_joins.table);
                ExprTarget::from(table)
            }
            _ => todo!("source={source:#?}"),
        }
    }

    pub(crate) fn from_insert_target(
        schema: &'a Schema,
        target: &stmt::InsertTarget,
    ) -> ExprTarget<'a> {
        match target {
            stmt::InsertTarget::Scope(query) => {
                let model_id = query.body.as_select().source.as_model_id();
                let model = schema.model(model_id);
                ExprTarget::from(model)
            }
            stmt::InsertTarget::Model(model_id) => {
                let model = schema.model(*model_id);
                ExprTarget::from(model)
            }
            stmt::InsertTarget::Table(table_with_columns) => {
                let table = schema.table(table_with_columns.table);
                ExprTarget::TableWithColumns(table, table_with_columns.columns.clone())
            }
            _ => todo!(),
        }
    }

    pub(crate) fn from_update_target(
        schema: &'a Schema,
        target: &stmt::UpdateTarget,
    ) -> ExprTarget<'a> {
        match target {
            stmt::UpdateTarget::Model(model_id) => {
                let model = schema.model(*model_id);
                ExprTarget::from(model)
            }
            stmt::UpdateTarget::Table(table_with_columns) => {
                let table = schema.table(table_with_columns.table);
                ExprTarget::Table(table)
            }
        }
    }

    pub(crate) fn is_const(&self) -> bool {
        matches!(self, ExprTarget::Const)
    }

    pub(crate) fn is_model(&self) -> bool {
        matches!(self, ExprTarget::Model(_))
    }
}

impl<'a> From<&'a Model> for ExprTarget<'a> {
    fn from(value: &'a Model) -> Self {
        ExprTarget::Model(value)
    }
}

impl<'a> From<&'a Table> for ExprTarget<'a> {
    fn from(value: &'a Table) -> Self {
        ExprTarget::Table(value)
    }
}
