use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub fn exec(schema: impl AsRef<Path>, target: impl AsRef<Path>) -> Result<()> {
    let target = target.as_ref();

    // Make sure the target directory exists
    fs::create_dir_all(target)?;

    // Parse the schema file
    let schema = toasty_core::schema::from_file(schema).unwrap();
    let codegen_output = toasty_codegen::generate(&schema, true);

    let module_file = target.join("mod.rs");
    let module_src = gen_module(&codegen_output);

    // Generate the module file
    println!("  {:>10}    {}", "writing", module_file.display());
    fs::write(module_file, module_src)?;

    for model_codegen_output in &codegen_output.models {
        let model_file = target_file(target, &model_codegen_output.module_name.to_string());
        let source = model_codegen_output.body.to_string();
        let source = rustfmt(source);

        println!("  {:>10}    {}", "writing", model_file.display());
        fs::write(model_file, source)?;
    }

    Ok(())
}

fn gen_module(codegen: &toasty_codegen::Output) -> String {
    let mut lines = vec![];

    lines.push("#![allow(non_upper_case_globals, dead_code, warnings)]".to_string());

    for output in &codegen.models {
        let module = &output.module_name;
        lines.push(format!(
            "mod {};\npub use {}::{};",
            module,
            module,
            output.model.name.upper_camel_case()
        ));
    }

    rustfmt(lines.join("\n\n"))
}

fn rustfmt(source: String) -> String {
    use std::io;
    use std::io::prelude::*;
    use std::process::{Command, Stdio};

    let mut child = Command::new("rustfmt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .args(["--emit", "stdout", "--edition", "2021"])
        .spawn()
        .unwrap();
    let mut child_stdin = child.stdin.take().unwrap();
    let mut child_stdout = child.stdout.take().unwrap();

    // Spawn a thread to write to stdin
    let th = std::thread::spawn(move || {
        let _ = child_stdin.write_all(source.as_bytes());
        source
    });

    let mut fmted = vec![];
    io::copy(&mut child_stdout, &mut fmted).unwrap();

    let status = child.wait().unwrap();
    let _source = th.join().expect("thread feeding `rustfmt` panicked.");

    match status.code() {
        Some(0) => {}
        Some(2) => panic!("rustfmt parsing errors."),
        Some(3) => panic!("rustfmt failed to format"),
        _ => panic!("something went wrong"),
    }

    String::from_utf8(fmted).unwrap()
}

fn target_file(dir: &Path, module_name: &str) -> PathBuf {
    let file = format!("{}.rs", module_name);
    dir.join(file)
}
