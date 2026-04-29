//! Build and execute the synthesized dumper crate, then parse its output.

use super::synth::Synth;
use anyhow::{Context, Result, anyhow, bail};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use toasty_core::schema::app;

/// Build the dumper crate, run it, and deserialize its stdout as
/// [`app::Schema`].
pub(super) fn build_and_run(synth: &Synth) -> Result<app::Schema> {
    let target_dir = synth.root.join("target");

    let output = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(&synth.manifest_path)
        .arg("--bin")
        .arg("toasty-dumper")
        .arg("--message-format=json-render-diagnostics")
        .env("CARGO_TARGET_DIR", &target_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("invoking `cargo build` for the dumper crate")?;

    if !output.status.success() {
        bail!(
            "`cargo build` for the dumper crate failed with status {}",
            output.status
        );
    }

    let bin_path = parse_dumper_executable(&output.stdout)?;

    let run = Command::new(&bin_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("running dumper binary at {}", bin_path.display()))?;

    if !run.status.success() {
        bail!("dumper binary exited with status {}", run.status);
    }

    serde_json::from_slice::<app::Schema>(&run.stdout)
        .context("deserializing dumper output as `app::Schema` JSON")
}

fn parse_dumper_executable(stdout: &[u8]) -> Result<PathBuf> {
    let text =
        std::str::from_utf8(stdout).context("`cargo build` produced non-UTF-8 output on stdout")?;

    for line in text.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if v.get("reason").and_then(|r| r.as_str()) != Some("compiler-artifact") {
            continue;
        }

        let target_name = v
            .get("target")
            .and_then(|t| t.get("name"))
            .and_then(|n| n.as_str());

        if target_name != Some("toasty-dumper") {
            continue;
        }

        if let Some(exe) = v.get("executable").and_then(|e| e.as_str()) {
            return Ok(PathBuf::from(exe));
        }
    }

    Err(anyhow!(
        "did not find a `compiler-artifact` line for `toasty-dumper` in cargo output"
    ))
}
