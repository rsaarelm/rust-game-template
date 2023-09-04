# Rust game template

Starter project with build tooling and current architecture best practices to
be used as base for Rust game projects.

Contents based on the [TCOD Roguelike
Tutorial](http://www.rogueliketutorials.com/tutorials/tcod/v2/).

The WASM build should be playable in browser at
<https://rsaarelm.github.io/rust-game-template/>.

## Instructions

Install the [Rust compiler toolchain](https://www.rust-lang.org/tools/install)
and call `rustup install nightly` to install the nightly version of the
compiler.

Build and run the desktop GUI version:

    cargo +nightly --release run

Build and run the TTY terminal version:

    cargo +nightly --release --no-default-features --features=tty run

If you're using NixOS, you can run `nix develop` in the project directory to
enter a development shell and then call `just run` or `just run-tty`.

## Features

- Uses [navni](https://github.com/rsaarelm/navni) to allow compiling for
  either into a GUI application or a terminal textmode application.

- Uses [miniquad](https://github.com/not-fl3/miniquad) for GUI and WASM
  builds.

- A WASM build is automatically built and deployed using Github actions.

- Uses [hecs](https://github.com/Ralith/hecs) as entity-component-system for
  storing runtime entities.

- Uses [IDM](https://github.com/rsaarelm/idm) for data files.

- Automatically detects Colemak and Dvorak keyboard layouts and reconfigures
  movement keys accordingly.
