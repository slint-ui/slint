# cSpell:ignore stdenv
{
  inputs.nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = [ "x86_64-linux" "aarch64-darwin" "aarch64-linux" ];
    mapListToAttrs = list: func:
        builtins.listToAttrs (builtins.map (v: { name = v; value = (func v); }) list);
  in {
    devShells = mapListToAttrs systems (system:
    let
      inherit (pkgs) lib;
      pkgs = import nixpkgs {inherit system;};
    in
    {
      default = with pkgs; let
        runtime-libs = [
          fontconfig
          libxkbcommon
          libGL

          libx11
          libxcursor
          libxi
          libxrandr
          vulkan-loader
        ] ++
        (
          lib.optionals pkgs.stdenv.hostPlatform.isLinux
          [
            wayland
          ]
        );
      in
        mkShell {
          nativeBuildInputs = [
            pkg-config
          ] ++ (lib.optional pkgs.stdenv.hostPlatform.isLinux perf);
          hardeningDisable = ["fortify"];
          buildInputs = [
            # Not strictly required, but helps with
            # https://github.com/NixOS/nixpkgs/issues/370494
            rust-jemalloc-sys
            libxkbcommon
            openssl
            libGL
            freetype
            fontconfig
            nodejs
            pnpm

            fontconfig
            runtime-libs
          ] ++ lib.optionals pkgs.stdenv.hostPlatform.isLinux [
            libgbm
            libinput
            # Merge the qt packages together to make a lighter version of qt6.full
            (symlinkJoin {
              name = "qt packages";
              paths = [
                qt6.qtbase
                # Required for 'QT_QPA_PLATFORM=wayland' to work
                qt6.qtwayland
              ];
            })
            seatd
            udev

            alsa-lib
          ];
          LD_LIBRARY_PATH = lib.makeLibraryPath runtime-libs;
        };
      spelling = with pkgs;
        mkShell {
          buildInputs = [
            (aspellWithDicts (d: [d.en]))
          ];
        };
    });
  };
}
