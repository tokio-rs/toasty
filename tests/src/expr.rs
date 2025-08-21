//! Expression utilities for testing with assert_struct!
//!
//! This module provides test utilities like the Any wildcard
//! for use with the Like trait in assert_struct! macros.

use assert_struct::Like;
use toasty_core::stmt::{Expr, Value};

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
