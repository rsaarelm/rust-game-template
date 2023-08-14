// Standard getradnom doesn't work for the minimal WASM build, add a crappy
// custom one.
//
// This should go in your toplevel application binary crate and go with this
// put in your Cargo.toml:
//
// [target.'cfg(target_arch = "wasm32")'.dependencies]
// getrandom = { version = "0.2", features = ["custom"] }
// quad-rand = "0.2.1"

#[cfg(target_arch = "wasm32")]
fn getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    for value in buf {
        *value = quad_rand::rand() as u8;
    }
    Ok(())
}
#[cfg(target_arch = "wasm32")]
getrandom::register_custom_getrandom!(getrandom);
