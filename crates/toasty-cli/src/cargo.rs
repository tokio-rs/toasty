//! Cargo interaction: `cargo metadata`, artifact builds, and JSON message
//! parsing. Cargo runs as a subprocess with its injected environment scrubbed
//! (see [`is_injected`]); stderr is inherited so the user sees build progress.

use anyhow::{Context, Result, bail};
use cargo_metadata::{CrateType, Message, Target, TargetKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Cargo's feature-selection flags, forwarded to the schema build.
///
/// A model behind a feature only exists in the artifact when the feature is
/// on, so the flags must reach the build.
#[derive(clap::Args, Debug, Default, Clone)]
pub struct Features {
    /// Space or comma separated list of features to activate
    #[arg(short = 'F', long = "features", global = true, value_name = "FEATURES")]
    features: Vec<String>,

    /// Activate all available features
    #[arg(long, global = true)]
    all_features: bool,

    /// Do not activate the `default` feature
    #[arg(long, global = true)]
    no_default_features: bool,
}

impl Features {
    /// Appends the flags verbatim; cargo splits the lists itself.
    fn apply_to(&self, cmd: &mut Command) {
        for features in &self.features {
            cmd.args(["--features", features]);
        }

        if self.all_features {
            cmd.arg("--all-features");
        }

        if self.no_default_features {
            cmd.arg("--no-default-features");
        }
    }
}

/// Output of `cargo metadata`.
pub struct Metadata {
    inner: cargo_metadata::Metadata,
}

impl Metadata {
    /// Runs `cargo metadata` in the current directory. The resolve is kept
    /// only for its root: without it, the "current directory" package falls
    /// back to the workspace root manifest instead of the package the user is
    /// standing in.
    pub fn load() -> Result<Self> {
        // `MetadataCommand` would spawn cargo itself, bypassing the scrubbing.
        let mut cmd = scrubbed_command("cargo");
        cmd.args(["metadata", "--format-version=1"]);

        let output = cmd
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

    /// Selects the target package: `-p <name>`, or the workspace root package.
    ///
    /// Only workspace members qualify: selecting a registry or out-of-workspace
    /// path dependency would write the migration tree into someone else's
    /// package.
    pub fn select_package(&self, name: Option<&str>) -> Result<Package<'_>> {
        if let Some(name) = name {
            return self
                .inner
                .workspace_packages()
                .into_iter()
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
            .workspace_packages()
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

    /// Names of the package's `[[bin]]` targets.
    pub fn bin_names(&self) -> Vec<String> {
        self.0
            .targets
            .iter()
            .filter(|target| target.is_kind(TargetKind::Bin))
            .map(|target| target.name.clone())
            .collect()
    }

    /// Returns `true` if the package has a lib target. Any linkable crate type
    /// counts: `cargo rustc --crate-type cdylib` overrides the manifest.
    pub fn has_lib(&self) -> bool {
        self.0.targets.iter().any(|target| {
            target.kind.iter().any(|kind| {
                matches!(
                    kind,
                    TargetKind::Lib
                        | TargetKind::RLib
                        | TargetKind::DyLib
                        | TargetKind::CDyLib
                        | TargetKind::StaticLib
                )
            })
        })
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

/// Returns `true` if `toasty` is a normal dependency of `package`, directly
/// or transitively, under the selected features.
///
/// Extraction runs the built artifact. Without `toasty` linked there is no
/// dump constructor, and running the artifact would run the user's `main`.
/// `cargo tree` is asked rather than `cargo metadata` because `-p` and the
/// feature flags scope it exactly like the build; only normal edges count,
/// since build- and proc-macro dependencies are compiled for the host and
/// never linked into the artifact.
pub fn links_toasty(workspace_root: &Path, package: &str, features: &Features) -> Result<bool> {
    let mut cmd = scrubbed_command("cargo");
    cmd.current_dir(workspace_root);
    cmd.args([
        "tree", "-p", package, "--edges", "normal", "--prefix", "none",
    ]);
    features.apply_to(&mut cmd);

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to run `cargo tree`")?;

    if !output.status.success() {
        bail!("`cargo tree -p {package}` failed");
    }

    Ok(lists_toasty(&output.stdout))
}

/// Whether a `cargo tree --prefix none` listing names the `toasty` package.
/// Each line is `<name> v<version> (<source>)`, so the name is matched up to
/// the space, which keeps `toasty-core` and friends out.
fn lists_toasty(tree: &[u8]) -> bool {
    String::from_utf8_lossy(tree)
        .lines()
        .any(|line| line.starts_with("toasty "))
}

/// Builds the selected target of `package` in the `dev` profile and returns
/// the artifact path.
pub fn build_artifact(
    workspace_root: &Path,
    package: &str,
    target: &BuildTarget,
    features: &Features,
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
    features.apply_to(&mut cmd);
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
                // Only the requested package is built as a cdylib. Cargo lists
                // both the `deps/` copy and the uplifted one; prefer the latter.
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

/// Creates a command with the environment variables cargo and rustup inject
/// removed. See [`is_injected`].
fn scrubbed_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    for (key, _) in std::env::vars_os() {
        let Some(key_str) = key.to_str() else {
            continue;
        };
        if is_injected(key_str) {
            cmd.env_remove(key);
        }
    }
    cmd
}

/// Whether an environment variable was injected by cargo or rustup rather
/// than set by the user.
///
/// Scrubbing them keeps builds spawned by the CLI fingerprint-identical to the
/// user's own (running the CLI under `cargo run` would otherwise cache-bust
/// them) and stops `RUSTUP_TOOLCHAIN` pinning the build to the CLI's
/// toolchain. Only the variables cargo sets for the crate it runs are listed;
/// `CARGO_HOME`, `CARGO_BUILD_RUSTFLAGS`, `CARGO_NET_*`, `RUSTFLAGS`, and the
/// like are the user's build configuration and must survive.
fn is_injected(key: &str) -> bool {
    matches!(
        key,
        "CARGO"
            | "CARGO_MANIFEST_DIR"
            | "CARGO_MANIFEST_PATH"
            | "CARGO_CRATE_NAME"
            | "CARGO_BIN_NAME"
            | "CARGO_PRIMARY_PACKAGE"
            | "CARGO_TARGET_TMPDIR"
            | "RUSTUP_TOOLCHAIN"
    ) || key.starts_with("CARGO_PKG_")
        || key.starts_with("CARGO_BIN_EXE_")
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
    fn toasty_is_matched_by_package_name_in_a_tree_listing() {
        // `--prefix none` prints one `<name> v<version> (<source>)` line per
        // package; `toasty-core` and the CLI's own package must not count.
        let tree = b"app v0.0.0 (/t)\ntoasty-core v0.11.0 (/x/toasty-core)\n\
                     toasty-cli v0.11.0 (/x/toasty-cli)\n";
        assert!(!lists_toasty(tree));

        let tree = b"app v0.0.0 (/t)\ntoasty v0.11.0 (/x/toasty)\n\
                     toasty-core v0.11.0 (/x/toasty-core)\n";
        assert!(lists_toasty(tree));

        assert!(!lists_toasty(b""));
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
    fn user_build_configuration_is_not_scrubbed() {
        // Cargo configuration the user set themselves, never injected.
        for key in [
            "CARGO_HOME",
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_NET_OFFLINE",
            "CARGO_REGISTRIES_MINE_TOKEN",
            "RUSTFLAGS",
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
        ] {
            assert!(!is_injected(key), "{key}");
        }
    }
}
