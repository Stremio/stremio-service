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
apt install libgtk-3-dev pkg-config libssl-dev libayatana-appindicator3-dev
```
```
cargo install cargo-deb
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