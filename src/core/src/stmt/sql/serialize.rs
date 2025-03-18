use super::*;

use crate::schema::db;

use crate::stmt::Statement as DataStatement;

use std::{
    fmt::{self, Display, Write},
    ops::Deref,
};

pub trait Params {
    fn push(&mut self, param: &stmt::Value);
}

/// Serialize a statement to a SQL string
pub struct Serializer<'a> {
    schema: &'a db::Schema,

    /// True if table names should be quoted
    quoted_table_names: bool,

    /// True if column names should be quotes
    quoted_column_names: bool,

    /// True if update statements can be used in CTE expressions.
    update_in_cte: bool,
}

struct MaybeQuote<'a> {
    value: &'a str,
    quote: bool,
}

impl<'a> MaybeQuote<'a> {
    fn new(value: &'a str, quote: bool) -> Self {
        Self { value, quote }
    }
}
impl Display for MaybeQuote<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.quote {
            write!(f, "\"{}\"", self.value)
        } else {
            write!(f, "{}", self.value)
        }
    }
}

struct Formatter<'a, T> {
    /// Where to write the SQL string
    dst: &'a mut String,

    /// The schema that the query references
    schema: &'a db::Schema,

    /// Serializer (which has configuration)
    serializer: &'a Serializer<'a>,

    /// Query paramaters (referenced by placeholders) are stored here.
    params: &'a mut T,
}

impl Params for Vec<stmt::Value> {
    fn push(&mut self, value: &stmt::Value) {
        self.push(value.clone());
    }
}

/// A serialization result.
pub struct SerializeResult(String);

impl SerializeResult {
    /// Creates a new serialization result.
    pub fn new(value: impl Into<String>) -> SerializeResult {
        Self(value.into())
    }

    /// Returns a reference to the inner string.
    pub fn inner(&self) -> &str {
        self.0.as_str()
    }

    /// Consumes `self` and returns the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Converts all question mark args (i.e., `?`) to numbered args (e.g., `$1`).
    pub fn into_numbered_args(self) -> Self {
        let mut result = String::with_capacity(self.0.len());
        let mut n = 1;

        for c in self.0.chars() {
            if c == '?' {
                result.push_str(&format!("${n}"));
                n += 1;
            } else {
                result.push(c);
            }
        }

        Self(result)
    }
}

impl Deref for SerializeResult {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SerializeResult {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl<'a> Serializer<'a> {
    pub fn new(schema: &'a db::Schema) -> Serializer<'a> {
        Serializer {
            schema,
            quoted_table_names: true,
            quoted_column_names: true,
            update_in_cte: false,
        }
    }
    pub fn with_quoted_table_names(mut self, enabled: bool) -> Self {
        self.quoted_table_names = enabled;
        self
    }
    pub fn with_quoted_column_names(mut self, enabled: bool) -> Self {
        self.quoted_column_names = enabled;
        self
    }

    pub fn with_update_in_cte(mut self, enabled: bool) -> Self {
        self.update_in_cte = enabled;
        self
    }

    pub fn serialize_stmt(
        &self,
        stmt: &DataStatement,
        params: &mut impl Params,
    ) -> SerializeResult {
        let mut ret = String::new();

        let mut fmt = Formatter {
            dst: &mut ret,
            schema: self.schema,
            params,
            serializer: self,
        };

        fmt.statement(stmt).unwrap();
        SerializeResult::new(ret)
    }

    pub fn serialize_sql_stmt(
        &self,
        stmt: &Statement,
        params: &mut impl Params,
    ) -> SerializeResult {
        let mut ret = String::new();

        let mut fmt = Formatter {
            dst: &mut ret,
            schema: self.schema,
            params,
            serializer: self,
        };

        fmt.sql_statement(stmt).unwrap();
        SerializeResult::new(ret)
    }
}

impl<T: Params> Formatter<'_, T> {
    fn statement(&mut self, statement: &DataStatement) -> fmt::Result {
        match statement {
            DataStatement::Delete(stmt) => self.delete(stmt)?,
            DataStatement::Insert(stmt) => self.insert(stmt)?,
            DataStatement::Query(stmt) => self.query(stmt)?,
            DataStatement::Update(stmt) => self.update(stmt)?,
        }

        write!(self.dst, ";")?;

        Ok(())
    }

    fn sql_statement(&mut self, statement: &Statement) -> fmt::Result {
        match statement {
            Statement::CreateIndex(stmt) => self.create_index(stmt)?,
            Statement::CreateTable(stmt) => self.create_table(stmt)?,
            Statement::DropTable(stmt) => self.drop_table(stmt)?,
            Statement::Delete(stmt) => self.delete(stmt)?,
            Statement::Insert(stmt) => self.insert(stmt)?,
            Statement::Query(stmt) => self.query(stmt)?,
            Statement::Update(stmt) => self.update(stmt)?,
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
        self.ident_str(&table.name, self.serializer.quoted_table_names)?;

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

    fn drop_table(&mut self, stmt: &DropTable) -> fmt::Result {
        write!(self.dst, "DROP TABLE ")?;

        if stmt.if_exists {
            write!(self.dst, "IF EXISTS")?;
        }

        self.name(&stmt.name)?;

        Ok(())
    }

    fn column_def(&mut self, stmt: &ColumnDef) -> fmt::Result {
        self.ident(&stmt.name, self.serializer.quoted_column_names)?;
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
            write!(
                self.dst,
                "{}",
                MaybeQuote::new(&table.name, self.serializer.quoted_table_names)
            )?;
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
            "INSERT INTO {} (",
            MaybeQuote::new(
                &self.schema.table(insert_target).name,
                self.serializer.quoted_table_names
            )
        )?;

        let mut s = "";
        for column_id in &insert_target.columns {
            write!(
                self.dst,
                "{}{}",
                s,
                MaybeQuote::new(
                    &self.schema.column(*column_id).name,
                    self.serializer.quoted_column_names
                )
            )?;
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

        // If there is an update condition, serialize the statement as a CTE
        if let Some(condition) = &update.condition {
            if !self.serializer.update_in_cte {
                panic!("Update conditions are not supported");
            }

            let table_name = MaybeQuote::new(&table.name, self.serializer.quoted_table_names);
            write!(
                self.dst,
                "WITH found AS (SELECT count(*) as total, count(*) FILTER (WHERE "
            )?;
            self.expr(condition)?;
            write!(self.dst, ") AS condition_matched FROM {}", table_name)?;

            if let Some(filter) = &update.filter {
                write!(self.dst, " WHERE ")?;
                self.expr(filter)?;
            }

            write!(self.dst, "), updated AS (")?;
        }

        write!(
            self.dst,
            "UPDATE {} SET",
            MaybeQuote::new(&table.name, self.serializer.quoted_table_names)
        )?;

        for (index, assignment) in update.assignments.iter() {
            let column = &table.columns[index];
            write!(
                self.dst,
                " {} = ",
                MaybeQuote::new(&column.name, self.serializer.quoted_column_names)
            )?;

            self.expr(&assignment.expr)?;
        }

        if update.filter.is_some() || update.condition.is_some() {
            write!(self.dst, " WHERE ")?;
        }

        if let Some(filter) = &update.filter {
            self.expr(filter)?;

            if update.condition.is_some() {
                write!(self.dst, " AND ")?;
            }
        }

        if update.condition.is_some() {
            write!(self.dst, "(SELECT total = condition_matched FROM found)")?;
        }

        if let Some(returning) = &update.returning {
            let Returning::Expr(returning) = returning else {
                todo!("update={update:#?}")
            };
            write!(self.dst, " RETURNING ")?;
            self.expr_as_list(returning)?;
        }

        if update.condition.is_some() {
            write!(self.dst, ") SELECT found.total, found.condition_matched")?;

            if update.returning.is_some() {
                write!(self.dst, ", updated.*")?;
            }

            write!(self.dst, " FROM found")?;

            if update.returning.is_some() {
                write!(self.dst, " LEFT JOIN updated ON TRUE")?;
            }
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
            write!(
                self.dst,
                "{}",
                MaybeQuote::new(&table.name, self.serializer.quoted_table_names)
            )?;
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
            Expr::List(expr) => self.expr_list(&expr.items),
            Expr::Value(Value::Record(expr)) => self.value_list(expr),
            Expr::Value(Value::List(expr)) => self.value_list(expr),
            _ => self.expr(expr),
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
                assert!(!lhs.is_value_null());
                assert!(!rhs.is_value_null());

                self.expr(lhs)?;
                write!(self.dst, " ")?;
                self.binary_op(op)?;
                write!(self.dst, " ")?;
                self.expr(rhs)?;
            }
            Expr::Column(expr) => {
                // TODO: at some point we need to conditionally scope the column
                // name.
                let column = self.schema.column(expr.column);
                self.ident_str(&column.name, self.serializer.quoted_column_names)?;
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
            Expr::IsNull(ExprIsNull { negate, expr }) => {
                let not = if *negate { "NOT " } else { "" };

                self.expr(expr)?;
                write!(self.dst, "IS {}NULL", not)?;
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
        match value {
            stmt::Value::Id(_) => todo!(),
            stmt::Value::Record(record) => {
                write!(self.dst, "(")?;
                self.value_list(record)?;
                write!(self.dst, ")")?;
            }
            _ => {
                self.params.push(value);
                write!(self.dst, "?")?;
            }
        }

        Ok(())
    }

    fn value_list(&mut self, values: &[Value]) -> fmt::Result {
        let mut s = "";

        for value in values {
            write!(self.dst, "{s}")?;
            self.value(value)?;
            s = ", ";
        }

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
            self.ident(ident, self.serializer.quoted_table_names)?;
            write!(self.dst, "{s}")?;
            s = ".";
        }

        Ok(())
    }

    fn ident(&mut self, ident: &Ident, quote: bool) -> fmt::Result {
        self.ident_str(&ident.0, quote)
    }

    fn ident_str(&mut self, ident: &str, quote: bool) -> fmt::Result {
        write!(self.dst, "{}", MaybeQuote::new(ident, quote))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_numbered_args() {
        let result = SerializeResult::new("INSERT INTO table (a, b, c) VALUES (?, ?, ?)");
        assert_eq!(
            result.into_numbered_args().inner(),
            "INSERT INTO table (a, b, c) VALUES ($1, $2, $3)"
        );
    }
}
