//! [`PartialEq`] implementations for [`Value`] and [`Expr`] with Rust
//! primitive types.
//!
//! This module enables direct comparison between `Value` / `Expr` enum
//! variants and their corresponding Rust types (e.g., `bool`, `String`,
//! `&str`), making assertions more ergonomic.
//!
//! # Examples
//!
//! ```
//! use toasty_core::stmt::Value;
//!
//! let v = Value::from("hello");
//! assert_eq!(v, "hello");
//!
//! let v = Value::from(true);
//! assert_eq!(v, true);
//! ```

use super::{Expr, Value};

/// Macro to implement PartialEq for numeric and simple types
macro_rules! impl_value_eq {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            /// PartialEq implementation for Value and primitive type
            impl PartialEq<$ty> for Value {
                fn eq(&self, other: &$ty) -> bool {
                    matches!(self, Value::$variant(val) if val == other)
                }
            }

            /// PartialEq implementation for Expr and primitive type
            impl PartialEq<$ty> for Expr {
                fn eq(&self, other: &$ty) -> bool {
                    matches!(self, Expr::Value(Value::$variant(val)) if val == other)
                }
            }

            /// Reverse PartialEq implementation for convenience
            impl PartialEq<Value> for $ty {
                fn eq(&self, other: &Value) -> bool {
                    other.eq(self)
                }
            }
        )*
    };
}

// Implement PartialEq for all numeric and boolean types
impl_value_eq! {
    bool => Bool,
}

// String types need special handling since they all map to Value::String

/// PartialEq<String> for Value
impl PartialEq<String> for Value {
    fn eq(&self, other: &String) -> bool {
        matches!(self, Value::String(val) if val == other)
    }
}

/// PartialEq<String> for Expr
impl PartialEq<String> for Expr {
    fn eq(&self, other: &String) -> bool {
        matches!(self, Expr::Value(Value::String(val)) if val == other)
    }
}

/// PartialEq<&str> for Value
impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        matches!(self, Value::String(val) if val == other)
    }
}

/// PartialEq<&str> for Expr
impl PartialEq<&str> for Expr {
    fn eq(&self, other: &&str) -> bool {
        matches!(self, Expr::Value(Value::String(val)) if val == other)
    }
}

/// PartialEq<str> for Value
impl PartialEq<str> for Value {
    fn eq(&self, other: &str) -> bool {
        matches!(self, Value::String(val) if val == other)
    }
}

/// PartialEq<str> for Expr
impl PartialEq<str> for Expr {
    fn eq(&self, other: &str) -> bool {
        matches!(self, Expr::Value(Value::String(val)) if val == other)
    }
}

// Reverse implementations for string types

/// PartialEq<Value> for String
impl PartialEq<Value> for String {
    fn eq(&self, other: &Value) -> bool {
        other.eq(self)
    }
}

/// PartialEq<Value> for &str
impl PartialEq<Value> for &str {
    fn eq(&self, other: &Value) -> bool {
        other.eq(self)
    }
}

/// PartialEq<Value> for str
impl PartialEq<Value> for str {
    fn eq(&self, other: &Value) -> bool {
        other.eq(self)
    }
}
