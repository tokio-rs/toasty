//! Cross-platform smoke test for the standalone-CLI design.
//!
//! See `docs/dev/design/standalone-cli.md`. The lib-only schema-extract path
//! depends on three properties holding on every supported platform:
//!
//!   1. `cargo rustc --crate-type cdylib` builds a plain `[lib]` package as a
//!      shared object without requiring a `Cargo.toml` change.
//!   2. Constructors contributed by the package (or its dependencies) are
//!      preserved through the cdylib link.
//!   3. `dlopen` invokes those constructors during library load on Linux,
//!      macOS, and Windows.
//!
//! This test exercises all three by building `tests-fixture-cdylib-ctor` as a
//! cdylib, loading it via `libloading`, and checking that a marker file the
//! fixture's constructor writes during load is present afterward.

#![allow(unsafe_code)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn ctor_fires_when_lib_is_built_as_cdylib_and_dlopened() {
    let fixture_manifest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cdylib-ctor/Cargo.toml");
    assert!(
        fixture_manifest.exists(),
        "fixture manifest missing at {}",
        fixture_manifest.display()
    );

    // Build the fixture as a cdylib in an isolated target dir. Using a tempdir
    // keeps the nested cargo build from contending with the outer
    // `cargo test` for the workspace target dir.
    let target_dir = tempfile::tempdir().expect("tempdir for cdylib build");

    let output = Command::new(env!("CARGO"))
        .arg("rustc")
        .arg("--manifest-path")
        .arg(&fixture_manifest)
        .arg("--lib")
        .arg("--crate-type=cdylib")
        .arg("--message-format=json-render-diagnostics")
        .env("CARGO_TARGET_DIR", target_dir.path())
        .output()
        .expect("invoking cargo rustc");

    assert!(
        output.status.success(),
        "cargo rustc --crate-type cdylib failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let cdylib_path = parse_cdylib_artifact(&output.stdout).unwrap_or_else(|| {
        panic!(
            "no cdylib artifact found in cargo output:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert!(
        cdylib_path.exists(),
        "cargo reported cdylib at {} but the file does not exist",
        cdylib_path.display()
    );

    // The fixture's ctor reads `TOASTY_CDYLIB_CTOR_OUT` and writes a marker
    // file at the supplied path. Set it before dlopen, unset right after, and
    // confirm the marker landed.
    //
    // `set_var` is technically unsound under concurrent `getenv`, but no other
    // test in this binary reads this variable (its name is unique to this
    // smoke test) and the variable is observed only by the ctor that runs
    // synchronously inside `Library::new`.
    let marker = target_dir.path().join("ctor_marker");
    unsafe {
        std::env::set_var("TOASTY_CDYLIB_CTOR_OUT", &marker);
    }
    let load_result = unsafe { libloading::Library::new(&cdylib_path) };
    unsafe {
        std::env::remove_var("TOASTY_CDYLIB_CTOR_OUT");
    }
    let _lib = load_result
        .unwrap_or_else(|err| panic!("dlopen of {} failed: {err}", cdylib_path.display()));

    assert!(
        marker.exists(),
        "ctor did not run during cdylib load — marker file missing at {}",
        marker.display()
    );
    let contents = std::fs::read_to_string(&marker).expect("read marker");
    assert_eq!(contents, "ctor fired");
}

/// Walk `cargo --message-format=json` lines for the cdylib artifact.
fn parse_cdylib_artifact(stdout: &[u8]) -> Option<PathBuf> {
    let text = std::str::from_utf8(stdout).ok()?;
    for line in text.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("reason").and_then(|r| r.as_str()) != Some("compiler-artifact") {
            continue;
        }
        let crate_types = v
            .get("target")
            .and_then(|t| t.get("crate_types"))
            .and_then(|c| c.as_array())?;
        if !crate_types.iter().any(|t| t.as_str() == Some("cdylib")) {
            continue;
        }
        let filenames = v.get("filenames").and_then(|f| f.as_array())?;
        for fname in filenames {
            let s = fname.as_str()?;
            if s.ends_with(".so") || s.ends_with(".dylib") || s.ends_with(".dll") {
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}
