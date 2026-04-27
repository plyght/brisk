<div align='center'>
    <h3>Brisk</h3>
    <p>Cargo-like builds for native Swift and Xcode-based macOS apps</p>
    <br/>
    <br/>
</div>

The direct path from native Swift projects to real macOS `.app` bundles. Brisk gives SwiftUI, AppKit, and Xcode-based macOS apps a fast Rust CLI workflow: create simple apps, compile direct Swift sources, build existing `.xcodeproj` and `.xcworkspace` projects, and launch without opening Xcode.

## Features

- **Native App Bundles**: Produces real `.app` bundles with `Contents/MacOS`, `Contents/Resources`, and `Info.plist`
- **SwiftUI First**: Generates a minimal SwiftUI macOS app layout ready to run
- **Xcode Project Support**: Builds existing `.xcodeproj` and `.xcworkspace` projects through `xcodebuild`
- **Hybrid Backend**: Prefers Brisk's direct `swiftc` app builder for `brisk.toml` projects and falls back to `xcodebuild` for existing Xcode projects
- **Full Xcode CLI Workflow**: Supports schemes, configurations, destinations, SDKs, extra `xcodebuild` flags, tests, archives, and project listing
- **Xcode-Free Workflow**: Uses Apple command line tools directly so Brisk-native builds do not require opening Xcode or running `xcodebuild`
- **Cargo-Like Commands**: Familiar `new`, `build`, `run`, `test`, `archive`, `list`, `clean`, and `path` commands
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

# Build an Xcode project or workspace
brisk build --scheme MyApp
brisk build --project MyApp.xcodeproj --scheme MyApp
brisk build --workspace MyApp.xcworkspace --scheme MyApp
brisk build --scheme MyApp --configuration Debug --destination 'platform=macOS'
brisk build --scheme MyApp --sdk macosx -- CODE_SIGNING_ALLOWED=NO
brisk build --backend direct
brisk build --backend xcode --scheme MyApp

# Run tests and create archives without opening Xcode
brisk test --scheme MyApp --destination 'platform=macOS'
brisk archive --scheme MyApp
brisk archive --scheme MyApp --archive-path .build/MyApp.xcarchive

# Inspect available Xcode schemes and targets
brisk list
brisk list --workspace MyApp.xcworkspace

# Print the current app bundle path
brisk path

# Check the local Apple toolchain
brisk doctor
```

Aliases and global flags:

```bash
brisk b                  build
brisk r                  run
brisk -v build           show underlying swiftc/xcodebuild/codesign commands
```

A direct Swift project follows this pipeline:

```text
Swift sources -> swiftc binary -> .app bundle -> ad-hoc codesign
```

An Xcode project follows this pipeline:

```text
.xcodeproj/.xcworkspace -> xcodebuild -> DerivedData product -> .app bundle
```

Direct Swift project output is written to:

```text
.build/debug/<name>.app
.build/release/<name>.app
```

Xcode project output is resolved from brisk-managed DerivedData:

```text
.brisk/DerivedData/Build/Products/<configuration>/<name>.app
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

Existing Xcode projects can keep their normal layout:

```text
MyApp/
  MyApp.xcodeproj
  MyApp/
    App.swift
    Assets.xcassets
```

If a directory has `brisk.toml`, Brisk uses the direct backend by default, even when an `.xcworkspace` or `.xcodeproj` is also present. If there is no `brisk.toml`, Brisk falls back to the Xcode backend when it finds an `.xcworkspace` or `.xcodeproj`. Use `--backend direct` or `--backend xcode` to override automatic selection. Pass `--scheme` when the scheme cannot be inferred from the project or workspace name.

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

- `main.rs`: CLI orchestration, backend selection, and top-level commands
- `config.rs`: Project configuration loading and writing
- `direct.rs`: Direct Swift source builds, scaffolding, bundling, and ad-hoc signing
- `xcode.rs`: `.xcodeproj` and `.xcworkspace` builds, tests, archives, listing, and product resolution through `xcodebuild`
- `cmd.rs`: Small command wrapper with readable error handling
- `ui.rs`: Status lines, spinners, and success output

## Current Scope

Brisk is focused on native Apple-platform macOS apps: Swift, SwiftUI, AppKit, and Xcode-based projects. It is not intended for Electron, web wrappers, or non-native desktop stacks.

The current implementation supports two backends:

```text
swiftc -> .app bundle -> ad-hoc codesign -> open
.xcodeproj/.xcworkspace -> xcodebuild -> .app bundle -> open
```

Future work may cover notarization, App Store packaging, signing profile management, and deeper SwiftPM integration.

## Development

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt
```

Key dependencies: clap, console, indicatif, serde, serde_json, thiserror, toml.

## License

MIT License
