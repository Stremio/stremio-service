# Stremio Service

## Development

### Requirements
#### Windows
Download & Install [Wix Toolset](https://github.com/wixtoolset/wix3/releases)  
Install `cargo-wix`
```
cargo install cargo-wix
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