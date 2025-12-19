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
                                let suite = $crate::IntegrationSuite::new($driver);
                                suite.run_test(concat!(
                                    stringify!($module), "::",
                                    stringify!($test), "::",
                                    stringify!($variant)
                                ));
                            }
                        )*
                    }
                )*
            }
        )*
    };
}
