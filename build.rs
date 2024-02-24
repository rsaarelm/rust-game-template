use std::process::Command;

fn main() -> anyhow::Result<()> {
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

    let short = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()?
            .stdout,
    )?;

    let long = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()?
            .stdout,
    )?;

    let is_release_commit = long.starts_with("7e1ea5e");

    if is_release_commit {
        Ok(version.into())
    } else {
        Ok(format!("{version}-{short}"))
    }
}
