# Stremio Service

[![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/stremio/stremio-service/build.yml?label=build%20(master))](https://github.com/Stremio/stremio-service/actions/workflows/build.yml?query=branch%3Amaster)

## Features

- `default` features - none
- `bundled` - uses binaries location for an installed(and bundled) application.

## Download

You can find the Stremio Service packages in the [releases asset files](https://github.com/Stremio/stremio-service/releases) or by using one of the following urls.

_For `dl.strem.io` urls replace `{VERSION}` with the latest release version of Stremio Service in the format `v*.*.*`._

- MacOS: https://dl.strem.io/stremio-service/{VERSION}/StremioService.dmg
- Windows: https://dl.strem.io/stremio-service/{VERSION}/StremioServiceSetup.exe
- Debian: https://dl.strem.io/stremio-service/{VERSION}/stremio-service_amd64.deb
- Redhat: https://dl.strem.io/stremio-service/{VERSION}/stremio-service_x86_64.rpm
- Flatpak package: https://flathub.org/apps/com.stremio.Service

## Development

```
git clone --recurse-submodules https://github.com/Stremio/stremio-service
```

### Requirements

#### Windows

Download & Install [Inno Setup](https://jrsoftware.org/isdl.php).


#### Ubuntu

```
apt install build-essential libgtk-3-dev pkg-config libssl-dev libayatana-appindicator3-dev
```

```
cargo install cargo-deb
```

#### Fedora
```
dnf install gtk3-devel
```
```
cargo install generate-rpm
```

#### MacOS

```
npm install -g create-dmg && brew install graphicsmagick imagemagick
```

### Run

By default the `stremio-service` binary is ran with `info` log level:

```
RUST_LOG=info cargo run
```

### Build

```
cargo build --release
```

### Package

#### Windows

Build the binaries on Windows in release using the `bundled` feature.

```
cargo build --release --features=bundled
```

Run the Inno Setup compiler `ISCC` command inside `Command Prompt` or `PowerShell` against the `StremioService.iss` script. Depending on your installation the path to `IISC` may vary. Here is an example with the default installation path, presuming your current working directory is the project's root:

```
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "setup\StremioService.iss"
```
If you use `PowerShell` you need to prepend `&` in the beginning of the line.
A new executable should be produced - `StremioServiceSetup.exe`


##### Cross-compilation from Linux

1. For cross-compiling on Linux, you need to add the `x86_64-pc-windows-gnu` target:

```
rustup target add x86_64-pc-windows-gnu
```

2. And build the binary using the `bundled` feature:

```
cargo build --release --target x86_64-pc-windows-gnu --features=bundled
```

**NOTE:** The Windows installed can **not** be built on other platforms, only **Windows**.

#### Ubuntu

```
cargo deb
```

#### Fedora

`cargo-generate-rpm` does not not build the binary nor strips debugging symbols as of version `0.9.1`.

This is why we need to first build the release (with the `bundled` feature):

```
cargo build --release --features=bundled
```

Strip the debugging symbols:

```
strip -s target/release/stremio-service
```

And finally run the `generate-rpm` cargo subcommand:

```
cargo generate-rpm
```

#### Flatpak

The Manifest is located [com.stremio.Service.json](./com.stremio.Service.json) and you can bundle the application using the script:

`./build-flatpak.sh`

#### MacOS

Use either `cargo run --bin bundle-macos` or its alias `cargo macos` to build the MacOS `.app` and then build the `dmg` package:

```
cargo macos && create-dmg --overwrite target/macos/*.app target/macos
```

## Releasing new version

### Release

1. Bump version and update Flatpak
- Bump version in `Cargo.toml`
- Flatpak packages - necessary to add the new version and it's date to the [com.stremio.Service.appdata.xml](./resources/com.stremio.service.metainfo.xml) file.
- Commit `Cargo.toml`, `Cargo.lock` and `resources/com.stremio.service.metainfo.xml`.

2. Make a new tag

`git tag -a v0.XX.XX -m "Service v0.XX.XX"`

3. Push it to the repo

`git push -u origin v0.XX.XX`

4. The [`release` workflow](./.github/workflows/release.yml) will be triggered

### Manual

The `generate_descriptor.js` script is used to generate new version descriptor and upload it to s3. This script is automatically called in the release workflows for Mac OS and Windows. The default behavior is to find the latest artifacts and generate a release candidate descriptor.

### Quick release example

Assuming the release actions finished successfully there will be already a release candidate descriptor. It can be tested by running the service with the `--release-candidate` argument and it should update. If so invoking the `generate_descriptor.js` script with `--release` flag will publish the descriptor to the release channel:

```
C:\stremio-service> node .\generate_descriptor.js --tag=v0.1.0 --release
Descriptor for tag v0.1.0 already exists in the RC folder. Moving it to the releases folder
Done
C:\stremio-service>
```

### Detailed description

In order to run the script the AWS Command Line Interface must be installed on the system and properly configured with credentials that have write permissions to the bucket.

If the `--release` flag is passed the release candidate is copied to the releases destination thus releasing a new version. If there is no release candidate yet for some reason a new release descriptor is generated skipping the candidate.

With the `--tag="vX.X.X..."` argument the script creates a descriptor for the given tag. If there is already released a descriptor for that tag the script exits with an error unless the `--force` flag is set. In this case new descriptor is always generated and the old is overwritten.

By default the script generates a descriptor as long as at least one file is built for the given tag. If the `--wait-all` flag is set the script will exit successfully but it will do nothing unless all the installers for the supported platforms are present. This option is used in the CI to reduce the load.

For testing purposes there is also a `--dry-run` flag. If you use it the descriptor will be generated and printed to the terminal. This flag should work even with read only AWS credentials. Here is an example of the `-dry-run` flag:

```
C:\stremio-service> node .\generate_descriptor.js --tag=v0.1.0 --dry-run
RC Descriptor for tag v0.1.0 already exists
C:\stremio-service> node .\generate_descriptor.js --tag=v0.1.0 --dry-run --force
Getting files for tag v0.1.0
Calculating hashes for files
The hash for StremioService.dmg is <dmg sha256 hash>
The hash for StremioServiceSetup.exe is <exe sha256 hash>
{
  "version": "0.1.0",
  "tag": "v0.1.0",
  "released": "2023-04-13T13:34:53.000Z",
  "files": [
    {
      "name": "StremioService.dmg",
      "url": "https://s3.example.com/stremio-service/v0.1.0/StremioService.dmg",
      "os": "macos",
      "date": "2023-04-13T13:34:53.000Z",
      "checksum": "<dmg sha256 hash>"
    },
    {
      "name": "StremioServiceSetup.exe",
      "url": "https://s3.example.com/stremio-service/v0.1.0/StremioServiceSetup.exe",
      "os": "windows",
      "date": "2023-04-13T13:30:17.000Z",
      "checksum": "<exe sha256 hash>"
    }
  ]
}
```

If the `--quiet` flag is used together with `--dry-run` only the descriptor is printed to `STDOUT`. In case of error the error is printed to `STDERR` and `STDOUT` is blank.

## License

GPL-2.0 [LICENSE.md](LICENSE.md)