# Stremio Service

## Features

`default` features - none
`bundled` - uses binaries location for an installed(and bundled) application

## Development

### Requirements

#### Windows
Download & Install [Wix Toolset](https://github.com/wixtoolset/wix3/releases)  

```
cargo install cargo-wix
```

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
cargo build --release --target x86_64-pc-windows-gnu --features=bundled
```

For cross-compiling on Linux, you need to add the `x86_64-pc-windows-gnu` target and run:

```
cargo build --release --target x86_64-pc-windows-gnu --features=bundled
```

Run `cargo-wix` command inside `Developer Command Prompt for VS 2019`

```
cargo wix
```

##### Cross-compilation from Linux

1. For cross-compiling on Linux, you need to add the `x86_64-pc-windows-gnu` target:

```
rustup target add x86_64-pc-windows-gnu
```

2. And build the binary using the `bundled` feature:

```
cargo build --release --target x86_64-pc-windows-gnu --features=bundled
```

**NOTE:** `cargo-wix` **does not** support building an `.msi` installer on other platforms, only Windows.

#### Ubuntu

```
cargo deb
```

#### Fedora

```
cargo generate-rpm
```

#### Flatpak

The Manifest is located [resources/com.stremio.Service.json](.resources/com.stremio.Service.json) and you can bundle the application using the script:

`./build-flatpak.sh`

#### MacOS

```
cargo run --bin build-macos && create-dmg --overwrite target/macos/*.app target/macos
```
