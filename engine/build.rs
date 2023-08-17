// Bake data directory contents into a single snap-packaged IDM file.

fn main() {
    // Make sure build.rs gets rerun if the output file disappears.
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../target/data.idm.z");
    let data = util::directory_to_idm("../data").unwrap();
    let z = fdeflate::compress_to_vec(data.as_bytes());
    std::fs::write("../target/data.idm.z", z).unwrap();
}