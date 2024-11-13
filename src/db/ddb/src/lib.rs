mod op;

use toasty_core::{
    driver::{
        capability,
        operation::{self, Operation},
        Capability, Driver, Response,
    },
    schema::{self, Column, ColumnId},
    stmt, Schema,
};

use anyhow::Result;
use aws_sdk_dynamodb::{
    error::SdkError, operation::update_item::UpdateItemError, types::*, Client,
};
use std::{collections::HashMap, fmt::Write};

#[derive(Debug)]
pub struct DynamoDB {
    /// Handle to the AWS SDK client
    client: Client,

    /// Prefix for all table names. Toasty schema table names have this prefix
    /// appended before passing it to the DDB client.
    table_prefix: Option<String>,
}

impl DynamoDB {
    pub fn new(client: Client, table_prefix: Option<String>) -> DynamoDB {
        DynamoDB {
            client,
            table_prefix,
        }
    }

    pub async fn from_env() -> Result<DynamoDB> {
        use aws_config::BehaviorVersion;
        use aws_sdk_dynamodb::config::Credentials;

        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region("foo")
            .credentials_provider(Credentials::for_tests())
            .endpoint_url("http://localhost:8000")
            .load()
            .await;

        let client = Client::new(&sdk_config);

        Ok(DynamoDB {
            client,
            table_prefix: None,
        })
    }

    pub async fn from_env_with_prefix(table_prefix: &str) -> Result<DynamoDB> {
        let mut ddb = DynamoDB::from_env().await?;
        ddb.table_prefix = Some(table_prefix.to_string());
        Ok(ddb)
    }
}

#[toasty_core::async_trait]
impl Driver for DynamoDB {
    fn capability(&self) -> &Capability {
        &Capability::KeyValue(capability::KeyValue {
            primary_key_ne_predicate: false,
        })
    }

    async fn register_schema(&mut self, _schema: &Schema) -> Result<()> {
        Ok(())
    }

    async fn exec<'stmt>(&self, schema: &Schema, op: Operation<'stmt>) -> Result<Response<'stmt>> {
        // self.exec2(schema, op).await
        todo!()
    }

    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        for table in schema.tables() {
            self.create_table(schema, table, true).await?;
        }

        Ok(())
    }
}

impl DynamoDB {
    async fn exec2<'stmt>(
        &self,
        schema: &Schema,
        op: Operation<'stmt>,
    ) -> Result<stmt::ValueStream<'stmt>> {
        /*
        use Operation::*;

        match op {
            Insert(op) => self.exec_insert(schema, op.into_insert()).await,
            GetByKey(op) => self.exec_get_by_key(schema, op).await,
            QueryPk(op) => self.exec_query_pk(schema, op).await,
            DeleteByKey(op) => self.exec_delete_by_key(schema, op).await,
            UpdateByKey(op) => self.exec_update_by_key(schema, op).await,
            FindPkByIndex(op) => self.exec_find_pk_by_index(schema, op).await,
            QuerySql(op) => match op.stmt {
                sql::Statement::Insert(op) => self.exec_insert(schema, op).await,
                _ => todo!("op={:#?}", op),
            },
        }
        */
        todo!()
    }

    fn table_name(&self, table: &schema::Table) -> String {
        if let Some(prefix) = &self.table_prefix {
            format!("{}{}", prefix, table.name)
        } else {
            table.name.to_string()
        }
    }

    fn index_table_name(&self, index: &schema::Index) -> String {
        if let Some(prefix) = &self.table_prefix {
            format!("{}{}", prefix, index.name)
        } else {
            index.name.to_string()
        }
    }
}

fn ddb_ty(ty: &stmt::Type) -> ScalarAttributeType {
    use stmt::Type::*;
    use ScalarAttributeType::*;

    match ty {
        Bool => N,
        String | Enum(..) => S,
        I64 => N,
        Id(_) => S,
        _ => todo!("ddb_ty; ty={:#?}", ty),
    }
}

fn ddb_key(table: &schema::Table, key: &stmt::Value<'_>) -> HashMap<String, AttributeValue> {
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
    Id(usize, String),
}

fn ddb_val(val: &stmt::Value<'_>) -> AttributeValue {
    match val {
        stmt::Value::Bool(val) => AttributeValue::Bool(*val),
        stmt::Value::String(val) => AttributeValue::S(val.to_string()),
        stmt::Value::I64(val) => AttributeValue::N(val.to_string()),
        stmt::Value::Id(val) => AttributeValue::S(val.to_string()),
        stmt::Value::Enum(val) => {
            let v = match &val.fields[..] {
                [] => V::Null,
                [stmt::Value::Bool(v)] => V::Bool(*v),
                [stmt::Value::String(v)] => V::String(v.to_string()),
                [stmt::Value::I64(v)] => V::I64(*v),
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

fn ddb_to_val<'a>(ty: &stmt::Type, val: &'a AttributeValue) -> stmt::Value<'a> {
    use stmt::Type;
    use AttributeValue::*;

    match (ty, val) {
        (Type::Bool, Bool(val)) => stmt::Value::from(*val),
        (Type::String, S(val)) => stmt::Value::from(val),
        (Type::I64, N(val)) => stmt::Value::from(val.parse::<i64>().unwrap()),
        (Type::Id(model), S(val)) => stmt::Value::from(stmt::Id::from_string(*model, val.clone())),
        (Type::Enum(..), S(val)) => {
            let (variant, rest) = val.split_once("#").unwrap();
            let variant: usize = variant.parse().unwrap();
            let v: V = serde_json::from_str(rest).unwrap();
            let value = match v {
                V::Bool(v) => stmt::Value::Bool(v),
                V::Null => stmt::Value::Null,
                V::String(v) => stmt::Value::String(v.into()),
                V::Id(model, v) => {
                    stmt::Value::Id(stmt::Id::from_string(schema::ModelId(model), v))
                }
                V::I64(v) => stmt::Value::I64(v),
            };

            if value.is_null() {
                stmt::ValueEnum {
                    variant,
                    fields: stmt::Record::from_vec(vec![]),
                }
                .into()
            } else {
                stmt::ValueEnum {
                    variant,
                    fields: stmt::Record::from_vec(vec![value]),
                }
                .into()
            }
        }
        _ => todo!("ty={:#?}; value={:#?}", ty, val),
    }
}

fn ddb_key_schema(
    partition: &schema::Column,
    range: Option<&schema::Column>,
) -> Vec<KeySchemaElement> {
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
    columns: impl Iterator<Item = &'a schema::Column>,
) -> Result<stmt::Record<'stmt>> {
    Ok(stmt::Record::from_vec(
        columns
            .map(|column| {
                if let Some(value) = item.get(&column.name) {
                    ddb_to_val(&column.ty, value).into_owned()
                } else {
                    stmt::Value::Null
                }
            })
            .collect(),
    ))
}

fn ddb_expression<'a>(
    schema: &'a Schema,
    attrs: &mut ExprAttrs,
    primary: bool,
    expr: &stmt::Expr<'_>,
) -> String {
    /*
    match expr {
        sql::Expr::BinaryOp(expr_binary_op) => {
            let lhs = ddb_expression(schema, attrs, primary, &expr_binary_op.lhs);
            let rhs = ddb_expression(schema, attrs, primary, &expr_binary_op.rhs);

            match expr_binary_op.op {
                sql::BinaryOp::Eq => format!("{lhs} = {rhs}"),
                sql::BinaryOp::Ne if primary => {
                    todo!("!= conditions on primary key not supported")
                }
                sql::BinaryOp::Ne => format!("{lhs} <> {rhs}"),
                sql::BinaryOp::Gt => format!("{lhs} > {rhs}"),
                sql::BinaryOp::Ge => format!("{lhs} >= {rhs}"),
                sql::BinaryOp::Lt => format!("{lhs} < {rhs}"),
                sql::BinaryOp::Le => format!("{lhs} <= {rhs}"),
                // stmt::BinaryOp::IsA => format!("begins_with({lhs}, {rhs})"),
                _ => todo!("OP {:?}", expr_binary_op.op),
            }
        }
        sql::Expr::Column(column_id) => {
            let column = schema.column(column_id);
            attrs.column(column).to_string()
        }
        sql::Expr::Value(val) => attrs.value(val),
        sql::Expr::And(expr_and) => {
            let operands = expr_and
                .operands
                .iter()
                .map(|operand| ddb_expression(schema, attrs, primary, operand))
                .collect::<Vec<_>>();
            operands.join(" AND ")
        }
        sql::Expr::BeginsWith(begins_with) => {
            let expr = ddb_expression(schema, attrs, primary, &begins_with.expr);
            let substr = ddb_expression(schema, attrs, primary, &begins_with.pattern);
            format!("begins_with({expr}, {substr})")
        }
        /*
        stmt::Expr::Type(expr_ty) => {
            let variant = expr_ty.variant.unwrap();
            let value = format!("{}#", variant);
            attrs.value(&value.into())
        }
        */
        _ => todo!("FILTER = {:#?}", expr),
    }
    */
    todo!()
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

    fn value(&mut self, val: &stmt::Value<'_>) -> String {
        self.ddb_value(ddb_val(val))
    }

    fn ddb_value(&mut self, val: AttributeValue) -> String {
        let i = self.attr_values.len();
        let name = format!(":v_{i}");
        self.attr_values.insert(name.clone(), val);
        name
    }
}
