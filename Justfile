update-toolchain:
    #! /usr/bin/env nu

    nix-shell -p nix-prefetch-docker --command "nix-prefetch-docker espressif/idf-rust all_latest"

flash:
    cargo build
    espflash flash -B 921600 -M target/xtensa-esp32-espidf/debug/rustgblitz

flash_slow:
    cargo build
    espflash flash -B 115200 -M target/xtensa-esp32-espidf/debug/rustgblitz

flash_release:
    cargo build --release
    espflash flash -B 921600 -M target/xtensa-esp32-espidf/release/rustgblitz

mon:
    espflash monitor

test:
    cargo +stable-x86_64-unknown-linux-gnu test -p colorutils --target x86_64-unknown-linux-gnu
