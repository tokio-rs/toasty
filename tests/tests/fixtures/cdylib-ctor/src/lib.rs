//! Fixture for the cdylib-ctor smoke test.
//!
//! When this crate is built as a `cdylib` and loaded with `dlopen`, the
//! constructor below fires during library load. It writes a marker file at
//! the path supplied via the `TOASTY_CDYLIB_CTOR_OUT` environment variable.
//! When the variable is unset (the rlib build the workspace performs during
//! ordinary `cargo test`), the constructor returns immediately.

#[linktime::ctor(unsafe)]
fn write_marker() {
    let Some(path) = std::env::var_os("TOASTY_CDYLIB_CTOR_OUT") else {
        return;
    };
    if let Err(err) = std::fs::write(&path, b"ctor fired") {
        eprintln!(
            "tests-fixture-cdylib-ctor: failed to write marker at {:?}: {err}",
            path
        );
    }
}
