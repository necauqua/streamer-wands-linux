{
  # mostly copied from jujutsu (https://github.com/martinvonz/jj)
  description = "A dumb websocket client hack thing for the Noita mod Streamer Wands";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }: {
    overlays.default = (final: prev: {
      streamer-wands-linux = self.packages.${final.system}.streamer-wands-linux;
    });
  } //
  (flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [
          rust-overlay.overlays.default
        ];
      };

      filterSrc = src: regexes:
        pkgs.lib.cleanSourceWith {
          inherit src;
          filter = path: type:
            let
              relPath = pkgs.lib.removePrefix (toString src + "/") (toString path);
            in
            pkgs.lib.all (re: builtins.match re relPath == null) regexes;
        };

      rust-version = pkgs.rust-bin.stable.latest.default;

      rust-platform = pkgs.makeRustPlatform {
        rustc = rust-version;
        cargo = rust-version;
      };

      systemDeps = [ ];
      darwinDeps = with pkgs; lib.optionals stdenv.isDarwin [ ];

      buildDeps = with pkgs; lib.optionals (systemDeps != [ ]) [ pkg-config ];
    in
    {
      packages = {
        streamer-wands-linux = rust-platform.buildRustPackage {
          pname = "streamer-wands-linux";
          version = "unstable-${self.shortRev or "dirty"}";
          src = filterSrc ./. [
            ".*\\.nix$"
            "^.jj/"
            "^flake\\.lock$"
            "^target/"
          ];

          cargoLock.lockFile = ./Cargo.lock;
          useNextest = true;

          nativeBuildInputs = buildDeps ++ [ pkgs.installShellFiles ];
          buildInputs = systemDeps ++ darwinDeps;

          # makes no sense in a nix package
          CARGO_INCREMENTAL = "0";

          preCheck = "export RUST_BACKTRACE=1";

          postInstall = ''
            $out/bin/streamer-wands-linux --man > ./streamer-wands-linux.1
            installManPage ./streamer-wands-linux.1

            installShellCompletion --cmd streamer-wands-linux \
              --bash <($out/bin/streamer-wands-linux --completion bash) \
              --fish <($out/bin/streamer-wands-linux --completion fish) \
              --zsh  <($out/bin/streamer-wands-linux --completion zsh)
          '';
        };
        default = self.packages.${system}.streamer-wands-linux;
      };
      apps.default = {
        type = "app";
        program = "${self.packages.${system}.streamer-wands-linux}/bin/streamer-wands-linux";
      };
      formatter = pkgs.nixpkgs-fmt;
      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rust-version

          # Make sure rust-analyzer is present
          rust-analyzer

          cargo-nextest
          # cargo-insta
          # cargo-deny
        ] ++ buildDeps ++ systemDeps ++ darwinDeps;

        shellHook = ''
          export RUST_BACKTRACE=1
        '';
      };
    }));
}
