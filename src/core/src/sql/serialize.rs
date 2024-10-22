use super::*;

use crate::Schema;

struct Formatter<'a, T> {
    /// Where to write the SQL string
    dst: &'a mut String,

    /// The schema that the query references
    schema: &'a Schema,

    /// Query paramaters (referenced by placeholders) are stored here.
    params: &'a mut T,

    /// Maps aliases to strings. For now, aliases are all tables. Eventually
    /// there will be more options.
    aliases: Vec<TableId>,
}

impl<'stmt> Statement<'stmt> {
    pub fn to_sql_string(&self, schema: &Schema, params: &mut impl Params<'stmt>) -> String {
        let mut ret = String::new();

        let mut fmt = Formatter {
            dst: &mut ret,
            schema,
            params,
            aliases: vec![],
        };

        fmt.build_alias_table(self);
        fmt.statement(self).unwrap();

        ret
    }
}

impl<'a, 'stmt, T: Params<'stmt>> Formatter<'a, T> {
    fn build_alias_table(&mut self, stmt: &Statement<'stmt>) {
        let table_with_join = match stmt {
            Statement::Delete(stmt) => {
                assert_eq!(1, stmt.from.len());
                &stmt.from[0]
            }
            Statement::Insert(stmt) => {
                assert!(stmt.source.is_values());
                return;
            }
            Statement::Query(stmt) => match &*stmt.body {
                ExprSet::Select(select) => {
                    assert_eq!(1, select.from.len());
                    &select.from[0]
                }
                _ => todo!(),
            },
            Statement::Update(stmt) => &stmt.table,
            _ => return,
        };

        assert_eq!(0, table_with_join.alias);
        assert!(self.aliases.is_empty());

        self.aliases.push(table_with_join.table);
    }

    fn statement(&mut self, statement: &Statement<'stmt>) -> fmt::Result {
        match statement {
            Statement::CreateIndex(stmt) => self.create_index(stmt)?,
            Statement::CreateTable(stmt) => self.create_table(stmt)?,
            Statement::Delete(stmt) => self.delete(stmt)?,
            Statement::Insert(stmt) => self.insert(stmt)?,
            Statement::Query(stmt) => self.query(stmt)?,
            Statement::Update(stmt) => self.update(stmt)?,
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
        }
    }

    fn delete(&mut self, delete: &Delete<'stmt>) -> fmt::Result {
        write!(self.dst, "DELETE FROM ")?;

        assert!(delete.returning.is_none());

        for table_with_join in &delete.from {
            let table = self.schema.table(table_with_join.table);
            write!(self.dst, "\"{}\"", table.name)?;
        }

        write!(self.dst, " WHERE ")?;

        let selection = delete.selection.as_ref().unwrap();
        self.expr(selection)?;

        Ok(())
    }

    fn insert(&mut self, stmt: &Insert<'stmt>) -> fmt::Result {
        write!(
            self.dst,
            "INSERT INTO \"{}\" (",
            self.schema.table(stmt.table).name
        )?;

        let mut s = "";
        for column_id in &stmt.columns {
            write!(self.dst, "{}\"{}\"", s, self.schema.column(column_id).name)?;
            s = ", ";
        }

        write!(self.dst, ") ")?;

        self.query(&stmt.source)?;

        if let Some(returning) = &stmt.returning {
            write!(self.dst, " RETURNING ")?;
            self.expr_list(returning)?;
        }

        Ok(())
    }

    fn update(&mut self, update: &Update<'stmt>) -> fmt::Result {
        let table = self.schema.table(update.table.table);

        write!(self.dst, "UPDATE \"{}\" SET", table.name)?;

        for assignment in &update.assignments {
            let column = self.schema.column(assignment.target);
            write!(self.dst, " \"{}\" = ", column.name)?;

            self.expr(&assignment.value)?;
        }

        if update.selection.is_some() || update.pre_condition.is_some() {
            write!(self.dst, " WHERE ")?;
        }

        if let Some(selection) = &update.selection {
            self.expr(selection)?;
        }

        if let Some(pre_condition) = &update.pre_condition {
            if update.selection.is_some() {
                write!(self.dst, " AND ")?;
            }

            self.expr(pre_condition)?;

            if update.returning.is_none() {
                write!(self.dst, " RETURNING true")?;
            }
        }

        if let Some(returning) = &update.returning {
            write!(self.dst, " RETURNING ")?;
            self.expr_list(returning)?;
        }

        Ok(())
    }

    fn select(&mut self, select: &Select<'stmt>) -> fmt::Result {
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
    }

    fn values(&mut self, values: &Values<'stmt>) -> fmt::Result {
        let mut s = "VALUES";
        for record in &values.rows {
            write!(self.dst, "{} (", s)?;
            self.expr_list(record)?;
            write!(self.dst, ")")?;
            s = ",";
        }

        Ok(())
    }

    fn expr_list(&mut self, exprs: &[Expr<'stmt>]) -> fmt::Result {
        let mut s = "";

        for expr in exprs {
            write!(self.dst, "{}", s)?;
            self.expr(expr)?;
            s = ", ";
        }

        Ok(())
    }

    fn expr(&mut self, expr: &Expr<'stmt>) -> fmt::Result {
        match expr {
            Expr::BeginsWith(ExprBeginsWith { expr, pattern }) => {
                let str = pattern.as_value().expect_string();
                let pattern = format!("{}%", str);
                self.expr(expr)?;
                write!(self.dst, " LIKE ")?;
                self.expr(&Expr::Value(pattern.into()))?;
            }
            Expr::BinaryOp(ExprBinaryOp { lhs, op, rhs }) => {
                self.expr(&*lhs)?;
                write!(self.dst, " ")?;
                self.binary_op(op)?;
                write!(self.dst, " ")?;
                self.expr(&rhs)?;
            }
            Expr::Value(value) => self.value(value)?,
            Expr::Column(column_id) => {
                // TODO: at some point we need to conditionally scope the column
                // name.
                let column = self.schema.column(*column_id);
                self.ident_str(&column.name)?;
                /*
                let table = self.schema.table(column_id.table);
                let column = self.schema.column(*column_id);

                write!(self.dst, "\"{}\".\"{}\"", table.name, column.name)?;
                */
            }
            Expr::Like(ExprLike { expr, pattern }) => {
                self.expr(expr)?;
                write!(self.dst, " LIKE ")?;
                self.expr(pattern)?;
            }
            Expr::InList(ExprInList { expr, list }) => {
                self.expr(expr)?;
                write!(self.dst, " IN (")?;

                let mut s = "";

                match list {
                    ExprList::Expr(exprs) => {
                        for e in exprs {
                            write!(self.dst, "{}", s)?;
                            self.expr(e)?;
                            s = ", ";
                        }
                    }
                    ExprList::Value(values) => {
                        for v in values {
                            write!(self.dst, "{}", s)?;
                            self.value(v)?;
                            s = ", ";
                        }
                    }
                    ExprList::Placeholder(_) => {
                        todo!("PLACEHOLDER");
                    }
                }

                write!(self.dst, ")")?;
            }
            Expr::InSubquery(ExprInSubquery { expr, subquery }) => {
                self.expr(expr)?;
                write!(self.dst, " IN (")?;

                self.query(&subquery)?;
                write!(self.dst, ")")?;
            }
            Expr::IsNotNull(expr) => {
                self.expr(expr)?;
                write!(self.dst, " IS NOT NULL")?;
            }
            Expr::And(ExprAnd { operands }) => {
                let mut s = "";

                for expr in operands {
                    write!(self.dst, "{}", s)?;
                    self.expr(expr)?;
                    s = " AND ";
                }
            }
            Expr::Or(ExprOr { operands }) => {
                let mut s = "";

                for expr in operands {
                    write!(self.dst, "{}", s)?;
                    self.expr(expr)?;
                    s = " OR ";
                }
            }
            Expr::Tuple(expr_tuple) => {
                write!(self.dst, "(")?;

                let mut s = "";
                for expr in &expr_tuple.exprs {
                    write!(self.dst, "{}", s)?;
                    self.expr(expr)?;
                    s = ", ";
                }

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
        write!(self.dst, "\"{}\"", ident)?;
        Ok(())
    }
}
