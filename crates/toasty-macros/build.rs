use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-env-changed=TOASTY_GUIDE_URL");

    let url = env::var("TOASTY_GUIDE_URL")
        .unwrap_or_else(|_| "https://github.com/tokio-rs/toasty/tree/main/docs/guide".to_string());

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(
        out.join("guide_link.md"),
        format!("[Toasty guide]: {url}\n"),
    )
    .unwrap();
}
