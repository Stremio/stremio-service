#!/bin/sh

cwd="flatpak"
pkg_id="com.stremio.Service"

python3 $cwd/flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock -o $cwd/cargo-sources.json

server_version=$(sed -n '/package.metadata.server/,$p' Cargo.toml | grep -m 1 'version =' | awk -F'"' '{print $2}')
server_file_url=https://dl.strem.io/server/$server_version/desktop/server.js
sha256_checksum=$(curl -sL $server_file_url | shasum -a 256 | awk '{print $1}')
server_source_template='{
  "type": "file",
  "url": "{server_file_url}",
  "sha256": "{sha256_checksum}"
}'
echo "$server_source_template" | sed "s|{server_file_url}|$server_file_url|" | sed "s|{sha256_checksum}|$sha256_checksum|" > $cwd/server-source.json

sed -e 's/usr/app/g' -e 's/com.stremio.service/com.stremio.Service/g' resources/com.stremio.service.desktop > $cwd/$pkg_id.desktop

flatpak-builder --repo=$cwd/repo --force-clean $cwd/build $cwd/$pkg_id.json
flatpak build-bundle $cwd/repo $cwd/$pkg_id.flatpak $pkg_id