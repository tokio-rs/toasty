use super::*;

use crate::stmt::Statement as DataStatement;

use std::fmt::{self, Write};

pub trait Params {
    fn push(&mut self, param: &stmt::Value);
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

impl Params for Vec<stmt::Value> {
    fn push(&mut self, value: &stmt::Value) {
        self.push(value.clone());
    }
}

impl<'a> Serializer<'a> {
    pub fn new(schema: &'a Schema) -> Serializer<'a> {
        Serializer { schema }
    }

    pub fn serialize_stmt(&self, stmt: &DataStatement, params: &mut impl Params) -> String {
        let mut ret = String::new();

        let mut fmt = Formatter {
            dst: &mut ret,
            schema: self.schema,
            params,
        };

        fmt.statement(stmt).unwrap();
        ret
    }

    pub fn serialize_sql_stmt(&self, stmt: &Statement, params: &mut impl Params) -> String {
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

impl<'a, 'stmt, T: Params> Formatter<'a, T> {
    fn statement(&mut self, statement: &DataStatement) -> fmt::Result {
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

    fn sql_statement(&mut self, statement: &Statement) -> fmt::Result {
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

    fn create_index(&mut self, stmt: &CreateIndex) -> fmt::Result {
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

    fn create_table(&mut self, stmt: &CreateTable) -> fmt::Result {
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

    fn query(&mut self, query: &Query) -> fmt::Result {
        match &*query.body {
            ExprSet::Select(select) => self.select(select),
            ExprSet::Values(values) => self.values(values),
            _ => todo!("query={query:#?}"),
        }
    }

    fn delete(&mut self, delete: &Delete) -> fmt::Result {
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

    fn insert(&mut self, stmt: &Insert) -> fmt::Result {
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

    fn update(&mut self, update: &Update) -> fmt::Result {
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
            self.expr_as_list(returning)?;
        }

        Ok(())
    }

    fn select(&mut self, select: &Select) -> fmt::Result {
        write!(self.dst, "SELECT ")?;

        match &select.returning {
            Returning::Expr(returning) => self.expr_as_list(returning)?,
            _ => todo!("select={select:#?}"),
        }

        write!(self.dst, " FROM ")?;

        for table_with_join in select.source.as_table_with_joins() {
            let table = self.schema.table(table_with_join.table);
            write!(self.dst, "\"{}\"", table.name)?;
        }

        write!(self.dst, " WHERE ")?;

        self.expr(&select.filter)?;

        Ok(())
    }

    fn values(&mut self, values: &Values) -> fmt::Result {
        let mut s = "VALUES";
        for record in &values.rows {
            write!(self.dst, "{s} (")?;
            self.expr_as_list(record)?;
            write!(self.dst, ")")?;
            s = ",";
        }

        Ok(())
    }

    fn expr_list(&mut self, exprs: &[Expr]) -> fmt::Result {
        let mut s = "";

        for expr in exprs {
            write!(self.dst, "{s}")?;
            self.expr(expr)?;
            s = ", ";
        }

        Ok(())
    }

    fn expr_as_list(&mut self, expr: &Expr) -> fmt::Result {
        match expr {
            Expr::Record(expr) => self.expr_list(expr),
            Expr::List(expr) => self.expr_list(expr),
            _ => todo!("expr={expr:#?}"),
        }
    }

    fn expr(&mut self, expr: &Expr) -> fmt::Result {
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
                assert!(!lhs.is_null());
                assert!(!rhs.is_null());

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
                self.expr_as_list(list)?;
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
            Expr::Pattern(ExprPattern::BeginsWith(expr)) => {
                let Expr::Value(pattern) = &*expr.pattern else {
                    todo!()
                };
                let pattern = pattern.expect_string();
                let pattern = format!("{pattern}%");
                self.expr(&expr.expr)?;
                write!(self.dst, " LIKE ")?;
                self.expr(&Expr::Value(pattern.into()))?;
            }
            Expr::ConcatStr(ExprConcatStr { exprs }) => {
                write!(self.dst, "concat(")?;
                self.expr_list(exprs)?;
                write!(self.dst, ")")?;
            }
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

    fn value(&mut self, value: &Value) -> fmt::Result {
        assert!(!value.is_id());
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
