<div align='center'>
    <h3>Brisk</h3>
    <p>Cargo-like builds for native Swift macOS apps</p>
    <br/>
    <br/>
</div>

Brisk is a small command-line tool for creating, building, running, testing, and archiving native macOS apps without making everyday Swift development feel like project-file maintenance. It can compile simple SwiftUI apps directly with `swiftc`, or use existing Xcode projects and workspaces through `xcodebuild` when a full Apple project is already present.

## Features

- **Cargo-Like Workflow**: Create, build, run, test, archive, clean, and inspect apps through one focused CLI
- **Direct SwiftUI Builds**: Builds lightweight native macOS app bundles from `Sources/*.swift` without requiring an Xcode project
- **Xcode Project Support**: Automatically detects `.xcodeproj` and `.xcworkspace` containers and delegates to `xcodebuild`
- **Automatic Backend Selection**: Uses `brisk.toml` projects directly and falls back to Xcode when project files or Xcode-specific flags are present
- **App Bundle Generation**: Creates `.app` bundles with `Info.plist`, executable layout, and ad-hoc signing
- **Release and Debug Profiles**: Supports optimized release builds and debug builds with symbols
- **Toolchain Checks**: Verifies required Apple command-line tools with `brisk doctor`
- **Clean Output Layout**: Keeps direct builds in `.build/` and Xcode derived data in `.brisk/`

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
name = "HelloBrisk"
bundle_id = "com.example.hellobrisk"
deployment_target = "13.0"
```

The direct backend expects Swift source files under `Sources/`:

```text
HelloBrisk/
├── brisk.toml
└── Sources/
    ├── App.swift
    └── ContentView.swift
```

By default, `brisk build` chooses the backend automatically:

- If `brisk.toml` exists, Brisk uses the direct `swiftc` backend
- If an Xcode workspace or project exists, Brisk uses the `xcodebuild` backend
- If Xcode-specific flags are provided, Brisk uses the `xcodebuild` backend

You can force a backend when needed:

```bash
brisk build --backend direct
brisk build --backend xcode --scheme MyApp
```

## Commands

```bash
brisk new <name> [--bundle-id <id>]       # Create a new SwiftUI macOS app
brisk build [--release]                   # Build the app bundle
brisk run [--release]                     # Build and launch the app
brisk path                                # Print the expected .app path
brisk test                                # Run Xcode tests
brisk archive [--archive-path <path>]     # Archive an Xcode app
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
- `direct.rs`: Direct SwiftUI project creation, `swiftc` compilation, app bundle generation, ad-hoc signing
- `xcode.rs`: Xcode container discovery, scheme inference, build/test/archive orchestration, app path resolution
- `config.rs`: `brisk.toml` loading and saving
- `cmd.rs`: Command execution and error handling
- `ui.rs`: Status output, sections, success messages, and spinners

## Development

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Requires macOS with Xcode Command Line Tools installed. Direct builds require `swiftc`, `codesign`, `open`, and `xcrun`; Xcode-backed workflows also require `xcodebuild`.

Key dependencies: clap, console, indicatif, serde, serde_json, thiserror, toml.

## License

MIT License
