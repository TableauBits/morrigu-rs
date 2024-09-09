let
  rustOverlay = builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz";
  pkgs = import <nixpkgs> {
    overlays = [ (import rustOverlay) ];
  };

in
pkgs.mkShell {
  name = "morrigu-rs nix dev shell";

  buildInputs = with pkgs; [
    (rust-bin.stable.latest.default.override {
      extensions = [ "rust-src" "rust-analyzer" ];
    })

    cmake
    python3
    libxkbcommon
    shaderc
    shaderc.bin
    shaderc.static
    shaderc.dev
    shaderc.lib
    vulkan-headers
    vulkan-loader
    vulkan-tools
    vulkan-validation-layers
    wayland

    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
  ];

  LD_LIBRARY_PATH="${pkgs.vulkan-loader}/lib:${pkgs.shaderc.lib}/lib:${pkgs.shaderc.dev}/lib:${pkgs.xorg.libX11}/lib:${pkgs.xorg.libXcursor}/lib:${pkgs.xorg.libXrandr}/lib:${pkgs.xorg.libXi}/lib:${pkgs.wayland}/lib:${pkgs.libxkbcommon}/lib";
  VK_LAYER_PATH="${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d/";
  VULKAN_LIB_DIR="${pkgs.shaderc.dev}/lib";
  RUST_BACKTRACE = 1;
}
