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
        T: TryFrom<toasty_core::stmt::Value, Error = toasty_core::Error>;
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
