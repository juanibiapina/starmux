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
          # Expose only script so Darwin's BSD tools do not replace build tools.
          testScript = pkgs.writeShellScriptBin "script" ''
            exec ${pkgs.lib.getBin (if pkgs.stdenv.hostPlatform.isDarwin
              then pkgs.darwin.shell_cmds else pkgs.util-linux)}/bin/script "$@"
          '';
          starmux = pkgs.rustPlatform.buildRustPackage {
            pname = manifest.package.name;
            version = manifest.package.version;
            src = pkgs.lib.cleanSource self;
            cargoLock.lockFile = ./Cargo.lock;

            nativeCheckInputs = [ pkgs.gitMinimal pkgs.tmux testScript ];
            # Concurrent sandbox tests intermittently fail lease and socket checks.
            dontUseCargoParallelTests = true;
            # Provider tests serve HTTP responses over localhost.
            __darwinAllowLocalNetworking = pkgs.stdenv.hostPlatform.isDarwin;

            # Tests use host paths that are absent in a Linux Nix sandbox.
            postPatch = ''
              substituteInPlace src/tmux.rs \
                --replace-fail /bin/pwd ${pkgs.coreutils}/bin/pwd
              substituteInPlace tests/actions.rs \
                --replace-fail /usr/bin/touch ${pkgs.coreutils}/bin/touch
              substituteInPlace tests/tmux.rs \
                --replace-fail /bin/echo ${pkgs.coreutils}/bin/echo \
                --replace-fail /usr/bin/false ${pkgs.coreutils}/bin/false \
                --replace-fail /bin/sleep ${pkgs.coreutils}/bin/sleep
            '';

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
