#[macro_export]
macro_rules! assert_err {
    ($e:expr $(, $($t:tt)* )?) => {
        match $e {
            Err(e) => e,
            actual => {
                use std::fmt::Write;
                let mut msg = format!("expected `Err`; actual={:?}", actual);

                $(
                    write!(msg, ", ").unwrap();
                    write!(msg, $($t)*).unwrap();
                )?

                panic!("{}", msg);
            }
        }
    };
}

#[macro_export]
macro_rules! assert_ok {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            actual => panic!("expected `Ok`; actual={:?}", actual),
        }
    };
}
