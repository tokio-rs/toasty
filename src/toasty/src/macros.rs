#[allow(unused_macros)]
macro_rules! dbg {
    ( $( $t:tt )* ) => {{
        if cfg!(debug_assertions) {
            eprintln!( $($t)* )
        }
    }}
}
