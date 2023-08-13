# Run game in TTY terminal mode.
run-tty *ARGS:
    @cargo run --release --features=tty --no-default-features -- {{ARGS}}

run *ARGS:
    @cargo run --release -- {{ARGS}}

# Build a WASM version
build-wasm:
    nix build .#gametemplate-wasm
    cp result/bin/gametemplate.wasm web/
    chmod u+w web/gametemplate.wasm

# Cross-compile a Windows executable
build-win:
    @nix build .#gametemplate-win
    @echo Built windows executable in result/bin/

# Update pinned nix flake programs.
update-flake:
    nix flake update

# Update Rust dependencies
update-cargo:
    cargo update

# Make git do automated tests before commit and push
register-githooks:
    git config --local core.hooksPath githooks/
