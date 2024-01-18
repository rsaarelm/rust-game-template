// Bake data directory contents into a single snap-packaged IDM file.

fn main() {
    // Make sure build.rs gets rerun if the output file disappears.
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../target/data.idm.sz");
    let data = util::dir_to_idm("../data").unwrap().to_string();
    // Save the uncompressed version for debugging.
    std::fs::write("../target/data.idm", data.as_bytes()).unwrap();
    // Save compressed data for embedding in game binary.
    let sz = snap::raw::Encoder::new()
        .compress_vec(data.as_bytes())
        .unwrap();
    std::fs::write("../target/data.idm.sz", sz).unwrap();
}
