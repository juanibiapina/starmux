{
  description = "A fast and configurable tmux sidebar";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          starmux = pkgs.rustPlatform.buildRustPackage {
            pname = manifest.package.name;
            version = manifest.package.version;
            src = pkgs.lib.cleanSource self;
            cargoLock.lockFile = ./Cargo.lock;

            # Project tests run in CI.
            doCheck = false;

            meta = {
              inherit (manifest.package) description homepage;
              license = pkgs.lib.licenses.mit;
              platforms = systems;
              mainProgram = "starmux";
            };
          };
        in
        {
          inherit starmux;
          default = starmux;
        });

      checks = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          package = self.packages.${system}.default;
        in
        {
          smoke = pkgs.runCommand "starmux-smoke" { } ''
            starmux_smoke_home="$TMPDIR/starmux-home"
            mkdir -p "$starmux_smoke_home"
            starmux() {
              env HOME="$starmux_smoke_home" XDG_CONFIG_HOME="$starmux_smoke_home/.config" \
                ${pkgs.lib.getExe package} "$@"
            }
            starmux --version > version
            grep -Fx 'starmux ${package.version}' version
            starmux check-config
            starmux init tmux > adapter
            grep -F 'render-query --width=#{e|-:#{side-status-width},1} --socket=' adapter
            touch "$out"
          '';
        });
    };
}
