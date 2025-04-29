#[macro_use]
mod macros;

pub mod db;

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
                #[tokio::test]
                $( #[$attrs] )*
                async fn $f() {
                    super::$f($crate::db::dynamodb::SetupDynamoDb).await;
                }
            )*
        }

        #[cfg(feature = "sqlite")]
        mod sqlite {
            $(
                #[tokio::test]
                $( #[$attrs] )*
                async fn $f() {
                    super::$f($crate::db::sqlite::SetupSqlite).await;
                }
            )*
        }

        #[cfg(feature = "mysql")]
        mod mysql {
            $(
                #[tokio::test]
                $( #[$attrs] )*
                async fn $f() {
                    super::$f($crate::db::mysql::SetupMySQL).await;
                }
            )*
        }

        #[cfg(feature = "postgresql")]
        mod postgresql {
            $(
                #[tokio::test]
                $( #[$attrs] )*
                async fn $f() {
                    super::$f($crate::db::postgresql::SetupPostgreSQL).await;
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
