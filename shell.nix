{ pkgs ? import <nixpkgs> {}
  , fetchzip ? pkgs.fetchzip
  , fetchgit ? pkgs.fetchgit
  , fetchurl ? pkgs.fetchurl
  , unzip ? pkgs.unzip
}:

pkgs.mkShell rec {
  buildInputs = [
    pkgs.gnumake
    pkgs.python39
    pkgs.gcc-arm-embedded
    pkgs.poetry
    pkgs.cacert
    (pkgs.callPackage custom_nix_pkgs/analog_openocd.nix { })
    pkgs.minicom
    pkgs.clang_14
    pkgs.llvmPackages.bintools
    pkgs.rustup
    pkgs.cargo
    pkgs.glibc_multi.dev
    pkgs.rsync 
  ];

  RUSTC_VERSION = pkgs.lib.readFile ./rust-toolchain.toml;

  # https://github.com/rust-lang/rust-bindgen#environment-variables
  LIBCLANG_PATH = pkgs.lib.makeLibraryPath [ pkgs.llvmPackages_latest.libclang.lib ];
  HISTFILE = toString ./.history;
  shellHook =
    ''
      export NIX_ENFORCE_PURITY=0
      export PATH=$PATH:''${CARGO_HOME:-~/.cargo}/bin
      export PATH=$PATH:''${RUSTUP_HOME:-~/.rustup}/toolchains/$RUSTC_VERSION-x86_64-unknown-linux-gnu/bin/
      export GCC_ARM_EMBEDDED_LIB=${pkgs.gcc-arm-embedded}/arm-none-eabi/lib
      cargo install cargo-make
    '';

  # Add libvmi precompiled library to rustc search path
  # RUSTFLAGS = (builtins.map (a: ''-L ${a}/lib'') [
  #   pkgs.libvmi
  # ]);
  # Add libvmi, glibc, clang, glib headers to bindgen search path
  BINDGEN_EXTRA_CLANG_ARGS = [''-I"${pkgs.gcc-arm-embedded}/arm-none-eabi/include"''
    ''-I"${pkgs.llvmPackages_latest.libclang.lib}/lib/clang/${pkgs.llvmPackages_latest.libclang.version}/include"''
  ];

}
