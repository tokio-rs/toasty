use crate::stmt;

/// A structured representation of the encoding between a model field and its
/// database column(s).
///
/// Every `Bijection` is a lossless, invertible transformation: model values can
/// be encoded to storage values and decoded back without loss. The framework
/// guarantees bijectivity by construction — complex mappings are built from
/// known-bijective primitives composed via rules that preserve bijectivity.
///
/// In addition to encode/decode, each bijection can answer whether a given
/// binary operator can be "pushed through" the encoding (operator
/// homomorphism). This determines whether filters and ordering can be evaluated
/// in storage space (efficient, index-friendly) or must fall back to
/// model-space evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum Bijection {
    /// No transformation — field type equals column type.
    Identity,

    /// Lossless cast between two types with the same information content.
    ///
    /// Examples: UUID ↔ String, Timestamp ↔ String (ISO 8601), integer
    /// widening/narrowing.
    Cast {
        /// The model-level (source) type.
        from: stmt::Type,
        /// The storage-level (target) type.
        to: stmt::Type,
    },

    /// Affine transformation: `x * k + c` (k ≠ 0). Inverse: `(x - c) / k`.
    ///
    /// Not yet used by the schema builder, but included for future computed
    /// fields (e.g., epoch-seconds timestamps with offset).
    #[allow(dead_code)]
    Affine {
        /// Multiplicative factor (must be non-zero).
        k: stmt::Value,
        /// Additive constant.
        c: stmt::Value,
    },

    /// `Option<T>` → nullable column. Wraps an inner bijection with
    /// `None ↔ NULL`.
    Nullable(Box<Bijection>),

    /// Embedded struct → multiple columns. Each component is an independent
    /// bijection on one field ↔ column pair.
    Product(Vec<Bijection>),

    /// Enum → discriminant column + per-variant columns.
    Coproduct {
        /// Bijection for the discriminant column.
        discriminant: Box<Bijection>,
        /// One arm per variant, in declaration order.
        variants: Vec<CoproductArm>,
    },

    /// Sequential composition: apply `inner` first, then `outer`.
    ///
    /// - encode = outer.encode(inner.encode(x))
    /// - decode = inner.decode(outer.decode(x))
    #[allow(dead_code)]
    Compose {
        inner: Box<Bijection>,
        outer: Box<Bijection>,
    },
}

/// One arm of a [`Bijection::Coproduct`].
#[derive(Debug, Clone, PartialEq)]
pub struct CoproductArm {
    /// The discriminant value that selects this arm.
    pub discriminant_value: i64,
    /// Bijection for this variant's fields (typically `Product` for
    /// data-carrying variants, `Identity` for unit variants with no extra
    /// data).
    pub body: Bijection,
}

/// The result of [`Bijection::can_distribute`]: a storage-level operator that
/// preserves the semantics of the model-level operator through the encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageOp {
    /// Standard equality (`=` in SQL).
    Eq,
    /// Standard inequality (`!=` / `<>` in SQL).
    Ne,
    /// Less than (`<`).
    Lt,
    /// Less than or equal (`<=`).
    Le,
    /// Greater than (`>`).
    Gt,
    /// Greater than or equal (`>=`).
    Ge,
    /// NULL-safe equality (`IS NOT DISTINCT FROM` in PostgreSQL, `<=>` in
    /// MySQL, `IS` in SQLite).
    IsNullSafe,
    /// IS NULL check.
    IsNull,
    /// IS NOT NULL check.
    IsNotNull,
}

impl Bijection {
    /// Returns the number of database columns this bijection spans.
    pub fn column_count(&self) -> usize {
        match self {
            Bijection::Identity | Bijection::Cast { .. } | Bijection::Affine { .. } => 1,
            Bijection::Nullable(inner) => inner.column_count(),
            Bijection::Product(components) => components.iter().map(|b| b.column_count()).sum(),
            Bijection::Coproduct {
                discriminant,
                variants,
            } => {
                discriminant.column_count()
                    + variants
                        .iter()
                        .map(|arm| arm.body.column_count())
                        .sum::<usize>()
            }
            Bijection::Compose { inner, .. } => {
                // The composed bijection has the same column count as the outer
                // bijection (which is what actually touches the columns), but
                // for safety we use `inner` since it determines the
                // model-facing shape and the outer should preserve column count.
                inner.column_count()
            }
        }
    }

    /// Returns `true` if this bijection is the identity (no transformation).
    pub fn is_identity(&self) -> bool {
        matches!(self, Bijection::Identity)
    }

    /// Encode a model-level value to a storage-level value.
    ///
    /// Applies the forward (field → column) direction of the bijection.
    pub fn encode(&self, value: stmt::Value) -> stmt::Value {
        match self {
            Bijection::Identity => value,

            Bijection::Cast { to, .. } => to.cast(value).expect("bijection encode cast failed"),

            Bijection::Affine { .. } => {
                todo!("Affine encode not yet implemented")
            }

            Bijection::Nullable(inner) => {
                if value.is_null() {
                    stmt::Value::Null
                } else {
                    inner.encode(value)
                }
            }

            Bijection::Product(components) => {
                let record = value.into_record();
                assert_eq!(
                    record.len(),
                    components.len(),
                    "product bijection arity mismatch"
                );
                let encoded: Vec<stmt::Value> = record
                    .into_iter()
                    .zip(components.iter())
                    .map(|(v, b)| b.encode(v))
                    .collect();
                stmt::Value::Record(stmt::ValueRecord::from_vec(encoded))
            }

            Bijection::Coproduct {
                discriminant: disc_bijection,
                variants,
            } => {
                // The model value for a coproduct is Record([discriminant, field1, field2, ...])
                // We need to encode the discriminant and the active variant's fields.
                let record = value.into_record();
                let disc_value = record[0].clone();

                // Encode the discriminant
                let encoded_disc = disc_bijection.encode(disc_value.clone());

                // Find the matching variant
                let disc_i64 = match &disc_value {
                    stmt::Value::I64(v) => *v,
                    _ => panic!("coproduct discriminant must be I64"),
                };

                let mut result = vec![encoded_disc];

                for arm in variants {
                    if arm.discriminant_value == disc_i64 {
                        // Active variant: encode its fields
                        match &arm.body {
                            Bijection::Product(fields) => {
                                // Fields start at index 1 in the record
                                for (i, field_bij) in fields.iter().enumerate() {
                                    result.push(field_bij.encode(record[1 + i].clone()));
                                }
                            }
                            Bijection::Identity => {
                                // Unit variant with no extra fields — nothing to add
                            }
                            other => {
                                // Single-field variant
                                result.push(other.encode(record[1].clone()));
                            }
                        }
                    } else {
                        // Inactive variant: emit NULLs for its columns
                        let col_count = arm.body.column_count();
                        for _ in 0..col_count {
                            result.push(stmt::Value::Null);
                        }
                    }
                }

                stmt::Value::Record(stmt::ValueRecord::from_vec(result))
            }

            Bijection::Compose { inner, outer } => outer.encode(inner.encode(value)),
        }
    }

    /// Query whether `model_op` can be pushed through this encoding.
    ///
    /// Returns the storage-level operator to use, or `None` if the operation
    /// must fall back to model-space evaluation.
    ///
    /// This is the key method for determining what can be pushed to the
    /// database. See the design doc's "Operator Homomorphism" section.
    pub fn can_distribute(&self, model_op: stmt::BinaryOp) -> Option<StorageOp> {
        match self {
            Bijection::Identity => Some(binary_op_to_storage_op(model_op)),

            Bijection::Cast { from, to } => cast_can_distribute(from, to, model_op),

            Bijection::Affine { .. } => {
                // Affine preserves == always. Preserves < if k > 0.
                // For now, conservatively only support ==.
                match model_op {
                    stmt::BinaryOp::Eq => Some(StorageOp::Eq),
                    stmt::BinaryOp::Ne => Some(StorageOp::Ne),
                    _ => None,
                }
            }

            Bijection::Nullable(inner) => {
                // Nullable wrapping changes == to NULL-safe equality
                match model_op {
                    stmt::BinaryOp::Eq => {
                        // Check if inner supports ==
                        inner.can_distribute(model_op)?;
                        Some(StorageOp::IsNullSafe)
                    }
                    stmt::BinaryOp::Ne => {
                        inner.can_distribute(stmt::BinaryOp::Eq)?;
                        // Ne on nullable: NOT (a IS NOT DISTINCT FROM b)
                        // For simplicity, fall back for now
                        None
                    }
                    _ => {
                        // For ordering ops on nullable, delegate to inner
                        // but the NULL handling is database-specific
                        inner.can_distribute(model_op)
                    }
                }
            }

            Bijection::Product(components) => {
                match model_op {
                    stmt::BinaryOp::Eq | stmt::BinaryOp::Ne => {
                        // == on product: all components must support ==
                        for component in components {
                            component.can_distribute(stmt::BinaryOp::Eq)?;
                        }
                        Some(binary_op_to_storage_op(model_op))
                    }
                    _ => {
                        // < on product requires lexicographic comparison
                        // and all components must support <. Generally not
                        // supported for now.
                        None
                    }
                }
            }

            Bijection::Coproduct {
                discriminant,
                variants,
            } => {
                match model_op {
                    stmt::BinaryOp::Eq | stmt::BinaryOp::Ne => {
                        // == on coproduct: discriminant + each arm must support ==
                        discriminant.can_distribute(stmt::BinaryOp::Eq)?;
                        for arm in variants {
                            arm.body.can_distribute(stmt::BinaryOp::Eq)?;
                        }
                        Some(binary_op_to_storage_op(model_op))
                    }
                    _ => {
                        // < across variants is generally meaningless
                        None
                    }
                }
            }

            Bijection::Compose { inner, outer } => {
                // Both must support the op for composition to work
                inner.can_distribute(model_op)?;
                outer.can_distribute(model_op)
            }
        }
    }
}

/// Convert a model-level `BinaryOp` to the corresponding standard `StorageOp`.
fn binary_op_to_storage_op(op: stmt::BinaryOp) -> StorageOp {
    match op {
        stmt::BinaryOp::Eq => StorageOp::Eq,
        stmt::BinaryOp::Ne => StorageOp::Ne,
        stmt::BinaryOp::Lt => StorageOp::Lt,
        stmt::BinaryOp::Le => StorageOp::Le,
        stmt::BinaryOp::Gt => StorageOp::Gt,
        stmt::BinaryOp::Ge => StorageOp::Ge,
    }
}

/// Determine operator homomorphism for a Cast bijection based on the
/// source and target types.
///
/// This encodes the homomorphism table from the design doc.
fn cast_can_distribute(
    from: &stmt::Type,
    to: &stmt::Type,
    model_op: stmt::BinaryOp,
) -> Option<StorageOp> {
    // All injective casts preserve ==
    match model_op {
        stmt::BinaryOp::Eq => return Some(StorageOp::Eq),
        stmt::BinaryOp::Ne => return Some(StorageOp::Ne),
        _ => {}
    }

    // For ordering operators, check specific type pairs
    match (from, to) {
        // Integer widening preserves all operators
        (a, b) if a.is_numeric() && b.is_numeric() => Some(binary_op_to_storage_op(model_op)),

        // Timestamp ↔ String: < is preserved with canonical ISO 8601 format
        #[cfg(feature = "jiff")]
        (stmt::Type::Timestamp, stmt::Type::String)
        | (stmt::Type::String, stmt::Type::Timestamp) => Some(binary_op_to_storage_op(model_op)),

        // Date ↔ String: < is preserved with canonical format
        #[cfg(feature = "jiff")]
        (stmt::Type::Date, stmt::Type::String) | (stmt::Type::String, stmt::Type::Date) => {
            Some(binary_op_to_storage_op(model_op))
        }

        // Time ↔ String: < is preserved with canonical format
        #[cfg(feature = "jiff")]
        (stmt::Type::Time, stmt::Type::String) | (stmt::Type::String, stmt::Type::Time) => {
            Some(binary_op_to_storage_op(model_op))
        }

        // DateTime ↔ String: < is preserved with canonical format
        #[cfg(feature = "jiff")]
        (stmt::Type::DateTime, stmt::Type::String) | (stmt::Type::String, stmt::Type::DateTime) => {
            Some(binary_op_to_storage_op(model_op))
        }

        // Timestamp ↔ DateTime, Timestamp ↔ Zoned, Zoned ↔ DateTime:
        // all operators preserved (same temporal semantics)
        #[cfg(feature = "jiff")]
        (stmt::Type::Timestamp, stmt::Type::DateTime)
        | (stmt::Type::DateTime, stmt::Type::Timestamp)
        | (stmt::Type::Timestamp, stmt::Type::Zoned)
        | (stmt::Type::Zoned, stmt::Type::Timestamp)
        | (stmt::Type::Zoned, stmt::Type::DateTime)
        | (stmt::Type::DateTime, stmt::Type::Zoned) => Some(binary_op_to_storage_op(model_op)),

        // UUID ↔ String/Bytes: ordering is NOT preserved
        (stmt::Type::Uuid, stmt::Type::String)
        | (stmt::Type::String, stmt::Type::Uuid)
        | (stmt::Type::Uuid, stmt::Type::Bytes)
        | (stmt::Type::Bytes, stmt::Type::Uuid) => None,

        // Zoned ↔ String: ordering NOT preserved (timezone variations)
        #[cfg(feature = "jiff")]
        (stmt::Type::Zoned, stmt::Type::String) | (stmt::Type::String, stmt::Type::Zoned) => None,

        // Decimal/BigDecimal ↔ String: ordering NOT preserved
        #[cfg(feature = "rust_decimal")]
        (stmt::Type::Decimal, stmt::Type::String) | (stmt::Type::String, stmt::Type::Decimal) => {
            None
        }

        #[cfg(feature = "bigdecimal")]
        (stmt::Type::BigDecimal, stmt::Type::String)
        | (stmt::Type::String, stmt::Type::BigDecimal) => None,

        // Unknown cast pair — conservatively refuse
        _ => None,
    }
}

impl StorageOp {
    /// Returns `true` if this is a NULL-safe or NULL-specific operator.
    pub fn is_null_aware(&self) -> bool {
        matches!(
            self,
            StorageOp::IsNullSafe | StorageOp::IsNull | StorageOp::IsNotNull
        )
    }
}