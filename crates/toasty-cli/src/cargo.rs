//! Cargo interaction: workspace metadata, artifact builds, and JSON message
//! parsing.
//!
//! Cargo is driven through subprocesses so that the environment can be
//! scrubbed (see [`scrubbed_command`]); its JSON output is then handed to
//! [`cargo_metadata`] for typed parsing. `cargo`'s stderr is inherited so the
//! user sees ordinary build progress and compiler diagnostics.

use anyhow::{Context, Result, bail};
use cargo_metadata::{CrateType, DependencyKind, Message, Target, TargetKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Output of `cargo metadata --no-deps`.
pub struct Metadata {
    inner: cargo_metadata::Metadata,
}

impl Metadata {
    /// Runs `cargo metadata --no-deps` in the current directory.
    pub fn load() -> Result<Self> {
        // `MetadataCommand` would spawn cargo itself, but the environment has
        // to be scrubbed first, so the command is run here and only its output
        // handed over for parsing.
        let output = scrubbed_command("cargo")
            .args(["metadata", "--no-deps", "--format-version=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .context("failed to run `cargo metadata`; is `cargo` installed?")?;

        if !output.status.success() {
            bail!("`cargo metadata` failed; run from inside a Cargo package");
        }

        let stdout =
            String::from_utf8(output.stdout).context("`cargo metadata` produced invalid UTF-8")?;

        let inner = cargo_metadata::MetadataCommand::parse(stdout)
            .context("failed to parse `cargo metadata`")?;

        Ok(Metadata { inner })
    }

    /// The workspace root directory.
    pub fn workspace_root(&self) -> &Path {
        self.inner.workspace_root.as_std_path()
    }

    /// Selects the target package: the named one when `-p` was given,
    /// otherwise the workspace root package.
    pub fn select_package(&self, name: Option<&str>) -> Result<Package<'_>> {
        if let Some(name) = name {
            return self
                .inner
                .packages
                .iter()
                .find(|pkg| pkg.name.as_str() == name)
                .map(Package)
                .with_context(|| {
                    format!(
                        "package `{name}` not found in this workspace; available packages: {}",
                        self.package_names().join(", ")
                    )
                });
        }

        self.inner.root_package().map(Package).with_context(|| {
            format!(
                "the workspace root has no package (virtual manifest); select one with \
                     `-p <package>`; available packages: {}",
                self.package_names().join(", ")
            )
        })
    }

    fn package_names(&self) -> Vec<&str> {
        self.inner
            .packages
            .iter()
            .map(|pkg| pkg.name.as_str())
            .collect()
    }
}

/// A package entry from `cargo metadata`.
pub struct Package<'a>(&'a cargo_metadata::Package);

impl Package<'_> {
    /// The package name.
    pub fn name(&self) -> &str {
        self.0.name.as_str()
    }

    /// The directory containing the package's `Cargo.toml`.
    pub fn root(&self) -> PathBuf {
        self.0
            .manifest_path
            .parent()
            .map(|path| path.as_std_path().to_path_buf())
            .unwrap_or_default()
    }

    /// Returns `true` if the package lists `toasty` as a normal dependency.
    ///
    /// Dev- and build-dependencies do not count: neither is linked into the
    /// artifact the schema is extracted from. A `false` here is only a hint,
    /// though — the constructor also fires when `toasty` is reached
    /// transitively, which is the usual shape of a `models` crate paired with
    /// a `server` binary.
    pub fn depends_on_toasty(&self) -> bool {
        self.0
            .dependencies
            .iter()
            .any(|dep| dep.name == "toasty" && dep.kind == DependencyKind::Normal)
    }

    /// Names of the package's `[[bin]]` targets.
    pub fn bin_names(&self) -> Vec<&str> {
        self.targets_with_kind(&[TargetKind::Bin])
    }

    /// Returns `true` if the package has a lib target that can be rebuilt as
    /// a `cdylib`.
    ///
    /// Every linkable crate type qualifies: `cargo rustc --crate-type cdylib`
    /// overrides whatever the manifest declares, so a lib that is normally
    /// built as a `staticlib` (or already as a `cdylib`) works just as well as
    /// the default `rlib`.
    pub fn has_lib(&self) -> bool {
        !self
            .targets_with_kind(&[
                TargetKind::Lib,
                TargetKind::RLib,
                TargetKind::DyLib,
                TargetKind::CDyLib,
                TargetKind::StaticLib,
            ])
            .is_empty()
    }

    fn targets_with_kind(&self, kinds: &[TargetKind]) -> Vec<&str> {
        self.0
            .targets
            .iter()
            .filter(|target| kinds.iter().any(|kind| target.is_kind(kind.clone())))
            .map(|target| target.name.as_str())
            .collect()
    }
}

/// The artifact to build for schema extraction.
#[derive(Debug, Clone)]
pub enum BuildTarget {
    /// Build a `[[bin]]` target; the artifact is executed directly.
    Bin(String),

    /// Build the lib target as a `cdylib`; the artifact is loaded with
    /// `dlopen` by `toasty __load-cdylib`.
    Cdylib,
}

/// Builds the selected target of `package` in the `dev` profile and returns
/// the artifact path.
pub fn build_artifact(
    workspace_root: &Path,
    package: &str,
    target: &BuildTarget,
) -> Result<PathBuf> {
    let mut cmd = scrubbed_command("cargo");
    cmd.current_dir(workspace_root);

    match target {
        BuildTarget::Bin(name) => {
            cmd.args(["build", "-p", package, "--bin", name]);
        }
        BuildTarget::Cdylib => {
            cmd.args(["rustc", "-p", package, "--lib", "--crate-type", "cdylib"]);
        }
    }
    cmd.arg("--message-format=json-render-diagnostics");

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to run `cargo build`")?;

    if !output.status.success() {
        bail!("building `{package}` failed");
    }

    find_artifact(&output.stdout, target)
        .with_context(|| format!("`cargo build` produced no artifact for `{package}`"))
}

/// Extracts the artifact path from a `--message-format=json` stream.
fn find_artifact(messages: &[u8], target: &BuildTarget) -> Option<PathBuf> {
    let mut fallback = None;

    for message in Message::parse_stream(messages).filter_map(Result::ok) {
        let Message::CompilerArtifact(artifact) = message else {
            continue;
        };

        match target {
            BuildTarget::Bin(name) => {
                if artifact.target.is_kind(TargetKind::Bin)
                    && artifact.target.name == *name
                    && let Some(path) = artifact.executable
                {
                    return Some(path.into_std_path_buf());
                }
            }
            BuildTarget::Cdylib => {
                // Only the requested package is built as a cdylib;
                // dependencies compile as rlibs, so a cdylib artifact is
                // unambiguous. Cargo lists both the `deps/` output and the
                // final uplifted copy; prefer the uplifted one.
                if !is_cdylib(&artifact.target) {
                    continue;
                }
                for path in artifact.filenames {
                    let path = path.into_std_path_buf();
                    if !is_dylib(&path) {
                        continue;
                    }
                    let in_deps = path
                        .parent()
                        .and_then(Path::file_name)
                        .is_some_and(|dir| dir == "deps");
                    if in_deps {
                        fallback = Some(path);
                    } else {
                        return Some(path);
                    }
                }
            }
        }
    }

    fallback
}

fn is_cdylib(target: &Target) -> bool {
    target.crate_types.contains(&CrateType::CDyLib)
}

fn is_dylib(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("so" | "dylib" | "dll")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_bin_artifact() {
        let messages = br#"
{"reason":"compiler-artifact","package_id":"path+file:///t#dep@0.0.0","manifest_path":"/t/Cargo.toml","target":{"kind":["lib"],"crate_types":["lib"],"name":"dep","src_path":"/t/src/lib.rs","edition":"2021","doctest":false,"test":true,"doc":true},"profile":{"opt_level":"0","debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/t/debug/deps/libdep.rlib"],"executable":null,"fresh":false}
{"reason":"compiler-artifact","package_id":"path+file:///t#app@0.0.0","manifest_path":"/t/Cargo.toml","target":{"kind":["bin"],"crate_types":["bin"],"name":"other","src_path":"/t/src/main.rs","edition":"2021","doctest":false,"test":true,"doc":true},"profile":{"opt_level":"0","debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/t/debug/other"],"executable":"/t/debug/other","fresh":false}
{"reason":"compiler-artifact","package_id":"path+file:///t#app@0.0.0","manifest_path":"/t/Cargo.toml","target":{"kind":["bin"],"crate_types":["bin"],"name":"app","src_path":"/t/src/main.rs","edition":"2021","doctest":false,"test":true,"doc":true},"profile":{"opt_level":"0","debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/t/debug/app"],"executable":"/t/debug/app","fresh":false}
{"reason":"build-finished","success":true}
"#;
        let path = find_artifact(messages, &BuildTarget::Bin("app".into())).unwrap();
        assert_eq!(path, PathBuf::from("/t/debug/app"));
    }

    #[test]
    fn find_cdylib_artifact_prefers_uplifted_copy() {
        let messages = br#"
{"reason":"compiler-artifact","package_id":"path+file:///t#dep@0.0.0","manifest_path":"/t/Cargo.toml","target":{"kind":["lib"],"crate_types":["lib"],"name":"dep","src_path":"/t/src/lib.rs","edition":"2021","doctest":false,"test":true,"doc":true},"profile":{"opt_level":"0","debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/t/debug/deps/libdep.rlib"],"executable":null,"fresh":false}
{"reason":"compiler-artifact","package_id":"path+file:///t#app@0.0.0","manifest_path":"/t/Cargo.toml","target":{"kind":["lib"],"crate_types":["cdylib"],"name":"app","src_path":"/t/src/lib.rs","edition":"2021","doctest":false,"test":true,"doc":true},"profile":{"opt_level":"0","debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/t/debug/deps/libapp.so","/t/debug/libapp.so"],"executable":null,"fresh":false}
"#;
        let path = find_artifact(messages, &BuildTarget::Cdylib).unwrap();
        assert_eq!(path, PathBuf::from("/t/debug/libapp.so"));

        // Fall back to the deps/ copy when no uplifted path is listed.
        let messages = br#"
{"reason":"compiler-artifact","package_id":"path+file:///t#app@0.0.0","manifest_path":"/t/Cargo.toml","target":{"kind":["lib"],"crate_types":["cdylib"],"name":"app","src_path":"/t/src/lib.rs","edition":"2021","doctest":false,"test":true,"doc":true},"profile":{"opt_level":"0","debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/t/debug/deps/libapp.so"],"executable":null,"fresh":false}
"#;
        let path = find_artifact(messages, &BuildTarget::Cdylib).unwrap();
        assert_eq!(path, PathBuf::from("/t/debug/deps/libapp.so"));
    }

    #[test]
    fn find_artifact_none_when_missing() {
        assert!(find_artifact(b"", &BuildTarget::Cdylib).is_none());
        assert!(find_artifact(b"not json\n", &BuildTarget::Bin("app".into())).is_none());
    }
}

/// Creates a command with cargo- and rustc-related environment variables
/// removed, so builds spawned by the CLI have the same fingerprint as builds
/// the user runs directly. Without this, running the CLI itself under
/// `cargo run` would cache-bust the user's builds. `CARGO_HOME` and
/// `CARGO_TARGET_DIR` are kept: the user's own shell would apply them too.
fn scrubbed_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    for (key, _) in std::env::vars_os() {
        let Some(key_str) = key.to_str() else {
            continue;
        };
        let scrub = (key_str.starts_with("CARGO")
            && !matches!(key_str, "CARGO_HOME" | "CARGO_TARGET_DIR"))
            || matches!(
                key_str,
                "RUSTC"
                    | "RUSTC_WRAPPER"
                    | "RUSTC_WORKSPACE_WRAPPER"
                    | "RUSTFLAGS"
                    | "RUSTUP_TOOLCHAIN"
            );
        if scrub {
            cmd.env_remove(key);
        }
    }
    cmd
}
