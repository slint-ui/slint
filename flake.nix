{
  inputs.nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";

  outputs = {
    self,
    nixpkgs,
  }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {inherit system;};
  in {
    packages.${system} = with pkgs; {
    };
    devShells.${system} = {
      default = with pkgs;
        mkShell {
          nativeBuildInputs = [renderdoc];
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
            pkg-config
            udev
            libGL
            seatd
            libgbm
            libinput
            freetype
            fontconfig
            nodejs
            pnpm
          ];
          LD_LIBRARY_PATH = lib.makeLibraryPath [
            fontconfig
            wayland
            libxkbcommon
            libGL

            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr # To use the x11 feature
          ];
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
