//! JSON encoding for `stmt::Value`s stored in document-backed columns
//! (MySQL `JSON`, SQLite TEXT via the JSON1 extension, and PostgreSQL `jsonb`
//! for `#[document]`-marked fields).
//!
//! The conversion does **not** go through an intermediate [`serde_json::Value`]
//! tree. Encoding streams a `stmt::Value` straight to JSON text via a
//! [`serde::Serialize`] wrapper ([`Encode`]); decoding parses JSON tokens
//! straight into a correctly-typed `stmt::Value` via a type-directed
//! [`serde::de::DeserializeSeed`] ([`Seed`]).
//!
//! The serde impls live on *local wrappers*, not on `stmt::Value` itself. The
//! encoding is opinionated (UUIDs / decimals / timestamps as JSON strings, to
//! match the per-column TEXT encoding the same scalar has at the SQL level) and
//! backends with typed document storage (BSON, DynamoDB) need different
//! representations — so `stmt::Value` is deliberately left without a canonical
//! serde representation. Decoding is *type-directed*: `Value::Uuid` vs
//! `Value::String` are both JSON strings on the wire, and `Value::I64` vs
//! `Value::U64` are both JSON numbers; only the caller's `stmt::Type`
//! distinguishes them, which is why decode carries the type as a seed rather
//! than a plain `Deserialize`.

use serde::de::{
    DeserializeSeed, Deserializer, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor,
};
use serde::ser::{Error as _, Serialize, SerializeMap, SerializeSeq, Serializer};
use std::fmt;
use toasty_core::stmt::{self, Value, ValueObject};

// ============================================================================
// Encoding: stmt::Value -> JSON text (no serde_json::Value intermediate)
// ============================================================================

/// A [`serde::Serialize`] wrapper that streams a `stmt::Value` as JSON.
///
/// Object entries whose value is [`Value::Null`] are omitted entirely — an
/// `Option::None` field produces a missing key, not an explicit `null`.
/// Non-finite floats (NaN / infinity, which have no JSON form) encode as
/// `null`. Shapes with no JSON representation (`Record`, `Bytes`,
/// `SparseRecord`) are a serialization error — document-stored values reach
/// the driver as `Object` / `List`, never `Record`.
pub struct Encode<'a>(pub &'a Value);

impl Serialize for Encode<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Null => s.serialize_unit(),
            Value::Bool(v) => s.serialize_bool(*v),
            Value::I8(v) => s.serialize_i8(*v),
            Value::I16(v) => s.serialize_i16(*v),
            Value::I32(v) => s.serialize_i32(*v),
            Value::I64(v) => s.serialize_i64(*v),
            Value::U8(v) => s.serialize_u8(*v),
            Value::U16(v) => s.serialize_u16(*v),
            Value::U32(v) => s.serialize_u32(*v),
            Value::U64(v) => s.serialize_u64(*v),
            // NaN / infinity have no JSON representation; encode as null.
            Value::F32(v) if v.is_finite() => s.serialize_f32(*v),
            Value::F32(_) => s.serialize_unit(),
            Value::F64(v) if v.is_finite() => s.serialize_f64(*v),
            Value::F64(_) => s.serialize_unit(),
            Value::String(v) => s.serialize_str(v),
            Value::Uuid(v) => s.collect_str(v),
            Value::List(items) => {
                let mut seq = s.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(&Encode(item))?;
                }
                seq.end()
            }
            Value::Object(object) => {
                let mut map = s.serialize_map(None)?;
                for (k, v) in object.iter() {
                    // `Option::None` -> omit the key entirely.
                    if v.is_null() {
                        continue;
                    }
                    map.serialize_entry(k, &Encode(v))?;
                }
                map.end()
            }
            #[cfg(feature = "rust_decimal")]
            Value::Decimal(v) => s.collect_str(v),
            #[cfg(feature = "bigdecimal")]
            Value::BigDecimal(v) => s.collect_str(v),
            // jiff temporal scalars store their ISO 8601 / RFC 3339 text form.
            // `Timestamp`, `Time`, and `DateTime` are truncated to microseconds
            // first: a `#[document]` leaf is read back through a SQL cast
            // (`(col->>'k')::timestamptz`, …) and the SQL temporal types only
            // hold microseconds, while jiff keeps nanoseconds. Truncating here —
            // matching the *truncating* native parameter binding the drivers use
            // — keeps the stored value and a bound comparison operand reducing to
            // the same instant, so an equality filter on a document leaf is exact
            // rather than off by a rounding step. `Date` has no sub-second part;
            // `Zoned` is rejected at schema-build (its RFC 9557 annotation has no
            // SQL cast), so it never reaches a document column.
            #[cfg(feature = "jiff")]
            Value::Timestamp(v) => s.collect_str(&trunc_timestamp_us(*v)),
            #[cfg(feature = "jiff")]
            Value::Zoned(v) => s.collect_str(v),
            #[cfg(feature = "jiff")]
            Value::Date(v) => s.collect_str(v),
            #[cfg(feature = "jiff")]
            Value::Time(v) => s.collect_str(&trunc_time_us(*v)),
            #[cfg(feature = "jiff")]
            Value::DateTime(v) => s.collect_str(&trunc_datetime_us(*v)),
            other => Err(S::Error::custom(format!("cannot encode {other:?} as JSON"))),
        }
    }
}

/// Truncate a timestamp to microsecond precision, toward zero, dropping any
/// sub-microsecond nanoseconds. Rounding can only fail at the extreme ends of
/// the representable range; fall back to the original value there rather than
/// failing the whole encode.
#[cfg(feature = "jiff")]
fn trunc_timestamp_us(v: jiff::Timestamp) -> jiff::Timestamp {
    v.round(
        jiff::TimestampRound::new()
            .smallest(jiff::Unit::Microsecond)
            .mode(jiff::RoundMode::Trunc),
    )
    .unwrap_or(v)
}

/// Truncate a civil time to microsecond precision, toward zero. See
/// [`trunc_timestamp_us`].
#[cfg(feature = "jiff")]
fn trunc_time_us(v: jiff::civil::Time) -> jiff::civil::Time {
    v.round(
        jiff::civil::TimeRound::new()
            .smallest(jiff::Unit::Microsecond)
            .mode(jiff::RoundMode::Trunc),
    )
    .unwrap_or(v)
}

/// Truncate a civil datetime to microsecond precision, toward zero. See
/// [`trunc_timestamp_us`].
#[cfg(feature = "jiff")]
fn trunc_datetime_us(v: jiff::civil::DateTime) -> jiff::civil::DateTime {
    v.round(
        jiff::civil::DateTimeRound::new()
            .smallest(jiff::Unit::Microsecond)
            .mode(jiff::RoundMode::Trunc),
    )
    .unwrap_or(v)
}

/// Encode a `stmt::Value` as a JSON string.
pub fn to_string(value: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string(&Encode(value))
}

/// Encode a `stmt::Value` as JSON UTF-8 bytes.
pub fn to_vec(value: &Value) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&Encode(value))
}

// ============================================================================
// Decoding: JSON text -> stmt::Value, directed by stmt::Type
// ============================================================================

/// A type-directed [`serde::de::DeserializeSeed`] that decodes JSON straight
/// into a `stmt::Value` of the seed's `stmt::Type` — no `serde_json::Value`
/// intermediate. The type is required because the wire form is ambiguous
/// (`Uuid`/`String` are both strings; the integer widths are all numbers; a
/// `Document`'s field names and types come from the schema).
pub struct Seed<'a>(pub &'a stmt::Type);

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, de: D) -> Result<Value, D::Error> {
        // JSON is self-describing, so `deserialize_any` lets the parser drive
        // the visit method by token; each method coerces using `self.0`.
        de.deserialize_any(ValueVisitor(self.0))
    }
}

struct ValueVisitor<'a>(&'a stmt::Type);

impl<'de> Visitor<'de> for ValueVisitor<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a JSON value decodable as {:?}", self.0)
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        // A JSON `null` — or an absent object key — is `Value::Null`,
        // regardless of the expected type (round-trips an `Option::None`).
        Ok(Value::Null)
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        match self.0 {
            stmt::Type::Bool => Ok(Value::Bool(v)),
            other => Err(E::custom(format!(
                "unexpected JSON bool for type {other:?}"
            ))),
        }
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        int_to_value(self.0, v as i128)
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        int_to_value(self.0, v as i128)
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        match self.0 {
            stmt::Type::F32 => Ok(Value::F32(v as f32)),
            stmt::Type::F64 => Ok(Value::F64(v)),
            other => Err(E::custom(format!(
                "unexpected JSON float for type {other:?}"
            ))),
        }
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
        match self.0 {
            stmt::Type::String => Ok(Value::String(v.to_owned())),
            stmt::Type::Uuid => Ok(Value::Uuid(v.parse().map_err(E::custom)?)),
            #[cfg(feature = "rust_decimal")]
            stmt::Type::Decimal => Ok(Value::Decimal(v.parse().map_err(E::custom)?)),
            #[cfg(feature = "bigdecimal")]
            stmt::Type::BigDecimal => Ok(Value::BigDecimal(v.parse().map_err(E::custom)?)),
            #[cfg(feature = "jiff")]
            stmt::Type::Timestamp => Ok(Value::Timestamp(v.parse().map_err(E::custom)?)),
            #[cfg(feature = "jiff")]
            stmt::Type::Zoned => Ok(Value::Zoned(v.parse().map_err(E::custom)?)),
            #[cfg(feature = "jiff")]
            stmt::Type::Date => Ok(Value::Date(v.parse().map_err(E::custom)?)),
            #[cfg(feature = "jiff")]
            stmt::Type::Time => Ok(Value::Time(v.parse().map_err(E::custom)?)),
            #[cfg(feature = "jiff")]
            stmt::Type::DateTime => Ok(Value::DateTime(v.parse().map_err(E::custom)?)),
            other => Err(E::custom(format!(
                "unexpected JSON string for type {other:?}"
            ))),
        }
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Value, A::Error> {
        match self.0 {
            stmt::Type::List(elem) => read_seq(seq, elem),
            other => Err(A::Error::custom(format!(
                "unexpected JSON array for type {other:?}"
            ))),
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let doc = match self.0 {
            stmt::Type::Document(doc) => doc,
            other => {
                return Err(A::Error::custom(format!(
                    "unexpected JSON object for type {other:?}"
                )));
            }
        };

        // Fill slots positionally as keys arrive (JSON key order is arbitrary);
        // unknown keys are ignored, absent keys stay `None` -> `Value::Null`.
        let mut slots: Vec<Option<Value>> = (0..doc.fields.len()).map(|_| None).collect();
        while let Some(key) = map.next_key::<String>()? {
            match doc.fields.iter().position(|f| f.name == key) {
                Some(idx) => slots[idx] = Some(map.next_value_seed(Seed(&doc.fields[idx].ty))?),
                None => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        let entries = doc
            .fields
            .iter()
            .zip(slots)
            .map(|(field, slot)| (field.name.clone(), slot.unwrap_or(Value::Null)))
            .collect();
        Ok(Value::Object(ValueObject::from_vec(entries)))
    }
}

/// Coerce a JSON integer (widened to `i128` so signed and unsigned tokens share
/// one path) into the integer or float `Value` named by `ty`. Casts are
/// truncating, matching how a scalar column reads the same wire value.
fn int_to_value<E: serde::de::Error>(ty: &stmt::Type, v: i128) -> Result<Value, E> {
    Ok(match ty {
        stmt::Type::I8 => Value::I8(v as i8),
        stmt::Type::I16 => Value::I16(v as i16),
        stmt::Type::I32 => Value::I32(v as i32),
        stmt::Type::I64 => Value::I64(v as i64),
        stmt::Type::U8 => Value::U8(v as u8),
        stmt::Type::U16 => Value::U16(v as u16),
        stmt::Type::U32 => Value::U32(v as u32),
        stmt::Type::U64 => Value::U64(v as u64),
        stmt::Type::F32 => Value::F32(v as f32),
        stmt::Type::F64 => Value::F64(v as f64),
        other => {
            return Err(E::custom(format!(
                "unexpected JSON integer for type {other:?}"
            )));
        }
    })
}

/// Decode a JSON array into a `Value::List`, seeding each element with
/// `elem_ty`. Shared by [`ValueVisitor::visit_seq`] and the list helpers.
fn read_seq<'de, A: SeqAccess<'de>>(mut seq: A, elem_ty: &stmt::Type) -> Result<Value, A::Error> {
    let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
    while let Some(value) = seq.next_element_seed(Seed(elem_ty))? {
        items.push(value);
    }
    Ok(Value::List(items))
}

/// A [`Visitor`] that accepts only a JSON array and decodes it into a
/// `Value::List` of `elem_ty` elements. Used by the list helpers, whose caller
/// holds the element type rather than a `Type::List`.
struct ListVisitor<'a>(&'a stmt::Type);

impl<'de> Visitor<'de> for ListVisitor<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a JSON array of {:?}", self.0)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Value, A::Error> {
        read_seq(seq, self.0)
    }
}

/// Decode a JSON document (string) into a `stmt::Value` of type `ty`.
pub fn from_str(text: &str, ty: &stmt::Type) -> Result<Value, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_str(text);
    let value = Seed(ty).deserialize(&mut de)?;
    de.end()?;
    Ok(value)
}

/// Decode a JSON document (UTF-8 bytes) into a `stmt::Value` of type `ty`.
pub fn from_slice(bytes: &[u8], ty: &stmt::Type) -> Result<Value, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let value = Seed(ty).deserialize(&mut de)?;
    de.end()?;
    Ok(value)
}

/// Decode a JSON array (string) into a `Value::List`, using `elem_ty` as the
/// per-element type.
pub fn list_from_str(text: &str, elem_ty: &stmt::Type) -> Result<Value, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_str(text);
    let value = de.deserialize_seq(ListVisitor(elem_ty))?;
    de.end()?;
    Ok(value)
}

/// Decode a JSON array (UTF-8 bytes) into a `Value::List`, using `elem_ty` as
/// the per-element type.
pub fn list_from_slice(bytes: &[u8], elem_ty: &stmt::Type) -> Result<Value, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let value = de.deserialize_seq(ListVisitor(elem_ty))?;
    de.end()?;
    Ok(value)
}
