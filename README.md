# Rust game template

Starter project with build tooling and current architecture best practices to
be used as base for Rust game projects.

Contents based on the [TCOD Roguelike
Tutorial](http://www.rogueliketutorials.com/tutorials/tcod/v2/).

The WASM build should be playable in browser at
<https://rsaarelm.github.io/rust-game-template/>.

## Features

- Uses [navni](https://github.com/rsaarelm/navni) to allow compiling for
  either into a GUI application or a terminal textmode application. You run
  the textmode version by compiling and running with

      cargo run --release --no-default-features --features=tty

- Uses [miniquad](https://github.com/not-fl3/miniquad) for GUI and WASM
  builds.

- A WASM build is automatically built and deployed using Github actions.

- Uses [hecs](https://github.com/Ralith/hecs) as entity-component-system for
  storing runtime entities.

- Uses [IDM](https://github.com/rsaarelm/idm) for data files.

- Automatically detects Colemak and Dvorak keyboard layouts and reconfigures
  movement keys accordingly.
