mod op;
mod r#type;
mod value;

pub(crate) use r#type::TypeExt;
pub(crate) use value::Value;

use toasty_core::{
    async_trait,
    driver::{operation::Operation, Capability, Driver, Response},
    schema::db::{self, Column, ColumnId, Migration, SchemaDiff, Table},
    stmt::{self, ExprContext},
    Error, Result, Schema,
};

use aws_sdk_dynamodb::{
    error::SdkError,
    operation::update_item::UpdateItemError,
    types::{
        AttributeDefinition, AttributeValue, Delete, GlobalSecondaryIndex, KeySchemaElement,
        KeyType, KeysAndAttributes, Projection, ProjectionType, ProvisionedThroughput, Put,
        PutRequest, ReturnValuesOnConditionCheckFailure, TransactWriteItem, Update, WriteRequest,
    },
    Client,
};
use std::{borrow::Cow, collections::HashMap, sync::Arc};

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
        unimplemented!("DynamoDB migrations are not yet supported. DynamoDB schema changes require manual table updates through the AWS console or SDK.")
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

#[derive(Debug)]
pub struct Connection {
    /// Handle to the AWS SDK client
    client: Client,
}

impl Connection {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        self.exec2(schema, op).await
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        for table in &schema.db.tables {
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
        _name: String,
        _migration: &toasty_core::schema::db::Migration,
    ) -> Result<()> {
        todo!("DynamoDB migrations are not yet implemented")
    }
}

impl Connection {
    async fn exec2(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
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
                    stmt::Statement::Insert(op) => self.exec_insert(&schema.db, op).await,
                    _ => todo!("op={:#?}", op),
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

/// Convert a DynamoDB key (HashMap<String, AttributeValue>) back to a stmt::Value.
/// For primary key queries, this extracts values for all primary key columns in order.
fn ddb_key_to_value(table: &Table, key: &HashMap<String, AttributeValue>) -> stmt::Value {
    let values: Vec<stmt::Value> = table
        .primary_key_columns()
        .map(|column| {
            key.get(&column.name)
                .map(|attr_val| Value::from_ddb(&column.ty, attr_val).into_inner())
                .unwrap_or(stmt::Value::Null)
        })
        .collect();

    if values.len() == 1 {
        values[0].clone()
    } else {
        stmt::Value::Record(stmt::ValueRecord::from_vec(values))
    }
}

fn ddb_key_schema(partition: &Column, range: Option<&Column>) -> Vec<KeySchemaElement> {
    let mut ks = vec![];

    ks.push(
        KeySchemaElement::builder()
            .attribute_name(&partition.name)
            .key_type(KeyType::Hash)
            .build()
            .unwrap(),
    );

    if let Some(range) = range {
        ks.push(
            KeySchemaElement::builder()
                .attribute_name(&range.name)
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
            let column = cx.resolve_expr_reference(expr_reference).expect_column();
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
