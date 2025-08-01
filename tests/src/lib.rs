#[macro_use]
mod macros;

pub mod db;
mod isolation;
mod toasty_test;

// Re-export for use in macros - needs to be public for macro expansion
pub use toasty_test::ToastyTest;

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
