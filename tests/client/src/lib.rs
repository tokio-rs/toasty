#[macro_use]
mod macros;

pub mod db;

use toasty::{schema::Schema, Db};
use toasty_core::driver::Capability;

pub use std_util::*;

#[async_trait::async_trait]
pub trait Setup {
    async fn setup(&self, schema: Schema) -> Db;

    fn capability(&self) -> &Capability;
}

#[macro_export]
macro_rules! schema {
    ($schema:literal) => {
        toasty_macros::schema!($schema);
    };
}

#[macro_export]
macro_rules! tests {
    (
        $(
            $( #[$attrs:meta] )*
            $f:ident
        ),+
    ) => {
        mod dynamodb {
            $(
                #[tokio::test]
                $( #[$attrs] )*
                async fn $f() {
                    super::$f($crate::db::SetupDynamoDb).await;
                }
            )*
        }

        mod sqlite {
            $(
                #[tokio::test]
                $( #[$attrs] )*
                async fn $f() {
                    super::$f($crate::db::SetupSqlite).await;
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
        crate::tests!( $(
            $( #[$attrs] )*
            $f
        ),+ );
    }
}
