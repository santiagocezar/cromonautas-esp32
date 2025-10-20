flash:
    cargo build
    espflash flash -M target/xtensa-esp32-espidf/debug/rustgblitz

flash_release:
    cargo build --release
    espflash flash -M target/xtensa-esp32-espidf/release/rustgblitz

mon:
    espflash monitor

test:
    cargo +stable-x86_64-unknown-linux-gnu test -p colorutils --target x86_64-unknown-linux-gnu