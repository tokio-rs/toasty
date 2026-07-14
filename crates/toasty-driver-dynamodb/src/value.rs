use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;
use toasty_core::stmt::{self, Value as CoreValue};

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
    /// Decoding is type-directed for scalar columns: `ty` (the column's
    /// storage-level type) determines how each attribute is interpreted. A
    /// `#[document]` column (`Type::Object`) decodes shape-directed instead —
    /// a Map `M` attribute becomes the named `Value::Object` wire form
    /// ([`from_ddb_any`](Self::from_ddb_any)), which the engine raises to the
    /// embedded model's positional record. No schema is consulted here.
    pub fn from_ddb(ty: &stmt::Type, val: &AttributeValue) -> Self {
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
                    .map(|item| Value::from_ddb(elem, item).into_inner())
                    .collect();
                stmt::Value::List(items)
            }
            // A `#[document]` column stored as a Map `M`: decode
            // shape-directed to the named wire object. The engine raises it
            // to the embed's positional record — the field layout is a
            // model-level concept this driver does not know.
            (Type::Object, val @ AV::M(_)) => Self::from_ddb_any(val),
            (_, AV::Null(_)) => stmt::Value::Null,
            // Scalars whose DynamoDB representation is their string form
            // (temporals, decimals): recover the typed value through the same
            // `Type::cast` conversions the engine uses for scalar columns.
            (ty, AV::S(val)) => ty
                .cast(&(), stmt::Value::String(val.clone()))
                .expect("string attribute does not convert to the column type"),
            _ => todo!("ty={:#?}; value={:#?}", ty, val),
        };

        Value(core_value)
    }

    /// Converts a DynamoDB AttributeValue to a Toasty value shape-directed —
    /// the document-interior decode, with no type to consult. Every attribute
    /// takes its wire-natural form: strings stay `String`, numbers decode by
    /// integer fit (`I64`, then `U64`, then `F64` — mirroring the JSON codec),
    /// maps become named `Value::Object`s in stored key order. The engine
    /// casts the leaves to their field types when it raises the document.
    fn from_ddb_any(val: &AttributeValue) -> CoreValue {
        use AttributeValue as AV;

        match val {
            AV::Bool(val) => stmt::Value::Bool(*val),
            AV::S(val) => stmt::Value::String(val.clone()),
            AV::N(val) => {
                if let Ok(v) = val.parse::<i64>() {
                    stmt::Value::I64(v)
                } else if let Ok(v) = val.parse::<u64>() {
                    stmt::Value::U64(v)
                } else {
                    stmt::Value::F64(
                        val.parse::<f64>()
                            .expect("numeric attribute is not a number"),
                    )
                }
            }
            AV::B(val) => stmt::Value::Bytes(val.clone().into_inner()),
            AV::Null(_) => stmt::Value::Null,
            AV::L(items) => stmt::Value::List(items.iter().map(Self::from_ddb_any).collect()),
            AV::M(map) => stmt::Value::Object(stmt::ValueObject::from_vec(
                map.iter()
                    .map(|(key, val)| (key.clone(), Self::from_ddb_any(val)))
                    .collect(),
            )),
            _ => todo!("value={:#?}", val),
        }
    }

    /// Converts a document-interior value to a DynamoDB AttributeValue,
    /// mirroring the SQL drivers' JSON document encoding:
    ///
    /// - an object's `Null` entries (an `Option` leaf holding `None`) are
    ///   omitted; they decode back from the missing key.
    /// - temporal and decimal leaves store the shared document text form
    ///   ([`stmt::Value::document_storage_text`], fixed six-digit sub-second
    ///   precision) — the exact text the engine's document lowering binds as
    ///   a comparison operand, so a stored leaf and an operand cannot drift
    ///   apart.
    /// - everything else shares the column wire forms of
    ///   [`to_ddb`](Self::to_ddb).
    fn to_ddb_document(value: &CoreValue) -> AttributeValue {
        use AttributeValue as AV;

        match value {
            stmt::Value::Object(object) => {
                let mut map = HashMap::new();
                for (key, value) in object.iter() {
                    if !value.is_null() {
                        map.insert(key.clone(), Self::to_ddb_document(value));
                    }
                }
                AV::M(map)
            }
            stmt::Value::List(items) => AV::L(items.iter().map(Self::to_ddb_document).collect()),
            value => match value.document_storage_text() {
                Some(text) => AV::S(text.to_string()),
                None => Value(value.clone()).to_ddb(),
            },
        }
    }

    /// Converts a Toasty value to a DynamoDB AttributeValue, shape-directed.
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
            // A `#[document]` value, named by the engine's document lowering:
            // encode through the document leaf encoding
            // ([`to_ddb_document`](Self::to_ddb_document)).
            value @ stmt::Value::Object(_) => Self::to_ddb_document(value),
            stmt::Value::Null => AV::Null(true),
            // Scalars whose DynamoDB representation is their string form
            // (temporals, decimals): the same `Type::cast` conversions the
            // engine applies to scalar columns of these types. The fixed
            // sub-second precision keeps lexicographic string order
            // chronological, so range filters on these attributes compare
            // correctly.
            value => match stmt::Type::String.cast(&(), value.clone()) {
                Ok(stmt::Value::String(s)) => AV::S(s),
                _ => todo!("{:#?}", self.0),
            },
        }
    }
}
