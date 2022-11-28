# Stremio Service

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

#### MacOS
```
npm install -g create-dmg && brew install graphicsmagick imagemagick
```

### Run
```
RUST_LOG=info cargo run --bin service
```

### Build
```
cargo build --release
```

### Package

#### Windows
Run this command inside `Developer Command Prompt for VS 2019`
```
cargo wix
```

#### Ubuntu
```
cargo deb
```

#### Fedora
```
cargo generate-rpm
```

#### MacOS
```
cargo run --bin build-macos && create-dmg --overwrite target/macos/*.app target/macos
```