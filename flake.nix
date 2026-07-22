{
  inputs.nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";

  outputs = {
    self,
    nixpkgs,
  }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {inherit system;};
  in {
    devShells.${system} = {
      default = with pkgs; let
        runtime-libs = [
          fontconfig
          wayland
          libxkbcommon
          libGL

          libx11
          libxcursor
          libxi
          libxrandr
          vulkan-loader
        ];
      in
        mkShell {
          nativeBuildInputs = [
            pkg-config
            perf
          ];
          hardeningDisable = ["fortify"];
          buildInputs = [
            # Not strictly required, but helps with
            # https://github.com/NixOS/nixpkgs/issues/370494
            rust-jemalloc-sys
            # Merge the qt packages together to make a lighter version of qt6.full
            (symlinkJoin {
              name = "qt packages";
              paths = [
                qt6.qtbase
                # Required for 'QT_QPA_PLATFORM=wayland' to work
                qt6.qtwayland
              ];
            })
            libxkbcommon
            openssl
            udev
            libGL
            seatd
            libgbm
            libinput
            freetype
            fontconfig
            nodejs
            pnpm

            alsa-lib
            fontconfig
            runtime-libs
          ];
          LD_LIBRARY_PATH = lib.makeLibraryPath runtime-libs;
        };
      spelling = with pkgs;
        mkShell {
          buildInputs = [
            (aspellWithDicts (d: [d.en]))
          ];
        };
    };
  };
}
