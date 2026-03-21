/// Constructs a [`stmt::Path`](crate::stmt::Path) from a dot-separated
/// sequence of field step expressions.
///
/// Each argument is an expression that implements `Into<PathStep>`. The macro
/// collects them into a `Path` via `FromIterator`.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::path;
/// use toasty_core::stmt::Path;
///
/// let p: Path = path![.0 .1];
/// assert_eq!(p.projection.len(), 2);
/// ```
#[macro_export]
macro_rules! path {
    (
        $( . $field:expr )+
    ) => {
        [ $( $field, )+ ].into_iter().collect::<$crate::stmt::Path>()
    };
}
