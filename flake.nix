# Based on https://github.com/nix-community/naersk/blob/master/examples/multi-target/flake.nix
{
  inputs = {
    fenix.url = "github:nix-community/fenix";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, fenix, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pname = "gametemplate";
        pkgs = (import nixpkgs) { inherit system; };

        toolchain = with fenix.packages.${system};
          combine [
            minimal.rustc
            minimal.cargo
            complete.clippy
            complete.rust-analyzer
            complete.rust-src
            complete.rustfmt

            targets.x86_64-unknown-linux-gnu.latest.rust-std
            targets.wasm32-unknown-unknown.latest.rust-std
            targets.x86_64-pc-windows-gnu.latest.rust-std
          ];

        naersk' = naersk.lib.${system}.override {
          cargo = toolchain;
          rustc = toolchain;
        };

        naerskBuildPackage = target: args:
          naersk'.buildPackage
          (args // { CARGO_BUILD_TARGET = target; } // cargoConfig);

        # All of the CARGO_* configurations which should be used for all
        # targets.
        #
        # Only use this for options which should be universally applied or which
        # can be applied to a specific target triple.
        #
        # This is also merged into the devShell.
        cargoConfig = {
          # Tells Cargo that it should use Wine to run tests.
          # (https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner)
          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER =
            pkgs.writeScript "wine-wrapper" ''
              export WINEPREFIX="$(mktemp -d)"
              exec wine64 $@
            '';
        };

        libPath = with pkgs;
          lib.makeLibraryPath [ libGL xorg.libX11 xorg.libXi alsa-lib ];

      in rec {
        defaultPackage = packages.gametemplate;

        # For `nix build .#gametemplate`
        packages.gametemplate =
          naerskBuildPackage "x86_64-unknown-linux-gnu" {
            src = ./.;
            pname = "gametemplate";
            doCheck = true;

            nativeBuildInputs = with pkgs; [ makeWrapper ];

            release = false;
            cargoBuildOptions = (x: x ++ [ "--profile=release-lto" ]);
            cargoTestOptions = (x: x ++ [ "--all" ]);

            postInstall = ''
              wrapProgram "$out/bin/${pname}" --prefix LD_LIBRARY_PATH : "${libPath}"
            '';
          };

        # For `nix build .#gametemplate-tty`
        packages.gametemplate-tty =
          naerskBuildPackage "x86_64-unknown-linux-gnu" {
            src = ./.;
            pname = "gametemplate-tty";
            doCheck = true;

            release = false;
            cargoBuildOptions = (x:
              x ++ [
                "--profile=release-lto"

                "--no-default-features"
                "--features=tty"
              ]);
            cargoTestOptions = (x: x ++ [ "--all" ]);

            # XXX: Is there a better way to specify target binary should be named differently for this target?
            postInstall = ''
              mv "$out/bin/${pname}" "$out/bin/${pname}-tty"
            '';
          };

        # For `nix build .#gametemplate-wasm`:
        packages.gametemplate-wasm =
          naerskBuildPackage "wasm32-unknown-unknown" {
            src = ./.;
            doCheck = false;
            strictDeps = true;

            release = false;
            cargoBuildOptions = (x: x ++ [ "--profile=release-lto" ]);
            cargoTestOptions = (x: x ++ [ "--all" ]);
          };

        # For `nix build .#gametemplate-win`:
        packages.gametemplate-win =
          naerskBuildPackage "x86_64-pc-windows-gnu" {
            src = ./.;
            # FIXME: Unit test running doensn't work
            #doCheck = true;
            strictDeps = true;

            release = false;
            cargoBuildOptions = (x: x ++ [ "--profile=release-lto" ]);
            cargoTestOptions = (x: x ++ [ "--all" ]);

            depsBuildBuild = with pkgs; [
              pkgsCross.mingwW64.stdenv.cc
              pkgsCross.mingwW64.windows.pthreads
            ];

            nativeBuildInputs = with pkgs;
              [
                # We need Wine to run tests:
                wineWowPackages.stable
              ];
          };

        devShell = pkgs.mkShell ({
          inputsFrom = with packages; [
            gametemplate
            gametemplate-win
          ];

          buildInputs = with pkgs; [
            cargo-outdated
            cargo-udeps

            # Needed by miniquad
            libGL
            xorg.libX11
            xorg.libXi
            alsa-lib

            # Profiling stuff, broken with 2023-01-22 flake.lock?
            #linuxPackages.perf
            #hotspot

            # Utils
            just
            tiled
            grafx2
          ];

          CARGO_BUILD_TARGET = "x86_64-unknown-linux-gnu";
          LD_LIBRARY_PATH = libPath;
          RUST_LOG = "info";
        } // cargoConfig);
      });
}
