mod op;

use toasty_core::{
    driver::{
        operation::{self, Operation},
        Capability, Driver, Response,
    },
    schema::{
        app,
        db::{Column, ColumnId, Schema, Table},
    },
    stmt,
};

use anyhow::Result;
use aws_sdk_dynamodb::{
    error::SdkError, operation::update_item::UpdateItemError, types::*, Client,
};
use std::{collections::HashMap, fmt::Write, sync::Arc};
use url::Url;

#[derive(Debug)]
pub struct DynamoDb {
    /// Handle to the AWS SDK client
    client: Client,
}

impl DynamoDb {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn connect(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;

        if url.scheme() != "dynamodb" {
            return Err(anyhow::anyhow!(
                "connection URL does not have a `dynamodb` scheme; url={url}"
            ));
        }

        use aws_config::BehaviorVersion;
        use aws_sdk_dynamodb::config::Credentials;

        let mut aws_config = aws_config::defaults(BehaviorVersion::latest())
            .credentials_provider(Credentials::for_tests());

        if let Some(host) = url.host() {
            let mut endpoint_url = format!("http://{host}");

            if let Some(port) = url.port() {
                endpoint_url.push_str(&format!(":{port}"));
            }

            aws_config = aws_config.endpoint_url(&endpoint_url);
        }

        let sdk_config = aws_config.load().await;

        let client = Client::new(&sdk_config);

        Ok(Self { client })
    }

    pub async fn from_env() -> Result<Self> {
        use aws_config::BehaviorVersion;
        use aws_sdk_dynamodb::config::Credentials;

        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region("foo")
            .credentials_provider(Credentials::for_tests())
            .endpoint_url("http://localhost:8000")
            .load()
            .await;

        let client = Client::new(&sdk_config);

        Ok(Self { client })
    }
}

#[toasty_core::async_trait]
impl Driver for DynamoDb {
    fn capability(&self) -> &Capability {
        &Capability::DYNAMODB
    }

    async fn register_schema(&mut self, _schema: &Schema) -> Result<()> {
        Ok(())
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        self.exec2(schema, op).await
    }

    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        for table in &schema.tables {
            self.create_table(schema, table, true).await?;
        }

        Ok(())
    }
}

impl DynamoDb {
    async fn exec2(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        use Operation::*;

        match op {
            GetByKey(op) => self.exec_get_by_key(schema, op).await,
            QueryPk(op) => self.exec_query_pk(schema, op).await,
            DeleteByKey(op) => self.exec_delete_by_key(schema, op).await,
            UpdateByKey(op) => self.exec_update_by_key(schema, op).await,
            FindPkByIndex(op) => self.exec_find_pk_by_index(schema, op).await,
            QuerySql(op) => match op.stmt {
                stmt::Statement::Insert(op) => self.exec_insert(schema, op).await,
                _ => todo!("op={:#?}", op),
            },
            _ => todo!("op={op:#?}"),
        }
    }
}

fn ddb_ty(ty: &stmt::Type) -> ScalarAttributeType {
    use stmt::Type::*;
    use ScalarAttributeType::*;

    match ty {
        Bool => N,
        String | Enum(..) => S,
        I64 | I32 => N,
        Id(_) => S,
        _ => todo!("ddb_ty; ty={:#?}", ty),
    }
}

fn ddb_key(table: &Table, key: &stmt::Value) -> HashMap<String, AttributeValue> {
    let mut ret = HashMap::new();

    for (index, column) in table.primary_key_columns().enumerate() {
        let value = match key {
            stmt::Value::Record(record) => &record[index],
            value => value,
        };

        ret.insert(column.name.clone(), ddb_val(value));
    }

    ret
}

#[derive(serde::Serialize, serde::Deserialize)]
enum V {
    Bool(bool),
    Null,
    String(String),
    I64(i64),
    I32(i32),
    Id(usize, String),
}

fn ddb_val(val: &stmt::Value) -> AttributeValue {
    match val {
        stmt::Value::Bool(val) => AttributeValue::Bool(*val),
        stmt::Value::String(val) => AttributeValue::S(val.to_string()),
        stmt::Value::I64(val) => AttributeValue::N(val.to_string()),
        stmt::Value::I32(val) => AttributeValue::N(val.to_string()),
        stmt::Value::Id(val) => AttributeValue::S(val.to_string()),
        stmt::Value::Enum(val) => {
            let v = match &val.fields[..] {
                [] => V::Null,
                [stmt::Value::Bool(v)] => V::Bool(*v),
                [stmt::Value::String(v)] => V::String(v.to_string()),
                [stmt::Value::I64(v)] => V::I64(*v),
                [stmt::Value::I32(v)] => V::I32(*v),
                [stmt::Value::Id(id)] => V::Id(id.model_id().0, id.to_string()),
                _ => todo!("val={:#?}", val.fields),
            };
            AttributeValue::S(format!(
                "{}#{}",
                val.variant,
                serde_json::to_string(&v).unwrap()
            ))
        }
        _ => todo!("{:#?}", val),
    }
}

fn ddb_to_val(ty: &stmt::Type, val: &AttributeValue) -> stmt::Value {
    use stmt::Type;
    use AttributeValue::*;

    match (ty, val) {
        (Type::Bool, Bool(val)) => stmt::Value::from(*val),
        (Type::String, S(val)) => stmt::Value::from(val.clone()),
        (Type::I64, N(val)) => stmt::Value::from(val.parse::<i64>().unwrap()),
        (Type::I32, N(val)) => stmt::Value::from(val.parse::<i32>().unwrap()),
        (Type::Id(model), S(val)) => stmt::Value::from(stmt::Id::from_string(*model, val.clone())),
        (Type::Enum(..), S(val)) => {
            let (variant, rest) = val.split_once("#").unwrap();
            let variant: usize = variant.parse().unwrap();
            let v: V = serde_json::from_str(rest).unwrap();
            let value = match v {
                V::Bool(v) => stmt::Value::Bool(v),
                V::Null => stmt::Value::Null,
                V::String(v) => stmt::Value::String(v),
                V::Id(model, v) => stmt::Value::Id(stmt::Id::from_string(app::ModelId(model), v)),
                V::I64(v) => stmt::Value::I64(v),
                V::I32(v) => stmt::Value::I32(v),
            };

            if value.is_null() {
                stmt::ValueEnum {
                    variant,
                    fields: stmt::ValueRecord::from_vec(vec![]),
                }
                .into()
            } else {
                stmt::ValueEnum {
                    variant,
                    fields: stmt::ValueRecord::from_vec(vec![value]),
                }
                .into()
            }
        }
        _ => todo!("ty={:#?}; value={:#?}", ty, val),
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
                    ddb_to_val(&column.ty, value)
                } else {
                    stmt::Value::Null
                }
            })
            .collect(),
    ))
}

fn ddb_expression(
    schema: &Schema,
    attrs: &mut ExprAttrs,
    primary: bool,
    expr: &stmt::Expr,
) -> String {
    match expr {
        stmt::Expr::BinaryOp(expr_binary_op) => {
            let lhs = ddb_expression(schema, attrs, primary, &expr_binary_op.lhs);
            let rhs = ddb_expression(schema, attrs, primary, &expr_binary_op.rhs);

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
                _ => todo!("OP {:?}", expr_binary_op.op),
            }
        }
        stmt::Expr::Column(stmt::ExprColumn::Column(column_id)) => {
            let column = schema.column(*column_id);
            attrs.column(column).to_string()
        }
        stmt::Expr::Value(val) => attrs.value(val),
        stmt::Expr::And(expr_and) => {
            let operands = expr_and
                .operands
                .iter()
                .map(|operand| ddb_expression(schema, attrs, primary, operand))
                .collect::<Vec<_>>();
            operands.join(" AND ")
        }
        stmt::Expr::Pattern(stmt::ExprPattern::BeginsWith(begins_with)) => {
            let expr = ddb_expression(schema, attrs, primary, &begins_with.expr);
            let substr = ddb_expression(schema, attrs, primary, &begins_with.pattern);
            format!("begins_with({expr}, {substr})")
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
        self.ddb_value(ddb_val(val))
    }

    fn ddb_value(&mut self, val: AttributeValue) -> String {
        let i = self.attr_values.len();
        let name = format!(":v_{i}");
        self.attr_values.insert(name.clone(), val);
        name
    }
}
