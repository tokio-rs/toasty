#[macro_export]
macro_rules! path {
    (
        $( . $field:expr )+
    ) => {
        [ $( $field, )+ ].into_iter().collect::<$crate::stmt::Path>()
    };
}
