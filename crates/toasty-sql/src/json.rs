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
//! serde representation. Decoding is *type-directed* for scalars: `Value::Uuid`
//! vs `Value::String` are both JSON strings on the wire, and `Value::I64` vs
//! `Value::U64` are both JSON numbers; only the caller's `stmt::Type`
//! distinguishes them, which is why decode carries the type as a seed rather
//! than a plain `Deserialize`.
//!
//! A `#[document]` column is typed by the structural `stmt::Type::Object` and
//! decodes *shape-directed*: a JSON object becomes a named `Value::Object` in
//! wire key order, and its interior leaves take their wire shapes (strings
//! stay strings, numbers decode by integer fit). The query engine — the only
//! party that knows which embedded model the column stores — raises the wire
//! object into a typed positional record; no schema is consulted here.

use serde::de::{DeserializeSeed, Deserializer, Error as _, MapAccess, SeqAccess, Visitor};
use serde::ser::{Error as _, Serialize, SerializeMap, SerializeSeq, Serializer};
use std::fmt;
use toasty_core::stmt::{self, Value};

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
            Value::Json(value) => EncodeJson(value).serialize(s),
            // Decimals and jiff temporal scalars store the shared document
            // text form ([`Value::document_storage_text`]): decimals as their
            // `Display` form, temporals as ISO 8601 / RFC 3339 text truncated
            // to microseconds (the precision the SQL temporal types hold) and
            // printed with fixed six-digit subsecond precision so
            // text-comparing backends (SQLite) order document leaves
            // chronologically. The engine's document lowering builds
            // comparison operands through the same method, so the stored form
            // and a bound operand cannot drift apart. `Zoned` is rejected at
            // schema-build (its RFC 9557 annotation has no SQL cast), so it
            // never reaches a document column.
            #[cfg(feature = "rust_decimal")]
            v @ Value::Decimal(_) => s.collect_str(
                &v.document_storage_text()
                    .expect("decimal value has a document text form"),
            ),
            #[cfg(feature = "bigdecimal")]
            v @ Value::BigDecimal(_) => s.collect_str(
                &v.document_storage_text()
                    .expect("decimal value has a document text form"),
            ),
            #[cfg(feature = "jiff")]
            v @ (Value::Timestamp(_) | Value::Date(_) | Value::Time(_) | Value::DateTime(_)) => {
                let text = v
                    .document_storage_text()
                    .expect("temporal value has a document text form");
                s.collect_str(&text)
            }
            #[cfg(feature = "jiff")]
            Value::Zoned(v) => s.collect_str(v),
            other => Err(S::Error::custom(format!("cannot encode {other:?} as JSON"))),
        }
    }
}

/// JSON values preserve explicit null object members, unlike typed document
/// objects where `Value::Null` represents an omitted `Option` field.
struct EncodeJson<'a>(&'a Value);

impl Serialize for EncodeJson<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Null => s.serialize_unit(),
            Value::Bool(v) => s.serialize_bool(*v),
            Value::I64(v) => s.serialize_i64(*v),
            Value::U64(v) => s.serialize_u64(*v),
            Value::F64(v) if v.is_finite() => s.serialize_f64(*v),
            Value::F64(_) => Err(S::Error::custom("non-finite JSON number")),
            Value::String(v) => s.serialize_str(v),
            Value::List(items) => {
                let mut seq = s.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(&EncodeJson(item))?;
                }
                seq.end()
            }
            Value::Object(object) => {
                let mut map = s.serialize_map(Some(object.entries.len()))?;
                for (key, value) in object.iter() {
                    map.serialize_entry(key, &EncodeJson(value))?;
                }
                map.end()
            }
            Value::Json(value) => EncodeJson(value).serialize(s),
            other => Err(S::Error::custom(format!(
                "cannot encode {other:?} as a dynamic JSON value"
            ))),
        }
    }
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
/// intermediate. The type is required because the wire form is ambiguous for
/// scalars (`Uuid`/`String` are both strings; the integer widths are all
/// numbers). A structural `Type::Object` position switches to shape-directed
/// decoding ([`AnySeed`]).
pub struct Seed<'a> {
    /// The expected type of the value being decoded.
    pub ty: &'a stmt::Type,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, de: D) -> Result<Value, D::Error> {
        if matches!(self.ty, stmt::Type::Json) {
            return de.deserialize_any(AnyVisitor { wrap_json: true });
        }

        // JSON is self-describing, so `deserialize_any` lets the parser drive
        // the visit method by token; each method coerces using `self.ty`.
        de.deserialize_any(ValueVisitor { ty: self.ty })
    }
}

struct ValueVisitor<'a> {
    ty: &'a stmt::Type,
}

/// A shape-directed [`serde::de::DeserializeSeed`] for the interior of a
/// document: every JSON token decodes to its wire-natural `stmt::Value` —
/// strings stay `String`, numbers decode by integer fit (`I64`, then `U64`,
/// then `F64`), objects become named `Value::Object`s in wire key order. The
/// engine casts the leaves to their field types when it raises the object to
/// a positional record.
struct AnySeed;

impl<'de> DeserializeSeed<'de> for AnySeed {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, de: D) -> Result<Value, D::Error> {
        de.deserialize_any(AnyVisitor { wrap_json: false })
    }
}

struct AnyVisitor {
    wrap_json: bool,
}

impl AnyVisitor {
    fn wrap(&self, value: Value) -> Value {
        if self.wrap_json {
            Value::Json(Box::new(value))
        } else {
            value
        }
    }
}

impl<'de> Visitor<'de> for AnyVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a JSON value inside a document")
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(self.wrap(Value::Null))
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(self.wrap(Value::Bool(v)))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(self.wrap(Value::I64(v)))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        // Integer fit: values representable as `i64` decode to `I64` so the
        // same stored number always has the same wire shape.
        Ok(self.wrap(match i64::try_from(v) {
            Ok(v) => Value::I64(v),
            Err(_) => Value::U64(v),
        }))
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        Ok(self.wrap(Value::F64(v)))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(self.wrap(Value::String(v.to_owned())))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(value) = seq.next_element_seed(AnySeed)? {
            items.push(value);
        }
        Ok(self.wrap(Value::List(items)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut entries = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            entries.push((key, map.next_value_seed(AnySeed)?));
        }
        Ok(self.wrap(Value::Object(stmt::ValueObject::from_vec(entries))))
    }
}

impl<'de> Visitor<'de> for ValueVisitor<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a JSON value decodable as {:?}", self.ty)
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        // A JSON `null` — or an absent object key — is `Value::Null`,
        // regardless of the expected type (round-trips an `Option::None`).
        Ok(Value::Null)
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        match self.ty {
            stmt::Type::Bool => Ok(Value::Bool(v)),
            other => Err(E::custom(format!(
                "unexpected JSON bool for type {other:?}"
            ))),
        }
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        int_to_value(self.ty, v as i128)
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        int_to_value(self.ty, v as i128)
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        match self.ty {
            stmt::Type::F32 => Ok(Value::F32(v as f32)),
            stmt::Type::F64 => Ok(Value::F64(v)),
            other => Err(E::custom(format!(
                "unexpected JSON float for type {other:?}"
            ))),
        }
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
        match self.ty {
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
        match self.ty {
            stmt::Type::List(elem) => read_seq(seq, elem),
            other => Err(A::Error::custom(format!(
                "unexpected JSON array for type {other:?}"
            ))),
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Value, A::Error> {
        // A `#[document]` column is typed by the structural `Type::Object`:
        // decode the JSON object shape-directed, keys as stored. The engine
        // raises the result to the embed's positional record — the field
        // layout is a model-level concept this codec does not know.
        match self.ty {
            stmt::Type::Object => AnyVisitor { wrap_json: false }.visit_map(map),
            other => Err(A::Error::custom(format!(
                "unexpected JSON object for type {other:?}"
            ))),
        }
    }
}

/// Coerce a JSON integer (widened to `i128` so signed and unsigned tokens share
/// one path) into the integer or float `Value` named by `ty`. Integer
/// conversions are checked: a stored value outside the target type's range
/// (e.g. `-1` decoded as `u64`) is a decode error naming the mismatch, not a
/// silent wraparound.
fn int_to_value<E: serde::de::Error>(ty: &stmt::Type, v: i128) -> Result<Value, E> {
    fn checked<T: TryFrom<i128>, E: serde::de::Error>(v: i128, ty: &stmt::Type) -> Result<T, E> {
        T::try_from(v)
            .map_err(|_| E::custom(format!("JSON integer {v} is out of range for type {ty:?}")))
    }

    Ok(match ty {
        stmt::Type::I8 => Value::I8(checked(v, ty)?),
        stmt::Type::I16 => Value::I16(checked(v, ty)?),
        stmt::Type::I32 => Value::I32(checked(v, ty)?),
        stmt::Type::I64 => Value::I64(checked(v, ty)?),
        stmt::Type::U8 => Value::U8(checked(v, ty)?),
        stmt::Type::U16 => Value::U16(checked(v, ty)?),
        stmt::Type::U32 => Value::U32(checked(v, ty)?),
        stmt::Type::U64 => Value::U64(checked(v, ty)?),
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
    while let Some(value) = seq.next_element_seed(Seed { ty: elem_ty })? {
        items.push(value);
    }
    Ok(Value::List(items))
}

/// A [`Visitor`] that accepts only a JSON array and decodes it into a
/// `Value::List` of `elem_ty` elements. Used by the list helpers, whose caller
/// holds the element type rather than a `Type::List`.
struct ListVisitor<'a> {
    elem_ty: &'a stmt::Type,
}

impl<'de> Visitor<'de> for ListVisitor<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a JSON array of {:?}", self.elem_ty)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Value, A::Error> {
        read_seq(seq, self.elem_ty)
    }
}

/// Decode a JSON document (string) into a `stmt::Value` of type `ty`.
pub fn from_str(text: &str, ty: &stmt::Type) -> Result<Value, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_str(text);
    let value = Seed { ty }.deserialize(&mut de)?;
    de.end()?;
    Ok(value)
}

/// Decode a JSON document (UTF-8 bytes) into a `stmt::Value` of type `ty`.
pub fn from_slice(bytes: &[u8], ty: &stmt::Type) -> Result<Value, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let value = Seed { ty }.deserialize(&mut de)?;
    de.end()?;
    Ok(value)
}

/// Decode a JSON array (string) into a `Value::List`, using `elem_ty` as the
/// per-element type.
pub fn list_from_str(text: &str, elem_ty: &stmt::Type) -> Result<Value, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_str(text);
    let value = de.deserialize_seq(ListVisitor { elem_ty })?;
    de.end()?;
    Ok(value)
}

/// Decode a JSON array (UTF-8 bytes) into a `Value::List`, using `elem_ty` as
/// the per-element type.
pub fn list_from_slice(bytes: &[u8], elem_ty: &stmt::Type) -> Result<Value, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let value = de.deserialize_seq(ListVisitor { elem_ty })?;
    de.end()?;
    Ok(value)
}
