use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;
use toasty_core::{
    schema::app,
    stmt::{self, Value as CoreValue},
};

#[derive(Debug)]
pub struct Value(CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
    }
}

impl Value {
    /// Converts this DynamoDB driver value into the core Toasty value.
    pub fn into_inner(self) -> CoreValue {
        self.0
    }

    /// Converts a DynamoDB AttributeValue to a Toasty value.
    ///
    /// Decoding is type-directed: `ty` (the column's engine type) determines
    /// how each attribute is interpreted. A `#[document]` column
    /// (`Type::Model`) decodes a Map `M` attribute to the positional
    /// `Value::Record` the engine consumes, resolving field names and order
    /// from the embed's schema — the DynamoDB analogue of the SQL drivers'
    /// type-directed JSON decoding.
    pub fn from_ddb(app: &app::Schema, ty: &stmt::Type, val: &AttributeValue) -> Self {
        use AttributeValue as AV;
        use stmt::Type;

        let core_value = match (ty, val) {
            (Type::Bool, AV::Bool(val)) => stmt::Value::from(*val),
            (Type::String, AV::S(val)) => stmt::Value::from(val.clone()),
            (Type::I8, AV::N(val)) => stmt::Value::from(val.parse::<i8>().unwrap()),
            (Type::I16, AV::N(val)) => stmt::Value::from(val.parse::<i16>().unwrap()),
            (Type::I32, AV::N(val)) => stmt::Value::from(val.parse::<i32>().unwrap()),
            (Type::I64, AV::N(val)) => stmt::Value::from(val.parse::<i64>().unwrap()),
            (Type::U8, AV::N(val)) => stmt::Value::from(val.parse::<u8>().unwrap()),
            (Type::U16, AV::N(val)) => stmt::Value::from(val.parse::<u16>().unwrap()),
            (Type::U32, AV::N(val)) => stmt::Value::from(val.parse::<u32>().unwrap()),
            (Type::U64, AV::N(val)) => stmt::Value::from(val.parse::<u64>().unwrap()),
            (Type::F32, AV::N(val)) => stmt::Value::from(val.parse::<f32>().unwrap()),
            (Type::F64, AV::N(val)) => stmt::Value::from(val.parse::<f64>().unwrap()),
            (Type::Bytes, AV::B(val)) => stmt::Value::Bytes(val.clone().into_inner()),
            (Type::Uuid, AV::S(val)) => stmt::Value::from(val.parse::<uuid::Uuid>().unwrap()),
            (Type::List(elem), AV::L(items)) => {
                let items = items
                    .iter()
                    .map(|item| Value::from_ddb(app, elem, item).into_inner())
                    .collect();
                stmt::Value::List(items)
            }
            // A `#[document]` embed stored as a Map `M`: decode to the
            // positional record the engine consumes, in schema field order. A
            // key the writer omitted (an `Option` leaf holding `None`) decodes
            // to `Null`.
            (Type::Model(embed_id), AV::M(map)) => {
                stmt::Value::Record(stmt::ValueRecord::from_vec(
                    app.document_fields(*embed_id)
                        .map(|(name, field_ty)| match map.get(name) {
                            Some(attr) => Value::from_ddb(app, field_ty, attr).into_inner(),
                            None => stmt::Value::Null,
                        })
                        .collect(),
                ))
            }
            (_, AV::Null(_)) => stmt::Value::Null,
            // Scalars whose DynamoDB representation is their string form
            // (temporals, decimals): recover the typed value through the same
            // `Type::cast` conversions the engine uses for scalar columns.
            (ty, AV::S(val)) => ty
                .cast(stmt::Value::String(val.clone()))
                .expect("string attribute does not convert to the column type"),
            _ => todo!("ty={:#?}; value={:#?}", ty, val),
        };

        Value(core_value)
    }

    /// Converts a Toasty value to a DynamoDB AttributeValue, directed by the
    /// value's engine type.
    ///
    /// The type only matters for `#[document]` columns: a `Type::Model` embed
    /// carries a positional `Value::Record` whose field names live in the
    /// schema, so encoding a Map `M` needs the embed's field list. Everything
    /// else defers to the shape-directed [`to_ddb`](Self::to_ddb).
    pub fn to_ddb_typed(app: &app::Schema, ty: &stmt::Type, value: &stmt::Value) -> AttributeValue {
        use AttributeValue as AV;
        use stmt::Type;

        match (ty, value) {
            // A `#[document]` embed: encode the positional record as a Map
            // keyed by the embed's field names. `Null` fields (an `Option`
            // leaf holding `None`) are omitted, matching the SQL drivers'
            // JSON encoding; they decode back from the missing key.
            (Type::Model(embed_id), stmt::Value::Record(record)) => {
                let mut map = HashMap::new();
                for ((name, field_ty), field) in app.document_fields(*embed_id).zip(record.iter()) {
                    if !field.is_null() {
                        map.insert(name.to_owned(), Self::to_ddb_typed(app, field_ty, field));
                    }
                }
                AV::M(map)
            }
            (Type::List(elem), stmt::Value::List(items)) => AV::L(
                items
                    .iter()
                    .map(|item| Self::to_ddb_typed(app, elem, item))
                    .collect(),
            ),
            (_, value) => Value(value.clone()).to_ddb(),
        }
    }

    /// Converts this value to a DynamoDB AttributeValue.
    pub fn to_ddb(&self) -> AttributeValue {
        use AttributeValue as AV;

        match &self.0 {
            stmt::Value::Bool(val) => AV::Bool(*val),
            stmt::Value::String(val) => AV::S(val.to_string()),
            stmt::Value::I8(val) => AV::N(val.to_string()),
            stmt::Value::I16(val) => AV::N(val.to_string()),
            stmt::Value::I32(val) => AV::N(val.to_string()),
            stmt::Value::I64(val) => AV::N(val.to_string()),
            stmt::Value::U8(val) => AV::N(val.to_string()),
            stmt::Value::U16(val) => AV::N(val.to_string()),
            stmt::Value::U32(val) => AV::N(val.to_string()),
            stmt::Value::U64(val) => AV::N(val.to_string()),
            stmt::Value::F32(val) => AV::N(val.to_string()),
            stmt::Value::F64(val) => AV::N(val.to_string()),
            stmt::Value::Bytes(val) => AV::B(val.clone().into()),
            stmt::Value::Uuid(val) => AV::S(val.to_string()),
            stmt::Value::List(vals) => {
                let items = vals
                    .iter()
                    .map(|val| Value(val.clone()).to_ddb())
                    .collect::<Vec<_>>();
                AV::L(items)
            }
            stmt::Value::Null => AV::Null(true),
            // Scalars whose DynamoDB representation is their string form
            // (temporals, decimals): the same `Type::cast` conversions the
            // engine applies to scalar columns of these types. The fixed
            // sub-second precision keeps lexicographic string order
            // chronological, so range filters on these attributes compare
            // correctly.
            value => match stmt::Type::String.cast(value.clone()) {
                Ok(stmt::Value::String(s)) => AV::S(s),
                _ => todo!("{:#?}", self.0),
            },
        }
    }
}
