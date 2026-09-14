//! Cargo interaction: workspace metadata, artifact builds, and JSON message
//! parsing.
//!
//! Cargo is driven through subprocesses so that the environment can be
//! scrubbed (see [`scrubbed_command`]); its JSON output is then handed to
//! [`cargo_metadata`] for typed parsing. `cargo`'s stderr is inherited so the
//! user sees ordinary build progress and compiler diagnostics.

use anyhow::{Context, Result, bail};
use cargo_metadata::{CrateType, DependencyKind, Message, Target, TargetKind};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Cargo's feature-selection flags, forwarded to every cargo invocation the
/// CLI makes.
///
/// Models behind a Cargo feature only exist in the artifact when that feature
/// is on, so a schema extracted without the flag is missing their tables and
/// diffs into a drop migration. A bin with `required-features` cannot be built
/// at all without them.
///
/// The flags reach `cargo metadata` too, not just the build: the resolved
/// graph is feature-dependent, so an optional `toasty` dependency is invisible
/// there — and [`Metadata::links_toasty`] refuses extraction — until the
/// feature enabling it is selected.
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
    /// Appends the flags to a cargo invocation.
    ///
    /// Values are forwarded verbatim rather than parsed: cargo already splits
    /// comma- and space-separated lists, and accepts both `feat` and
    /// `pkg/feat`.
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
    /// Runs `cargo metadata` in the current directory.
    ///
    /// The dependency graph is resolved — not `--no-deps` — because schema
    /// extraction runs the built artifact, and running it is only safe once
    /// `toasty` is known to be somewhere in the graph. See
    /// [`Metadata::links_toasty`].
    pub fn load(features: &Features) -> Result<Self> {
        // `MetadataCommand` would spawn cargo itself, but the environment has
        // to be scrubbed first, so the command is run here and only its output
        // handed over for parsing.
        let mut cmd = scrubbed_command("cargo");
        cmd.args(["metadata", "--format-version=1"]);
        features.apply_to(&mut cmd);

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

    /// Selects the target package: the named one when `-p` was given,
    /// otherwise the workspace root package.
    ///
    /// `-p` only matches workspace members. The resolved graph also contains
    /// registry and out-of-workspace path dependencies, and selecting one of
    /// those would point the migration directory at someone else's package —
    /// `~/.cargo/registry/src/…` or a sibling checkout.
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

    /// Returns `true` if `toasty` is reachable from `package` through normal
    /// dependencies.
    ///
    /// Schema extraction runs the built artifact. When `toasty` is linked the
    /// constructor dumps and exits before `main`; when it is not, the artifact
    /// is just the user's program, and running it executes whatever `main`
    /// does. So this gates extraction rather than merely shaping an error.
    ///
    /// The whole graph is walked, not just direct dependencies: a `models`
    /// crate paired with a `server` binary reaches `toasty` transitively, and
    /// the constructor is linked into the binary all the same.
    pub fn links_toasty(&self, package: &Package<'_>) -> bool {
        let Some(resolve) = &self.inner.resolve else {
            // Without a resolved graph there is nothing to check against;
            // treat it as reachable rather than blocking extraction outright.
            return true;
        };

        let nodes: HashMap<_, _> = resolve.nodes.iter().map(|node| (&node.id, node)).collect();

        let mut seen = HashSet::new();
        let mut queue = vec![&package.0.id];

        while let Some(id) = queue.pop() {
            if !seen.insert(id) {
                continue;
            }

            let Some(node) = nodes.get(id) else { continue };

            for dep in &node.deps {
                // Dev- and build-dependencies are not linked into the
                // artifact being run.
                let normal = dep.dep_kinds.is_empty()
                    || dep
                        .dep_kinds
                        .iter()
                        .any(|kind| kind.kind == DependencyKind::Normal);

                if !normal {
                    continue;
                }

                if self
                    .inner
                    .packages
                    .iter()
                    .any(|pkg| pkg.id == dep.pkg && pkg.name.as_str() == "toasty")
                {
                    return true;
                }

                queue.push(&dep.pkg);
            }
        }

        false
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

/// Whether an environment variable was put there by cargo or rustup, rather
/// than by the user.
///
/// Injected variables are scrubbed so builds spawned by the CLI have the same
/// fingerprint as builds the user runs directly; without this, running the CLI
/// itself under `cargo run` would cache-bust the user's builds.
/// `RUSTUP_TOOLCHAIN` counts as injected — rustup's proxy sets it, and keeping
/// it would pin the build to whichever toolchain launched the CLI instead of
/// the one the target project's `rust-toolchain.toml` asks for.
///
/// `CARGO_HOME` and `CARGO_TARGET_DIR` are exceptions: the user's own shell
/// would apply them too.
///
/// Everything else is the user's to set. Cargo does not set `RUSTFLAGS`,
/// `RUSTC`, or the compiler wrappers, so a value in one of those came from the
/// caller, and dropping it would build the project differently than they build
/// it themselves — silently losing `--cfg`-gated models, or bypassing a
/// toolchain or wrapper the project requires. `RUSTFLAGS` is part of cargo's
/// fingerprint besides, so scrubbing it forces exactly the rebuild this
/// scrubbing exists to avoid.
fn is_injected(key: &str) -> bool {
    key.starts_with("CARGO") && !matches!(key, "CARGO_HOME" | "CARGO_TARGET_DIR")
        || key == "RUSTUP_TOOLCHAIN"
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

    #[test]
    fn only_cargo_and_rustup_injected_variables_are_scrubbed() {
        for key in ["CARGO", "CARGO_PKG_NAME", "CARGO_MANIFEST_DIR"] {
            assert!(is_injected(key), "{key}");
        }
        assert!(is_injected("RUSTUP_TOOLCHAIN"));

        // The user's own shell would apply these to a direct build too.
        assert!(!is_injected("CARGO_HOME"));
        assert!(!is_injected("CARGO_TARGET_DIR"));

        // Cargo never sets these, so a value is the caller's. Scrubbing
        // `RUSTFLAGS` would drop `--cfg`-gated models from the dump.
        for key in [
            "RUSTFLAGS",
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
        ] {
            assert!(!is_injected(key), "{key}");
        }
    }

    #[test]
    fn feature_flags_are_forwarded_verbatim() {
        let mut cmd = Command::new("cargo");
        Features {
            features: vec!["a,b".to_string(), "pkg/c".to_string()],
            all_features: true,
            no_default_features: true,
        }
        .apply_to(&mut cmd);

        let args: Vec<_> = cmd.get_args().map(|arg| arg.to_string_lossy()).collect();
        assert_eq!(
            args,
            [
                "--features",
                "a,b",
                "--features",
                "pkg/c",
                "--all-features",
                "--no-default-features"
            ]
        );
    }

    #[test]
    fn no_feature_flags_are_added_by_default() {
        let mut cmd = Command::new("cargo");
        Features::default().apply_to(&mut cmd);
        assert_eq!(cmd.get_args().count(), 0);
    }
}
