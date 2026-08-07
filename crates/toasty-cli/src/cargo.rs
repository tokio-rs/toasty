//! Cargo interaction: workspace metadata, artifact builds, and JSON message
//! parsing.
//!
//! Cargo is driven through subprocesses. `cargo`'s stderr is inherited so the
//! user sees ordinary build progress and compiler diagnostics; stdout carries
//! the `--message-format=json-render-diagnostics` stream the artifact paths
//! are parsed from.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Output of `cargo metadata --no-deps`.
pub struct Metadata {
    json: Value,
}

impl Metadata {
    /// Runs `cargo metadata --no-deps` in the current directory.
    pub fn load() -> Result<Self> {
        let output = scrubbed_command("cargo")
            .args(["metadata", "--no-deps", "--format-version=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .context("failed to run `cargo metadata`; is `cargo` installed?")?;

        if !output.status.success() {
            bail!("`cargo metadata` failed; run from inside a Cargo package");
        }

        let json: Value =
            serde_json::from_slice(&output.stdout).context("failed to parse `cargo metadata`")?;

        Ok(Metadata { json })
    }

    /// The workspace root directory.
    pub fn workspace_root(&self) -> &str {
        self.json["workspace_root"].as_str().unwrap_or(".")
    }

    fn packages(&self) -> &[Value] {
        self.json["packages"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Selects the target package: the named one when `-p` was given,
    /// otherwise the workspace root package.
    pub fn select_package(&self, name: Option<&str>) -> Result<Package<'_>> {
        if let Some(name) = name {
            return self
                .packages()
                .iter()
                .find(|pkg| pkg["name"].as_str() == Some(name))
                .map(Package)
                .with_context(|| {
                    format!(
                        "package `{name}` not found in this workspace; available packages: {}",
                        self.package_names().join(", ")
                    )
                });
        }

        let root_manifest = Path::new(self.workspace_root()).join("Cargo.toml");
        self.packages()
            .iter()
            .find(|pkg| {
                pkg["manifest_path"]
                    .as_str()
                    .is_some_and(|path| Path::new(path) == root_manifest)
            })
            .map(Package)
            .with_context(|| {
                format!(
                    "the workspace root has no package (virtual manifest); select one with \
                     `-p <package>`; available packages: {}",
                    self.package_names().join(", ")
                )
            })
    }

    fn package_names(&self) -> Vec<&str> {
        self.packages()
            .iter()
            .filter_map(|pkg| pkg["name"].as_str())
            .collect()
    }
}

/// A package entry from `cargo metadata`.
pub struct Package<'a>(&'a Value);

impl Package<'_> {
    /// The package name.
    pub fn name(&self) -> &str {
        self.0["name"].as_str().unwrap_or_default()
    }

    /// The directory containing the package's `Cargo.toml`.
    pub fn root(&self) -> PathBuf {
        Path::new(self.0["manifest_path"].as_str().unwrap_or_default())
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    /// Returns `true` if the package has `toasty` as a direct dependency.
    pub fn depends_on_toasty(&self) -> bool {
        self.0["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|dep| dep["name"].as_str() == Some("toasty"))
    }

    /// Names of the package's `[[bin]]` targets.
    pub fn bin_names(&self) -> Vec<&str> {
        self.targets_with_kind("bin")
    }

    /// Returns `true` if the package has a lib target that can be rebuilt as
    /// a `cdylib`.
    pub fn has_lib(&self) -> bool {
        !self.targets_with_kind("lib").is_empty() || !self.targets_with_kind("rlib").is_empty()
    }

    fn targets_with_kind(&self, kind: &str) -> Vec<&str> {
        self.0["targets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|target| {
                target["kind"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|k| k.as_str() == Some(kind))
            })
            .filter_map(|target| target["name"].as_str())
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
    workspace_root: &str,
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    find_artifact(&stdout, target)
        .with_context(|| format!("`cargo build` produced no artifact for `{package}`"))
}

/// Extracts the artifact path from a `--message-format=json` stream.
fn find_artifact(messages: &str, target: &BuildTarget) -> Option<PathBuf> {
    let mut fallback = None;

    for line in messages.lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if message["reason"].as_str() != Some("compiler-artifact") {
            continue;
        }

        match target {
            BuildTarget::Bin(name) => {
                let is_bin = message["target"]["kind"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|k| k.as_str() == Some("bin"));
                if is_bin
                    && message["target"]["name"].as_str() == Some(name)
                    && let Some(path) = message["executable"].as_str()
                {
                    return Some(PathBuf::from(path));
                }
            }
            BuildTarget::Cdylib => {
                // Only the requested package is built as a cdylib;
                // dependencies compile as rlibs, so a cdylib artifact is
                // unambiguous. Cargo lists both the `deps/` output and the
                // final uplifted copy; prefer the uplifted one.
                let is_cdylib = message["target"]["crate_types"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|k| k.as_str() == Some("cdylib"));
                if !is_cdylib {
                    continue;
                }
                for path in message["filenames"].as_array().into_iter().flatten() {
                    let Some(path) = path.as_str() else { continue };
                    let path = Path::new(path);
                    if !is_dylib(path) {
                        continue;
                    }
                    let in_deps = path
                        .parent()
                        .and_then(Path::file_name)
                        .is_some_and(|dir| dir == "deps");
                    if in_deps {
                        fallback = Some(path.to_path_buf());
                    } else {
                        return Some(path.to_path_buf());
                    }
                }
            }
        }
    }

    fallback
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
        let messages = r#"
{"reason":"compiler-artifact","target":{"kind":["lib"],"crate_types":["lib"],"name":"dep"},"filenames":["/t/debug/deps/libdep.rlib"],"executable":null}
{"reason":"compiler-artifact","target":{"kind":["bin"],"crate_types":["bin"],"name":"other"},"filenames":["/t/debug/other"],"executable":"/t/debug/other"}
{"reason":"compiler-artifact","target":{"kind":["bin"],"crate_types":["bin"],"name":"app"},"filenames":["/t/debug/app"],"executable":"/t/debug/app"}
{"reason":"build-finished","success":true}
"#;
        let path = find_artifact(messages, &BuildTarget::Bin("app".into())).unwrap();
        assert_eq!(path, PathBuf::from("/t/debug/app"));
    }

    #[test]
    fn find_cdylib_artifact_prefers_uplifted_copy() {
        let messages = r#"
{"reason":"compiler-artifact","target":{"kind":["lib"],"crate_types":["lib"],"name":"dep"},"filenames":["/t/debug/deps/libdep.rlib"],"executable":null}
{"reason":"compiler-artifact","target":{"kind":["lib"],"crate_types":["cdylib"],"name":"app"},"filenames":["/t/debug/deps/libapp.so","/t/debug/libapp.so"],"executable":null}
"#;
        let path = find_artifact(messages, &BuildTarget::Cdylib).unwrap();
        assert_eq!(path, PathBuf::from("/t/debug/libapp.so"));

        // Fall back to the deps/ copy when no uplifted path is listed.
        let messages = r#"
{"reason":"compiler-artifact","target":{"kind":["lib"],"crate_types":["cdylib"],"name":"app"},"filenames":["/t/debug/deps/libapp.so"],"executable":null}
"#;
        let path = find_artifact(messages, &BuildTarget::Cdylib).unwrap();
        assert_eq!(path, PathBuf::from("/t/debug/deps/libapp.so"));
    }

    #[test]
    fn find_artifact_none_when_missing() {
        assert!(find_artifact("", &BuildTarget::Cdylib).is_none());
        assert!(find_artifact("not json\n", &BuildTarget::Bin("app".into())).is_none());
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
