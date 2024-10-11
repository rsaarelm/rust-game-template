pname := "gametemplate"

# Run game in TTY terminal mode.
run-tty *ARGS:
    @cargo run --release --features=tty --no-default-features -- {{ARGS}}

run *ARGS:
    @cargo run --release -- {{ARGS}}

# Spin up a test web server to run the WASM binary
serve: build-wasm
    @cargo install basic-http-server
    @echo Starting WASM game server at http://localhost:4000/
    ~/.cargo/bin/basic-http-server web/

# Build a WASM version
build-wasm:
    cargo build --target=wasm32-unknown-unknown --profile=release-lto
    cp target/wasm32-unknown-unknown/release-lto/{{pname}}.wasm web/

# Build a WASM version (use nix build)
build-wasm-nix:
    nix build .#{{pname}}-wasm
    cp result/bin/{{pname}}.wasm web/
    chmod u+w web/{{pname}}.wasm

# Cross-compile a Windows executable
build-win:
    @nix build .#{{pname}}-win
    @echo Built windows executable in result/bin/

profile-debug *ARGS:
    @cargo build
    perf record -- ./target/x86_64-unknown-linux-gnu/debug/{{pname}} {{ARGS}}
    hotspot ./perf.data

profile-release *ARGS:
    @cargo build --release
    perf record -- ./target/x86_64-unknown-linux-gnu/release/{{pname}} {{ARGS}}
    hotspot ./perf.data

# Extract Tiled JSON map from IDM map.
extract-json-map idm-map:
    cargo run --example tiled-export -- extract {{idm-map}}

# Inject changes from JSON map back into IDM map.
inject-json-map json-map:
    cargo run --example tiled-export -- inject {{json-map}}

# Display current saved game as plaintext
show-save:
    snunzip -ct raw ~/.local/share/gametemplate/saved.idm.sz

# Update pinned nix flake programs.
update-flake:
    rm -rf .direnv/
    nix flake update

# Update Rust dependencies
update-cargo:
    cargo update

# Create an .envrc file that uses the Nix flake as direnv.
setup-envrc:
    #!/bin/sh
    if [ ! -f .envrc ]; then
        echo "use flake" > .envrc
    else
        echo ".envrc exists" >&2
    fi

# Get latest versions of the JS shim files and minify them.
generate-minified-js:
    #!/bin/sh
    OUT=$(pwd)/web
    TMPDIR=$(mktemp -d)
    cd $TMPDIR

    wget https://raw.githubusercontent.com/not-fl3/quad-snd/master/js/audio.js
    wget https://raw.githubusercontent.com/not-fl3/miniquad/master/js/gl.js
    wget https://raw.githubusercontent.com/optozorax/quad-storage/master/js/quad-storage.js
    wget https://raw.githubusercontent.com/not-fl3/sapp-jsutils/master/js/sapp_jsutils.js

    minify audio.js > $OUT/audio.js
    minify gl.js > $OUT/gl.js
    minify quad-storage.js > $OUT/quad-storage.js
    minify sapp_jsutils.js > $OUT/sapp_jsutils.js

update-dependencies: update-cargo generate-minified-js update-flake

# Make git do automated tests before commit and push
register-githooks:
    git config --local core.hooksPath githooks/
