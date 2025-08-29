//! Like trait implementations for semantic pattern matching with assert_struct!
//!
//! This module provides essential implementations of the `Like` trait for Toasty expression types,
//! enabling semantic validation that works across different structural representations
//! of the same logical data.
//!
//! The primary use case is handling polymorphic AST structures where different database
//! drivers generate different representations for the same semantic content.

use super::{Expr, ExprColumn, ExprSet, Value};
use crate::schema::db::ColumnId;
use assert_struct::Like;

/// Helper function to extract Values from an Expr (handles both polymorphic representations)
fn extract_values_from_expr(expr: &Expr) -> Option<Vec<&Value>> {
    match expr {
        Expr::Value(Value::Record(record)) => Some(record.fields.iter().collect()),
        Expr::Record(record) => {
            let mut values = Vec::new();
            for field in &record.fields {
                match field {
                    Expr::Value(v) => values.push(v),
                    _ => return None,
                }
            }
            Some(values)
        }
        _ => None,
    }
}

/// Like implementation for expressions against Vec<Value> patterns
impl Like<Vec<Value>> for Expr {
    fn like(&self, pattern: &Vec<Value>) -> bool {
        if let Some(values) = extract_values_from_expr(self) {
            values.len() == pattern.len()
                && values
                    .iter()
                    .zip(pattern)
                    .all(|(value, expected)| value == &expected)
        } else {
            false
        }
    }
}

/// Like implementation for expressions against arrays of Values
impl<const N: usize> Like<[Value; N]> for Expr {
    fn like(&self, pattern: &[Value; N]) -> bool {
        if let Some(values) = extract_values_from_expr(self) {
            values.len() == N
                && values
                    .iter()
                    .zip(pattern)
                    .all(|(value, expected)| value == &expected)
        } else {
            false
        }
    }
}

/// Convenience implementation for matching Value against string literals
impl Like<&str> for Value {
    fn like(&self, pattern: &&str) -> bool {
        matches!(self, Value::String(s) if s == pattern)
    }
}

/// Convenience implementation for matching Value against i32
impl Like<i32> for Value {
    fn like(&self, pattern: &i32) -> bool {
        matches!(self, Value::I32(v) if v == pattern)
    }
}

/// Convenience implementation for matching Value against String
impl Like<String> for Value {
    fn like(&self, pattern: &String) -> bool {
        matches!(self, Value::String(s) if s == pattern)
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
            $(Value: Like<$t>),+
        {
            fn like(&self, pattern: &($($t,)+)) -> bool {
                if let Some(values) = extract_values_from_expr(self) {
                    values.len() == impl_like_tuple!(@count $($t)+) && $(
                        values[$idx].like(&pattern.$idx)
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

/// Like implementation for Expr and String (delegates to PartialEq)
impl Like<String> for Expr {
    fn like(&self, pattern: &String) -> bool {
        self == pattern
    }
}

/// Like implementation for Expr and &str (delegates to PartialEq)  
impl Like<&str> for Expr {
    fn like(&self, pattern: &&str) -> bool {
        self == *pattern
    }
}

/// Like implementation for Expr and ColumnId - matches column references
impl Like<ColumnId> for Expr {
    fn like(&self, pattern: &ColumnId) -> bool {
        match self {
            Expr::Column(ExprColumn::Column(column_id)) => column_id == pattern,
            _ => false,
        }
    }
}
