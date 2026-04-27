<div align='center'>
    <h3>Brisk</h3>
    <p>Cargo-like project management for native Swift macOS apps</p>
    <br/>
    <br/>
</div>

Brisk is a native macOS app build system and project manager for Swift. It replaces day-to-day Xcode project workflows with a small manifest, predictable commands, direct app bundle generation, configurable signing, resources, tests, and archives while keeping Xcode project support available as a compatibility backend.

## Features

- **Cargo-Like Workflow**: Create, build, run, test, archive, clean, and inspect apps through one focused CLI
- **Brisk Manifest**: Uses `brisk.toml` as the source of truth for app metadata, sources, resources, signing, tests, and archive output
- **Direct SwiftUI Builds**: Builds native macOS app bundles from Swift source without requiring an Xcode project
- **Resource Bundling**: Copies configured resource files and directories into `Contents/Resources`
- **Info.plist Generation**: Generates the app plist from the manifest with support for custom keys
- **Configurable Signing**: Supports ad-hoc or identity-based signing, entitlements, and hardened runtime
- **Direct Tests**: Compiles and runs Swift test executables from a configured tests directory
- **Direct Archives**: Produces archived `.app` bundles without requiring Xcode
- **Xcode Compatibility**: Automatically detects `.xcodeproj` and `.xcworkspace` containers when no Brisk manifest is present
- **Clean Output Layout**: Keeps direct builds in `.build/` and derived/archive output in `.brisk/`

## Install

```bash
# From source
git clone https://github.com/plyght/brisk.git
cd brisk
cargo build --release
sudo cp target/release/brisk /usr/local/bin/
```

## Usage

```bash
# Create a new SwiftUI macOS app
brisk new HelloBrisk
cd HelloBrisk

# Build the app bundle
brisk build

# Build and launch the app
brisk run

# Run direct Swift tests
brisk test

# Archive the built app
brisk archive --release

# Print the built .app path
brisk path

# Remove build output
brisk clean
```

For Xcode projects, run Brisk from a directory containing an `.xcodeproj` or `.xcworkspace`:

```bash
# List schemes and targets
brisk list

# Build the inferred scheme
brisk build

# Build a specific scheme
brisk build --scheme MyApp

# Run tests
brisk test --scheme MyApp

# Archive a release build
brisk archive --scheme MyApp
```

Pass additional `xcodebuild` arguments after `--`:

```bash
brisk build --scheme MyApp -- -allowProvisioningUpdates
```

## Configuration

Direct Brisk projects use `brisk.toml` at the project root:

```toml
[package]
name = "HelloBrisk"
version = "0.1.0"

[app]
bundle_id = "com.example.hellobrisk"
deployment_target = "13.0"
sources = "Sources"
resources = ["Resources"]
entitlements = "Entitlements.plist"

[app.info]
NSHumanReadableCopyright = "Copyright 2026"
LSApplicationCategoryType = "public.app-category.developer-tools"

[signing]
identity = "-"
hardened_runtime = false

[test]
sources = "Tests"

[archive]
path = ".brisk/Archives/HelloBrisk.app"
```

New projects are created with this layout:

```text
HelloBrisk/
├── brisk.toml
├── Resources/
├── Sources/
│   ├── App.swift
│   └── ContentView.swift
└── Tests/
    └── SmokeTests.swift
```

By default, `brisk build`, `brisk test`, and `brisk archive` choose the backend automatically:

- If `brisk.toml` exists, Brisk uses the direct backend
- If an Xcode workspace or project exists, Brisk uses the `xcodebuild` compatibility backend
- If Xcode-specific flags are provided, Brisk uses the `xcodebuild` compatibility backend

You can force a backend when needed:

```bash
brisk build --backend direct
brisk build --backend xcode --scheme MyApp
```

Legacy top-level manifests are still accepted:

```toml
name = "HelloBrisk"
bundle_id = "com.example.hellobrisk"
deployment_target = "13.0"
```

## Commands

```bash
brisk new <name> [--bundle-id <id>]       # Create a new SwiftUI macOS app
brisk build [--release]                   # Build the app bundle
brisk run [--release]                     # Build and launch the app
brisk path                                # Print the expected .app path
brisk test                                # Run direct or Xcode tests
brisk archive [--release]                 # Archive the app
brisk list                                # List Xcode schemes and targets
brisk doctor                              # Check required Apple CLI tools
brisk clean                               # Remove build output
```

Common Xcode options:

```bash
--workspace <path>        # Xcode workspace
--project <path>          # Xcode project
--scheme <name>           # Xcode scheme
--configuration <name>    # Build configuration
--destination <specifier> # Xcode destination
--sdk <sdk>               # Xcode SDK
```

Use `-v` or `--verbose` to print the underlying commands Brisk runs.

## Architecture

- `main.rs`: CLI parsing, command routing, backend selection, toolchain checks
- `direct.rs`: Project creation, direct Swift compilation, test execution, resource bundling, app generation, signing, archiving
- `xcode.rs`: Xcode container discovery, scheme inference, build/test/archive orchestration, app path resolution
- `config.rs`: Manifest model, legacy manifest normalization, loading, and saving
- `cmd.rs`: Command execution and error handling
- `ui.rs`: Status output, sections, success messages, and spinners

## Development

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Requires macOS with Xcode Command Line Tools installed. Direct workflows require `swiftc`, `codesign`, `open`, and `xcrun`; Xcode-backed workflows also require `xcodebuild`.

Key dependencies: clap, console, indicatif, serde, serde_json, thiserror, toml.

## License

MIT License
