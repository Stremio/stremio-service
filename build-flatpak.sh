#!/bin/sh
python flatpak/flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock
flatpak-builder --force-clean flatpak/build com.stremio.Service.json
flatpak build-export flatpak/repo flatpak/build
flatpak build-bundle flatpak/repo flatpak/com.stremio.Service.flatpak com.stremio.Service