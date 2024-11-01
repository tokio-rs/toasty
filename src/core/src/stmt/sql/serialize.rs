use super::*;

use crate::stmt::Statement as DataStatement;

use std::fmt::{self, Write};

pub trait Params<'stmt> {
    fn push(&mut self, param: &stmt::Value<'stmt>);
}

/// Serialize a statement to a SQL string
pub struct Serializer<'a> {
    schema: &'a Schema,
}

struct Formatter<'a, T> {
    /// Where to write the SQL string
    dst: &'a mut String,

    /// The schema that the query references
    schema: &'a Schema,

    /// Query paramaters (referenced by placeholders) are stored here.
    params: &'a mut T,
}

impl<'stmt> Params<'stmt> for Vec<stmt::Value<'stmt>> {
    fn push(&mut self, value: &stmt::Value<'stmt>) {
        self.push(value.clone());
    }
}

impl<'a> Serializer<'a> {
    pub fn new(schema: &'a Schema) -> Serializer<'a> {
        Serializer { schema }
    }

    pub fn serialize_stmt<'stmt>(
        &self,
        stmt: &DataStatement<'stmt>,
        params: &mut impl Params<'stmt>,
    ) -> String {
        let mut ret = String::new();

        let mut fmt = Formatter {
            dst: &mut ret,
            schema: self.schema,
            params,
        };

        fmt.statement(stmt).unwrap();
        ret
    }

    pub fn serialize_sql_stmt<'stmt>(
        &self,
        stmt: &Statement<'stmt>,
        params: &mut impl Params<'stmt>,
    ) -> String {
        let mut ret = String::new();

        let mut fmt = Formatter {
            dst: &mut ret,
            schema: self.schema,
            params,
        };

        fmt.sql_statement(stmt).unwrap();
        ret
    }
}

impl<'a, 'stmt, T: Params<'stmt>> Formatter<'a, T> {
    fn statement(&mut self, statement: &DataStatement<'stmt>) -> fmt::Result {
        match statement {
            /*
            Statement::CreateIndex(stmt) => self.create_index(stmt)?,
            Statement::CreateTable(stmt) => self.create_table(stmt)?,
            */
            DataStatement::Delete(stmt) => self.delete(stmt)?,
            DataStatement::Insert(stmt) => self.insert(stmt)?,
            DataStatement::Query(stmt) => self.query(stmt)?,
            DataStatement::Update(stmt) => self.update(stmt)?,
            _ => todo!("stmt = {statement:#?}"),
        }

        write!(self.dst, ";")?;

        Ok(())
    }

    fn sql_statement(&mut self, statement: &Statement<'stmt>) -> fmt::Result {
        match statement {
            Statement::CreateIndex(stmt) => self.create_index(stmt)?,
            Statement::CreateTable(stmt) => self.create_table(stmt)?,
            Statement::Delete(stmt) => self.delete(stmt)?,
            Statement::Insert(stmt) => self.insert(stmt)?,
            Statement::Query(stmt) => self.query(stmt)?,
            Statement::Update(stmt) => self.update(stmt)?,
            _ => todo!("stmt = {statement:#?}"),
        }

        write!(self.dst, ";")?;

        Ok(())
    }

    fn create_index(&mut self, stmt: &CreateIndex<'stmt>) -> fmt::Result {
        write!(
            self.dst,
            "CREATE {}INDEX ",
            if stmt.unique { "UNIQUE " } else { "" }
        )?;

        self.name(&stmt.name)?;
        write!(self.dst, " ON ")?;

        let table = self.schema.table(stmt.on);
        self.ident_str(&table.name)?;

        write!(self.dst, " (")?;

        let mut s = "";
        for index_column in &stmt.columns {
            self.expr(&index_column.expr)?;

            if let Some(desc) = index_column.order {
                write!(
                    self.dst,
                    " {}",
                    match desc {
                        Direction::Asc => "ASC",
                        Direction::Desc => "DESC",
                    }
                )?;
            }

            write!(self.dst, "{s}")?;
            s = ", ";
        }

        write!(self.dst, ")")?;

        Ok(())
    }

    fn create_table(&mut self, stmt: &CreateTable<'stmt>) -> fmt::Result {
        write!(self.dst, "CREATE TABLE ")?;
        self.name(&stmt.name)?;

        write!(self.dst, " (")?;

        for column_def in &stmt.columns {
            self.column_def(column_def)?;
            write!(self.dst, ", ")?;
        }

        write!(self.dst, "PRIMARY KEY ")?;

        self.expr(stmt.primary_key.as_deref().unwrap())?;

        write!(self.dst, ")")?;

        Ok(())
    }

    fn column_def(&mut self, stmt: &ColumnDef) -> fmt::Result {
        self.ident(&stmt.name)?;
        write!(self.dst, " ")?;
        self.ty(&stmt.ty)?;
        Ok(())
    }

    fn query(&mut self, query: &Query<'stmt>) -> fmt::Result {
        match &*query.body {
            ExprSet::Select(select) => self.select(select),
            ExprSet::Values(values) => self.values(values),
            _ => todo!("query={query:#?}"),
        }
    }

    fn delete(&mut self, delete: &Delete<'stmt>) -> fmt::Result {
        write!(self.dst, "DELETE FROM ")?;

        assert!(delete.returning.is_none());

        for table_with_join in delete.from.as_table_with_joins() {
            let table = self.schema.table(table_with_join.table);
            write!(self.dst, "\"{}\"", table.name)?;
        }

        write!(self.dst, " WHERE ")?;

        self.expr(&delete.filter)?;

        Ok(())
    }

    fn insert(&mut self, stmt: &Insert<'stmt>) -> fmt::Result {
        let InsertTarget::Table(insert_target) = &stmt.target else {
            todo!()
        };

        write!(
            self.dst,
            "INSERT INTO \"{}\" (",
            self.schema.table(insert_target).name
        )?;

        let mut s = "";
        for column_id in &insert_target.columns {
            write!(self.dst, "{}\"{}\"", s, self.schema.column(column_id).name)?;
            s = ", ";
        }

        write!(self.dst, ") ")?;

        self.query(&stmt.source)?;

        if let Some(returning) = &stmt.returning {
            let Returning::Expr(returning) = returning else {
                todo!("returning={returning:#?}")
            };
            write!(self.dst, " RETURNING ")?;
            self.expr_as_list(returning)?;
        }

        Ok(())
    }

    fn update(&mut self, update: &Update<'stmt>) -> fmt::Result {
        let table = self.schema.table(update.target.as_table().table);

        write!(self.dst, "UPDATE \"{}\" SET", table.name)?;

        for (index, expr) in update.assignments.iter() {
            let column = &table.columns[index];
            write!(self.dst, " \"{}\" = ", column.name)?;

            self.expr(expr)?;
        }

        if update.filter.is_some() || update.condition.is_some() {
            write!(self.dst, " WHERE ")?;
        }

        if let Some(filter) = &update.filter {
            self.expr(filter)?;
        }

        if let Some(condition) = &update.condition {
            if update.filter.is_some() {
                write!(self.dst, " AND ")?;
            }

            self.expr(condition)?;

            if update.returning.is_none() {
                write!(self.dst, " RETURNING true")?;
            }
        }

        if let Some(returning) = &update.returning {
            let Returning::Expr(returning) = returning else {
                todo!("update={update:#?}")
            };
            write!(self.dst, " RETURNING ")?;
            // self.expr_list(returning)?;
            todo!("returning={returning:#?}");
        }

        Ok(())
    }

    fn select(&mut self, select: &Select<'stmt>) -> fmt::Result {
        /*
        write!(self.dst, "SELECT ")?;

        self.expr_list(&select.project)?;

        write!(self.dst, " FROM ")?;

        for table_with_join in &select.from {
            let table = self.schema.table(table_with_join.table);
            write!(self.dst, "\"{}\"", table.name)?;
        }

        write!(self.dst, " WHERE ")?;

        let selection = select.selection.as_ref().unwrap();
        self.expr(selection)?;

        Ok(())
        */
        todo!("stmt={select:#?}");
    }

    fn values(&mut self, values: &Values<'stmt>) -> fmt::Result {
        let mut s = "VALUES";
        for record in &values.rows {
            write!(self.dst, "{s} (")?;
            self.expr_as_list(record)?;
            write!(self.dst, ")")?;
            s = ",";
        }

        Ok(())
    }

    fn expr_list(&mut self, exprs: &[Expr<'stmt>]) -> fmt::Result {
        let mut s = "";

        for expr in exprs {
            write!(self.dst, "{s}")?;
            self.expr(expr)?;
            s = ", ";
        }

        Ok(())
    }

    fn expr_as_list(&mut self, expr: &Expr<'stmt>) -> fmt::Result {
        let Expr::Record(expr_list) = expr else {
            todo!()
        };
        self.expr_list(expr_list)?;
        Ok(())
    }

    fn expr(&mut self, expr: &Expr<'stmt>) -> fmt::Result {
        match expr {
            Expr::And(ExprAnd { operands }) => {
                let mut s = "";

                for expr in operands {
                    write!(self.dst, "{s}")?;
                    self.expr(expr)?;
                    s = " AND ";
                }
            }
            Expr::BinaryOp(ExprBinaryOp { lhs, op, rhs }) => {
                self.expr(&*lhs)?;
                write!(self.dst, " ")?;
                self.binary_op(op)?;
                write!(self.dst, " ")?;
                self.expr(&rhs)?;
            }
            Expr::Column(expr) => {
                // TODO: at some point we need to conditionally scope the column
                // name.
                let column = self.schema.column(expr.column);
                self.ident_str(&column.name)?;
            }
            Expr::InList(ExprInList { expr, list }) => {
                self.expr(expr)?;
                write!(self.dst, " IN (")?;

                let mut s = "";

                /*
                match list {
                    ExprList::Expr(exprs) => {
                        for e in exprs {
                            write!(self.dst, "{s}")?;
                            self.expr(e)?;
                            s = ", ";
                        }
                    }
                    ExprList::Value(values) => {
                        for v in values {
                            write!(self.dst, "{s}")?;
                            self.value(v)?;
                            s = ", ";
                        }
                    }
                    ExprList::Placeholder(_) => {
                        todo!("PLACEHOLDER");
                    }
                }
                */
                todo!("expr={expr:#?}");

                write!(self.dst, ")")?;
            }
            Expr::InSubquery(ExprInSubquery { expr, query }) => {
                self.expr(expr)?;
                write!(self.dst, " IN (")?;

                self.query(query)?;
                write!(self.dst, ")")?;
            }
            Expr::Or(ExprOr { operands }) => {
                let mut s = "";

                for expr in operands {
                    write!(self.dst, "{s}")?;
                    self.expr(expr)?;
                    s = " OR ";
                }
            }
            Expr::Record(expr_record) => {
                write!(self.dst, "(")?;

                let mut s = "";
                for expr in expr_record {
                    write!(self.dst, "{s}")?;
                    self.expr(expr)?;
                    s = ", ";
                }

                write!(self.dst, ")")?;
            }
            Expr::Value(value) => self.value(value)?,
            /*
            Expr::BeginsWith(ExprBeginsWith { expr, pattern }) => {
                let str = pattern.as_value().expect_string();
                let pattern = format!("{str}%");
                self.expr(expr)?;
                write!(self.dst, " LIKE ")?;
                self.expr(&Expr::Value(pattern.into()))?;
            }
            Expr::Like(ExprLike { expr, pattern }) => {
                self.expr(expr)?;
                write!(self.dst, " LIKE ")?;
                self.expr(pattern)?;
            }
            Expr::IsNotNull(expr) => {
                self.expr(expr)?;
                write!(self.dst, " IS NOT NULL")?;
            }
            */
            _ => todo!("expr = {:#?}", expr),
        }

        Ok(())
    }

    fn binary_op(&mut self, binary_op: &BinaryOp) -> fmt::Result {
        write!(
            self.dst,
            "{}",
            match binary_op {
                BinaryOp::Eq => "=",
                BinaryOp::Gt => ">",
                BinaryOp::Ge => ">=",
                BinaryOp::Lt => "<",
                BinaryOp::Le => "<=",
                BinaryOp::Ne => "<>",
                _ => todo!(),
            }
        )
    }

    fn value(&mut self, value: &Value<'stmt>) -> fmt::Result {
        self.params.push(value);
        write!(self.dst, "?")?;
        Ok(())
    }

    fn ty(&mut self, stmt: &Type) -> fmt::Result {
        write!(
            self.dst,
            "{}",
            match stmt {
                Type::Boolean => "BOOLEAN",
                Type::Integer => "INTEGER",
                Type::Text => "TEXT",
            }
        )
    }

    fn name(&mut self, name: &Name) -> fmt::Result {
        let mut s = "";
        for ident in &name.0 {
            self.ident(ident)?;
            write!(self.dst, "{s}")?;
            s = ".";
        }

        Ok(())
    }

    fn ident(&mut self, ident: &Ident) -> fmt::Result {
        self.ident_str(&ident.0)
    }

    fn ident_str(&mut self, ident: &str) -> fmt::Result {
        write!(self.dst, "\"{ident}\"")?;
        Ok(())
    }
}
