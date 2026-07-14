//! Expression utilities for testing with assert_struct!
//!
//! This module provides test utilities like the Any wildcard
//! for use with the Like trait in assert_struct! macros.

use assert_struct::Like;
use toasty_core::stmt::{Expr, Value, ValueStream};

/// Wildcard type that matches any expression - useful for ignoring fields in patterns
#[derive(Debug, Clone)]
pub struct Any;

/// Any wildcard matches any expression
impl Like<Any> for Expr {
    fn like(&self, _pattern: &Any) -> bool {
        true // Any matches everything
    }
}

impl PartialEq<Any> for Expr {
    fn eq(&self, _: &Any) -> bool {
        true
    }
}

/// Any wildcard matches any value
impl Like<Any> for Value {
    fn like(&self, _pattern: &Any) -> bool {
        true // Any matches everything
    }
}

impl PartialEq<Any> for Value {
    fn eq(&self, _: &Any) -> bool {
        true
    }
}

/// Matches either an extracted bind-parameter placeholder (`Expr::Arg`, emitted
/// by SQL drivers after parameter extraction) at a specific position, or an
/// inline `Expr::Value` matching the contained pattern (emitted by non-SQL
/// drivers like DynamoDB).
///
/// Use this when a single assertion needs to cover both representations:
///
/// ```ignore
/// let val = if test.capability().sql {
///     ArgOr::Arg(0)
/// } else {
///     ArgOr::Value(1i64)
/// };
/// assert_struct!(op, ..., rows: [=~ (Any, Any, val)]);
/// ```
pub enum ArgOr<V> {
    /// Matches `Expr::Arg(_)` whose `position` equals the given index.
    Arg(usize),
    /// Matches `Expr::Value(_)` using the inner pattern via `Like<V>`.
    Value(V),
}

impl<V> Like<ArgOr<V>> for Expr
where
    Expr: Like<V>,
{
    fn like(&self, pattern: &ArgOr<V>) -> bool {
        match pattern {
            ArgOr::Arg(pos) => matches!(self, Expr::Arg(arg) if arg.position == *pos),
            ArgOr::Value(v) => self.like(v),
        }
    }
}

/// Extension trait for ValueStream providing convenient testing methods
pub trait ValueStreamExt {
    /// Returns buffered values, asserting that the stream is fully buffered
    ///
    /// This method will panic if the stream is not fully buffered (i.e., if there
    /// are still pending values in the stream that haven't been loaded into the buffer).
    /// Use this in tests when you want to access buffered values synchronously.
    fn buffered(&self) -> Vec<Value>;
}

/// Blanket implementation of ValueStreamExt for ValueStream
impl ValueStreamExt for ValueStream {
    fn buffered(&self) -> Vec<Value> {
        assert!(
            self.is_buffered(),
            "ValueStream is not fully buffered - call .buffer().await first"
        );
        self.buffered_to_vec()
    }
}
