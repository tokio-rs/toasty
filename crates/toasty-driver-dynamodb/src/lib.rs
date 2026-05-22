#![warn(missing_docs)]

//! Toasty driver for [Amazon DynamoDB](https://aws.amazon.com/dynamodb/) using
//! the [`aws-sdk-dynamodb`](https://docs.rs/aws-sdk-dynamodb) SDK.
//!
//! # Examples
//!
//! ```no_run
//! # async fn example() -> toasty_core::Result<()> {
//! use toasty_driver_dynamodb::DynamoDb;
//!
//! let driver = DynamoDb::from_env("dynamodb://localhost".to_string()).await?;
//! # Ok(())
//! # }
//! ```

mod op;
mod r#type;
mod value;

pub(crate) use r#type::TypeExt;
pub(crate) use value::Value;

use async_trait::async_trait;
use toasty_core::{
    Error, Result, Schema,
    driver::{Capability, Driver, ExecResponse, operation::Operation},
    schema::{
        app::ModelId,
        db::{self, Column, ColumnId, IndexScope, Migration, Table},
    },
    stmt::{self, Expr, ExprContext, Visit},
};

use aws_sdk_dynamodb::{
    Client,
    error::SdkError,
    operation::transact_write_items::TransactWriteItemsError,
    operation::update_item::UpdateItemError,
    types::{
        AttributeDefinition, AttributeValue, BillingMode, Delete, GlobalSecondaryIndex,
        KeySchemaElement, KeyType, KeysAndAttributes, Projection, ProjectionType, Put, PutRequest,
        ReturnValuesOnConditionCheckFailure, TransactWriteItem, Update, WriteRequest,
    },
};
use std::{borrow::Cow, collections::HashMap, sync::Arc};
use toasty_core::schema::diff;

/// A DynamoDB [`Driver`] backed by the AWS SDK.
///
/// Create one with [`DynamoDb::from_env`] to load AWS credentials and region
/// from the environment, or [`DynamoDb::new`] / [`DynamoDb::with_sdk_config`]
/// for manual setup.
#[derive(Debug, Clone)]
pub struct DynamoDb {
    url: String,
    client: Client,
}

impl DynamoDb {
    /// Create driver with pre-built client (backward compatible, synchronous)
    pub fn new(url: String, client: Client) -> Self {
        Self { url, client }
    }

    /// Create driver loading AWS config from environment (async factory)
    /// Reads: AWS_REGION, AWS_ENDPOINT_URL_DYNAMODB, AWS credentials, etc.
    pub async fn from_env(url: String) -> Result<Self> {
        use aws_config::BehaviorVersion;

        let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let client = Client::new(&sdk_config);
        Ok(Self::new(url, client))
    }

    /// Create driver with custom SdkConfig (synchronous)
    pub fn with_sdk_config(url: String, sdk_config: &aws_config::SdkConfig) -> Self {
        let client = Client::new(sdk_config);
        Self::new(url, client)
    }
}

#[async_trait]
impl Driver for DynamoDb {
    fn url(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.url)
    }

    fn capability(&self) -> &'static Capability {
        &Capability::DYNAMODB
    }

    async fn connect(&self) -> toasty_core::Result<Box<dyn toasty_core::driver::Connection>> {
        // Clone the shared client - cheap operation (Client uses Arc internally)
        Ok(Box::new(Connection::new(self.client.clone())))
    }

    fn generate_migration(&self, _schema_diff: &diff::Schema<'_>) -> Migration {
        unimplemented!(
            "DynamoDB migrations are not yet supported. DynamoDB schema changes require manual table updates through the AWS console or SDK."
        )
    }

    async fn reset_db(&self) -> toasty_core::Result<()> {
        // Use shared client directly
        let mut exclusive_start_table_name = None;
        loop {
            let mut req = self.client.list_tables();
            if let Some(start) = &exclusive_start_table_name {
                req = req.exclusive_start_table_name(start);
            }

            let resp = req
                .send()
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;

            if let Some(table_names) = &resp.table_names {
                for table_name in table_names {
                    self.client
                        .delete_table()
                        .table_name(table_name)
                        .send()
                        .await
                        .map_err(toasty_core::Error::driver_operation_failed)?;
                }
            }

            exclusive_start_table_name = resp.last_evaluated_table_name;
            if exclusive_start_table_name.is_none() {
                break;
            }
        }

        Ok(())
    }
}

/// An open connection to DynamoDB.
#[derive(Debug)]
pub struct Connection {
    /// Handle to the AWS SDK client
    client: Client,
}

impl Connection {
    /// Wrap an existing [`aws_sdk_dynamodb::Client`] as a Toasty connection.
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<ExecResponse> {
        self.exec2(schema, op).await
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        for table in &schema.db.tables {
            tracing::debug!(table = %table.name, "creating table");
            self.create_table(&schema.db, table, true).await?;
        }
        Ok(())
    }

    async fn applied_migrations(
        &mut self,
    ) -> Result<Vec<toasty_core::schema::db::AppliedMigration>> {
        todo!("DynamoDB migrations are not yet implemented")
    }

    async fn apply_migration(
        &mut self,
        _id: u64,
        _name: &str,
        _migration: &toasty_core::schema::db::Migration,
    ) -> Result<()> {
        todo!("DynamoDB migrations are not yet implemented")
    }
}

impl Connection {
    async fn exec2(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<ExecResponse> {
        match op {
            Operation::GetByKey(op) => self.exec_get_by_key(schema, op).await,
            Operation::QueryPk(op) => self.exec_query_pk(schema, op).await,
            Operation::DeleteByKey(op) => self.exec_delete_by_key(schema, op).await,
            Operation::UpdateByKey(op) => self.exec_update_by_key(schema, op).await,
            Operation::FindPkByIndex(op) => self.exec_find_pk_by_index(schema, op).await,
            Operation::QuerySql(op) => {
                assert!(
                    op.last_insert_id_hack.is_none(),
                    "last_insert_id_hack is MySQL-specific and should not be set for DynamoDB"
                );
                match op.stmt {
                    stmt::Statement::Insert(insert) => self.exec_insert(&schema.db, insert).await,
                    _ => todo!("op={:#?}", op.stmt),
                }
            }
            Operation::Scan(op) => self.exec_scan(schema, op).await,
            Operation::RawSql(_) => Err(Error::unsupported_feature(
                "raw SQL is only supported by SQL drivers",
            )),
            Operation::Transaction(_) => Err(Error::unsupported_feature(
                "transactions are not supported by the DynamoDB driver",
            )),
            _ => todo!("op={op:#?}"),
        }
    }
}

fn ddb_key(table: &Table, key: &stmt::Value) -> HashMap<String, AttributeValue> {
    let mut ret = HashMap::new();
    let pk_index = &table.indices[table.primary_key.index.index];
    let mut sk_values: Vec<(&str, stmt::Value)> = Vec::new();

    for (index, index_column) in pk_index.columns.iter().enumerate() {
        let column = table.column(index_column.column);
        let value = match key {
            stmt::Value::Record(record) => record[index].clone(),
            value => value.clone(),
        };

        match index_column.scope {
            IndexScope::Local => {
                sk_values.push((&column.name, value));
            }
            IndexScope::Partition => {
                ret.insert(column.name.clone(), Value::from(value).to_ddb());
            }
        }
    }

    if sk_values.len() > 1 {
        // Stop at the first Null — root-model rows (e.g. User) leave child-only
        // sort-key components absent, so the composite key is a prefix only.
        let parts: Vec<String> = sk_values
            .iter()
            .take_while(|(_, v)| !v.is_null())
            .map(|(_, v)| value_to_sk_part(v))
            .collect();
        assert!(
            !parts.is_empty(),
            "at least one sort-key component must be non-null"
        );
        let mut sk = parts.join("#");
        sk.push('#');
        ret.insert("__sk".to_string(), AttributeValue::S(sk));
    } else if let Some((name, val)) = sk_values.into_iter().next() {
        ret.insert(name.to_string(), Value::from(val).to_ddb());
    }

    ret
}

fn value_to_sk_part(val: &stmt::Value) -> String {
    match val {
        stmt::Value::String(s) => s.clone(),
        stmt::Value::Uuid(u) => u.to_string(),
        stmt::Value::I64(n) => n.to_string(),
        stmt::Value::U64(n) => n.to_string(),
        _ => panic!("unsupported sort-key value type: {val:?}"),
    }
}

/// Convert a DynamoDB AttributeValue to stmt::Value (type-inferred).
fn attr_value_to_stmt_value(attr: &AttributeValue) -> stmt::Value {
    use AttributeValue as AV;

    match attr {
        AV::S(s) => stmt::Value::String(s.clone()),
        AV::N(n) => {
            // Try to parse as i64 first (most common), fallback to string
            n.parse::<i64>()
                .map(stmt::Value::I64)
                .unwrap_or_else(|_| stmt::Value::String(n.clone()))
        }
        AV::Bool(b) => stmt::Value::Bool(*b),
        AV::B(bytes) => stmt::Value::Bytes(bytes.clone().into_inner()),
        AV::Null(_) => stmt::Value::Null,
        // For complex types, convert to string representation
        _ => stmt::Value::String(format!("{:?}", attr)),
    }
}

/// Serialize a DynamoDB LastEvaluatedKey (for pagination) into stmt::Value.
/// Format: flat record [name1, value1, name2, value2, ...]
/// Example: { "pk": S("abc"), "sk": N("42") } → Record([String("pk"), String("abc"), String("sk"), I64(42)])
fn serialize_ddb_cursor(last_key: &HashMap<String, AttributeValue>) -> stmt::Value {
    let mut fields = Vec::with_capacity(last_key.len() * 2);

    for (name, attr_value) in last_key {
        fields.push(stmt::Value::String(name.clone()));
        fields.push(attr_value_to_stmt_value(attr_value));
    }

    stmt::Value::Record(stmt::ValueRecord::from_vec(fields))
}

/// Deserialize a stmt::Value cursor into a DynamoDB ExclusiveStartKey.
/// Expects flat record format: [name1, value1, name2, value2, ...]
fn deserialize_ddb_cursor(cursor: &stmt::Value) -> HashMap<String, AttributeValue> {
    let mut ret = HashMap::new();

    if let stmt::Value::Record(fields) = cursor {
        // Process pairs: [name, value, name, value, ...]
        for chunk in fields.chunks(2) {
            if chunk.len() == 2
                && let (stmt::Value::String(name), value) = (&chunk[0], &chunk[1])
            {
                ret.insert(name.clone(), Value::from(value.clone()).to_ddb());
            }
        }
    }

    ret
}

fn ddb_key_schema(partition: &[&str], range: &[&str]) -> Vec<KeySchemaElement> {
    let mut ks = vec![];

    for name in partition {
        ks.push(
            KeySchemaElement::builder()
                .attribute_name(*name)
                .key_type(KeyType::Hash)
                .build()
                .unwrap(),
        );
    }

    for name in range {
        ks.push(
            KeySchemaElement::builder()
                .attribute_name(*name)
                .key_type(KeyType::Range)
                .build()
                .unwrap(),
        );
    }

    ks
}

fn sort_key_columns(table: &Table) -> Vec<ColumnId> {
    table.indices[table.primary_key.index.index]
        .columns
        .iter()
        .filter(|c| matches!(c.scope, IndexScope::Local))
        .map(|c| table.column(c.column).id)
        .collect()
}

fn item_to_record<'a>(
    item: &HashMap<String, AttributeValue>,
    columns: impl Iterator<Item = &'a Column>,
    sk_cols: &[ColumnId],
) -> Result<stmt::ValueRecord> {
    // Parse __sk back into individual column values when the table uses a
    // composite sort key.
    let mut sk_vals: HashMap<ColumnId, stmt::Value> = HashMap::new();
    if sk_cols.len() > 1
        && let Some(AttributeValue::S(sk)) = item.get("__sk")
    {
        let mut parts: Vec<&str> = sk.split('#').collect();
        // We write a trailing delimiter so pop the empty tail.
        if parts.last() == Some(&"") {
            parts.pop();
        }
        for (i, part) in parts.iter().enumerate() {
            if let Some(&col_id) = sk_cols.get(i) {
                sk_vals.insert(col_id, stmt::Value::String((*part).to_string()));
            }
        }
    }

    Ok(stmt::ValueRecord::from_vec(
        columns
            .map(|column| {
                if let Some(value) = item.get(&column.name) {
                    Value::from_ddb(&column.ty, value).into_inner()
                } else if let Some(value) = sk_vals.get(&column.id) {
                    // Re-parse the raw string into the column's proper type.
                    let attr = AttributeValue::S(match value {
                        stmt::Value::String(s) => s.clone(),
                        _ => unreachable!(),
                    });
                    Value::from_ddb(&column.ty, &attr).into_inner()
                } else {
                    stmt::Value::Null
                }
            })
            .collect(),
    ))
}

/// Builds the DynamoDB key condition expression for a primary-key query.
///
/// For simple tables (single sort key or no sort key) this delegates to
/// `ddb_expression` unchanged.  For item-collection tables where the sort key
/// is synthesized as `__sk = "val1#val2#…"` this decomposes the filter into a
/// hash-key equality condition plus a `begins_with(__sk, prefix)` expression.
/// One sort-key segment as resolved during `BuildKeyExpression::visit_expr`.
///
/// Each variant captures *what the segment contributes to the SK prefix*,
/// already in the form the consumer needs — no more re-parsing AST shapes.
#[derive(Debug)]
enum SkComponent {
    /// The column is bound to a literal value; render the value as the segment.
    Literal(stmt::Value),
    /// The column is the item-collection discriminator; render the model
    /// name (in upper-camel case) as the segment.
    Model(ModelId),
    /// The column is absent for this model type (matched via `IS NULL`); skip.
    Skip,
}

struct BuildKeyExpression<'a> {
    table: &'a Table,
    attrs: &'a mut ExprAttrs,
    /// Column IDs of the Local-scoped PK columns (sort-key components).
    sk_cols: &'a [ColumnId],
    /// Resolved sort-key segments keyed by column ID.
    sk_components: HashMap<ColumnId, SkComponent>,
    /// The partition-key equality sub-expression.
    pk_component: Option<stmt::Expr>,
    schema: Arc<Schema>,
}

impl<'a> BuildKeyExpression<'a> {
    fn build(mut self, cx: &ExprContext<'_, db::Schema>, expr: &stmt::Expr) -> String {
        if self.sk_cols.len() <= 1 {
            // No composite sort key — pass through unchanged.
            return ddb_expression(&self.schema, cx, self.attrs, true, expr);
        }

        // Collect sub-expressions per column.
        self.visit_expr(expr);

        let pk_expr = self
            .pk_component
            .as_ref()
            .expect("key expression must include a hash-key condition");
        let mut key_expr = ddb_expression(&self.schema, cx, self.attrs, true, pk_expr);

        // Build the sort-key prefix from the collected components.
        let mut missing = false;
        let mut sk_prefix = String::new();
        for &sk_col_id in self.sk_cols {
            let Some(sk_sub) = self.sk_components.get(&sk_col_id) else {
                missing = true;
                continue;
            };

            assert!(!missing, "gap in sort-key component conditions");

            match sk_sub {
                SkComponent::Skip => {}
                SkComponent::Literal(val) => {
                    sk_prefix.push_str(&value_to_sk_part(val));
                    sk_prefix.push('#');
                }
                SkComponent::Model(model) => {
                    let model_name = self.schema.app.model(*model).name().upper_camel_case();
                    sk_prefix.push_str(&model_name);
                    sk_prefix.push('#');
                }
            }
        }

        let sk_prefix_attr = self.attrs.literal(sk_prefix);
        self.attrs
            .attr_names
            .insert("#__sk".to_string(), "__sk".to_string());
        key_expr.push_str(&format!(" AND begins_with(#__sk, {sk_prefix_attr})"));
        key_expr
    }
}

impl Visit for BuildKeyExpression<'_> {
    fn visit_expr(&mut self, i: &Expr) {
        match i {
            Expr::And(and) => self.visit_expr_and(and),
            Expr::BinaryOp(binop) => self.visit_expr_binary_op(binop),
            Expr::IsNull(isnull) => self.visit_expr_is_null(isnull),
            Expr::IsModel(model) => self.visit_expr_is_model(model),
            _ => todo!("BuildKeyExpression::visit_expr: {i:#?}"),
        }
    }

    fn visit_expr_binary_op(&mut self, i: &stmt::ExprBinaryOp) {
        assert!(
            matches!(i.op, stmt::BinaryOp::Eq),
            "key condition must be equality; got {i:#?}"
        );
        let Expr::Reference(refer) = i.lhs.as_ref() else {
            todo!("key lhs is not a reference: {i:#?}");
        };

        // To avoid needing a &ExprContext here we pattern-match on the column
        // index directly — the reference's column index into the table.
        let col_idx = match refer {
            stmt::ExprReference::Column(ec) => ec.column,
            _ => todo!("key reference is not a column: {refer:#?}"),
        };

        let col_id = ColumnId {
            table: self.table.id,
            index: col_idx,
        };

        if self.sk_cols.contains(&col_id) {
            // Sort-key component: extract the bound literal so the consumer
            // doesn't have to re-pattern-match the AST.
            let stmt::Expr::Value(val) = i.rhs.as_ref() else {
                todo!("sort-key equality must bind a literal value; got {i:#?}");
            };
            self.sk_components
                .insert(col_id, SkComponent::Literal(val.clone()));
        } else {
            self.pk_component = Some(Expr::BinaryOp(i.clone()));
        }
    }

    fn visit_expr_is_null(&mut self, i: &stmt::ExprIsNull) {
        let Expr::Reference(refer) = i.expr.as_ref() else {
            return;
        };
        let col_idx = match refer {
            stmt::ExprReference::Column(ec) => ec.column,
            _ => return,
        };
        let col_id = ColumnId {
            table: self.table.id,
            index: col_idx,
        };
        if self.sk_cols.contains(&col_id) {
            self.sk_components.insert(col_id, SkComponent::Skip);
        }
    }

    fn visit_expr_is_model(&mut self, model: &stmt::ExprIsModel) {
        let mapping = self.schema.mapping_for(model.model);
        let disc_col_id = mapping
            .item_collection
            .model_column
            .expect("IsModel emitted for model without discriminator");
        self.sk_components
            .insert(disc_col_id, SkComponent::Model(model.model));
    }
}

fn ddb_expression(
    schema: &Schema,
    cx: &ExprContext<'_, db::Schema>,
    attrs: &mut ExprAttrs,
    primary: bool,
    expr: &stmt::Expr,
) -> String {
    match expr {
        stmt::Expr::Between(expr_between) => {
            let field = ddb_expression(cx, attrs, primary, &expr_between.expr);
            let low = ddb_expression(cx, attrs, primary, &expr_between.low);
            let high = ddb_expression(cx, attrs, primary, &expr_between.high);
            format!("{field} BETWEEN {low} AND {high}")
        }
        stmt::Expr::BinaryOp(expr_binary_op) => {
            let lhs = ddb_expression(schema, cx, attrs, primary, &expr_binary_op.lhs);
            let rhs = ddb_expression(schema, cx, attrs, primary, &expr_binary_op.rhs);

            match expr_binary_op.op {
                stmt::BinaryOp::Eq => format!("{lhs} = {rhs}"),
                stmt::BinaryOp::Ne if primary => {
                    todo!("!= conditions on primary key not supported")
                }
                stmt::BinaryOp::Ne => format!("{lhs} <> {rhs}"),
                stmt::BinaryOp::Gt => format!("{lhs} > {rhs}"),
                stmt::BinaryOp::Ge => format!("{lhs} >= {rhs}"),
                stmt::BinaryOp::Lt => format!("{lhs} < {rhs}"),
                stmt::BinaryOp::Le => format!("{lhs} <= {rhs}"),
                // DynamoDB condition expressions don't support arithmetic
                // between operands. Arithmetic ops belong in update
                // expressions (handled by `update_by_key.rs`), not in
                // condition/filter expressions.
                stmt::BinaryOp::Add | stmt::BinaryOp::Sub => {
                    todo!(
                        "arithmetic operators in DynamoDB condition expressions are not supported"
                    )
                }
            }
        }
        stmt::Expr::Reference(expr_reference) => {
            let (column, col_alias) = column_alias(cx, attrs, expr_reference);
            // A bare boolean column reference used as a predicate (result of
            // `field = true` simplification) needs an explicit equality check.
            if column.ty.is_bool() {
                let true_val = attrs.ddb_value(aws_sdk_dynamodb::types::AttributeValue::Bool(true));
                format!("{col_alias} = {true_val}")
            } else {
                col_alias
            }
        }
        stmt::Expr::Value(val) => attrs.value(val),
        stmt::Expr::And(expr_and) => {
            let operands = expr_and
                .operands
                .iter()
                .map(|operand| ddb_expression(schema, cx, attrs, primary, operand))
                .collect::<Vec<_>>();
            operands.join(" AND ")
        }
        stmt::Expr::Or(expr_or) => {
            let operands = expr_or
                .operands
                .iter()
                .map(|operand| ddb_expression(schema, cx, attrs, primary, operand))
                .collect::<Vec<_>>();
            format!("({})", operands.join(" OR "))
        }
        stmt::Expr::InList(in_list) => {
            let expr = ddb_expression(schema, cx, attrs, primary, &in_list.expr);

            // Extract the list items and create individual attribute values
            let items = match &*in_list.list {
                stmt::Expr::Value(stmt::Value::List(vals)) => vals
                    .iter()
                    .map(|val| attrs.value(val))
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => {
                    // If it's not a literal list, treat it as a single expression
                    ddb_expression(schema, cx, attrs, primary, &in_list.list)
                }
            };

            format!("{expr} IN ({items})")
        }
        stmt::Expr::IsNull(expr_is_null) => {
            // `attribute_not_exists` takes a bare attribute name. Resolve the
            // column alias directly rather than through `ddb_expression`, which
            // would expand a bool column to `#col = :true` — a comparison valid
            // only in predicate position, not as a function argument. (Without
            // this, `.is_none()` on any `Option<bool>` — including an
            // `Option<Embed>` presence column — produces invalid syntax.)
            let inner = match &*expr_is_null.expr {
                stmt::Expr::Reference(expr_reference) => column_alias(cx, attrs, expr_reference).1,
                other => ddb_expression(schema, cx, attrs, primary, other),
            };
            format!("attribute_not_exists({inner})")
        }
        stmt::Expr::IsModel(e) => {
            // On DynamoDB the discriminator column is not stored as a top-level
            // attribute on rows; it's encoded as the leading segment of the
            // synthesized __sk. Filter by the SK prefix so scans and other
            // filter-expression contexts find the right rows.
            let model_name = schema.app.model(e.model).name().upper_camel_case();
            let prefix_attr = attrs.value(&stmt::Value::String(format!("{model_name}#")));
            attrs
                .attr_names
                .insert("#__sk".to_string(), "__sk".to_string());
            format!("begins_with(#__sk, {prefix_attr})")
        }
        stmt::Expr::Not(expr_not) => {
            let inner = ddb_expression(schema, cx, attrs, primary, &expr_not.expr);
            format!("(NOT {inner})")
        }
        stmt::Expr::StartsWith(expr_starts_with) => {
            let expr = ddb_expression(schema, cx, attrs, primary, &expr_starts_with.expr);
            let prefix = ddb_expression(schema, cx, attrs, primary, &expr_starts_with.prefix);
            format!("begins_with({expr}, {prefix})")
        }
        stmt::Expr::Like(_) => {
            panic!(
                "LIKE is not supported by the DynamoDB driver; use starts_with for prefix matching"
            )
        }
        stmt::Expr::AnyOp(any) if matches!(any.op, stmt::BinaryOp::Eq) => {
            // `Path::contains(value)` lowers to `value = ANY(col)`. On
            // DynamoDB that's `contains(path, value)` — the standard List
            // membership filter.
            let value = ddb_expression(schema, cx, attrs, primary, &any.lhs);
            let path = ddb_expression(schema, cx, attrs, primary, &any.rhs);
            format!("contains({path}, {value})")
        }
        stmt::Expr::Length(expr) => {
            let inner = ddb_expression(schema, cx, attrs, primary, &expr.expr);
            format!("size({inner})")
        }
        stmt::Expr::Cast(expr_cast) if expr_cast.ty == stmt::Type::Bool => {
            // Bool key/index fields bridge through I8 (db::Type::Integer(1) via
            // bridge_type). The lowering wraps the I8 column ref in
            // Cast(col_ref, Bool) when the field appears as a bare predicate
            // (result of `field = true` simplification). In predicate position
            // this means "is true"; the `field = false` case arrives as
            // Not(Cast(col_ref, Bool)) and is handled by the Not arm above.
            let col_alias = ddb_expression(cx, attrs, primary, &expr_cast.expr);
            let true_val =
                attrs.ddb_value(aws_sdk_dynamodb::types::AttributeValue::N("1".to_string()));
            format!("{col_alias} = {true_val}")
        }
        _ => todo!("FILTER = {:#?}", expr),
    }
}

/// Resolves a column reference to its DynamoDB attribute alias (e.g. `#col_3`),
/// registering the underlying attribute name in `attrs`. Returns the resolved
/// column alongside the alias so callers can inspect its storage type.
fn column_alias<'a>(
    cx: &ExprContext<'a, db::Schema>,
    attrs: &mut ExprAttrs,
    expr_reference: &stmt::ExprReference,
) -> (&'a Column, String) {
    let column = cx.resolve_expr_reference(expr_reference).as_column_unwrap();
    let alias = attrs.column(column).to_string();
    (column, alias)
}

#[derive(Default)]
struct ExprAttrs {
    columns: HashMap<ColumnId, String>,
    attr_names: HashMap<String, String>,
    attr_values: HashMap<String, AttributeValue>,
}

impl ExprAttrs {
    fn column(&mut self, column: &Column) -> &str {
        use std::collections::hash_map::Entry;

        match self.columns.entry(column.id) {
            Entry::Vacant(e) => {
                let name = format!("#col_{}", column.id.index);
                self.attr_names.insert(name.clone(), column.name.clone());
                e.insert(name)
            }
            Entry::Occupied(e) => e.into_mut(),
        }
    }

    fn literal(&mut self, s: impl Into<String>) -> String {
        self.ddb_value(AttributeValue::S(s.into()))
    }

    fn value(&mut self, val: &stmt::Value) -> String {
        self.ddb_value(Value::from(val.clone()).to_ddb())
    }

    fn ddb_value(&mut self, val: AttributeValue) -> String {
        let i = self.attr_values.len();
        let name = format!(":v_{i}");
        self.attr_values.insert(name.clone(), val);
        name
    }
}
