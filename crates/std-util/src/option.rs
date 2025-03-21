#[macro_export]
macro_rules! assert_none {
    ($e:expr) => {
        match &$e {
            None => {}
            actual => panic!("expected `None`; actual={:?}", actual),
        }
    };
}
