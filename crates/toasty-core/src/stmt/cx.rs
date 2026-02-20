use crate::{
    schema::{
        app::{Field, Model, ModelId},
        db::{self, Column, ColumnId, Table, TableId},
    },
    stmt::{
        Delete, Expr, ExprArg, ExprColumn, ExprReference, ExprSet, Insert, InsertTarget, Query,
        Returning, Select, Source, SourceTable, Statement, TableFactor, TableRef, Type, Update,
        UpdateTarget,
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

    /// A resolved reference to a model
    Model(&'a Model),

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

    /// A resolved reference to a derived table (subquery in FROM clause) column.
    ///
    /// Contains the nesting level and column index for derived table references when
    /// resolving ExprReference::Column expressions that point to derived table outputs.
    /// Similar to CTEs, derived tables use col_<index> naming for their columns.
    ///
    /// Example: Resolving a reference to the first column of a derived table at the
    /// current nesting level returns Derived { nesting: 0, index: 0 }.
    Derived { nesting: usize, index: usize },
}

#[derive(Debug, Clone, Copy)]
pub enum ExprTarget<'a> {
    /// Expression does *not* reference any model or table.
    Free,

    /// Expression references a single model
    Model(&'a Model),

    /// Expression references a single table
    ///
    /// Used primarily by database drivers
    Table(&'a Table),

    // Reference statement targets directly
    Source(&'a SourceTable),
}

pub trait Resolve {
    fn table_for_model(&self, model: &Model) -> Option<&Table>;

    /// Returns a reference to the application Model with the specified ID.
    ///
    /// Used during high-level query building to access model metadata such as
    /// field definitions, relationships, and validation rules. Returns None if
    /// the model ID is not found in the application schema.
    fn model(&self, id: ModelId) -> Option<&Model>;

    /// Returns a reference to the database Table with the specified ID.
    ///
    /// Used during SQL generation and query execution to access table metadata
    /// including column definitions, constraints, and indexes. Returns None if
    /// the table ID is not found in the database schema.
    fn table(&self, id: TableId) -> Option<&Table>;
}

pub trait IntoExprTarget<'a, T = Schema> {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a>;
}

#[derive(Debug)]
struct ArgTyStack<'a> {
    tys: &'a [Type],
    parent: Option<&'a ArgTyStack<'a>>,
}

impl<'a, T> ExprContext<'a, T> {
    pub fn schema(&self) -> &'a T {
        self.schema
    }

    pub fn target(&self) -> ExprTarget<'a> {
        self.target
    }

    /// Return the target at a specific nesting
    pub fn target_at(&self, nesting: usize) -> &ExprTarget<'a> {
        let mut curr = self;

        // Walk up the stack to the correct nesting level
        for _ in 0..nesting {
            let Some(parent) = curr.parent else {
                todo!("bug: invalid nesting level");
            };

            curr = parent;
        }

        &curr.target
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

impl<'a, T: Resolve> ExprContext<'a, T> {
    pub fn new(schema: &'a T) -> ExprContext<'a, T> {
        ExprContext::new_with_target(schema, ExprTarget::Free)
    }

    pub fn new_with_target(
        schema: &'a T,
        target: impl IntoExprTarget<'a, T>,
    ) -> ExprContext<'a, T> {
        let target = target.into_expr_target(schema);
        ExprContext {
            schema,
            parent: None,
            target,
        }
    }

    pub fn scope<'child>(
        &'child self,
        target: impl IntoExprTarget<'child, T>,
        // target: impl Into<ExprTarget<'child>>,
    ) -> ExprContext<'child, T> {
        let target = target.into_expr_target(self.schema);
        ExprContext {
            schema: self.schema,
            parent: Some(self),
            target,
        }
    }

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
            ExprReference::Column(expr_column) => expr_column.nesting,
            ExprReference::Field { nesting, .. } => *nesting,
            ExprReference::Model { nesting } => *nesting,
        };

        let target = self.target_at(nesting);

        match target {
            ExprTarget::Free => todo!("cannot resolve column in free context"),
            ExprTarget::Model(model) => match expr_reference {
                ExprReference::Model { .. } => ResolvedRef::Model(model),
                ExprReference::Field { index, .. } => ResolvedRef::Field(&model.fields[*index]),
                ExprReference::Column(expr_column) => {
                    assert_eq!(expr_column.table, 0, "TODO: is this true?");

                    let Some(table) = self.schema.table_for_model(model) else {
                        panic!("Failed to find database table for model '{:?}' - model may not be mapped to a table", model.name)
                    };
                    ResolvedRef::Column(&table.columns[expr_column.column])
                }
            },
            ExprTarget::Table(table) => match expr_reference {
                ExprReference::Model { .. } => panic!(
                    "Cannot resolve ExprReference::Model in Table target context"
                ),
                ExprReference::Field {.. } => panic!(
                    "Cannot resolve ExprReference::Field in Table target context - use ExprReference::Column instead"
                ),
                ExprReference::Column(expr_column) => ResolvedRef::Column(&table.columns[expr_column.column]),
            },
            ExprTarget::Source(source_table) => {
                match expr_reference {
                    ExprReference::Column(expr_column) => {
                        // Get the table reference at the specified index
                        let table_ref = &source_table.tables[expr_column.table];
                        match table_ref {
                            TableRef::Table(table_id) => {
                                let Some(table) = self.schema.table(*table_id) else {
                                    panic!(
                                    "Failed to resolve table with ID {:?} - table not found in schema.",
                                    table_id,
                                );
                                };
                                ResolvedRef::Column(&table.columns[expr_column.column])
                            }
                            TableRef::Derived { .. } => {
                                // Derived tables use col_<index> naming like CTEs
                                ResolvedRef::Derived {
                                    nesting: expr_column.nesting,
                                    index: expr_column.column,
                                }
                            }
                            TableRef::Cte {
                                nesting: cte_nesting,
                                index,
                            } => {
                                // TODO: return more info
                                ResolvedRef::Cte {
                                    nesting: expr_column.nesting + cte_nesting,
                                    index: *index,
                                }
                            }
                            TableRef::Arg(_) => todo!(),
                        }
                    }
                    ExprReference::Model { .. } => panic!(
                        "Cannot resolve ExprReference::Model in Source::Table context"
                    ),
                    ExprReference::Field { .. } => panic!(
                        "Cannot resolve ExprReference::Field in Source::Table context - use ExprReference::Column instead"
                    ),
                }
            }
        }
    }

    pub fn infer_stmt_ty(&self, stmt: &Statement, args: &[Type]) -> Type {
        let cx = self.scope(stmt);

        match stmt {
            Statement::Delete(stmt) => stmt
                .returning
                .as_ref()
                .map(|returning| cx.infer_returning_ty(returning, args, false))
                .unwrap_or(Type::Unit),
            Statement::Insert(stmt) => stmt
                .returning
                .as_ref()
                .map(|returning| cx.infer_returning_ty(returning, args, stmt.source.single))
                .unwrap_or(Type::Unit),
            Statement::Query(stmt) => match &stmt.body {
                ExprSet::Select(body) => cx.infer_returning_ty(&body.returning, args, stmt.single),
                ExprSet::SetOp(_body) => todo!(),
                ExprSet::Update(_body) => todo!(),
                ExprSet::Values(_body) => todo!(),
                ExprSet::Insert(body) => body
                    .returning
                    .as_ref()
                    .map(|returning| cx.infer_returning_ty(returning, args, stmt.single))
                    .unwrap_or(Type::Unit),
            },
            Statement::Update(stmt) => stmt
                .returning
                .as_ref()
                .map(|returning| cx.infer_returning_ty(returning, args, false))
                .unwrap_or(Type::Unit),
        }
    }

    fn infer_returning_ty(&self, returning: &Returning, args: &[Type], single: bool) -> Type {
        let arg_ty_stack = ArgTyStack::new(args);

        match returning {
            Returning::Model { .. } => {
                let ty = Type::Model(
                    self.target
                        .model_id()
                        .expect("returning `Model` when not in model context"),
                );

                if single {
                    ty
                } else {
                    Type::list(ty)
                }
            }
            Returning::Changed => todo!(),
            Returning::Expr(expr) => {
                let ty = self.infer_expr_ty2(&arg_ty_stack, expr, false);

                if single {
                    ty
                } else {
                    Type::list(ty)
                }
            }
            Returning::Value(expr) => self.infer_expr_ty2(&arg_ty_stack, expr, true),
        }
    }

    pub fn infer_expr_ty(&self, expr: &Expr, args: &[Type]) -> Type {
        let arg_ty_stack = ArgTyStack::new(args);
        self.infer_expr_ty2(&arg_ty_stack, expr, false)
    }

    fn infer_expr_ty2(&self, args: &ArgTyStack<'_>, expr: &Expr, returning_expr: bool) -> Type {
        match expr {
            Expr::Arg(e) => args.resolve_arg_ty(e).clone(),
            Expr::And(_) => Type::Bool,
            Expr::BinaryOp(_) => Type::Bool,
            Expr::Cast(e) => e.ty.clone(),
            Expr::Reference(expr_ref) => {
                assert!(
                    !returning_expr,
                    "should have been handled in Expr::Project. Invalid expr?"
                );
                self.infer_expr_reference_ty(expr_ref)
            }
            Expr::IsNull(_) => Type::Bool,
            Expr::List(e) => {
                debug_assert!(!e.items.is_empty());
                Type::list(self.infer_expr_ty2(args, &e.items[0], returning_expr))
            }
            Expr::Map(e) => {
                // Compute the map base type
                let base = self.infer_expr_ty2(args, &e.base, returning_expr);

                // The base type should be a list (as it is being mapped)
                let Type::List(item) = base else {
                    todo!("error handling; base={base:#?}")
                };

                let scope_tys = &[*item];

                // Create a new type scope
                let args = args.scope(scope_tys);

                // Infer the type of each map call
                let ty = self.infer_expr_ty2(&args, &e.map, returning_expr);

                // The mapped type is a list
                Type::list(ty)
            }
            Expr::Or(_) => Type::Bool,
            Expr::Project(e) => {
                if returning_expr {
                    match &*e.base {
                        Expr::Arg(expr_arg) => {
                            // When `returning_expr` is `true`, the expression is being
                            // evaluated from a RETURNING EXPR clause. In this case, the
                            // returning expression is *not* a projection. Referencing a
                            // column implies a *list* of
                            assert!(e.projection.as_slice().len() == 1);
                            return args.resolve_arg_ty(expr_arg).clone();
                        }
                        Expr::Reference(expr_reference) => {
                            // When `returning_expr` is `true`, the expression is being
                            // evaluated from a RETURNING EXPR clause. In this case, the
                            // returning expression is *not* a projection. Referencing a
                            // column implies a *list* of
                            assert!(e.projection.as_slice().len() == 1);
                            return self.infer_expr_reference_ty(expr_reference);
                        }
                        _ => {}
                    }
                }

                let mut base = self.infer_expr_ty2(args, &e.base, returning_expr);

                for step in e.projection.iter() {
                    base = match base {
                        Type::Record(mut fields) => {
                            std::mem::replace(&mut fields[*step], Type::Null)
                        }
                        Type::List(items) => *items,
                        expr => todo!(
                            "returning_expr={returning_expr:#?}; expr={expr:#?}; project={e:#?}"
                        ),
                    }
                }

                base
            }
            Expr::Record(e) => Type::Record(
                e.fields
                    .iter()
                    .map(|field| self.infer_expr_ty2(args, field, returning_expr))
                    .collect(),
            ),
            Expr::Value(value) => value.infer_ty(),
            _ => todo!("{expr:#?}"),
        }
    }

    pub fn infer_expr_reference_ty(&self, expr_reference: &ExprReference) -> Type {
        match self.resolve_expr_reference(expr_reference) {
            ResolvedRef::Model(model) => Type::Model(model.id),
            ResolvedRef::Column(column) => column.ty.clone(),
            ResolvedRef::Field(field) => field.expr_ty().clone(),
            ResolvedRef::Cte { .. } => todo!("type inference for CTE columns not implemented"),
            ResolvedRef::Derived { .. } => {
                todo!("type inference for derived table columns not implemented")
            }
        }
    }
}

impl<'a> ExprContext<'a, Schema> {
    pub fn target_as_model(&self) -> Option<&'a Model> {
        let model_id = self.target.model_id()?;
        Some(self.schema.app.model(model_id))
    }

    pub fn expr_ref_column(&self, column_id: impl Into<ColumnId>) -> ExprReference {
        let column_id = column_id.into();

        match self.target {
            ExprTarget::Free => {
                panic!("Cannot create ExprColumn in free context - no table target available")
            }
            ExprTarget::Model(model) => {
                let Some(table) = self.schema.table_for_model(model) else {
                    panic!("Failed to find database table for model '{:?}' - model may not be mapped to a table", model.name)
                };

                assert_eq!(table.id, column_id.table);
            }
            ExprTarget::Table(table) => assert_eq!(table.id, column_id.table),
            ExprTarget::Source(source_table) => {
                let [TableRef::Table(table_id)] = source_table.tables[..] else {
                    panic!(
                        "Expected exactly one table reference, found {} tables",
                        source_table.tables.len()
                    );
                };
                assert_eq!(table_id, column_id.table);
            }
        }

        ExprReference::Column(ExprColumn {
            nesting: 0,
            table: 0,
            column: column_id.index,
        })
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

    #[track_caller]
    pub fn expect_field(self) -> &'a Field {
        match self {
            ResolvedRef::Field(field) => field,
            _ => panic!("Expected ResolvedRef::Field, found {:?}", self),
        }
    }

    #[track_caller]
    pub fn expect_model(self) -> &'a Model {
        match self {
            ResolvedRef::Model(model) => model,
            _ => panic!("Expected ResolvedRef::Model, found {:?}", self),
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

    fn table_for_model(&self, model: &Model) -> Option<&Table> {
        Some(self.table_for(model))
    }
}

impl Resolve for db::Schema {
    fn model(&self, _id: ModelId) -> Option<&Model> {
        None
    }

    fn table(&self, id: TableId) -> Option<&Table> {
        Some(db::Schema::table(self, id))
    }

    fn table_for_model(&self, _model: &Model) -> Option<&Table> {
        None
    }
}

impl Resolve for () {
    fn model(&self, _id: ModelId) -> Option<&Model> {
        None
    }

    fn table(&self, _id: TableId) -> Option<&Table> {
        None
    }

    fn table_for_model(&self, _model: &Model) -> Option<&Table> {
        None
    }
}

impl<'a> ExprTarget<'a> {
    pub fn as_model(self) -> Option<&'a Model> {
        match self {
            ExprTarget::Model(model) => Some(model),
            _ => None,
        }
    }

    #[track_caller]
    pub fn as_model_unwrap(self) -> &'a Model {
        match self.as_model() {
            Some(model) => model,
            _ => panic!("expected ExprTarget::Model; was {self:#?}"),
        }
    }

    pub fn model_id(self) -> Option<ModelId> {
        Some(match self {
            ExprTarget::Model(model) => model.id,
            _ => return None,
        })
    }

    #[track_caller]
    pub fn as_table_unwrap(self) -> &'a Table {
        match self {
            ExprTarget::Table(table) => table,
            _ => panic!("expected ExprTarget::Table; was {self:#?}"),
        }
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for ExprTarget<'a> {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        match self {
            ExprTarget::Source(source_table) => {
                if source_table.from.len() == 1 && source_table.from[0].joins.is_empty() {
                    match &source_table.from[0].relation {
                        TableFactor::Table(source_table_id) => {
                            debug_assert_eq!(0, source_table_id.0);
                            debug_assert_eq!(1, source_table.tables.len());

                            match &source_table.tables[0] {
                                TableRef::Table(table_id) => {
                                    let table = schema.table(*table_id).unwrap();
                                    ExprTarget::Table(table)
                                }
                                _ => self,
                            }
                        }
                    }
                } else {
                    self
                }
            }
            _ => self,
        }
    }
}

impl<'a, T> IntoExprTarget<'a, T> for &'a Model {
    fn into_expr_target(self, _schema: &'a T) -> ExprTarget<'a> {
        ExprTarget::Model(self)
    }
}

impl<'a, T> IntoExprTarget<'a, T> for &'a Table {
    fn into_expr_target(self, _schema: &'a T) -> ExprTarget<'a> {
        ExprTarget::Table(self)
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a Query {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        self.body.into_expr_target(schema)
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a ExprSet {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        match self {
            ExprSet::Select(select) => select.into_expr_target(schema),
            ExprSet::SetOp(_) => todo!(),
            ExprSet::Update(update) => update.into_expr_target(schema),
            ExprSet::Values(_) => ExprTarget::Free,
            ExprSet::Insert(insert) => insert.into_expr_target(schema),
        }
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a Select {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        self.source.into_expr_target(schema)
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a Insert {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        self.target.into_expr_target(schema)
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a Update {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        self.target.into_expr_target(schema)
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a Delete {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        self.from.into_expr_target(schema)
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a InsertTarget {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        match self {
            InsertTarget::Scope(query) => query.into_expr_target(schema),
            InsertTarget::Model(model) => {
                let Some(model) = schema.model(*model) else {
                    todo!()
                };
                ExprTarget::Model(model)
            }
            InsertTarget::Table(insert_table) => {
                let table = schema.table(insert_table.table).unwrap();
                ExprTarget::Table(table)
            }
        }
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a UpdateTarget {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        match self {
            UpdateTarget::Query(query) => query.into_expr_target(schema),
            UpdateTarget::Model(model) => {
                let Some(model) = schema.model(*model) else {
                    todo!()
                };
                ExprTarget::Model(model)
            }
            UpdateTarget::Table(table_id) => {
                let Some(table) = schema.table(*table_id) else {
                    todo!()
                };
                ExprTarget::Table(table)
            }
        }
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a Source {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        match self {
            Source::Model(source_model) => {
                let Some(model) = schema.model(source_model.model) else {
                    todo!()
                };
                ExprTarget::Model(model)
            }
            Source::Table(source_table) => {
                ExprTarget::Source(source_table).into_expr_target(schema)
            }
        }
    }
}

impl<'a, T: Resolve> IntoExprTarget<'a, T> for &'a Statement {
    fn into_expr_target(self, schema: &'a T) -> ExprTarget<'a> {
        match self {
            Statement::Delete(stmt) => stmt.into_expr_target(schema),
            Statement::Insert(stmt) => stmt.into_expr_target(schema),
            Statement::Query(stmt) => stmt.into_expr_target(schema),
            Statement::Update(stmt) => stmt.into_expr_target(schema),
        }
    }
}

impl<'a> ArgTyStack<'a> {
    fn new(tys: &'a [Type]) -> ArgTyStack<'a> {
        ArgTyStack { tys, parent: None }
    }

    fn resolve_arg_ty(&self, expr_arg: &ExprArg) -> &'a Type {
        let mut nesting = expr_arg.nesting;
        let mut args = self;

        while nesting > 0 {
            args = args.parent.unwrap();
            nesting -= 1;
        }

        &args.tys[expr_arg.position]
    }

    fn scope<'child>(&'child self, tys: &'child [Type]) -> ArgTyStack<'child> {
        ArgTyStack {
            tys,
            parent: Some(self),
        }
    }
}
