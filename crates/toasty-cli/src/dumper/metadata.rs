//! Resolve information about the user's package via `cargo metadata`.

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};

/// Metadata captured from the user's project for synthesizing the dumper.
pub(super) struct ProjectMetadata {
    /// The resolved cargo target directory. Honors `CARGO_TARGET_DIR`,
    /// `[build] target-dir` in cargo config, and workspace-level overrides
    /// — we cannot assume `<workspace_root>/target`.
    pub target_directory: PathBuf,

    /// The user's root package — the one whose schema we are extracting.
    pub package: PackageInfo,

    /// Resolved info for the `toasty` dependency.
    pub toasty: PackageDep,
}

pub(super) struct PackageInfo {
    pub name: String,
    pub edition: String,
    /// Directory containing the package's `Cargo.toml`.
    pub manifest_dir: PathBuf,
    /// Whether the package has a library target.
    pub has_lib: bool,
}

/// A resolved version (and possibly local path) for a workspace dep.
pub(super) struct PackageDep {
    pub version: String,
    /// Set when the dependency resolves to a local path (e.g., a path
    /// dependency or workspace member). When `None`, the registry version
    /// is used.
    pub path: Option<PathBuf>,
    /// Features the user's package enables on this dep. The dumper crate
    /// mirrors these so it builds with the same `toasty` configuration the
    /// user's crate was compiled against — otherwise feature-gated `Model`
    /// fields (e.g., jiff types) would not match.
    pub features: Vec<String>,
    /// Whether the user's dep entry uses default features.
    pub default_features: bool,
}

pub(super) fn load(project_root: &Path) -> Result<ProjectMetadata> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .current_dir(project_root)
        .exec()
        .context("running `cargo metadata` on the user's project")?;

    let root_id = metadata
        .resolve
        .as_ref()
        .and_then(|r| r.root.as_ref())
        .ok_or_else(|| {
            anyhow!("cargo metadata did not report a root package — virtual workspaces are not supported yet")
        })?
        .clone();

    let root_pkg = metadata
        .packages
        .iter()
        .find(|p| p.id == root_id)
        .ok_or_else(|| anyhow!("root package id {root_id} not present in metadata.packages"))?;

    let manifest_dir = root_pkg
        .manifest_path
        .parent()
        .ok_or_else(|| anyhow!("root manifest path has no parent"))?
        .as_std_path()
        .to_path_buf();

    let has_lib = root_pkg.targets.iter().any(|t| {
        t.kind.iter().any(|k| {
            matches!(
                k,
                cargo_metadata::TargetKind::Lib | cargo_metadata::TargetKind::RLib
            )
        })
    });

    let toasty = find_dep(&metadata, root_pkg, "toasty")?;

    Ok(ProjectMetadata {
        target_directory: metadata.target_directory.into_std_path_buf(),
        package: PackageInfo {
            name: root_pkg.name.to_string(),
            edition: root_pkg.edition.to_string(),
            manifest_dir,
            has_lib,
        },
        toasty,
    })
}

fn find_dep(
    metadata: &cargo_metadata::Metadata,
    root_pkg: &cargo_metadata::Package,
    name: &str,
) -> Result<PackageDep> {
    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == name)
        .ok_or_else(|| anyhow!("`{name}` is not in the resolved dependency graph — is it listed in your Cargo.toml?"))?;

    // A path/workspace dep has `source == None`; registry deps have a source.
    let path = if pkg.source.is_none() {
        Some(
            pkg.manifest_path
                .parent()
                .ok_or_else(|| anyhow!("`{name}` manifest path has no parent"))?
                .as_std_path()
                .to_path_buf(),
        )
    } else {
        None
    };

    // Pick up the feature set the user enabled on this dep in their
    // Cargo.toml. Skip dev/build-only entries.
    let dep_entry = root_pkg
        .dependencies
        .iter()
        .find(|d| d.name == name && matches!(d.kind, cargo_metadata::DependencyKind::Normal));

    let (features, default_features) = match dep_entry {
        Some(d) => (d.features.clone(), d.uses_default_features),
        None => (Vec::new(), true),
    };

    Ok(PackageDep {
        version: pkg.version.to_string(),
        path,
        features,
        default_features,
    })
}
