use crate::{
    schema::{
        app::{Field, Model, ModelId},
        db::{self, Column, ColumnId, Table, TableId},
    },
    stmt::{
        Delete, Expr, ExprReference, ExprSet, Insert, InsertTarget, Query, Select, Source,
        TableRef, Type, Update, UpdateTarget,
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

/// Result of resolving an `ExprReference` to its concrete schema location.
///
/// When an expression references a field or column (e.g., `user.name` in a
/// WHERE clause), the `ExprContext::resolve_expr_reference()` method returns
/// this enum to indicate whether the reference points to an application field,
/// physical table column, or CTE column.
///
/// This distinction is important for different processing stages: application
/// fields are used during high-level query building, physical columns during
/// SQL generation, and CTE columns require special handling with generated
/// identifiers based on position.
#[derive(Debug)]
pub enum ResolvedRef<'a> {
    /// A resolved reference to a physical database column.
    ///
    /// Contains a reference to the actual Column struct with column metadata including
    /// name, type, and constraints. Used when resolving ExprReference::Column expressions
    /// that point to concrete table columns in the database schema.
    ///
    /// Example: Resolving `user.name` in a query returns Column with name="name",
    /// ty=Type::String from the users table schema.
    Column(&'a Column),

    /// A resolved reference to an application-level field.
    ///
    /// Contains a reference to the Field struct from the application schema,
    /// which includes field metadata like name, type, and model relationships.
    /// Used when resolving ExprReference::Field expressions that point to
    /// model fields before they are lowered to database columns.
    ///
    /// Example: Resolving `User::name` in a query returns Field with name="name"
    /// from the User model's field definitions.
    Field(&'a Field),

    /// A resolved reference to a Common Table Expression (CTE) column.
    ///
    /// Contains the nesting level and column index for CTE references when resolving
    /// ExprReference::Column expressions that point to CTE outputs rather than physical
    /// table columns. The nesting indicates how many query levels to traverse upward,
    /// and index identifies which column within the CTE's output.
    ///
    /// Example: In a WITH clause, resolving a reference to the second column of a CTE
    /// defined 1 level up returns Cte { nesting: 1, index: 1 }.
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

pub trait Resolve {
    fn model(&self, id: ModelId) -> Option<&Model>;

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

    pub fn expr_column(&self, column_id: impl Into<ColumnId>) -> ExprReference {
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

        ExprReference::Column {
            nesting: 0,
            table: 0,
            column: column_id.index,
        }
    }
}

impl<'a, T: Resolve> ExprContext<'a, T> {
    /// Resolves an ExprReference::Column reference to the actual database Column it
    /// represents.
    ///
    /// Given an ExprReference::Column (which contains table/column indices and nesting
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
    pub fn resolve_expr_reference(&self, expr_reference: &ExprReference) -> ResolvedRef<'a> {
        let nesting = match expr_reference {
            ExprReference::Field { nesting, .. } => nesting,
            ExprReference::Column { nesting, .. } => nesting,
        };

        let mut curr = self;

        // Walk up the stack to the correct nesting level
        for _ in 0..*nesting {
            let Some(parent) = curr.parent else {
                todo!("bug: invalid nesting level");
            };

            curr = parent;
        }

        match curr.target {
            ExprTarget::Free => todo!("cannot resolve column in free context"),
            ExprTarget::Model(model) => match expr_reference {
                ExprReference::Field { index, .. } => ResolvedRef::Field(&model.fields[*index]),
                ExprReference::Column { .. } => panic!(
                    "Cannot resolve ExprReference::Column in Model target context - use ExprReference::Field instead"
                ),
            },
            ExprTarget::Table(table) => match expr_reference {
                ExprReference::Field {.. } => panic!(
                    "Cannot resolve ExprReference::Field in Table target context - use ExprReference::Column instead"
                ),
                ExprReference::Column { column, .. } => ResolvedRef::Column(&table.columns[*column]),
            },
            ExprTarget::Source(source) => match source {
                Source::Model(source_model) => match expr_reference {
                    ExprReference::Field { index, .. } => {
                        let Some(model) = self.schema.model(source_model.model) else {
                            panic!(
                                "Failed to resolve model with ID {:?} - model not found in schema",
                                source_model.model
                            )
                        };

                        ResolvedRef::Field(&model.fields[*index])
                    }
                    ExprReference::Column { .. } => panic!(
                        "Cannot resolve ExprReference::Column in Source::Model context - use ExprReference::Field instead"
                    ),
                },
                Source::Table(source_table) => match expr_reference {
                    ExprReference::Column { table, column, .. } => {
                        // Get the table reference at the specified index
                        let table_ref = &source_table.tables[*table];
                        match table_ref {
                            TableRef::Table(table_id) => {
                                let Some(table) = self.schema.table(*table_id) else {
                                    panic!(
                                    "Failed to resolve table with ID {:?} - table not found in schema",
                                    table_id
                                );
                                };
                                ResolvedRef::Column(&table.columns[*column])
                            }
                            TableRef::Cte {
                                nesting: cte_nesting,
                                index,
                            } => {
                                // TODO: return more info
                                ResolvedRef::Cte {
                                    nesting: *nesting + cte_nesting,
                                    index: *index,
                                }
                            }
                        }
                    }
                    ExprReference::Field { .. } => panic!(
                        "Cannot resolve ExprReference::Field in Source::Table context - use ExprReference::Column instead"
                    ),
                }
            },
            ExprTarget::Insert(insert_target) => match insert_target {
                InsertTarget::Model(model_id) => {
                    match expr_reference {
                        ExprReference::Field { index, .. } => {
                            let Some(model) = self.schema.model(*model_id) else {
                                panic!(
                                    "Failed to resolve model with ID {:?} for INSERT target - model not found in schema",
                                    model_id
                                )
                            };
                            ResolvedRef::Field(&model.fields[*index])
                        }
                        ExprReference::Column { .. } => panic!("ExprColumn should only be used with lowered InsertTarget::Table"),
                    }
                }
                InsertTarget::Table(insert_table) => {
                    match expr_reference {
                        ExprReference::Field { .. } => panic!(
                            "Cannot resolve ExprReference::Field in InsertTarget::Table context - use ExprReference::Column instead"
                        ),
                        ExprReference::Column { column, .. } => {
                            let Some(table) = self.schema.table(insert_table.table) else {
                                panic!("Failed to resolve table with ID {:?} for INSERT target - table not found in schema", insert_table.table);
                            };
                            ResolvedRef::Column(&table.columns[*column])
                        }
                    }
                }
                InsertTarget::Scope(_) => {
                    todo!()
                }
            },
            ExprTarget::Update(update_target) => match update_target {
                UpdateTarget::Model(model_id) => {
                    match expr_reference {
                        ExprReference::Field { index, .. } => {
                            let Some(model) = self.schema.model(*model_id) else {
                                panic!(
                                    "Failed to resolve model with ID {:?} for UPDATE target - model not found in schema",
                                    model_id
                                )
                            };
                            ResolvedRef::Field(&model.fields[*index])
                        }
                        ExprReference::Column { .. } => panic!("ExprColumn should only be used with lowered UpdateTarget::Table"),
                    }
                }
                UpdateTarget::Table(table_id) => {
                    match expr_reference {
                        ExprReference::Field { .. } => panic!(),
                        ExprReference::Column { column, .. } => {
                            let Some(table) = self.schema.table(*table_id) else {
                                panic!("Failed to resolve table with ID {:?} for UPDATE target - table not found in schema", table_id);
                            };
                            ResolvedRef::Column(&table.columns[*column])
                        }
                    }
                }
                UpdateTarget::Query(_) => {
                    todo!("ExprColumn should only be used with lowered UpdateTarget::Table")
                }
            },
        }
    }

    pub fn infer_expr_ty(&self, expr: &Expr, args: &[Type]) -> Type {
        match expr {
            Expr::Arg(e) => args[e.position].clone(),
            Expr::And(_) => Type::Bool,
            Expr::BinaryOp(_) => Type::Bool,
            Expr::Cast(e) => e.ty.clone(),
            Expr::Reference(expr_ref @ ExprReference::Column { .. }) => {
                match self.resolve_expr_reference(expr_ref) {
                    ResolvedRef::Column(column) => column.ty.clone(),
                    _ => todo!(),
                }
            }
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

impl Resolve for Schema {
    fn model(&self, id: ModelId) -> Option<&Model> {
        Some(self.app.model(id))
    }

    fn table(&self, id: TableId) -> Option<&Table> {
        Some(self.db.table(id))
    }
}

impl Resolve for db::Schema {
    fn model(&self, _id: ModelId) -> Option<&Model> {
        None
    }

    fn table(&self, id: TableId) -> Option<&Table> {
        Some(db::Schema::table(self, id))
    }
}

impl Resolve for () {
    fn model(&self, _id: ModelId) -> Option<&Model> {
        None
    }

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
