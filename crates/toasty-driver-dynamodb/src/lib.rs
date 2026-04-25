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
    schema::db::{self, Column, ColumnId, Migration, SchemaDiff, Table},
    stmt::{self, ExprContext},
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

    fn generate_migration(&self, _schema_diff: &SchemaDiff<'_>) -> Migration {
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
            Operation::DeleteByKey(op) => self.exec_delete_by_key(&schema.db, op).await,
            Operation::UpdateByKey(op) => self.exec_update_by_key(&schema.db, op).await,
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
            Operation::Transaction(_) => Err(Error::unsupported_feature(
                "transactions are not supported by the DynamoDB driver",
            )),
            _ => todo!("op={op:#?}"),
        }
    }
}

fn ddb_key(table: &Table, key: &stmt::Value) -> HashMap<String, AttributeValue> {
    let mut ret = HashMap::new();

    for (index, column) in table.primary_key_columns().enumerate() {
        let value = match key {
            stmt::Value::Record(record) => &record[index],
            value => value,
        };

        ret.insert(column.name.clone(), Value::from(value.clone()).to_ddb());
    }

    ret
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

fn ddb_key_schema(
    partition_columns: &[&Column],
    range_columns: &[&Column],
) -> Vec<KeySchemaElement> {
    let mut ks = vec![];

    for col in partition_columns {
        ks.push(
            KeySchemaElement::builder()
                .attribute_name(&col.name)
                .key_type(KeyType::Hash)
                .build()
                .unwrap(),
        );
    }

    for col in range_columns {
        ks.push(
            KeySchemaElement::builder()
                .attribute_name(&col.name)
                .key_type(KeyType::Range)
                .build()
                .unwrap(),
        );
    }

    ks
}

fn item_to_record<'a, 'stmt>(
    item: &HashMap<String, AttributeValue>,
    columns: impl Iterator<Item = &'a Column>,
) -> Result<stmt::ValueRecord> {
    Ok(stmt::ValueRecord::from_vec(
        columns
            .map(|column| {
                if let Some(value) = item.get(&column.name) {
                    Value::from_ddb(&column.ty, value).into_inner()
                } else {
                    stmt::Value::Null
                }
            })
            .collect(),
    ))
}

fn ddb_expression(
    cx: &ExprContext<'_, db::Schema>,
    attrs: &mut ExprAttrs,
    primary: bool,
    expr: &stmt::Expr,
) -> String {
    match expr {
        stmt::Expr::BinaryOp(expr_binary_op) => {
            let lhs = ddb_expression(cx, attrs, primary, &expr_binary_op.lhs);
            let rhs = ddb_expression(cx, attrs, primary, &expr_binary_op.rhs);

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
            }
        }
        stmt::Expr::Reference(expr_reference) => {
            let column = cx.resolve_expr_reference(expr_reference).as_column_unwrap();
            attrs.column(column).to_string()
        }
        stmt::Expr::Value(val) => attrs.value(val),
        stmt::Expr::And(expr_and) => {
            let operands = expr_and
                .operands
                .iter()
                .map(|operand| ddb_expression(cx, attrs, primary, operand))
                .collect::<Vec<_>>();
            operands.join(" AND ")
        }
        stmt::Expr::Or(expr_or) => {
            let operands = expr_or
                .operands
                .iter()
                .map(|operand| ddb_expression(cx, attrs, primary, operand))
                .collect::<Vec<_>>();
            operands.join(" OR ")
        }
        stmt::Expr::InList(in_list) => {
            let expr = ddb_expression(cx, attrs, primary, &in_list.expr);

            // Extract the list items and create individual attribute values
            let items = match &*in_list.list {
                stmt::Expr::Value(stmt::Value::List(vals)) => vals
                    .iter()
                    .map(|val| attrs.value(val))
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => {
                    // If it's not a literal list, treat it as a single expression
                    ddb_expression(cx, attrs, primary, &in_list.list)
                }
            };

            format!("{expr} IN ({items})")
        }
        stmt::Expr::IsNull(expr_is_null) => {
            let inner = ddb_expression(cx, attrs, primary, &expr_is_null.expr);
            format!("attribute_not_exists({inner})")
        }
        stmt::Expr::Not(expr_not) => {
            let inner = ddb_expression(cx, attrs, primary, &expr_not.expr);
            format!("(NOT {inner})")
        }
        _ => todo!("FILTER = {:#?}", expr),
    }
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
