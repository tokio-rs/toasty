//! Like trait implementations for semantic pattern matching with assert_struct!
//!
//! This module provides essential implementations of the `Like` trait for Toasty expression types,
//! enabling semantic validation that works across different structural representations
//! of the same logical data.
//!
//! The primary use case is handling polymorphic AST structures where different database
//! drivers generate different representations for the same semantic content.

use super::{Expr, ExprSet, Value};
use assert_struct::Like;
use uuid::Uuid;

/// Helper function to extract Values from an Expr (handles both polymorphic representations)
fn extract_exprs_from_record(expr: &Expr) -> Option<Vec<Expr>> {
    match expr {
        Expr::Value(Value::Record(record)) => {
            Some(record.fields.iter().cloned().map(Into::into).collect())
        }
        Expr::Record(record) => Some(record.fields.clone()),
        _ => None,
    }
}

impl Like<Value> for Expr {
    fn like(&self, other: &Value) -> bool {
        match (self, other) {
            (Expr::Value(value), rhs) => value == rhs,
            (Expr::Record(lhs), Value::Record(rhs)) => {
                lhs.len() == rhs.len()
                    && lhs
                        .fields
                        .iter()
                        .zip(rhs.fields.iter())
                        .all(|(lhs, rhs)| lhs.like(rhs))
            }
            _ => false,
        }
    }
}

/// Like implementation for expressions against Vec<Value> patterns
impl Like<Vec<Value>> for Expr {
    fn like(&self, pattern: &Vec<Value>) -> bool {
        self.like(&&pattern[..])
    }
}

/// Like implementation for expressions against arrays of Values
impl<const N: usize> Like<[Value; N]> for Expr {
    fn like(&self, pattern: &[Value; N]) -> bool {
        self.like(&&pattern[..])
    }
}

impl Like<&[Value]> for Expr {
    fn like(&self, other: &&[Value]) -> bool {
        if let Some(values) = extract_exprs_from_record(self) {
            values.len() == other.len()
                && values
                    .iter()
                    .zip(*other)
                    .all(|(value, expected)| value.like(expected))
        } else {
            false
        }
    }
}

impl Like<&str> for Value {
    fn like(&self, pattern: &&str) -> bool {
        matches!(self, Value::String(value) if value == pattern)
    }
}

impl Like<&str> for Expr {
    fn like(&self, pattern: &&str) -> bool {
        matches!(self, Expr::Value(value) if value.like(pattern))
    }
}

impl Like<&[u8]> for Value {
    fn like(&self, pattern: &&[u8]) -> bool {
        matches!(self, Value::Bytes(value) if value == pattern)
    }
}

impl Like<&[u8]> for Expr {
    fn like(&self, pattern: &&[u8]) -> bool {
        matches!(self, Expr::Value(value) if value.like(pattern))
    }
}

/// Convenience implementation for matching Value against String
impl Like<String> for Value {
    fn like(&self, pattern: &String) -> bool {
        matches!(self, Value::String(s) if s == pattern)
    }
}

impl Like<&String> for Value {
    fn like(&self, pattern: &&String) -> bool {
        self.like(&**pattern)
    }
}

/// Like implementation for Expr and String (delegates to PartialEq)
impl Like<String> for Expr {
    fn like(&self, pattern: &String) -> bool {
        self == pattern
    }
}

impl Like<&String> for Expr {
    fn like(&self, pattern: &&String) -> bool {
        self == *pattern
    }
}

impl Like<Uuid> for Value {
    fn like(&self, other: &Uuid) -> bool {
        matches!(self, Value::Uuid(value) if value == other)
    }
}

impl Like<Uuid> for Expr {
    fn like(&self, other: &Uuid) -> bool {
        matches!(self, Expr::Value(value) if value.like(other))
    }
}

/// Macro to generate Like implementations for tuple patterns using Alice's pattern
///
/// Based on: https://users.rust-lang.org/t/macro-to-impl-trait-for-tuple/79165/2
/// Takes index-type pairs and generates both Expr and Value implementations.
macro_rules! impl_like_tuple {
    ($($idx:tt $t:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($t),+> Like<($($t,)+)> for Expr
        where
            $(Expr: Like<$t>),+
        {
            fn like(&self, pattern: &($($t,)+)) -> bool {
                if let Some(exprs) = extract_exprs_from_record(self) {
                    exprs.len() == impl_like_tuple!(@count $($t)+) && $(
                        exprs[$idx].like(&pattern.$idx)
                    )&&+
                } else {
                    false
                }
            }
        }

        #[allow(non_snake_case)]
        impl<$($t),+> Like<($($t,)+)> for Value
        where
            $(Value: Like<$t>),+
        {
            fn like(&self, pattern: &($($t,)+)) -> bool {
                if let Value::Record(record) = self {
                    record.fields.len() == impl_like_tuple!(@count $($t)+) && $(
                        record.fields[$idx].like(&pattern.$idx)
                    )&&+
                } else {
                    false
                }
            }
        }
    };

    // Helper to count elements
    (@count $t:ident) => (1);
    (@count $t:ident $($ts:ident)+) => (1 + impl_like_tuple!(@count $($ts)+));
}

// Generate implementations for tuples from size 1 to 12
// Using Alice's clean pattern: index-type pairs
impl_like_tuple!(0 T1);
impl_like_tuple!(0 T1, 1 T2);
impl_like_tuple!(0 T1, 1 T2, 2 T3);
impl_like_tuple!(0 T1, 1 T2, 2 T3, 3 T4);
impl_like_tuple!(0 T1, 1 T2, 2 T3, 3 T4, 4 T5);
impl_like_tuple!(0 T1, 1 T2, 2 T3, 3 T4, 4 T5, 5 T6);
impl_like_tuple!(0 T1, 1 T2, 2 T3, 3 T4, 4 T5, 5 T6, 6 T7);
impl_like_tuple!(0 T1, 1 T2, 2 T3, 3 T4, 4 T5, 5 T6, 6 T7, 7 T8);
impl_like_tuple!(0 T1, 1 T2, 2 T3, 3 T4, 4 T5, 5 T6, 6 T7, 7 T8, 8 T9);
impl_like_tuple!(0 T1, 1 T2, 2 T3, 3 T4, 4 T5, 5 T6, 6 T7, 7 T8, 8 T9, 9 T10);
impl_like_tuple!(0 T1, 1 T2, 2 T3, 3 T4, 4 T5, 5 T6, 6 T7, 7 T8, 8 T9, 9 T10, 10 T11);
impl_like_tuple!(0 T1, 1 T2, 2 T3, 3 T4, 4 T5, 5 T6, 6 T7, 7 T8, 8 T9, 9 T10, 10 T11, 11 T12);

/// Support for matching ExprSet against arrays of any length using const generics
impl<T, const N: usize> Like<[T; N]> for ExprSet
where
    Expr: Like<T>,
{
    fn like(&self, pattern: &[T; N]) -> bool {
        match self {
            ExprSet::Values(values) => {
                values.rows.len() == N
                    && values
                        .rows
                        .iter()
                        .zip(pattern)
                        .all(|(expr, p)| expr.like(p))
            }
            _ => false,
        }
    }
}

/// Support for matching ExprSet against Vec patterns
impl<T> Like<Vec<T>> for ExprSet
where
    Expr: Like<T>,
{
    fn like(&self, pattern: &Vec<T>) -> bool {
        match self {
            ExprSet::Values(values) => {
                values.rows.len() == pattern.len()
                    && values
                        .rows
                        .iter()
                        .zip(pattern)
                        .all(|(expr, p)| expr.like(p))
            }
            _ => false,
        }
    }
}

impl<const N: usize> Like<[&str; N]> for Expr {
    fn like(&self, other: &[&str; N]) -> bool {
        match self {
            Expr::Value(Value::Record(v)) if v.len() == other.len() => {
                v.iter().zip(other.iter()).all(|(lhs, rhs)| lhs.like(rhs))
            }
            Expr::Record(v) if v.len() == other.len() => {
                v.iter().zip(other.iter()).all(|(lhs, rhs)| lhs.like(rhs))
            }
            _ => false,
        }
    }
}

impl<const N: usize> Like<&[u8; N]> for Expr {
    fn like(&self, other: &&[u8; N]) -> bool {
        self.like(&&other[..])
    }
}
