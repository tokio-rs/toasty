use anyhow::Result;
use std::path::PathBuf;

pub fn create_example(name: Option<String>) -> Result<PathBuf> {
    let target_dir = std::env::current_dir().unwrap();

    if let Some(name) = name {
        if target_dir.ends_with("toasty") {
            return Ok(target_dir.join("toasty-examples").join("examples").join(name));
        } else if target_dir.ends_with("examples") {
            return Ok(target_dir.join(name));
        } else {
            Err(anyhow::anyhow!(
                "You must be in the toasty or examples directory to create an example"
            ))
        }
    } else {
        // check if we are in a subdirectory of the examples directory
        if target_dir.parent().unwrap().ends_with("examples") {
            return Ok(target_dir.to_path_buf());
        } else {
            Err(anyhow::anyhow!(
                "You must be in the examples directory to create an example"
            ))
        }
    }
}


pub fn generate_main_file_for_example(target: &PathBuf) -> Result<()> {
    const MAIN_TEMPLATE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/main.rs"));

    let main_file = target.join("main.rs");
    let main_src = MAIN_TEMPLATE.replace("EXAMPLE_NAME", &target.file_name().unwrap().to_str().unwrap());

    std::fs::write(main_file, main_src)?;
   

    Ok(())
}