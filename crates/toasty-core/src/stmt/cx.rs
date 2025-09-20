use crate::{
    schema::{
        app::{Field, Model, ModelId},
        db::{self, Column, ColumnId, Table, TableId},
    },
    stmt::{
        Delete, Expr, ExprColumn, ExprReference, ExprSet, Insert, InsertTarget, Query, Select,
        Source, TableRef, Type, Update, UpdateTarget,
    },
    Schema,
};

// TODO: we probably want two lifetimes here. One for &Schema and one for the stmt.
#[derive(Debug)]
pub struct ExprContext<'a, T = Schema> {
    schema: &'a T,
    parent: Option<&'a ExprContext<'a, T>>,
    target: ExprTarget<'a>,
}

#[derive(Debug)]
pub enum ResolvedRef<'a> {
    /// TODO: docs
    Column(&'a Column),

    /// TODO: fill this out
    Cte { nesting: usize, index: usize },
}

#[derive(Debug, Clone, Copy)]
pub enum ExprTarget<'a> {
    /// Expression does *not* reference any model or table.
    Free,

    /// Expression references a single model
    Model(&'a Model),

    /// Expression references a single table
    Table(&'a Table),

    // Reference statement targets directly
    Insert(&'a InsertTarget),
    Source(&'a Source),
    Update(&'a UpdateTarget),
}

pub trait DbSchema {
    fn table(&self, id: TableId) -> Option<&Table>;
}

impl<'a, T> ExprContext<'a, T> {
    pub fn new(schema: &'a T) -> ExprContext<'a, T> {
        ExprContext::new_with_target(schema, ExprTarget::Free)
    }

    pub fn new_with_target(schema: &'a T, target: impl Into<ExprTarget<'a>>) -> ExprContext<'a, T> {
        ExprContext {
            schema,
            parent: None,
            target: target.into(),
        }
    }

    pub fn schema(&self) -> &'a T {
        self.schema
    }

    pub fn target(&self) -> ExprTarget<'a> {
        self.target
    }

    pub fn scope<'child>(
        &'child self,
        target: impl Into<ExprTarget<'child>>,
    ) -> ExprContext<'child, T> {
        ExprContext {
            schema: self.schema,
            parent: Some(self),
            target: target.into(),
        }
    }
}

impl<'a> ExprContext<'a, ()> {
    pub fn new_free() -> ExprContext<'a, ()> {
        ExprContext {
            schema: &(),
            parent: None,
            target: ExprTarget::Free,
        }
    }
}

impl<'a> ExprContext<'a, Schema> {
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
            ExprTarget::Free => todo!("fail"),
            ExprTarget::Model(model) => &model.fields[*index],
            ExprTarget::Table(_) => todo!(),

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

    pub fn expr_column(&self, column_id: impl Into<ColumnId>) -> ExprColumn {
        let column_id = column_id.into();

        match self.target {
            ExprTarget::Free => {
                panic!("Cannot create ExprColumn in free context - no table target available")
            }
            ExprTarget::Model(_) => panic!(
                "Cannot create ExprColumn for model target - use resolve_expr_reference instead"
            ),
            ExprTarget::Table(table) => assert_eq!(table.id, column_id.table),
            ExprTarget::Insert(_) => todo!(),
            ExprTarget::Source(source) => match source {
                Source::Model(_) => panic!(
                    "Cannot create ExprColumn for model source - should be lowered to table first"
                ),
                Source::Table(source_table) => {
                    let [TableRef::Table(table_id)] = source_table.tables[..] else {
                        panic!(
                            "Expected exactly one table reference, found {} tables",
                            source_table.tables.len()
                        );
                    };
                    assert_eq!(table_id, column_id.table);
                }
            },
            ExprTarget::Update(_) => todo!(),
        }

        ExprColumn {
            nesting: 0,
            table: 0,
            column: column_id.index,
        }
    }
}

impl<'a, T: DbSchema> ExprContext<'a, T> {
    /// Resolves an ExprColumn reference to the actual database Column it
    /// represents.
    ///
    /// Given an ExprColumn (which contains table/column indices and nesting
    /// info), returns the Column struct containing the column's name, type,
    /// constraints, and other metadata.
    ///
    /// Handles:
    /// - Nested query scopes (walking up parent contexts based on nesting
    ///   level)
    /// - Different statement targets (INSERT, UPDATE, SELECT with joins, etc.)
    /// - Table references in multi-table operations (using the table index)
    ///
    /// Used by SQL serialization to get column names, query planning to
    /// match index columns, and key extraction to identify column IDs.
    pub fn resolve_expr_column(&self, expr_column: &ExprColumn) -> ResolvedRef<'a> {
        let mut curr = self;

        // Walk up the stack to the correct nesting level
        for _ in 0..expr_column.nesting {
            let Some(parent) = curr.parent else {
                todo!("bug: invalid nesting level");
            };

            curr = parent;
        }

        match curr.target {
            ExprTarget::Free => todo!("cannot resolve column in free context"),
            ExprTarget::Model(_) => todo!("cannot resolve column in model context"),
            ExprTarget::Table(table) => ResolvedRef::Column(&table.columns[expr_column.column]),
            ExprTarget::Source(Source::Table(source_table)) => {
                // Get the table reference at the specified index
                let table_ref = &source_table.tables[expr_column.table];
                match table_ref {
                    TableRef::Table(table_id) => {
                        let Some(table) = self.schema.table(*table_id) else {
                            panic!(
                                "Failed to resolve table with ID {:?} - table not found in schema",
                                table_id
                            );
                        };
                        ResolvedRef::Column(&table.columns[expr_column.column])
                    }
                    TableRef::Cte { nesting, index } => {
                        // TODO: return more info
                        ResolvedRef::Cte {
                            nesting: expr_column.nesting + nesting,
                            index: *index,
                        }
                    }
                }
            }
            ExprTarget::Source(Source::Model(_)) => {
                todo!("ExprColumn should only be used with lowered Source::Table")
            }
            ExprTarget::Insert(InsertTarget::Table(insert_table)) => {
                let Some(table) = self.schema.table(insert_table.table) else {
                    panic!("Failed to resolve table with ID {:?} for INSERT target - table not found in schema", insert_table.table);
                };
                ResolvedRef::Column(&table.columns[expr_column.column])
            }
            ExprTarget::Insert(InsertTarget::Model(_)) => {
                todo!("ExprColumn should only be used with lowered InsertTarget::Table")
            }
            ExprTarget::Insert(InsertTarget::Scope(_)) => {
                todo!("ExprColumn should only be used with lowered InsertTarget::Table")
            }
            ExprTarget::Update(UpdateTarget::Table(table_id)) => {
                let Some(table) = self.schema.table(*table_id) else {
                    panic!("Failed to resolve table with ID {:?} for UPDATE target - table not found in schema", table_id);
                };
                ResolvedRef::Column(&table.columns[expr_column.column])
            }
            ExprTarget::Update(UpdateTarget::Model(_)) => {
                todo!("ExprColumn should only be used with lowered UpdateTarget::Table")
            }
            ExprTarget::Update(UpdateTarget::Query(_)) => {
                todo!("ExprColumn should only be used with lowered UpdateTarget::Table")
            }
        }
    }

    pub fn infer_expr_ty(&self, expr: &Expr, args: &[Type]) -> Type {
        match expr {
            Expr::Arg(e) => args[e.position].clone(),
            Expr::And(_) => Type::Bool,
            Expr::BinaryOp(_) => Type::Bool,
            Expr::Cast(e) => e.ty.clone(),
            Expr::Column(e) => match self.resolve_expr_column(e) {
                ResolvedRef::Column(column) => column.ty.clone(),
                _ => todo!(),
            },
            Expr::Reference(_) => todo!(),
            Expr::IsNull(_) => Type::Bool,
            Expr::Map(e) => {
                let base = self.infer_expr_ty(&e.base, args);
                let ty = self.infer_expr_ty(&e.map, &[base]);
                Type::list(ty)
            }
            Expr::Or(_) => Type::Bool,
            Expr::Project(e) => {
                let mut base = self.infer_expr_ty(&e.base, args);

                for step in e.projection.iter() {
                    base = match &mut base {
                        Type::Record(fields) => std::mem::replace(&mut fields[*step], Type::Null),
                        expr => todo!("expr={expr:#?}"),
                    }
                }

                base
            }
            Expr::Record(e) => Type::Record(
                e.fields
                    .iter()
                    .map(|field| self.infer_expr_ty(field, args))
                    .collect(),
            ),
            Expr::Value(value) => value.infer_ty(),
            // -- hax
            Expr::DecodeEnum(_, ty, _) => ty.clone(),
            _ => todo!("{expr:#?}"),
        }
    }
}

impl<'a, T> Clone for ExprContext<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Copy for ExprContext<'a, T> {}

impl<'a> ResolvedRef<'a> {
    #[track_caller]
    pub fn expect_column(self) -> &'a Column {
        match self {
            ResolvedRef::Column(column) => column,
            _ => panic!("Expected ResolvedRef::Column, found {:?}", self),
        }
    }
}

impl DbSchema for Schema {
    fn table(&self, id: TableId) -> Option<&Table> {
        Some(self.db.table(id))
    }
}

impl DbSchema for db::Schema {
    fn table(&self, id: TableId) -> Option<&Table> {
        Some(db::Schema::table(self, id))
    }
}

impl DbSchema for () {
    fn table(&self, _: TableId) -> Option<&Table> {
        None
    }
}

impl<'a> ExprTarget<'a> {
    pub fn as_model_id(self) -> Option<ModelId> {
        Some(match self {
            ExprTarget::Model(model) => model.id,
            ExprTarget::Source(Source::Model(source_model)) => source_model.model,
            ExprTarget::Update(UpdateTarget::Model(model_id)) => *model_id,
            ExprTarget::Insert(InsertTarget::Model(model_id)) => *model_id,
            _ => return None,
        })
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
            ExprSet::Values(_) => ExprTarget::Free,
        }
    }
}

impl<'a> From<&'a Select> for ExprTarget<'a> {
    fn from(value: &'a Select) -> Self {
        ExprTarget::from(&value.source)
    }
}

impl<'a> From<&'a Insert> for ExprTarget<'a> {
    fn from(value: &'a Insert) -> Self {
        ExprTarget::from(&value.target)
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

impl<'a> From<&'a Delete> for ExprTarget<'a> {
    fn from(value: &'a Delete) -> Self {
        ExprTarget::from(&value.from)
    }
}
