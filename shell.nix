with import <nixpkgs> { };

let
  image = dockerTools.pullImage {
    imageName = "espressif/idf-rust";
    imageDigest = "sha256:146ab2f7674cc5d3143db651c3adbc7fbb62a736302c018bc40b2378ca584936";
    hash = "sha256-PN0CwOtbyuLiwiFmH9IChDRm2KmUQYEEsZI/9hfH4ik==";
    finalImageName = "espressif/idf-rust";
    finalImageTag = "all_latest";
  };
  esp32 = stdenv.mkDerivation {
    name = "esp32";
    src = image;
    unpackPhase = ''
      mkdir -p source
      tar -C source -xvf $src
    '';
    sourceRoot = "source";
    nativeBuildInputs = [
      autoPatchelfHook
      jq
    ];

    buildInputs = [
      xz
      zlib
      libxml2_13
      python3
      libudev-zero
      stdenv.cc.cc
    ];

    buildPhase = ''
      jq -r '.[0].Layers | @tsv' < manifest.json > layers
    '';

    installPhase = ''
      mkdir -p $out
      for i in $(< layers); do
        tar -C $out -xvf "$i" home/esp/.cargo home/esp/.rustup || true
      done
      mv -t $out $out/home/esp/{.cargo,.rustup}
      rmdir $out/home/esp
      rmdir $out/home

      # [ -d $out/.cargo ] && [ -d $out/.rustup ]
    '';
  };
in

mkShell {
  nativeBuildInputs = [
    capnproto
    esp32
    # rustup
    rust-analyzer

    # Tools required to use ESP-IDF.
    git
    wget
    gnumake

    flex
    bison
    gperf
    pkg-config

    ninja
    libclang

    ncurses5

    python3
    python3Packages.pip
    python3Packages.virtualenv
    libudev-zero
    ldproxy

    espflash
    openssl
    trunk

    # (rust-bin.selectLatestNightlyWith (
    #   toolchain:
    #   toolchain.default.override {
    #     extensions = [ "rust-src" ];
    #     targets = [ "wasm32-unknown-unknown" ];
    #   }
    # ))
    # cargo-generate
  ];

  shellHook = ''
    export PATH=${esp32}/.rustup/toolchains/esp/bin:${esp32}/.rustup/toolchains/esp/xtensa-esp-elf-esp-13.2.0_20230928/stensa-esp-elf/bin:$PATH
    export RUST_SRC_PATH="$(rustc --print sysroot)/lib/rustlib/src/rust/src"
    export LIBCLANG_PATH=${libclang.lib}/lib
    export LD_LIBRARY_PATH="${
      lib.makeLibraryPath [
        libxml2_13
        zlib
        stdenv.cc.cc.lib
      ]
    }"
  '';

  # RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
}
