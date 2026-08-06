//! The parts of `cargo` the CLI drives: reading a workspace's layout and
//! building one target out of it.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Where to find the workspace and which member to work on.
///
/// These mirror `cargo`'s own flags and are passed straight through, so their
/// meaning matches `cargo` exactly.
#[derive(Debug, Default, Clone, clap::Args)]
#[command(next_help_heading = "Target selection")]
pub struct TargetFlags {
    /// Package to extract the schema from
    #[arg(short, long, value_name = "SPEC", global = true)]
    pub package: Option<String>,

    /// Binary target to extract the schema from
    #[arg(long, value_name = "NAME", global = true)]
    pub bin: Option<String>,

    /// Path to Cargo.toml
    #[arg(long, value_name = "PATH", global = true)]
    pub manifest_path: Option<PathBuf>,
}

/// A workspace as `cargo metadata` describes it.
#[derive(Deserialize)]
pub struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
    /// The resolved dependency graph. Absent only if cargo omits it.
    #[serde(default)]
    resolve: Option<Resolve>,
}

/// One package in the workspace.
#[derive(Deserialize)]
pub struct Package {
    /// The package name, as `-p` accepts it.
    pub name: String,
    /// Opaque package identifier, used to match against `workspace_members`
    /// and the resolve graph.
    pub id: String,
    /// Absolute path to the package's `Cargo.toml`.
    pub manifest_path: PathBuf,
    /// The package's build targets.
    pub targets: Vec<Target>,
}

/// One build target of a [`Package`].
#[derive(Deserialize)]
pub struct Target {
    /// The target name — for a bin, what `--bin` takes.
    pub name: String,
    /// Target kinds, e.g. `["bin"]`, `["lib"]`, `["proc-macro"]`.
    pub kind: Vec<String>,
}

/// The resolved dependency graph, one node per package.
#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<ResolveNode>,
}

#[derive(Deserialize)]
struct ResolveNode {
    id: String,
    dependencies: Vec<String>,
}

impl Package {
    /// The directory holding this package's `Cargo.toml`. Migration files and
    /// `Toasty.toml` are resolved relative to it, so the CLI behaves the same
    /// from anywhere in the workspace.
    pub fn root_dir(&self) -> &Path {
        self.manifest_path
            .parent()
            .expect("a manifest path always has a parent")
    }

    /// The names of this package's `[[bin]]` targets.
    pub fn bin_names(&self) -> Vec<&str> {
        self.targets
            .iter()
            .filter(|t| t.kind.iter().any(|k| k == "bin"))
            .map(|t| t.name.as_str())
            .collect()
    }

    /// Whether this package has a `[lib]` target that can be rebuilt as a
    /// `cdylib`. A proc-macro crate has a `lib` section but cannot.
    pub fn has_linkable_lib(&self) -> bool {
        self.targets.iter().any(|t| {
            t.kind
                .iter()
                .any(|k| matches!(k.as_str(), "lib" | "rlib" | "cdylib" | "dylib"))
        })
    }
}

impl Metadata {
    /// Reads the workspace layout and resolved dependency graph with
    /// `cargo metadata`.
    pub fn load(flags: &TargetFlags) -> Result<Self> {
        let mut cmd = Command::new(cargo_bin());
        cmd.args(["metadata", "--format-version", "1"]);

        if let Some(manifest_path) = &flags.manifest_path {
            cmd.arg("--manifest-path").arg(manifest_path);
        }

        let output = cmd
            .stderr(Stdio::inherit())
            .output()
            .context("failed to run `cargo metadata`; is cargo on PATH?")?;

        if !output.status.success() {
            bail!("`cargo metadata` failed; see the error above");
        }

        serde_json::from_slice(&output.stdout).context("could not parse `cargo metadata`")
    }

    /// Whether `package` links `toasty`, directly or through another crate.
    ///
    /// The dump constructor comes from `toasty`; a package that does not reach
    /// it would build fine and produce nothing. The whole graph is searched
    /// rather than the direct dependencies alone, because a binary whose models
    /// live in a sibling library reaches `toasty` only through that library.
    pub fn links_toasty(&self, package: &Package) -> bool {
        let Some(resolve) = &self.resolve else {
            // Without a resolve graph there is nothing to check; let the build
            // proceed and report a missing dump afterwards.
            return true;
        };

        let toasty_ids: Vec<&str> = self
            .packages
            .iter()
            .filter(|p| p.name == "toasty")
            .map(|p| p.id.as_str())
            .collect();

        if toasty_ids.is_empty() {
            return false;
        }

        let mut seen = vec![package.id.as_str()];
        let mut queue = vec![package.id.as_str()];

        while let Some(id) = queue.pop() {
            if toasty_ids.contains(&id) {
                return true;
            }

            let Some(node) = resolve.nodes.iter().find(|n| n.id == id) else {
                continue;
            };

            for dep in &node.dependencies {
                if !seen.contains(&dep.as_str()) {
                    seen.push(dep);
                    queue.push(dep);
                }
            }
        }

        false
    }

    /// The package to extract a schema from: the one `-p` names, or the
    /// workspace root package.
    ///
    /// A virtual manifest has no root package, so `-p` is required there. The
    /// error lists the members rather than making the user go read the
    /// manifest.
    pub fn target_package(&self, flags: &TargetFlags) -> Result<&Package> {
        if let Some(name) = &flags.package {
            return self.members().find(|p| p.name == *name).with_context(|| {
                format!("no package named `{name}` in this workspace{}", {
                    let members = self.member_list();
                    format!("\n\nworkspace members:\n{members}")
                })
            });
        }

        let root_manifest = self.workspace_root.join("Cargo.toml");
        self.members()
            .find(|p| p.manifest_path == root_manifest)
            .with_context(|| {
                format!(
                    "this workspace has no root package, so there is nothing to \
                     extract a schema from by default; pass `-p <package>`\n\n\
                     workspace members:\n{}",
                    self.member_list()
                )
            })
    }

    fn members(&self) -> impl Iterator<Item = &Package> {
        self.packages
            .iter()
            .filter(|p| self.workspace_members.contains(&p.id))
    }

    fn member_list(&self) -> String {
        let mut names: Vec<_> = self.members().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        names
            .iter()
            .map(|name| format!("  {name}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// What to build in order to get a schema out of a package.
#[derive(Debug, PartialEq, Eq)]
pub enum BuildTarget {
    /// Build this `[[bin]]`. The dump constructor runs before its `main`.
    Bin(String),

    /// Rebuild the `[lib]` as a `cdylib`, which `dlopen` can load and whose
    /// constructors run at load time.
    Cdylib,
}

impl BuildTarget {
    /// Chooses what to build for `package`.
    ///
    /// A bin is preferred: it is a plain process to spawn. A lib-only package
    /// gets rebuilt as a `cdylib`, which is what makes the extraction work
    /// without a `main` to run. `--bin` disambiguates a package with several.
    pub fn select(package: &Package, requested_bin: Option<&str>) -> Result<Self> {
        let bins = package.bin_names();

        if let Some(name) = requested_bin {
            if !bins.contains(&name) {
                bail!(
                    "package `{}` has no bin target named `{name}`{}",
                    package.name,
                    format_alternatives("available bins", &bins),
                );
            }
            return Ok(Self::Bin(name.to_string()));
        }

        match bins.as_slice() {
            [only] => Ok(Self::Bin((*only).to_string())),
            [] if package.has_linkable_lib() => Ok(Self::Cdylib),
            [] => bail!(
                "package `{}` has neither a bin nor a lib target, so there is \
                 nothing to extract a schema from",
                package.name
            ),
            _ => bail!(
                "package `{}` has several bin targets; pick one with `--bin <name>`{}",
                package.name,
                format_alternatives("bins", &bins),
            ),
        }
    }
}

fn format_alternatives(label: &str, names: &[&str]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let list = names
        .iter()
        .map(|n| format!("  {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("\n\n{label}:\n{list}")
}

/// Builds `target` in `package` and returns the path of the artifact.
///
/// Always the `dev` profile: the dump constructor is `cfg(debug_assertions)`,
/// and a release build's dead-stripping and cross-crate LTO have no reason to
/// preserve it.
pub fn build(flags: &TargetFlags, package: &Package, target: &BuildTarget) -> Result<PathBuf> {
    let mut cmd = Command::new(cargo_bin());

    match target {
        BuildTarget::Bin(name) => {
            cmd.args(["build", "--bin", name]);
        }
        // `cargo rustc --crate-type cdylib` overrides the crate type for this
        // one invocation, so the user's `Cargo.toml` stays untouched.
        BuildTarget::Cdylib => {
            cmd.args(["rustc", "--lib", "--crate-type", "cdylib"]);
        }
    }

    cmd.args(["--package", &package.name]);
    cmd.arg("--message-format=json-render-diagnostics");

    if let Some(manifest_path) = &flags.manifest_path {
        cmd.arg("--manifest-path").arg(manifest_path);
    }

    // The environment is passed through untouched, so this build sees the same
    // `CARGO_TARGET_DIR`, `RUSTFLAGS`, and toolchain settings as a `cargo
    // build` the user would run from here — and reuses the artifacts.

    // `json-render-diagnostics` sends rendered diagnostics to stderr, leaving
    // stdout to the artifact messages alone.
    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to run cargo; is cargo on PATH?")?;

    if !output.status.success() {
        bail!("building `{}` failed; see the errors above", package.name);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    artifact_path(&stdout, target).with_context(|| {
        format!(
            "cargo reported no {} artifact for `{}`",
            match target {
                BuildTarget::Bin(_) => "executable",
                BuildTarget::Cdylib => "cdylib",
            },
            package.name
        )
    })
}

/// Picks the built artifact out of cargo's JSON message stream.
///
/// Cargo reports an artifact for every crate it compiles, so the requested one
/// has to be identified rather than assumed: it sets `executable` only on the
/// bin targets that were asked for, and it uplifts only the requested
/// package's library out of `deps/`. A proc-macro dependency also produces a
/// shared object, which is why the crate type is checked too.
fn artifact_path(stdout: &str, target: &BuildTarget) -> Option<PathBuf> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<ArtifactMessage>(line).ok())
        .filter(|message| message.reason == "compiler-artifact")
        .find_map(|message| match target {
            BuildTarget::Bin(_) => message.executable,
            BuildTarget::Cdylib => {
                if !message.target.crate_types.iter().any(|ty| ty == "cdylib") {
                    return None;
                }
                message
                    .filenames
                    .into_iter()
                    .find(|path| is_final_shared_library(path))
            }
        })
}

#[derive(Deserialize)]
struct ArtifactMessage {
    reason: String,
    #[serde(default)]
    target: ArtifactTarget,
    #[serde(default)]
    executable: Option<PathBuf>,
    #[serde(default)]
    filenames: Vec<PathBuf>,
}

#[derive(Deserialize, Default)]
struct ArtifactTarget {
    #[serde(default)]
    crate_types: Vec<String>,
}

/// Whether `path` is the build's own loadable library: a shared object by
/// extension — as opposed to the rlib, import library, or dep-info file cargo
/// lists beside it — that cargo uplifted out of `deps/`, which it does only
/// for the requested package.
fn is_final_shared_library(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("so" | "dylib" | "dll")
    ) && path
        .parent()
        .and_then(Path::file_name)
        .is_none_or(|dir| dir != "deps")
}

/// The cargo to invoke: the one running us when there is one, so a `+toolchain`
/// or a rustup override is honored, and plain `cargo` otherwise.
fn cargo_bin() -> PathBuf {
    std::env::var_os("CARGO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn package(name: &str, targets: serde_json::Value) -> Package {
        serde_json::from_value(json!({
            "name": name,
            "id": format!("{name} 0.1.0"),
            "manifest_path": format!("/ws/{name}/Cargo.toml"),
            "targets": targets,
        }))
        .unwrap()
    }

    /// A workspace of `app -> models -> toasty`, so the search has to go two
    /// hops.
    fn workspace() -> Metadata {
        serde_json::from_value(json!({
            "packages": [
                { "name": "app", "id": "app", "manifest_path": "/ws/app/Cargo.toml", "targets": [] },
                { "name": "models", "id": "models", "manifest_path": "/ws/models/Cargo.toml", "targets": [] },
                { "name": "toasty", "id": "toasty", "manifest_path": "/reg/toasty/Cargo.toml", "targets": [] },
                { "name": "lonely", "id": "lonely", "manifest_path": "/ws/lonely/Cargo.toml", "targets": [] },
            ],
            "workspace_members": ["app", "models", "lonely"],
            "workspace_root": "/ws",
            "resolve": {
                "nodes": [
                    { "id": "app", "dependencies": ["models"] },
                    { "id": "models", "dependencies": ["toasty"] },
                    { "id": "toasty", "dependencies": [] },
                    { "id": "lonely", "dependencies": [] },
                ],
            },
        }))
        .unwrap()
    }

    fn find<'a>(metadata: &'a Metadata, name: &str) -> &'a Package {
        metadata.packages.iter().find(|p| p.name == name).unwrap()
    }

    #[test]
    fn toasty_is_found_through_an_intermediate_crate() {
        // A binary whose models live in a sibling library never names `toasty`
        // in its own manifest, but still links the dump constructor.
        let metadata = workspace();
        assert!(metadata.links_toasty(find(&metadata, "app")));
    }

    #[test]
    fn a_package_outside_the_toasty_graph_is_rejected() {
        let metadata = workspace();
        assert!(!metadata.links_toasty(find(&metadata, "lonely")));
    }

    #[test]
    fn a_lone_bin_is_selected_without_asking() {
        let package = package("app", json!([{ "name": "app", "kind": ["bin"] }]));
        assert_eq!(
            BuildTarget::select(&package, None).unwrap(),
            BuildTarget::Bin("app".into())
        );
    }

    #[test]
    fn a_bin_is_preferred_over_the_lib_beside_it() {
        // Spawning a process is simpler than dlopening a library, so a package
        // that has both takes the bin path.
        let package = package(
            "app",
            json!([
                { "name": "app", "kind": ["lib"] },
                { "name": "app", "kind": ["bin"] },
            ]),
        );
        assert_eq!(
            BuildTarget::select(&package, None).unwrap(),
            BuildTarget::Bin("app".into())
        );
    }

    #[test]
    fn a_lib_only_package_is_rebuilt_as_a_cdylib() {
        let package = package("lib", json!([{ "name": "lib", "kind": ["lib"] }]));
        assert_eq!(
            BuildTarget::select(&package, None).unwrap(),
            BuildTarget::Cdylib
        );
    }

    #[test]
    fn several_bins_require_an_explicit_choice() {
        let package = package(
            "app",
            json!([
                { "name": "server", "kind": ["bin"] },
                { "name": "worker", "kind": ["bin"] },
            ]),
        );

        let err = BuildTarget::select(&package, None).unwrap_err().to_string();
        assert!(err.contains("--bin"), "{err}");
        assert!(err.contains("server") && err.contains("worker"), "{err}");

        assert_eq!(
            BuildTarget::select(&package, Some("worker")).unwrap(),
            BuildTarget::Bin("worker".into())
        );
    }

    #[test]
    fn an_unknown_bin_name_lists_the_real_ones() {
        let package = package("app", json!([{ "name": "server", "kind": ["bin"] }]));
        let err = BuildTarget::select(&package, Some("nope"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("no bin target named `nope`"), "{err}");
        assert!(err.contains("server"), "{err}");
    }

    #[test]
    fn a_proc_macro_package_has_nothing_to_extract_from() {
        // A proc-macro crate is a compiler plugin: it has a lib section, but
        // it cannot be rebuilt as a cdylib and links no models.
        let package = package(
            "macros",
            json!([{ "name": "macros", "kind": ["proc-macro"] }]),
        );
        let err = BuildTarget::select(&package, None).unwrap_err().to_string();
        assert!(err.contains("neither a bin nor a lib"), "{err}");
    }

    #[test]
    fn the_executable_is_read_from_the_artifact_stream() {
        let stdout = [
            json!({ "reason": "compiler-artifact", "executable": null, "filenames": ["/t/debug/deps/libdep.rlib"] }),
            json!({ "reason": "compiler-artifact", "executable": "/t/debug/app", "filenames": ["/t/debug/app"] }),
            json!({ "reason": "build-finished", "success": true }),
        ]
        .map(|v| v.to_string())
        .join("\n");

        assert_eq!(
            artifact_path(&stdout, &BuildTarget::Bin("app".into())),
            Some(PathBuf::from("/t/debug/app"))
        );
    }

    #[test]
    fn the_cdylib_is_picked_over_its_rlib_and_dep_info_companions() {
        let stdout = json!({
            "reason": "compiler-artifact",
            "target": { "crate_types": ["cdylib"] },
            "executable": null,
            "filenames": ["/t/debug/libapp.rlib", "/t/debug/libapp.so", "/t/debug/libapp.d"],
        })
        .to_string();

        assert_eq!(
            artifact_path(&stdout, &BuildTarget::Cdylib),
            Some(PathBuf::from("/t/debug/libapp.so"))
        );
    }

    #[test]
    fn a_proc_macro_dependency_is_not_mistaken_for_the_cdylib() {
        // Every proc-macro in the dependency graph compiles to a shared
        // object, and cargo reports it before the crate being built.
        let stdout = [
            json!({
                "reason": "compiler-artifact",
                "target": { "crate_types": ["proc-macro"] },
                "executable": null,
                "filenames": ["/t/debug/deps/libserde_derive.so"],
            }),
            json!({
                "reason": "compiler-artifact",
                "target": { "crate_types": ["cdylib"] },
                "executable": null,
                "filenames": ["/t/debug/libapp.so"],
            }),
        ]
        .map(|v| v.to_string())
        .join("\n");

        assert_eq!(
            artifact_path(&stdout, &BuildTarget::Cdylib),
            Some(PathBuf::from("/t/debug/libapp.so"))
        );
    }

    #[test]
    fn a_dependency_that_is_itself_a_cdylib_is_left_in_deps() {
        // Cargo uplifts only the requested package's library; anything still
        // in `deps/` belongs to a dependency.
        let stdout = json!({
            "reason": "compiler-artifact",
            "target": { "crate_types": ["cdylib"] },
            "executable": null,
            "filenames": ["/t/debug/deps/libdep.so"],
        })
        .to_string();

        assert_eq!(artifact_path(&stdout, &BuildTarget::Cdylib), None);
    }

    #[test]
    fn non_json_lines_do_not_derail_artifact_parsing() {
        let stdout = format!(
            "not json\n{}\n",
            json!({ "reason": "compiler-artifact", "executable": "/t/debug/app", "filenames": [] })
        );
        assert_eq!(
            artifact_path(&stdout, &BuildTarget::Bin("app".into())),
            Some(PathBuf::from("/t/debug/app"))
        );
    }
}
