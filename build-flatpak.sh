#!/bin/sh
python flatpak/flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock -o flatpak/cargo-sources.json

server_version=$(sed -n '/package.metadata.server/,$p' Cargo.toml | grep -m 1 'version =' | awk -F'"' '{print $2}')
server_file_url=https://dl.strem.io/server/$server_version/desktop/server.js
sha256_checksum=$(curl -sL $server_file_url | shasum -a 256 | awk '{print $1}')
server_source_template='{
  "type": "file",
  "url": "{server_file_url}",
  "sha256": "{sha256_checksum}"
}'
echo "$server_source_template" | sed "s|{server_file_url}|$server_file_url|" | sed "s|{sha256_checksum}|$sha256_checksum|" > flatpak/server-source.json

flatpak-builder --force-clean flatpak/build com.stremio.Service.json
flatpak build-export flatpak/repo flatpak/build
flatpak build-bundle flatpak/repo flatpak/com.stremio.Service.flatpak com.stremio.Service