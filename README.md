# brisk

Cargo-like CLI for native macOS SwiftUI apps.

```sh
brisk new Hello
cd Hello
brisk run
```

`brisk` builds a real `.app` bundle without an Xcode project for the simple path: Swift files in, app bundle out.

## Commands

```sh
brisk new <name>          create a SwiftUI macOS app
brisk build              build .build/debug/<name>.app
brisk run                build and launch the app
brisk path               print the app path
brisk doctor             check Apple CLI tools
brisk clean              remove .build
```

Aliases:

```sh
brisk b                  build
brisk r                  run
```

## Project layout

```text
Hello/
  brisk.toml
  Sources/
    App.swift
    ContentView.swift
```

## Config

```toml
name = "Hello"
bundle_id = "com.example.hello"
deployment_target = "13.0"
```

## Current scope

The first backend is intentionally direct and fast:

```text
swiftc -> .app bundle -> ad-hoc codesign -> open
```

This is for simple SwiftUI/AppKit-style macOS apps. A future Xcode backend can cover asset catalogs, entitlements, SwiftPM dependencies, notarization, and App Store builds.
