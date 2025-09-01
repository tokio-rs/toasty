use crate::{
    schema::{app::FieldId, Schema},
    stmt::*,
};

impl Statement {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        match self {
            Statement::Delete(d) => d.infer_ty(schema, args),
            Statement::Insert(i) => i.infer_ty(schema, args),
            Statement::Query(q) => q.infer_ty(schema, args),
            Statement::Update(u) => u.infer_ty(schema, args),
        }
    }
}

impl Query {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        match &self.body {
            ExprSet::Select(select) => select.infer_ty(schema, args),
            ExprSet::SetOp(set_op) => set_op.infer_ty(schema, args),
            ExprSet::Values(values) => values.infer_ty(schema, args),
            ExprSet::Update(update) => update.infer_ty(schema, args),
        }
    }
}

impl Select {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        match &self.returning {
            Returning::Star => {
                // For SELECT *, infer based on the source
                match &self.source {
                    Source::Model(source_model) => {
                        let model = schema.app.model(source_model.model);
                        let field_types: Vec<Type> =
                            model.fields.iter().map(|f| f.expr_ty().clone()).collect();
                        Type::Record(field_types)
                    }
                    Source::Table(table_with_joins) => {
                        // For table-based sources, infer from the first table's columns
                        if let Some(first_table) = table_with_joins.first() {
                            match &first_table.table {
                                TableRef::Table(table_id) => {
                                    let table = &schema.db.tables[table_id.0];
                                    let column_types: Vec<Type> =
                                        table.columns.iter().map(|c| c.ty.clone()).collect();
                                    Type::Record(column_types)
                                }
                                TableRef::Cte { .. } => {
                                    // CTE references are not yet supported
                                    Type::Unknown
                                }
                            }
                        } else {
                            Type::Unknown
                        }
                    }
                }
            }
            Returning::Expr(expr) => expr.infer_ty(schema, args),
            Returning::Changed => Type::I64, // Returns count of changed rows
        }
    }
}

impl ExprSetOp {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        // All branches of a set operation should have the same type
        if let Some(first) = self.operands.first() {
            let first_ty = first.infer_ty(schema, args);

            // Debug assertion to check that every operand has the same type
            #[cfg(debug_assertions)]
            {
                for operand in &self.operands[1..] {
                    debug_assert_eq!(
                        operand.infer_ty(schema, args),
                        first_ty,
                        "All operands in a set operation should have the same type"
                    );
                }
            }

            first_ty
        } else {
            Type::Unknown
        }
    }
}

impl ExprSet {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        match self {
            ExprSet::Select(select) => select.infer_ty(schema, args),
            ExprSet::SetOp(set_op) => set_op.infer_ty(schema, args),
            ExprSet::Values(values) => values.infer_ty(schema, args),
            ExprSet::Update(update) => update.infer_ty(schema, args),
        }
    }
}

impl Values {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        if self.rows.is_empty() {
            return Type::Unknown;
        }

        // Infer from the first row
        let first_row_ty = self.rows[0].infer_ty(schema, args);

        // Debug assertion to check that every row has the same type
        #[cfg(debug_assertions)]
        {
            for row in &self.rows[1..] {
                debug_assert_eq!(
                    row.infer_ty(schema, args),
                    first_row_ty,
                    "All rows in VALUES should have the same type"
                );
            }
        }

        Type::List(Box::new(first_row_ty))
    }
}

impl Insert {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        match &self.returning {
            Some(returning) => {
                match returning {
                    Returning::Star => {
                        // Return all fields from the target being inserted into
                        match &self.target {
                            InsertTarget::Model(model_id) => {
                                let model = schema.app.model(*model_id);
                                let field_types: Vec<Type> =
                                    model.fields.iter().map(|f| f.expr_ty().clone()).collect();
                                Type::Record(field_types)
                            }
                            InsertTarget::Scope(query) => {
                                // For scope targets, infer from the underlying query
                                query.infer_ty(schema, args)
                            }
                            InsertTarget::Table(_) => {
                                // For table targets, we'd need table schema info
                                Type::Unknown
                            }
                        }
                    }
                    Returning::Expr(expr) => expr.infer_ty(schema, args),
                    Returning::Changed => Type::I64, // Returns count of changed rows
                }
            }
            None => Type::Null, // INSERT without RETURNING returns nothing
        }
    }
}

impl Update {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        match &self.returning {
            Some(returning) => {
                match returning {
                    Returning::Star => {
                        // Return all fields from the target being updated
                        match &self.target {
                            UpdateTarget::Model(model_id) => {
                                let model = schema.app.model(*model_id);
                                let field_types: Vec<Type> =
                                    model.fields.iter().map(|f| f.expr_ty().clone()).collect();
                                Type::Record(field_types)
                            }
                            UpdateTarget::Query(query) => {
                                // For query targets, infer from the underlying query
                                query.infer_ty(schema, args)
                            }
                            UpdateTarget::Table(_) => {
                                // For table targets, we'd need table schema info
                                Type::Unknown
                            }
                        }
                    }
                    Returning::Expr(expr) => expr.infer_ty(schema, args),
                    Returning::Changed => Type::I64, // Returns count of changed rows
                }
            }
            None => Type::Null, // UPDATE without RETURNING returns nothing
        }
    }
}

impl Delete {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        match &self.returning {
            Some(returning) => {
                match returning {
                    Returning::Star => {
                        // Return all fields from the source being deleted from
                        match &self.from {
                            Source::Model(source_model) => {
                                let model = schema.app.model(source_model.model);
                                let field_types: Vec<Type> =
                                    model.fields.iter().map(|f| f.expr_ty().clone()).collect();
                                Type::Record(field_types)
                            }
                            Source::Table(_) => {
                                // For table sources, we'd need table schema info
                                Type::Unknown
                            }
                        }
                    }
                    Returning::Expr(expr) => expr.infer_ty(schema, args),
                    Returning::Changed => Type::I64, // Returns count of changed rows
                }
            }
            None => Type::Null, // DELETE without RETURNING returns nothing
        }
    }
}

impl Expr {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        match self {
            // Boolean expressions
            Expr::And(_)
            | Expr::Or(_)
            | Expr::BinaryOp(_)
            | Expr::IsNull(_)
            | Expr::Pattern(_)
            | Expr::InList(_)
            | Expr::InSubquery(_) => Type::Bool,

            // Argument reference
            Expr::Arg(e) => {
                assert!(
                    e.position < args.len(),
                    "Argument position {} is out of bounds (args length: {})",
                    e.position,
                    args.len()
                );
                args[e.position].clone()
            }

            // Schema-dependent references
            Expr::Column(e) => {
                let column_id = e
                    .try_to_column_id()
                    .expect("Column expression must reference a valid column");
                schema.db.column(column_id).ty.clone()
            }

            Expr::Reference(ref_expr) => match ref_expr {
                ExprReference::Field { model, index } => {
                    let field_id = FieldId {
                        model: *model,
                        index: *index,
                    };
                    schema.app.field(field_id).expr_ty().clone()
                }
                ExprReference::Cte { .. } => {
                    todo!("Handle CTE references")
                }
            },

            Expr::Key(e) => {
                let model = schema.app.model(e.model);
                if model.primary_key.fields.len() == 1 {
                    // Single field primary key
                    let field = schema.app.field(model.primary_key.fields[0]);
                    field.expr_ty().clone()
                } else {
                    // Composite primary key - return record of field types
                    let field_types: Vec<Type> = model
                        .primary_key
                        .fields
                        .iter()
                        .map(|field_id| schema.app.field(*field_id).expr_ty().clone())
                        .collect();
                    Type::Record(field_types)
                }
            }

            // Type-preserving operations
            Expr::Cast(e) => e.ty.clone(),

            // Collection operations
            Expr::Map(e) => e.infer_ty(schema, args),
            Expr::List(e) => e.infer_ty(schema, args),

            // Structure operations
            Expr::Project(e) => e.infer_ty(schema, args),
            Expr::Record(e) => e.infer_ty(schema, args),

            // Functions
            Expr::Func(e) => e.infer_ty(schema, args),

            // Concatenation
            Expr::Concat(e) => e.infer_ty(schema, args),
            Expr::ConcatStr(_) => Type::String,

            // Subqueries and statements
            Expr::Stmt(e) => e.stmt.infer_ty(schema, args),

            // Enums
            Expr::Enum(e) => e.infer_ty(schema, args),
            Expr::Type(_) => panic!("Type references should not be reached during type inference"),

            // Values
            Expr::Value(v) => v.infer_ty(schema, args),

            // Special cases
            Expr::DecodeEnum(_, ty, _) => ty.clone(),
        }
    }
}

impl ExprMap {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        let base_ty = self.base.infer_ty(schema, args);

        // Extract the element type from the base
        let element_ty = match &base_ty {
            Type::List(inner) => *inner.clone(),
            _ => panic!(
                "Map operation requires a List base type, got: {:?}",
                base_ty
            ),
        };

        // Infer the mapped type by treating the element as arg[0]
        let map_args = vec![element_ty];
        let mapped_ty = self.map.infer_ty(schema, &map_args);

        Type::List(Box::new(mapped_ty))
    }
}

impl ExprList {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        if self.items.is_empty() {
            return Type::Unknown;
        }

        // Infer from the first item
        let item_ty = self.items[0].infer_ty(schema, args);

        // Debug assertion to check that all items have the same type
        #[cfg(debug_assertions)]
        {
            for item in &self.items[1..] {
                debug_assert_eq!(
                    item.infer_ty(schema, args),
                    item_ty,
                    "All items in a list should have the same type"
                );
            }
        }

        Type::List(Box::new(item_ty))
    }
}

impl ExprProject {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        let mut base_ty = self.base.infer_ty(schema, args);

        // Navigate through the projection path
        for step in self.projection.iter() {
            match &base_ty {
                Type::Record(fields) => {
                    if *step < fields.len() {
                        base_ty = fields[*step].clone();
                    } else {
                        panic!(
                            "Projection index {} out of bounds for record with {} fields",
                            step,
                            fields.len()
                        );
                    }
                }
                _ => panic!("Cannot project into non-record type: {:?}", base_ty),
            }
        }

        base_ty
    }
}

impl ExprRecord {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        let field_types: Vec<Type> = self
            .fields
            .iter()
            .map(|field| field.infer_ty(schema, args))
            .collect();
        Type::Record(field_types)
    }
}

impl ExprFunc {
    pub fn infer_ty(&self, _schema: &Schema, _args: &[Type]) -> Type {
        match self {
            ExprFunc::Count(_) => Type::I64,
        }
    }
}

impl ExprConcat {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        if self.exprs.is_empty() {
            return Type::Unknown;
        }

        let first_ty = self.exprs[0].infer_ty(schema, args);

        // Debug assertion to check that all concatenated expressions have the same type
        #[cfg(debug_assertions)]
        {
            for expr in &self.exprs[1..] {
                debug_assert_eq!(
                    expr.infer_ty(schema, args),
                    first_ty,
                    "All expressions in concatenation should have the same type"
                );
            }
        }

        first_ty
    }
}

impl ExprEnum {
    pub fn infer_ty(&self, _schema: &Schema, _args: &[Type]) -> Type {
        todo!("Need to get the actual enum variant from schema")
    }
}

impl Value {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        match self {
            Value::Bool(_) => Type::Bool,
            Value::Enum(_e) => {
                todo!("Need to get the actual enum variant from schema")
            }
            Value::I8(_) => Type::I8,
            Value::I16(_) => Type::I16,
            Value::I32(_) => Type::I32,
            Value::I64(_) => Type::I64,
            Value::U8(_) => Type::U8,
            Value::U16(_) => Type::U16,
            Value::U32(_) => Type::U32,
            Value::U64(_) => Type::U64,
            Value::Id(id) => Type::Id(id.model_id()),
            Value::SparseRecord(r) => Type::SparseRecord(r.fields.clone()),
            Value::Null => Type::Null,
            Value::Record(r) => r.infer_ty(schema, args),
            Value::List(items) => {
                if items.is_empty() {
                    Type::Unknown
                } else {
                    let item_ty = items[0].infer_ty(schema, args);
                    Type::List(Box::new(item_ty))
                }
            }
            Value::String(_) => Type::String,
        }
    }
}

impl ValueRecord {
    pub fn infer_ty(&self, schema: &Schema, args: &[Type]) -> Type {
        let field_types: Vec<Type> = self
            .fields
            .iter()
            .map(|field| field.infer_ty(schema, args))
            .collect();
        Type::Record(field_types)
    }
}
