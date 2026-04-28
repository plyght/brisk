<div align='center'>
    <h3>Brisk</h3>
    <p>Build, test, sign, and ship native Swift macOS apps without living in Xcode</p>
    <br/>
    <br/>
</div>

Brisk is a native macOS app build system and project manager for Swift. It replaces day-to-day Xcode project workflows with a small manifest, predictable commands, direct app bundle generation, configurable signing, resources, tests, and archives while keeping Xcode project support available as a compatibility backend.

## Features

- **Xcode-Free Workflow**: Create, build, run, test, archive, clean, and inspect apps through one native CLI
- **Brisk Manifest**: Uses `.brisk.toml` as the source of truth for app metadata, sources, resources, signing, tests, and archive output
- **Direct SwiftUI Builds**: Builds native macOS app bundles from Swift source without requiring an Xcode project
- **Resource Bundling**: Copies configured resource files and directories into `Contents/Resources`
- **Asset Catalogs**: Compiles configured `.xcassets` catalogs with `actool` and supports manifest-defined app icons
- **Info.plist Generation**: Generates the app plist from the manifest with support for custom keys
- **Configurable Architectures**: Builds direct apps for configured macOS architectures such as `arm64` and `x86_64`
- **SwiftPM Dependencies**: Generates a Swift package when dependencies are configured and builds through SwiftPM
- **Configurable Signing**: Supports ad-hoc or identity-based signing, entitlements, hardened runtime, zipped exports, and notarytool submission
- **Direct Tests**: Compiles and runs Swift test executables or routes XCTest projects through SwiftPM
- **Direct Archives**: Produces archived `.app` bundles and optional zipped exports without requiring an Xcode project
- **Xcode Compatibility**: Automatically detects `.xcodeproj` and `.xcworkspace` containers when no Brisk manifest is present
- **Clean Output Layout**: Keeps direct builds in `.build/` and derived/archive output in `.brisk/`

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/plyght/brisk/master/install.sh | bash

# From source
git clone https://github.com/plyght/brisk.git
cd brisk
./install.sh

cargo build --release
cp target/release/brisk ~/.local/bin/
```

## Usage

```bash
# Create a new SwiftUI macOS app
brisk new HelloBrisk
cd HelloBrisk

# Build the app bundle
brisk build

# Adopt an existing SwiftPM app by generating .brisk.toml
brisk init

# Build and live-run the app until it quits or you press Ctrl-C
brisk run

# Run direct Swift tests
brisk test

# Archive the built app
brisk archive --release

# Print the built .app path
brisk path

# Remove build output
brisk clean

# Update brisk
brisk update
brisk update --nightly
```

For existing SwiftPM macOS apps, run `brisk init` from the project root. Brisk infers the package name, source path, deployment target, linked frameworks, bundle identifier, app version, and common Info.plist keys when possible. Use `brisk init --force` to overwrite an existing Brisk manifest.

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

Direct Brisk projects use `.brisk.toml` at the project root:

```toml
[package]
name = "HelloBrisk"
version = "0.1.0"

[app]
bundle_id = "com.example.hellobrisk"
deployment_target = "13.0"
sources = "Sources"
resources = ["Resources"]
asset_catalogs = ["Assets.xcassets"]
app_icon = "AppIcon"
entitlements = "Entitlements.plist"
frameworks = ["AppKit"]
swift_flags = []
linker_flags = []

[build]
architectures = ["arm64", "x86_64"]
platform = "macos"

[[dependencies]]
url = "https://github.com/apple/swift-argument-parser.git"
package = "swift-argument-parser"
products = ["ArgumentParser"]

[dependencies.requirement]
from = "1.3.0"

[app.info]
NSHumanReadableCopyright = "Copyright 2026"
LSApplicationCategoryType = "public.app-category.developer-tools"

[signing]
identity = "-"
hardened_runtime = false
notarize = false
keychain_profile = "DeveloperID"

[test]
sources = "Tests"
xctest = false

[archive]
path = ".brisk/Archives/HelloBrisk.app"
export_path = ".brisk/Archives/HelloBrisk.zip"
zip = true
```

New projects are created with this layout:

```text
HelloBrisk/
├── .brisk.toml
├── Resources/
├── Sources/
│   ├── App.swift
│   └── ContentView.swift
└── Tests/
    └── SmokeTests.swift
```

By default, `brisk build`, `brisk test`, and `brisk archive` choose the backend automatically:

- If `.brisk.toml` exists, Brisk uses the direct backend
- If legacy `brisk.toml` exists, Brisk uses the direct backend
- If an Xcode workspace or project exists, Brisk uses the `xcodebuild` compatibility backend
- If Xcode-specific flags are provided, Brisk uses the `xcodebuild` compatibility backend

You can force a backend when needed:

```bash
brisk build --backend direct
brisk build --backend xcode --scheme MyApp
```

Global defaults can be stored in `~/.config/brisk/config.toml`. Project `.brisk.toml` values override global values:

```toml
[defaults]
organization_id = "com.example"
deployment_target = "14.0"
architectures = ["arm64"]

[signing]
identity = "Developer ID Application: Example"
team_id = "ABCDE12345"
hardened_runtime = true
```

Legacy `brisk.toml` manifests are still accepted:

```toml
name = "HelloBrisk"
bundle_id = "com.example.hellobrisk"
deployment_target = "13.0"
```

## Commands

```bash
brisk new <name> [--bundle-id <id>]       # Create a new SwiftUI macOS app
brisk build [--release]                   # Build the app bundle
brisk run [--release]                     # Build and live-run the app until it quits or Ctrl-C
brisk path                                # Print the expected .app path
brisk test                                # Run direct or Xcode tests
brisk archive [--release]                 # Archive the app
brisk list                                # List Xcode schemes and targets
brisk update [--nightly]                  # Update brisk from crates.io or GitHub HEAD
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

## Xcode Replacement Scope

Brisk now replaces the repeatable build-system parts of Xcode for direct macOS SwiftUI apps: app bundle creation, manifest metadata, resource copying, asset catalog compilation, SwiftPM dependencies, smoke tests, XCTest routing, signing, archives, zipped exports, notarization submission, and universal binary generation. Existing Xcode projects still use the compatibility backend for project features that are not safely modeled by `.brisk.toml` yet.

It does not replace Xcode's graphical editor, Interface Builder, visual debugger, Instruments, provisioning UI, simulator/device management UI, or every project-file feature. The direct backend is intended to keep expanding until most app builds do not need an Xcode project at all.

## Examples

The `examples/` directory includes small manifest-driven projects:

- `examples/basic`: direct SwiftUI app with smoke tests
- `examples/assets`: direct SwiftUI app configured for an asset catalog and app icon
- `examples/xctest`: SwiftPM-backed XCTest example

## Architecture

- `main.rs`: CLI parsing, command routing, backend selection, toolchain checks
- `direct.rs`: Project creation, direct Swift compilation, SwiftPM generation, XCTest routing, asset compilation, resource bundling, app generation, signing, notarization, archiving
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
