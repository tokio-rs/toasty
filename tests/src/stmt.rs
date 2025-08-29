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

/// Any wildcard matches any value
impl Like<Any> for Value {
    fn like(&self, _pattern: &Any) -> bool {
        true // Any matches everything
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
