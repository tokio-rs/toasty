use super::*;

/// Prepared query
///
/// For paths, `self` is 0, arguments come after that.
#[derive(Debug, PartialEq)]
pub struct Query {
    /// Uniquely identifies a query
    pub id: QueryId,

    /// Query name, used when scoped by the target (ret).
    pub name: String,

    /// Full query name, used in a global namespace.
    pub full_name: String,

    /// When true, the query is a "find_many_by"
    pub many: bool,

    /// Query arguments
    pub args: Vec<Arg>,

    /// Return type
    pub ret: ModelId,

    /// Implementation
    pub stmt: stmt::Query<'static>,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct QueryId(pub usize);

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct FindByArg {
    /// The field being used as the argument
    field_id: FieldId,

    /// Whether or not the argument is passed in as a model or a foreign key.
    /// This only applies when the field is a BelongsTo type.
    ty: FindByArgType,
}

#[derive(Debug)]
pub(crate) struct FindByBuilder<'a> {
    /// The ID to assign the query
    id: QueryId,

    /// Model being queried
    model: &'a Model,

    /// Whether arguments should default to being queried by foreign key or not.
    by_fk: bool,

    /// When `true`, this is a `find_many_by` query
    many: bool,

    /// Query arguments. The first is the field being queried. The `bool` is
    /// whether or not to take the argument as a model reference or as foreign
    /// key components.
    ///
    /// This is exposed so the caller can check if there already is a generated
    /// query for the arg combo.
    pub(crate) args: Vec<FindByArg>,
}

/// How the arg is passed in
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum FindByArgType {
    /// The argument is passed in using the same representation as the field
    /// type. Primitives are primitives, BelongsTo is a model reference.
    Expr,

    /// The field is a BelongsTo relation and is passed in using the foreign key
    /// fields.
    ForeignKey,

    /// The argument is passed in as a query.
    Query,
}

impl Query {
    pub fn apply<'stmt>(&self, input: impl stmt::substitute::Input<'stmt>) -> stmt::Query<'stmt> {
        let mut stmt: stmt::Query<'stmt> = self.stmt.clone();

        stmt.substitute(input);
        stmt
    }

    pub(crate) fn find_by<'a>(id: QueryId, model: &'a Model, by_fk: bool) -> FindByBuilder<'a> {
        FindByBuilder {
            id,
            model,
            by_fk,
            many: false,
            args: vec![],
        }
    }
}

impl<'a> FindByBuilder<'a> {
    pub(crate) fn many(&mut self) -> &mut Self {
        self.many = true;
        self
    }

    pub(crate) fn field(&mut self, field: impl Into<FieldId>) -> &mut Self {
        let field_id = field.into();

        let is_belongs_to = self.model.field(field_id).ty.is_belongs_to();

        self.args.push(FindByArg {
            field_id,
            ty: if self.by_fk && is_belongs_to {
                FindByArgType::ForeignKey
            } else {
                FindByArgType::Expr
            },
        });

        self
    }

    /// A scope argument. These are passed in as a query
    pub(crate) fn scope(&mut self, field: impl Into<FieldId>) -> &mut Self {
        let field_id = field.into();

        assert!(self.model.field(field_id).ty.is_belongs_to());

        self.args.push(FindByArg {
            field_id,
            ty: FindByArgType::Query,
        });

        self
    }

    pub(crate) fn build(&mut self) -> Query {
        Query {
            id: self.id,
            many: self.many,
            name: self.query_name(false),
            full_name: self.query_name(true),
            args: self.query_args(),
            ret: self.model.id,
            stmt: if self.many {
                self.many_query_body()
            } else {
                self.query_body()
            },
        }
    }

    fn query_name(&self, full: bool) -> String {
        let base = if self.many { "find_many" } else { "find" };

        let mut query_name = if full {
            format!("{}_{}_by", base, self.model.name.snake_case())
        } else {
            format!("{base}_by")
        };

        let mut parts = vec![];

        for find_by_arg in &self.args {
            let field = self.model.field(find_by_arg.field_id);
            if find_by_arg.ty.is_foreign_key() {
                let rel = field.ty.expect_belongs_to();

                for fk_field in &rel.foreign_key.fields {
                    parts.push(&self.model.field(fk_field.source).name);
                }
            } else {
                parts.push(&field.name);
            }
        }

        for (i, part) in parts.into_iter().enumerate() {
            query_name.push('_');

            if i > 0 {
                query_name.push_str("and_");
            }

            query_name.push_str(part);
        }

        query_name
    }

    fn query_args(&self) -> Vec<Arg> {
        let mut args = vec![];

        for find_by_arg in &self.args {
            let field = self.model.field(find_by_arg.field_id);

            if find_by_arg.ty.is_foreign_key() {
                let rel = field.ty.expect_belongs_to();

                for fk_field in &rel.foreign_key.fields {
                    args.push(Arg {
                        name: self.model.field(fk_field.source).name.clone(),
                        ty: self
                            .model
                            .field(fk_field.source)
                            .ty
                            .expect_primitive()
                            .ty
                            .clone(),
                    });
                }
            } else {
                args.push(Arg {
                    name: field.name.clone(),
                    ty: match &field.ty {
                        FieldTy::Primitive(primitive) => primitive.ty.clone(),
                        FieldTy::BelongsTo(_) => stmt::Type::ForeignKey(field.id),
                        _ => todo!("field={:#?}", field),
                    },
                    /*
                    ty: match &field.ty {
                        FieldTy::Primitive(primitive) => primitive.ty.clone(),
                        FieldTy::BelongsTo(rel) => rel.expr_ty.clone(),
                        _ => todo!("field={:#?}", field),
                    },
                    */
                });
            }
        }

        args
    }

    fn query_body(&self) -> stmt::Query<'static> {
        let mut exprs = vec![];

        for find_by_arg in &self.args {
            let field = self.model.field(find_by_arg.field_id);
            // let lhs = stmt::Path::from(field.id);

            match find_by_arg.ty {
                FindByArgType::Expr => {
                    let arg = stmt::Expr::arg(exprs.len());
                    exprs.push(stmt::Expr::eq(field, arg));
                }
                FindByArgType::ForeignKey => {
                    let arg = stmt::Expr::arg(exprs.len());
                    exprs.push(stmt::Expr::eq(field, arg));
                }
                FindByArgType::Query => {
                    let rel = field.ty.expect_belongs_to();
                    let arg = stmt::Expr::arg(exprs.len());
                    let query = stmt::Query::filter(rel.target, arg);

                    exprs.push(stmt::Expr::in_subquery(field, query));
                }
            }
        }

        let filter = if exprs.len() == 1 {
            exprs.pop().unwrap()
        } else {
            stmt::ExprAnd::new(exprs).into()
        };

        stmt::Query::filter(self.model.id, filter)
    }

    fn many_query_body(&self) -> stmt::Query<'static> {
        let mut exprs = vec![];
        let mut tys = vec![];

        for find_by_arg in &self.args {
            let field = self.model.field(find_by_arg.field_id);

            match find_by_arg.ty {
                FindByArgType::Expr => {
                    exprs.push(stmt::Expr::project(field));
                    tys.push(field.expr_ty().clone());
                }
                FindByArgType::ForeignKey => {
                    let rel = field.ty.expect_belongs_to();

                    match &rel.foreign_key.fields[..] {
                        [] => panic!("foreign keys cannot be empty"),
                        [fk_field] => {
                            exprs.push(stmt::Expr::project([field.id, fk_field.target]));
                            tys.push(
                                self.model
                                    .field(fk_field.source)
                                    .ty
                                    .expect_primitive()
                                    .ty
                                    .clone(),
                            );
                        }
                        _ => todo!("composite FKs"),
                    }
                }
                _ => todo!("{:#?}", find_by_arg),
            }
        }

        let filter = if exprs.len() == 1 {
            let lhs = exprs.pop().unwrap();

            stmt::Expr::in_list(lhs, stmt::Expr::arg(0))
        } else {
            stmt::Expr::in_list(stmt::ExprRecord::from_vec(exprs), stmt::Expr::arg(0))
        };

        stmt::Query::filter(self.model.id, filter)
    }
}

impl FindByArgType {
    fn is_foreign_key(self) -> bool {
        matches!(self, FindByArgType::ForeignKey)
    }
}

impl QueryId {
    pub(crate) const fn placeholder() -> QueryId {
        QueryId(usize::MAX)
    }
}

impl Into<QueryId> for &QueryId {
    fn into(self) -> QueryId {
        *self
    }
}
