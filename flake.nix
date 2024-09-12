# Based on https://github.com/nix-community/naersk/blob/master/examples/multi-target/flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, fenix, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pname = "gametemplate";
        pkgs = (import nixpkgs) { inherit system; };

        toolchain = with fenix.packages.${system};
          combine [
            stable.rustc
            stable.cargo
            stable.clippy
            stable.rust-analyzer
            stable.rust-src
            stable.rustfmt

            targets.x86_64-unknown-linux-gnu.stable.rust-std
            targets.wasm32-unknown-unknown.stable.rust-std
            targets.x86_64-pc-windows-gnu.stable.rust-std
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
          lib.makeLibraryPath [ libGL xorg.libX11 xorg.libXi libxkbcommon alsa-lib ];

      in rec {
        defaultPackage = packages.${pname};

        # For `nix build .#${pname}`
        packages.${pname} =
          naerskBuildPackage "x86_64-unknown-linux-gnu" {
            src = ./.;
            pname = "${pname}";
            doCheck = true;

            nativeBuildInputs = with pkgs; [ makeWrapper ];

            release = false;
            cargoBuildOptions = (x: x ++ [ "--profile=release-lto" ]);
            cargoTestOptions = (x: x ++ [ "--all" ]);

            postInstall = ''
              wrapProgram "$out/bin/${pname}" --prefix LD_LIBRARY_PATH : "${libPath}"
            '';
          };

        # For `nix build .#${pname}-tty`
        packages."${pname}-tty" =
          naerskBuildPackage "x86_64-unknown-linux-gnu" {
            src = ./.;
            pname = "${pname}-tty";
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

        # For `nix build .#${pname}-wasm`:
        packages."${pname}-wasm" =
          naerskBuildPackage "wasm32-unknown-unknown" {
            src = ./.;
            doCheck = false;
            strictDeps = true;

            release = false;
            cargoBuildOptions = (x: x ++ [ "--profile=release-lto" ]);
            cargoTestOptions = (x: x ++ [ "--all" ]);
          };

        # For `nix build .#${pname}-win`:
        packages."${pname}-win" =
          naerskBuildPackage "x86_64-pc-windows-gnu" {
            src = ./.;
            # FIXME: Unit test running doesn't work in cross-compile build
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
          inputsFrom = [ packages."${pname}" packages."${pname}-win" ];

          buildInputs = with pkgs; [
            cargo-outdated
            cargo-udeps

            # Needed by miniquad
            libGL
            xorg.libX11
            xorg.libXi
            libxkbcommon
            alsa-lib

            # JS minifier
            minify

            # Profiling
            linuxPackages.perf
            hotspot

            # Utils
            grafx2
            just
            neovim-qt
            optipng
            snzip
            tiled
          ];

          CARGO_BUILD_TARGET = "x86_64-unknown-linux-gnu";
          LD_LIBRARY_PATH = libPath;
          RUST_BACKTRACE = "1";
          RUST_LOG = "info";
        } // cargoConfig);
      });
}
