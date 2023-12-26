fn main() {
    std::fs::write(
        "src/version.rs",
        format!("pub const GIT_HEAD: &'static str = \"{}\";", get_git_hash()),
    )
    .unwrap();
}

fn get_git_hash() -> String {
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--short")
        .arg("HEAD")
        .output()
        .expect("Couldn't get git HEAD");

    String::from_utf8(output.stdout)
        .expect("Failed to parse git HEAD")
        .trim()
        .into()
}
