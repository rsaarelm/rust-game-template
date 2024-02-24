# Run game in TTY terminal mode.
run-tty *ARGS:
    @cargo run --release --features=tty --no-default-features -- {{ARGS}}

run *ARGS:
    @cargo run --release -- {{ARGS}}

# Spin up a test web server to run the WASM binary
run-wasm: build-wasm
    @cargo install basic-http-server
    @echo Starting WASM game server at http://localhost:4000/
    ~/.cargo/bin/basic-http-server web/

# Build a WASM version
build-wasm:
    cargo build --target=wasm32-unknown-unknown --profile=release-lto
    cp target/wasm32-unknown-unknown/release-lto/gametemplate.wasm web/

# Build a WASM version (use nix build)
build-wasm-nix:
    nix build .#gametemplate-wasm
    cp result/bin/gametemplate.wasm web/
    chmod u+w web/gametemplate.wasm

# Cross-compile a Windows executable
build-win:
    @nix build .#gametemplate-win
    @echo Built windows executable in result/bin/

profile-debug *ARGS:
    @cargo build
    perf record -- ./target/x86_64-unknown-linux-gnu/debug/gametemplate {{ARGS}}
    hotspot ./perf.data

profile-release *ARGS:
    @cargo build --release
    perf record -- ./target/x86_64-unknown-linux-gnu/release/gametemplate {{ARGS}}
    hotspot ./perf.data

# Force release git commit to have specific prefix
force-ref:
    go run github.com/bradfitz/gitbrute@54f9ed31374ed1b70f48c7bfc4fe83deea59c7c6 --prefix 7e1ea5e

# Update pinned nix flake programs.
update-flake:
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

# Make git do automated tests before commit and push
register-githooks:
    git config --local core.hooksPath githooks/
