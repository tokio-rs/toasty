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
macro_rules! generate_driver_tests_impl {
    (
        $driver:expr,
        $($module:ident {
            $($test:ident { $($variant:ident)* })*
        })*
    ) => {
        $(
            mod $module {
                use super::*;

                $(
                    mod $test {
                        use super::*;

                        $(
                            #[test]
                            fn $variant() {
                                let mut test = $crate::Test::new(
                                    ::std::sync::Arc::new($driver)
                                );
                                test.run(async move |t| {
                                    $crate::tests::$module::$test::$variant(t).await;
                                });
                            }
                        )*
                    }
                )*
            }
        )*
    };
}
