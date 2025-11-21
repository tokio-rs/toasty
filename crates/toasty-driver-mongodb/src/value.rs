use bson::Bson;
use toasty_core::stmt;

pub fn to_bson(value: &stmt::Value) -> Bson {
    match value {
        stmt::Value::Null => Bson::Null,
        stmt::Value::Bool(v) => Bson::Boolean(*v),
        stmt::Value::String(v) => Bson::String(v.to_string()),
        stmt::Value::I8(v) => Bson::Int32(*v as i32),
        stmt::Value::I16(v) => Bson::Int32(*v as i32),
        stmt::Value::I32(v) => Bson::Int32(*v),
        stmt::Value::I64(v) => Bson::Int64(*v),
        stmt::Value::U8(v) => Bson::Int32(*v as i32),
        stmt::Value::U16(v) => Bson::Int32(*v as i32),
        stmt::Value::U32(v) => Bson::Int64(*v as i64),
        stmt::Value::U64(v) => {
            if *v <= i64::MAX as u64 {
                Bson::Int64(*v as i64)
            } else {
                Bson::String(v.to_string())
            }
        }
        stmt::Value::Uuid(v) => Bson::String(v.to_string()),
        stmt::Value::Id(v) => {
            let id_str = v.to_string();
            if let Ok(oid) = bson::oid::ObjectId::parse_str(&id_str) {
                Bson::ObjectId(oid)
            } else {
                Bson::String(id_str)
            }
        }
        stmt::Value::Enum(v) => {
            let mut doc = bson::Document::new();
            doc.insert("variant", v.variant as i32);

            if !v.fields.is_empty() {
                let fields: Vec<Bson> = v.fields.iter().map(to_bson).collect();
                doc.insert("fields", fields);
            }

            Bson::Document(doc)
        }
        stmt::Value::Record(values) => {
            let fields: Vec<Bson> = values.iter().map(to_bson).collect();
            Bson::Array(fields)
        }
        stmt::Value::SparseRecord(_) => {
            todo!("SparseRecord to BSON conversion")
        }
        stmt::Value::List(_) => {
            todo!("List to BSON conversion")
        }
    }
}

pub fn from_bson(bson: &Bson, ty: &stmt::Type) -> stmt::Value {
    match (bson, ty) {
        (Bson::Null, _) => stmt::Value::Null,
        (Bson::Boolean(v), stmt::Type::Bool) => stmt::Value::Bool(*v),
        (Bson::String(v), stmt::Type::String) => stmt::Value::String(v.clone().into()),
        (Bson::String(v), stmt::Type::Id(model_id)) => {
            stmt::Value::Id(stmt::Id::from_string(*model_id, v.clone()))
        }
        (Bson::String(v), stmt::Type::Uuid) => {
            stmt::Value::Uuid(uuid::Uuid::parse_str(v).unwrap())
        }
        (Bson::ObjectId(oid), stmt::Type::Id(model_id)) => {
            stmt::Value::Id(stmt::Id::from_string(*model_id, oid.to_string()))
        }
        (Bson::Int32(v), stmt::Type::I8) => stmt::Value::I8(*v as i8),
        (Bson::Int32(v), stmt::Type::I16) => stmt::Value::I16(*v as i16),
        (Bson::Int32(v), stmt::Type::I32) => stmt::Value::I32(*v),
        (Bson::Int64(v), stmt::Type::I64) => stmt::Value::I64(*v),
        (Bson::Int32(v), stmt::Type::U8) => stmt::Value::U8(*v as u8),
        (Bson::Int32(v), stmt::Type::U16) => stmt::Value::U16(*v as u16),
        (Bson::Int64(v), stmt::Type::U32) => stmt::Value::U32(*v as u32),
        (Bson::Int64(v), stmt::Type::U64) => stmt::Value::U64(*v as u64),
        _ => todo!("from_bson conversion for {:?} to {:?}", bson, ty),
    }
}
