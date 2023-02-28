# Stremio Service

## Features

- `default` features - none
- `bundled` - uses binaries location for an installed(and bundled) application.

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

By default the `stremio-service` binary is ran:

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

The Manifest is located [resources/com.stremio.Service.json](.resources/com.stremio.Service.json) and you can bundle the application using the script:

`./build-flatpak.sh`

#### MacOS

```
cargo run --bin bundle-macos && create-dmg --overwrite target/macos/*.app target/macos
```
