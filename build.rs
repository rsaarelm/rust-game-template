use std::process::Command;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=src/version.rs");
    std::fs::write(
        "src/version.rs",
        format!(
            "// THIS FILE IS GENERATED BY build.rs, DO NOT EDIT OR PLACE IN VERSION CONTROL.\npub const GIT_HEAD: &'static str = \"{}\";\n",
            get_git_hash()?
        ),
    )?;
    Ok(())
}

fn get_git_hash() -> anyhow::Result<String> {
    let version = env!("CARGO_PKG_VERSION");

    let hash = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()?
            .stdout,
    )?;

    let message = String::from_utf8(
        Command::new("git")
            .args(["show", "-s", "--format=%s"])
            .output()?
            .stdout,
    )?;

    if message.starts_with("Release ") {
        Ok(version.into())
    } else {
        Ok(format!("{version}-{}", hash.trim()))
    }
}
