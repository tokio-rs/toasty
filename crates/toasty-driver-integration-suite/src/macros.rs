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

macro_rules! models {
    (
        $( $model:ident ),*
    ) => {{
        let mut builder = toasty::Db::builder();
        $( builder.register::<$model>(); )*
        builder
    }};
}
