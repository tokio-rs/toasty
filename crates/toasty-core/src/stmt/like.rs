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

/// Generic support for 3-tuple patterns
impl<T1, T2, T3> Like<(T1, T2, T3)> for Expr
where
    Value: Like<T1> + Like<T2> + Like<T3>,
{
    fn like(&self, pattern: &(T1, T2, T3)) -> bool {
        if let Some(values) = extract_values_from_expr(self) {
            if values.len() != 3 {
                return false;
            }

            values[0].like(&pattern.0) && values[1].like(&pattern.1) && values[2].like(&pattern.2)
        } else {
            false
        }
    }
}

/// Generic support for 2-tuple patterns  
impl<T1, T2> Like<(T1, T2)> for Expr
where
    Value: Like<T1> + Like<T2>,
{
    fn like(&self, pattern: &(T1, T2)) -> bool {
        if let Some(values) = extract_values_from_expr(self) {
            if values.len() != 2 {
                return false;
            }

            values[0].like(&pattern.0) && values[1].like(&pattern.1)
        } else {
            false
        }
    }
}

/// Generic support for 1-tuple patterns
impl<T1> Like<(T1,)> for Expr
where
    Value: Like<T1>,
{
    fn like(&self, pattern: &(T1,)) -> bool {
        if let Some(values) = extract_values_from_expr(self) {
            if values.len() != 1 {
                return false;
            }

            values[0].like(&pattern.0)
        } else {
            false
        }
    }
}

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
