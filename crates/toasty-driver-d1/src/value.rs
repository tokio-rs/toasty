use toasty_core::stmt::Value;

#[cfg(target_arch = "wasm32")]
use toasty_core::stmt;

pub(crate) const MIN_SAFE_INTEGER: i64 = -9_007_199_254_740_991;
pub(crate) const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
pub(crate) const MAX_SAFE_UNSIGNED_INTEGER: u64 = MAX_SAFE_INTEGER as u64;
pub(crate) const MAX_VALUE_BYTES: usize = 2_000_000;
pub(crate) const MAX_BIND_PARAMETERS: usize = 100;
pub(crate) const MAX_PATTERN_BYTES: usize = 50;
pub(crate) const MAX_SQL_BYTES: usize = 100_000;

fn invalid_value(message: impl Into<String>) -> toasty_core::Error {
    toasty_core::Error::validation_failed(message)
}

pub(crate) fn validate_i64(value: i64) -> toasty_core::Result<()> {
    if (MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&value) {
        Ok(())
    } else {
        Err(invalid_value(format!(
            "D1 signed integer exceeds JavaScript's safe range ({MIN_SAFE_INTEGER}..={MAX_SAFE_INTEGER})"
        )))
    }
}

pub(crate) fn validate_u64(value: u64) -> toasty_core::Result<()> {
    if value <= MAX_SAFE_UNSIGNED_INTEGER {
        Ok(())
    } else {
        Err(invalid_value(format!(
            "D1 unsigned integer exceeds JavaScript's safe maximum ({MAX_SAFE_UNSIGNED_INTEGER})"
        )))
    }
}

pub(crate) fn validate_f64(value: f64, ty: &str) -> toasty_core::Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(invalid_value(format!("D1 {ty} values must be finite")))
    }
}

fn validate_bytes(len: usize, ty: &str) -> toasty_core::Result<()> {
    if len <= MAX_VALUE_BYTES {
        Ok(())
    } else {
        Err(invalid_value(format!(
            "D1 {ty} exceeds the {MAX_VALUE_BYTES}-byte value limit"
        )))
    }
}

pub(crate) fn validate_parameter_count(count: usize) -> toasty_core::Result<()> {
    if count <= MAX_BIND_PARAMETERS {
        Ok(())
    } else {
        Err(invalid_value(format!(
            "D1 statement has {count} bind parameters; maximum is {MAX_BIND_PARAMETERS}"
        )))
    }
}

pub(crate) fn validate_pattern(pattern: &str) -> toasty_core::Result<()> {
    if pattern.len() <= MAX_PATTERN_BYTES {
        Ok(())
    } else {
        Err(invalid_value(format!(
            "D1 LIKE/GLOB pattern is {} bytes; maximum is {MAX_PATTERN_BYTES}",
            pattern.len()
        )))
    }
}

pub(crate) fn validate_sql(sql: &str) -> toasty_core::Result<()> {
    if sql.len() <= MAX_SQL_BYTES {
        Ok(())
    } else {
        Err(invalid_value(format!(
            "D1 SQL statement is {} bytes; maximum is {MAX_SQL_BYTES}",
            sql.len()
        )))
    }
}

fn text_value(value: &Value) -> toasty_core::Result<Option<String>> {
    Ok(match value {
        Value::List(_) | Value::Object(_) => Some(
            toasty_sql::json::to_string(value)
                .map_err(|error| invalid_value(format!("D1 JSON serialization failed: {error}")))?,
        ),
        #[cfg(feature = "rust_decimal")]
        Value::Decimal(value) => Some(value.to_string()),
        #[cfg(feature = "bigdecimal")]
        Value::BigDecimal(value) => Some(value.to_string()),
        #[cfg(feature = "jiff")]
        Value::Timestamp(value) => Some(value.to_string()),
        #[cfg(feature = "jiff")]
        Value::Zoned(value) => Some(value.to_string()),
        #[cfg(feature = "jiff")]
        Value::Date(value) => Some(value.to_string()),
        #[cfg(feature = "jiff")]
        Value::Time(value) => Some(value.to_string()),
        #[cfg(feature = "jiff")]
        Value::DateTime(value) => Some(value.to_string()),
        _ => None,
    })
}

pub(crate) fn validate(value: &Value) -> toasty_core::Result<()> {
    match value {
        Value::I8(value) => validate_i64(i64::from(*value)),
        Value::I16(value) => validate_i64(i64::from(*value)),
        Value::I32(value) => validate_i64(i64::from(*value)),
        Value::I64(value) => validate_i64(*value),
        Value::U8(value) => validate_u64(u64::from(*value)),
        Value::U16(value) => validate_u64(u64::from(*value)),
        Value::U32(value) => validate_u64(u64::from(*value)),
        Value::U64(value) => validate_u64(*value),
        Value::F32(value) => validate_f64(f64::from(*value), "f32"),
        Value::F64(value) => validate_f64(*value, "f64"),
        Value::String(value) => validate_bytes(value.len(), "string"),
        Value::Bytes(value) => validate_bytes(value.len(), "BLOB"),
        Value::Uuid(value) => {
            let text = value.hyphenated().to_string();
            if text.len() == 36 {
                Ok(())
            } else {
                Err(invalid_value("D1 UUID is not in canonical text form"))
            }
        }
        Value::Null | Value::Bool(_) => Ok(()),
        value if text_value(value)?.is_some() => {
            let text = text_value(value)?.expect("checked above");
            validate_bytes(text.len(), "text-encoded value")
        }
        value => Err(invalid_value(format!(
            "D1 cannot bind Toasty value type {:?}",
            value.infer_ty()
        ))),
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm {
    use super::*;
    use worker::{
        js_sys::{Array, Uint8Array},
        wasm_bindgen::{JsCast, JsValue},
    };

    pub(crate) fn bind(value: Value) -> toasty_core::Result<JsValue> {
        validate(&value)?;

        Ok(match value {
            Value::Null => JsValue::null(),
            Value::Bool(value) => JsValue::from_bool(value),
            Value::I8(value) => JsValue::from_f64(f64::from(value)),
            Value::I16(value) => JsValue::from_f64(f64::from(value)),
            Value::I32(value) => JsValue::from_f64(f64::from(value)),
            Value::I64(value) => JsValue::from_f64(value as f64),
            Value::U8(value) => JsValue::from_f64(f64::from(value)),
            Value::U16(value) => JsValue::from_f64(f64::from(value)),
            Value::U32(value) => JsValue::from_f64(f64::from(value)),
            Value::U64(value) => JsValue::from_f64(value as f64),
            Value::F32(value) => JsValue::from_f64(f64::from(value)),
            Value::F64(value) => JsValue::from_f64(value),
            Value::String(value) => JsValue::from_str(&value),
            Value::Bytes(value) => Uint8Array::from(value.as_slice()).into(),
            Value::Uuid(value) => JsValue::from_str(&value.hyphenated().to_string()),
            value => JsValue::from_str(
                &text_value(&value)?
                    .ok_or_else(|| invalid_value("D1 value has no supported text encoding"))?,
            ),
        })
    }

    pub(crate) fn decode_typed(
        value: JsValue,
        ty: &stmt::Type,
        index: usize,
    ) -> toasty_core::Result<Value> {
        if value.is_null() || value.is_undefined() {
            return Ok(Value::Null);
        }

        let invalid = || {
            toasty_core::Error::invalid_result(format!(
                "D1 column {index} expected {ty:?}, received {}",
                category(&value)
            ))
        };

        match ty {
            stmt::Type::Bool => {
                if let Some(value) = value.as_bool() {
                    return Ok(Value::Bool(value));
                }
                match value.as_f64() {
                    Some(0.0) => Ok(Value::Bool(false)),
                    Some(1.0) => Ok(Value::Bool(true)),
                    _ => Err(invalid()),
                }
            }
            stmt::Type::I8 => integer(&value, index)
                .and_then(|value| i8::try_from(value).map(Value::I8).map_err(|_| invalid())),
            stmt::Type::I16 => integer(&value, index)
                .and_then(|value| i16::try_from(value).map(Value::I16).map_err(|_| invalid())),
            stmt::Type::I32 => integer(&value, index)
                .and_then(|value| i32::try_from(value).map(Value::I32).map_err(|_| invalid())),
            stmt::Type::I64 => integer(&value, index).map(Value::I64),
            stmt::Type::U8 => integer(&value, index)
                .and_then(|value| u8::try_from(value).map(Value::U8).map_err(|_| invalid())),
            stmt::Type::U16 => integer(&value, index)
                .and_then(|value| u16::try_from(value).map(Value::U16).map_err(|_| invalid())),
            stmt::Type::U32 => integer(&value, index)
                .and_then(|value| u32::try_from(value).map(Value::U32).map_err(|_| invalid())),
            stmt::Type::U64 => integer(&value, index)
                .and_then(|value| u64::try_from(value).map(Value::U64).map_err(|_| invalid())),
            stmt::Type::F32 => finite_number(&value, index).map(|value| Value::F32(value as f32)),
            stmt::Type::F64 => finite_number(&value, index).map(Value::F64),
            stmt::Type::String => value.as_string().map(Value::String).ok_or_else(invalid),
            stmt::Type::Uuid => value
                .as_string()
                .ok_or_else(&invalid)?
                .parse()
                .map(Value::Uuid)
                .map_err(|_| invalid()),
            stmt::Type::Bytes => decode_bytes(value, index).map(Value::Bytes),
            stmt::Type::List(elem) => {
                let text = value.as_string().ok_or_else(&invalid)?;
                toasty_sql::json::list_from_str(&text, elem).map_err(|error| {
                    toasty_core::Error::invalid_result(format!(
                        "D1 column {index} contains invalid JSON: {error}"
                    ))
                })
            }
            stmt::Type::Object => {
                let text = value.as_string().ok_or_else(&invalid)?;
                toasty_sql::json::from_str(&text, ty).map_err(|error| {
                    toasty_core::Error::invalid_result(format!(
                        "D1 column {index} contains invalid JSON: {error}"
                    ))
                })
            }
            #[cfg(feature = "rust_decimal")]
            stmt::Type::Decimal => parse_text(value, index, "Decimal").map(Value::Decimal),
            #[cfg(feature = "bigdecimal")]
            stmt::Type::BigDecimal => parse_text(value, index, "BigDecimal").map(Value::BigDecimal),
            #[cfg(feature = "jiff")]
            stmt::Type::Timestamp => parse_text(value, index, "Timestamp").map(Value::Timestamp),
            #[cfg(feature = "jiff")]
            stmt::Type::Zoned => parse_text(value, index, "Zoned").map(Value::Zoned),
            #[cfg(feature = "jiff")]
            stmt::Type::Date => parse_text(value, index, "Date").map(Value::Date),
            #[cfg(feature = "jiff")]
            stmt::Type::Time => parse_text(value, index, "Time").map(Value::Time),
            #[cfg(feature = "jiff")]
            stmt::Type::DateTime => parse_text(value, index, "DateTime").map(Value::DateTime),
            _ => Err(invalid()),
        }
    }

    pub(crate) fn decode_infer(value: JsValue, index: usize) -> toasty_core::Result<Value> {
        if value.is_null() || value.is_undefined() {
            return Ok(Value::Null);
        }
        if let Some(value) = value.as_string() {
            return Ok(Value::String(value));
        }
        if let Some(value) = value.as_bool() {
            return Ok(Value::Bool(value));
        }
        if value.is_instance_of::<Uint8Array>() || Array::is_array(&value) {
            return decode_bytes(value, index).map(Value::Bytes);
        }
        if let Some(value) = value.as_f64() {
            validate_f64(value, "raw numeric result").map_err(|_| {
                toasty_core::Error::invalid_result(format!(
                    "D1 column {index} contains a non-finite number"
                ))
            })?;
            if value.fract() == 0.0
                && value >= MIN_SAFE_INTEGER as f64
                && value <= MAX_SAFE_INTEGER as f64
            {
                return Ok(Value::I64(value as i64));
            }
            return Ok(Value::F64(value));
        }

        Err(toasty_core::Error::invalid_result(format!(
            "D1 column {index} has unsupported raw value category {}",
            category(&value)
        )))
    }

    pub(crate) fn row(value: JsValue) -> toasty_core::Result<Array> {
        value.dyn_into::<Array>().map_err(|value| {
            toasty_core::Error::invalid_result(format!(
                "D1 raw query returned {}, expected an array row",
                category(&value)
            ))
        })
    }

    fn finite_number(value: &JsValue, index: usize) -> toasty_core::Result<f64> {
        let value = value.as_f64().ok_or_else(|| {
            toasty_core::Error::invalid_result(format!(
                "D1 column {index} expected a number, received {}",
                category(value)
            ))
        })?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(toasty_core::Error::invalid_result(format!(
                "D1 column {index} contains a non-finite number"
            )))
        }
    }

    fn integer(value: &JsValue, index: usize) -> toasty_core::Result<i64> {
        let value = finite_number(value, index)?;
        if value.fract() != 0.0
            || value < MIN_SAFE_INTEGER as f64
            || value > MAX_SAFE_INTEGER as f64
        {
            return Err(toasty_core::Error::invalid_result(format!(
                "D1 column {index} expected a safe integral number"
            )));
        }
        Ok(value as i64)
    }

    fn decode_bytes(value: JsValue, index: usize) -> toasty_core::Result<Vec<u8>> {
        if value.is_instance_of::<Uint8Array>() {
            return Ok(Uint8Array::new(&value).to_vec());
        }
        if Array::is_array(&value) {
            let array = Array::from(&value);
            let mut bytes = Vec::with_capacity(array.length() as usize);
            for item in array.iter() {
                let number = item.as_f64().ok_or_else(|| {
                    toasty_core::Error::invalid_result(format!(
                        "D1 column {index} BLOB contains a non-number"
                    ))
                })?;
                if number.fract() != 0.0 || !(0.0..=255.0).contains(&number) {
                    return Err(toasty_core::Error::invalid_result(format!(
                        "D1 column {index} BLOB contains a byte outside 0..=255"
                    )));
                }
                bytes.push(number as u8);
            }
            return Ok(bytes);
        }
        Err(toasty_core::Error::invalid_result(format!(
            "D1 column {index} expected a BLOB, received {}",
            category(&value)
        )))
    }

    #[cfg(any(feature = "rust_decimal", feature = "bigdecimal", feature = "jiff"))]
    fn parse_text<T: std::str::FromStr>(
        value: JsValue,
        index: usize,
        ty: &str,
    ) -> toasty_core::Result<T> {
        value
            .as_string()
            .ok_or_else(|| {
                toasty_core::Error::invalid_result(format!(
                    "D1 column {index} expected {ty} text, received {}",
                    category(&value)
                ))
            })?
            .parse()
            .map_err(|_| {
                toasty_core::Error::invalid_result(format!(
                    "D1 column {index} contains invalid {ty} text"
                ))
            })
    }

    fn category(value: &JsValue) -> &'static str {
        if value.is_null() {
            "null"
        } else if value.is_undefined() {
            "undefined"
        } else if value.is_string() {
            "string"
        } else if value.as_bool().is_some() {
            "boolean"
        } else if value.as_f64().is_some() {
            "number"
        } else if value.is_instance_of::<Uint8Array>() {
            "Uint8Array"
        } else if Array::is_array(value) {
            "array"
        } else {
            "object"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_javascript_safe_integer_bounds() {
        assert!(validate_i64(MIN_SAFE_INTEGER).is_ok());
        assert!(validate_i64(MAX_SAFE_INTEGER).is_ok());
        assert!(validate_i64(MIN_SAFE_INTEGER - 1).is_err());
        assert!(validate_i64(MAX_SAFE_INTEGER + 1).is_err());
        assert!(validate_u64(MAX_SAFE_UNSIGNED_INTEGER).is_ok());
        assert!(validate_u64(MAX_SAFE_UNSIGNED_INTEGER + 1).is_err());
    }

    #[test]
    fn validates_every_integer_width() {
        assert!(validate(&Value::I8(i8::MIN)).is_ok());
        assert!(validate(&Value::I16(i16::MIN)).is_ok());
        assert!(validate(&Value::I32(i32::MIN)).is_ok());
        assert!(validate(&Value::I64(MAX_SAFE_INTEGER)).is_ok());
        assert!(validate(&Value::U8(u8::MAX)).is_ok());
        assert!(validate(&Value::U16(u16::MAX)).is_ok());
        assert!(validate(&Value::U32(u32::MAX)).is_ok());
        assert!(validate(&Value::U64(MAX_SAFE_UNSIGNED_INTEGER)).is_ok());
    }

    #[test]
    fn rejects_non_finite_floats() {
        assert!(validate(&Value::F32(1.5)).is_ok());
        assert!(validate(&Value::F64(-2.5)).is_ok());
        assert!(validate(&Value::F32(f32::NAN)).is_err());
        assert!(validate(&Value::F64(f64::INFINITY)).is_err());
        assert!(validate(&Value::F64(f64::NEG_INFINITY)).is_err());
    }

    #[test]
    fn validates_string_and_blob_limits() {
        assert!(validate(&Value::String(String::new())).is_ok());
        assert!(validate(&Value::String("ok".into())).is_ok());
        assert!(validate(&Value::String("x".repeat(MAX_VALUE_BYTES))).is_ok());
        assert!(validate(&Value::String("x".repeat(MAX_VALUE_BYTES + 1))).is_err());
        assert!(validate(&Value::Bytes(vec![0; MAX_VALUE_BYTES])).is_ok());
        assert!(validate(&Value::Bytes(vec![0; MAX_VALUE_BYTES + 1])).is_err());
    }

    #[test]
    fn validates_statement_and_pattern_limits() {
        assert!(validate_parameter_count(MAX_BIND_PARAMETERS).is_ok());
        assert!(validate_parameter_count(MAX_BIND_PARAMETERS + 1).is_err());
        assert!(validate_pattern(&"x".repeat(MAX_PATTERN_BYTES)).is_ok());
        assert!(validate_pattern(&"x".repeat(MAX_PATTERN_BYTES + 1)).is_err());
        assert!(validate_sql(&"x".repeat(MAX_SQL_BYTES)).is_ok());
        assert!(validate_sql(&"x".repeat(MAX_SQL_BYTES + 1)).is_err());
    }

    #[test]
    fn validates_uuid_and_json_text() {
        let uuid = "67e55044-10b1-426f-9247-bb680e5fe0c8".parse().unwrap();
        assert!(validate(&Value::Uuid(uuid)).is_ok());
        assert!(validate(&Value::List(vec![Value::I64(1)])).is_ok());
    }
}
