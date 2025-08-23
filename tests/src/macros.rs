#[macro_export]
macro_rules! assert_eq_unordered {
    ($actual:expr, $expect:expr) => {
        let mut vals = std::collections::HashSet::new();

        for val in $actual {
            assert!(vals.insert(val));
        }

        for val in $expect {
            assert!(vals.remove(val), "`{:#?}` missing", val);
        }

        assert!(vals.is_empty());
    };
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
                    let mut test = $crate::DbTest::new(
                        Box::new($crate::db::dynamodb::SetupDynamoDb::new())
                    );

                    test.run_test(move |test| Box::pin(async move {
                        super::$f(test).await;
                    }));
                }
            )*
        }

        #[cfg(feature = "sqlite")]
        mod sqlite {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::DbTest::new(
                        Box::new($crate::db::sqlite::SetupSqlite::new())
                    );

                    test.run_test(move |test| Box::pin(async move {
                        super::$f(test).await;
                    }));
                }
            )*
        }

        #[cfg(feature = "mysql")]
        mod mysql {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::DbTest::new(
                        Box::new($crate::db::mysql::SetupMySQL::new())
                    );

                    test.run_test(move |test| Box::pin(async move {
                        super::$f(test).await;
                    }));
                }
            )*
        }

        #[cfg(feature = "postgresql")]
        mod postgresql {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::DbTest::new(
                        Box::new($crate::db::postgresql::SetupPostgreSQL::new())
                    );

                    test.run_test(move |test| Box::pin(async move {
                        super::$f(test).await;
                    }));
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
