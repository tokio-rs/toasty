use crate::{capability, DynamoDb};
use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;
use toasty_core::{
    driver::{Capability, TestDriver},
    stmt, Result,
};

impl TestDriver for DynamoDb {
    const CAPABILITY: Capability = capability::CAPABILITY;

    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, stmt::Value>,
    ) -> Result<stmt::Value> {
        // Convert filter to DynamoDB key
        let mut key = HashMap::new();
        for (col_name, value) in filter {
            let attr_value = stmt_value_to_dynamodb_attr(&value)?;
            key.insert(col_name, attr_value);
        }

        // Get item from DynamoDB
        let response = self
            .client
            .get_item()
            .table_name(table)
            .set_key(Some(key))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("DynamoDB get_item failed: {e}"))?;

        if let Some(item) = response.item {
            if let Some(attr_value) = item.get(column) {
                dynamodb_attr_to_stmt_value(attr_value)
            } else {
                Err(anyhow::anyhow!("Column '{column}' not found in DynamoDB item"))
            }
        } else {
            Err(anyhow::anyhow!("No item found in DynamoDB"))
        }
    }
}

fn stmt_value_to_dynamodb_attr(value: &stmt::Value) -> Result<AttributeValue> {
    match value {
        stmt::Value::String(s) => Ok(AttributeValue::S(s.clone())),
        stmt::Value::I64(i) => Ok(AttributeValue::N(i.to_string())),
        stmt::Value::U64(u) => Ok(AttributeValue::N(u.to_string())),
        stmt::Value::I32(i) => Ok(AttributeValue::N(i.to_string())),
        stmt::Value::I16(i) => Ok(AttributeValue::N(i.to_string())),
        stmt::Value::I8(i) => Ok(AttributeValue::N(i.to_string())),
        stmt::Value::U32(u) => Ok(AttributeValue::N(u.to_string())),
        stmt::Value::U16(u) => Ok(AttributeValue::N(u.to_string())),
        stmt::Value::U8(u) => Ok(AttributeValue::N(u.to_string())),
        stmt::Value::Bool(b) => Ok(AttributeValue::Bool(*b)),
        stmt::Value::Id(id) => Ok(AttributeValue::S(id.to_string())),
        stmt::Value::Null => Ok(AttributeValue::Null(true)),
        _ => todo!("Unsupported stmt::Value type for DynamoDB: {value:?}"),
    }
}

fn dynamodb_attr_to_stmt_value(attr: &AttributeValue) -> Result<stmt::Value> {
    match attr {
        AttributeValue::S(s) => Ok(stmt::Value::String(s.clone())),
        AttributeValue::N(n) => {
            // DynamoDB stores all numbers as strings, so we return as String
            // and let the TryFrom implementation handle the parsing
            Ok(stmt::Value::String(n.clone()))
        }
        AttributeValue::B(b) => Ok(stmt::Value::Bytes(b.clone().into_inner())),
        AttributeValue::Bool(b) => Ok(stmt::Value::Bool(*b)),
        AttributeValue::Null(_) => Ok(stmt::Value::Null),
        _ => todo!("Unsupported DynamoDB AttributeValue type: {attr:?}"),
    }
}
