<div align='center'>
    <h3>Brisk</h3>
    <p>Cargo-like builds for native macOS SwiftUI apps without an Xcode project</p>
    <br/>
    <br/>
</div>

The direct path from Swift files to a real macOS `.app` bundle. Brisk gives small native SwiftUI projects a fast Rust CLI workflow: create an app, compile it with `swiftc`, assemble the bundle, ad-hoc sign it, and launch it without generating or maintaining Xcode project files.

## Features

- **Native App Bundles**: Produces real `.app` bundles with `Contents/MacOS`, `Contents/Resources`, and `Info.plist`
- **SwiftUI First**: Generates a minimal SwiftUI macOS app layout ready to run
- **Xcode-Free Workflow**: Uses Apple command line tools directly instead of project generation
- **Cargo-Like Commands**: Familiar `new`, `build`, `run`, `clean`, and `path` commands
- **Debug and Release Profiles**: Builds into `.build/debug` or `.build/release`
- **Toolchain Checks**: Verifies required Apple CLI tools with `brisk doctor`
- **Readable Output**: Clean status lines, quiet defaults, and verbose command tracing when needed

## Install

```bash
# From source
git clone https://github.com/plyght/brisk.git
cd brisk
cargo build --release
sudo cp target/release/brisk /usr/local/bin/
```

Requires macOS, Rust, and Apple command line tools. Install the Apple tools with:

```bash
xcode-select --install
```

## Usage

```bash
# Create a new SwiftUI macOS app
brisk new Hello
cd Hello

# Build the debug app bundle
brisk build

# Build and launch the app
brisk run

# Build an optimized release bundle
brisk build --release

# Print the current app bundle path
brisk path

# Check the local Apple toolchain
brisk doctor
```

Aliases and global flags:

```bash
brisk b                  build
brisk r                  run
brisk -v build           show underlying swiftc/codesign commands
```

A typical build follows this pipeline:

```text
Swift sources -> swiftc binary -> .app bundle -> ad-hoc codesign
```

The output bundle is written to:

```text
.build/debug/<name>.app
.build/release/<name>.app
```

## Project Layout

```text
Hello/
  brisk.toml
  Sources/
    App.swift
    ContentView.swift
```

`Sources/` may contain additional `.swift` files. Brisk recursively collects Swift files below this directory and passes them to `swiftc`.

## Configuration

Brisk projects are configured with `brisk.toml`:

```toml
name = "Hello"
bundle_id = "com.example.hello"
deployment_target = "13.0"
```

- `name`: App name, executable name, and bundle display name
- `bundle_id`: macOS bundle identifier used in `Info.plist`
- `deployment_target`: Minimum macOS version passed to the Swift compiler

## Architecture

- `Cli` / `Commands`: Command-line interface using clap
- `BriskConfig`: Project configuration loading and writing
- `new_app`: Project scaffolding for SwiftUI apps
- `build_app`: Toolchain validation, compilation, bundling, and signing
- `create_bundle`: `.app` directory and `Info.plist` generation
- `Cmd`: Small command wrapper with readable error handling
- `doctor`: Apple toolchain checks

## Current Scope

Brisk is intentionally focused on the simple native macOS path:

```text
swiftc -> .app bundle -> ad-hoc codesign -> open
```

This works well for small SwiftUI and AppKit-style apps that do not need an Xcode project. Future backends may cover asset catalogs, entitlements, SwiftPM dependencies, notarization, App Store packaging, and richer build settings.

## Development

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt
```

Key dependencies: clap, console, indicatif, serde, thiserror, toml.

## License

MIT License
