#[macro_use]
mod macros;

pub mod db;
mod isolation;
mod toasty_test;

// Re-export for use in macros - needs to be public for macro expansion
pub use toasty_test::ToastyTest;

use std::collections::HashMap;
use toasty::Db;
use toasty_core::driver::Capability;

pub use std_util::*;

/// Trait for types that can be extracted from raw database storage
pub trait RawValue: Sized + Send + Sync + 'static {
    /// Extract this type from raw database storage, validating that the stored
    /// value actually represents this type correctly (no overflow, truncation, etc.)
    fn from_raw_storage(value: toasty_core::stmt::Value) -> Result<Self, String>;
}

impl RawValue for u8 {
    fn from_raw_storage(value: toasty_core::stmt::Value) -> Result<Self, String> {
        match value {
            toasty_core::stmt::Value::U8(val) => Ok(val),
            toasty_core::stmt::Value::I8(val) => {
                if val < 0 {
                    return Err(format!("u8 value stored as negative i8: {}", val));
                }
                Ok(val as u8)
            }
            toasty_core::stmt::Value::I16(val) => {
                if val < 0 || val > u8::MAX as i16 {
                    return Err(format!("u8 value out of range when stored as i16: {}", val));
                }
                Ok(val as u8)
            }
            _ => Err(format!("Cannot convert {:?} to u8", value)),
        }
    }
}

impl RawValue for u16 {
    fn from_raw_storage(value: toasty_core::stmt::Value) -> Result<Self, String> {
        match value {
            toasty_core::stmt::Value::U16(val) => Ok(val),
            toasty_core::stmt::Value::I16(val) => {
                if val < 0 {
                    return Err(format!("u16 value stored as negative i16: {}", val));
                }
                Ok(val as u16)
            }
            toasty_core::stmt::Value::I32(val) => {
                if val < 0 || val > u16::MAX as i32 {
                    return Err(format!(
                        "u16 value out of range when stored as i32: {}",
                        val
                    ));
                }
                Ok(val as u16)
            }
            _ => Err(format!("Cannot convert {:?} to u16", value)),
        }
    }
}

impl RawValue for u32 {
    fn from_raw_storage(value: toasty_core::stmt::Value) -> Result<Self, String> {
        match value {
            toasty_core::stmt::Value::U32(val) => Ok(val),
            toasty_core::stmt::Value::I32(val) => {
                if val < 0 {
                    return Err(format!("u32 value stored as negative i32: {}", val));
                }
                Ok(val as u32)
            }
            toasty_core::stmt::Value::I64(val) => {
                if val < 0 || val > u32::MAX as i64 {
                    return Err(format!(
                        "u32 value out of range when stored as i64: {}",
                        val
                    ));
                }
                Ok(val as u32)
            }
            _ => Err(format!("Cannot convert {:?} to u32", value)),
        }
    }
}

impl RawValue for u64 {
    fn from_raw_storage(value: toasty_core::stmt::Value) -> Result<Self, String> {
        match value {
            toasty_core::stmt::Value::U64(val) => Ok(val), // Native u64 storage (SQLite, etc.)
            toasty_core::stmt::Value::I64(val) => {
                // PostgreSQL case: stored as i64, validate it's actually unsigned
                if val < 0 {
                    return Err(format!(
                        "u64 value stored as negative i64: {}. This indicates overflow/corruption!",
                        val
                    ));
                }
                Ok(val as u64)
            }
            toasty_core::stmt::Value::String(s) => {
                // DynamoDB case: numbers stored as strings
                s.parse::<u64>()
                    .map_err(|e| format!("Failed to parse u64 from string '{}': {}", s, e))
            }
            _ => Err(format!("Cannot convert {:?} to u64", value)),
        }
    }
}

#[async_trait::async_trait]
pub trait Setup: Send + Sync + 'static {
    async fn setup(&self, db: toasty::db::Builder) -> Db {
        let db = self.connect(db).await.unwrap();
        db.reset_db().await.unwrap();
        db
    }

    async fn connect(&self, mut builder: toasty::db::Builder) -> toasty::Result<Db>;

    fn capability(&self) -> &Capability;

    /// Clean up tables created by this specific setup instance.
    ///
    /// This method should drop only the tables that belong to this test,
    /// identified by the table prefix used during setup.
    async fn cleanup_my_tables(&self) -> toasty::Result<()>;

    /// Get the raw value stored in the database for verification
    ///
    /// - `table`: Table name WITHOUT prefix (e.g., "foo", not "test_123_foo")
    /// - `column`: Column name to retrieve (e.g., "val")  
    /// - `filter`: WHERE clause conditions as column_name -> value pairs
    /// - `T`: The expected application type - implementation validates the raw storage
    async fn get_raw_column_value<T>(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<T>
    where
        T: RawValue;
}

#[macro_export]
macro_rules! models {
    (
        $( $model:ident ),*
    ) => {{
        let mut builder = toasty::Db::builder();
        $( builder.register::<$model>(); )*
        builder
    }};
}

#[macro_export]
macro_rules! tests {
    (
        $(
            $( #[$attrs:meta] )*
            $f:ident
        ),+
    ) => {
        #[cfg(feature = "dynamodb")]
        mod dynamodb {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::ToastyTest::new(
                        $crate::db::dynamodb::SetupDynamoDb::new()
                    );

                    test.run_test(|setup| async move {
                        super::$f(setup).await;
                    });
                }
            )*
        }

        #[cfg(feature = "sqlite")]
        mod sqlite {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::ToastyTest::new(
                        $crate::db::sqlite::SetupSqlite::new()
                    );

                    test.run_test(|setup| async move {
                        super::$f(setup).await;
                    });
                }
            )*
        }

        #[cfg(feature = "mysql")]
        mod mysql {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::ToastyTest::new(
                        $crate::db::mysql::SetupMySQL::new()
                    );

                    test.run_test(|setup| async move {
                        super::$f(setup).await;
                    });
                }
            )*
        }

        #[cfg(feature = "postgresql")]
        mod postgresql {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::ToastyTest::new(
                        $crate::db::postgresql::SetupPostgreSQL::new()
                    );

                    test.run_test(|setup| async move {
                        super::$f(setup).await;
                    });
                }
            )*
        }
    };
    (
        $(
            $( #[$attrs:meta] )*
            $f:ident,
        )+
    ) => {
        $crate::tests!( $(
            $( #[$attrs] )*
            $f
        ),+ );
    }
}
