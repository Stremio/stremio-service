#!/bin/sh
python flatpak/flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock
flatpak-builder flatpak/flatpak-build com.stremio.Service.json --force-clean
flatpak-builder --repo=flatpak/stremio-flatpak-repo --force-clean flatpak/flatpak-build com.stremio.Service.json
flatpak build-bundle flatpak/stremio-flatpak-repo flatpak/com.stremio.Service.flatpak com.stremio.Service master