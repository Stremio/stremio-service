#!/bin/sh
python flatpak/flatpak-cargo-generator.py Cargo.lock
flatpak-builder flatpak-build com.stremio.Service.json --force-clean
flatpak-builder --repo=stremio-flatpak-repo --force-clean flatpak-build com.stremio.Service.json
flatpak build-bundle stremio-flatpak-repo Service.flatpak com.stremio.Service master