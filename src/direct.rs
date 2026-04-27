use crate::cmd::command;
use crate::config::{BriskConfig, new_config};
use crate::ui::{spinner, status, status_dim, success};
use crate::{BriskError, Result, profile};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub fn new_app(name: &str, bundle_id: Option<String>) -> Result<()> {
    validate_app_name(name)?;
    let root = std::env::current_dir()?.join(name);
    if root.exists() {
        return Err(BriskError::Message(format!(
            "{} already exists",
            root.display()
        )));
    }

    let config = new_config(
        name,
        bundle_id.unwrap_or_else(|| format!("com.example.{}", sanitize_bundle_part(name))),
    );

    fs::create_dir_all(root.join("Sources"))?;
    fs::create_dir_all(root.join("Resources"))?;
    fs::create_dir_all(root.join("Tests"))?;
    config.save(&root)?;
    fs::write(root.join("Sources").join("App.swift"), app_swift(name))?;
    fs::write(
        root.join("Sources").join("ContentView.swift"),
        content_view_swift(name),
    )?;
    fs::write(
        root.join("Tests").join("SmokeTests.swift"),
        smoke_test_swift(),
    )?;

    status("create", root.display());
    status_dim("write", root.join("brisk.toml").display());
    status_dim("write", root.join("Sources/App.swift").display());
    status_dim("write", root.join("Sources/ContentView.swift").display());
    status_dim("write", root.join("Tests/SmokeTests.swift").display());
    println!("\n{}", console::style("next").bold());
    println!("  cd {name}");
    println!("  brisk run");
    Ok(())
}

pub fn build_direct_app(root: &Path, release: bool, verbose: bool) -> Result<PathBuf> {
    let started = Instant::now();
    let config = BriskConfig::load(root)?;
    let profile = profile(release);
    let build_dir = root.join(".build").join(profile);
    let bin_path = build_dir.join(config.app_name());
    let app = app_path(root, &config, profile);

    fs::create_dir_all(&build_dir)?;

    let swift_files = collect_swift_files(&root.join(&config.app.sources))?;
    if swift_files.is_empty() {
        return Err(BriskError::Message(format!(
            "no Swift files found in {}",
            config.app.sources.display()
        )));
    }

    status("backend", "swiftc");
    status("profile", profile);
    status("sources", swift_files.len());
    if verbose {
        for file in &swift_files {
            status_dim("source", file.display());
        }
    }

    let compile_spinner = spinner("compiling Swift");
    let mut swiftc = command("swiftc");
    swiftc
        .arg("-target")
        .arg(format!("arm64-apple-macos{}", config.deployment_target()))
        .arg("-parse-as-library")
        .arg("-framework")
        .arg("SwiftUI")
        .arg("-o")
        .arg(&bin_path);
    if release {
        swiftc.arg("-O");
    } else {
        swiftc.arg("-Onone").arg("-g");
    }
    for file in &swift_files {
        swiftc.arg(file);
    }
    if verbose {
        status_dim("run", swiftc.display());
    }
    swiftc
        .run_silent()
        .inspect_err(|_| compile_spinner.finish_and_clear())?;
    compile_spinner.finish_and_clear();
    status("compile", bin_path.display());

    create_bundle(root, &config, &bin_path, &app)?;
    status("bundle", app.display());

    sign_app(root, &config, &app, verbose)?;
    success(format!(
        "built {} in {:.1}s",
        app.display(),
        started.elapsed().as_secs_f32()
    ));

    Ok(app)
}

pub fn test_direct_app(root: &Path, verbose: bool) -> Result<()> {
    let config = BriskConfig::load(root)?;
    let test_dir = root.join(&config.test.sources);
    let test_files = collect_swift_files(&test_dir)?;
    if test_files.is_empty() {
        return Err(BriskError::Message(format!(
            "no Swift tests found in {}",
            config.test.sources.display()
        )));
    }
    let mut support_files = collect_swift_files(&root.join(&config.app.sources))?;
    support_files.retain(|path| path.file_name() != Some(OsStr::new("App.swift")));
    let build_dir = root.join(".build").join("debug");
    fs::create_dir_all(&build_dir)?;
    let test_bin = build_dir.join(format!("{}Tests", config.app_name()));

    status("backend", "swiftc");
    status("tests", test_files.len());
    let test_spinner = spinner("compiling Swift tests");
    let mut swiftc = command("swiftc");
    swiftc
        .arg("-target")
        .arg(format!("arm64-apple-macos{}", config.deployment_target()))
        .arg("-framework")
        .arg("SwiftUI")
        .arg("-o")
        .arg(&test_bin);
    for file in &support_files {
        swiftc.arg(file);
    }
    for file in &test_files {
        swiftc.arg(file);
    }
    if verbose {
        status_dim("run", swiftc.display());
    }
    swiftc
        .run_silent()
        .inspect_err(|_| test_spinner.finish_and_clear())?;
    test_spinner.finish_and_clear();

    status("run", test_bin.display());
    command(test_bin.to_string_lossy().as_ref()).run()?;
    success("tests passed");
    Ok(())
}

pub fn archive_direct_app(
    root: &Path,
    release: bool,
    verbose: bool,
    archive_path: Option<PathBuf>,
) -> Result<PathBuf> {
    let app = build_direct_app(root, release, verbose)?;
    let config = BriskConfig::load(root)?;
    let archive = archive_path
        .or(config.archive.path.clone())
        .unwrap_or_else(|| {
            root.join(".brisk")
                .join("Archives")
                .join(format!("{}.app", config.app_name()))
        });
    if archive.exists() {
        fs::remove_dir_all(&archive)?;
    }
    if let Some(parent) = archive.parent() {
        fs::create_dir_all(parent)?;
    }
    copy_dir(&app, &archive)?;
    status("archive", archive.display());
    success(format!("archived {}", archive.display()));
    Ok(archive)
}

fn create_bundle(root: &Path, config: &BriskConfig, bin_path: &Path, app: &Path) -> Result<()> {
    if app.exists() {
        fs::remove_dir_all(app)?;
    }
    let contents = app.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos)?;
    fs::create_dir_all(&resources)?;
    fs::copy(bin_path, macos.join(config.app_name()))?;
    fs::write(contents.join("Info.plist"), info_plist(config))?;
    for resource in &config.app.resources {
        let source = root.join(resource);
        if !source.exists() {
            continue;
        }
        let destination = resources.join(source.file_name().ok_or_else(|| {
            BriskError::Message(format!("invalid resource path {}", source.display()))
        })?);
        if source.is_dir() {
            copy_dir(&source, &destination)?;
        } else {
            fs::copy(&source, &destination)?;
        }
        status_dim("resource", source.display());
    }
    Ok(())
}

fn sign_app(root: &Path, config: &BriskConfig, app: &Path, verbose: bool) -> Result<()> {
    let signing_spinner = spinner("signing app");
    let mut codesign = command("codesign");
    codesign.arg("--force").arg("--deep");
    if config.signing.hardened_runtime {
        codesign.arg("--options").arg("runtime");
    }
    if let Some(entitlements) = &config.app.entitlements {
        codesign.arg("--entitlements").arg(root.join(entitlements));
    }
    codesign
        .arg("--sign")
        .arg(&config.signing.identity)
        .arg(app);
    if verbose {
        status_dim("run", codesign.display());
    }
    codesign
        .run_silent()
        .inspect_err(|_| signing_spinner.finish_and_clear())?;
    signing_spinner.finish_and_clear();
    status("sign", &config.signing.identity);
    Ok(())
}

fn collect_swift_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_swift_files_inner(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_swift_files_inner(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_swift_files_inner(&path, files)?;
        } else if path.extension() == Some(OsStr::new("swift")) {
            files.push(path);
        }
    }
    Ok(())
}

fn copy_dir(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

pub fn app_path(root: &Path, config: &BriskConfig, profile: &str) -> PathBuf {
    root.join(".build")
        .join(profile)
        .join(format!("{}.app", config.app_name()))
}

fn validate_app_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(BriskError::Message(
            "app name must contain only letters, numbers, _ or -".to_string(),
        ));
    }
    Ok(())
}

fn sanitize_bundle_part(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn app_swift(name: &str) -> String {
    format!(
        r#"import SwiftUI

@main
struct {name}: App {{
    var body: some Scene {{
        WindowGroup {{
            ContentView()
        }}
    }}
}}
"#
    )
}

fn content_view_swift(name: &str) -> String {
    format!(
        r#"import SwiftUI

struct ContentView: View {{
    var body: some View {{
        VStack(spacing: 12) {{
            Text("{name}")
                .font(.system(size: 40, weight: .semibold, design: .rounded))
            Text("Built with brisk")
                .foregroundStyle(.secondary)
        }}
        .frame(width: 520, height: 320)
    }}
}}
"#
    )
}

fn smoke_test_swift() -> String {
    r#"import Foundation

print("Smoke tests passed")
"#
    .to_string()
}

fn info_plist(config: &BriskConfig) -> String {
    let mut entries = vec![
        plist_entry("CFBundleDevelopmentRegion", "en"),
        plist_entry("CFBundleExecutable", config.app_name()),
        plist_entry("CFBundleIdentifier", config.bundle_id()),
        plist_entry("CFBundleInfoDictionaryVersion", "6.0"),
        plist_entry("CFBundleName", config.app_name()),
        plist_entry("CFBundlePackageType", "APPL"),
        plist_entry("CFBundleShortVersionString", &config.package.version),
        plist_entry("CFBundleVersion", "1"),
        plist_entry("LSMinimumSystemVersion", config.deployment_target()),
        plist_entry("NSPrincipalClass", "NSApplication"),
    ];
    for (key, value) in &config.app.info {
        entries.push(plist_value_entry(key, value));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
{}</dict>
</plist>
"#,
        entries.join("")
    )
}

fn plist_entry(key: &str, value: &str) -> String {
    format!(
        "    <key>{}</key>\n    <string>{}</string>\n",
        xml_escape(key),
        xml_escape(value)
    )
}

fn plist_value_entry(key: &str, value: &toml::Value) -> String {
    match value {
        toml::Value::Boolean(value) => format!(
            "    <key>{}</key>\n    <{} />\n",
            xml_escape(key),
            if *value { "true" } else { "false" }
        ),
        toml::Value::Integer(value) => {
            format!(
                "    <key>{}</key>\n    <integer>{}</integer>\n",
                xml_escape(key),
                value
            )
        }
        toml::Value::Float(value) => {
            format!(
                "    <key>{}</key>\n    <real>{}</real>\n",
                xml_escape(key),
                value
            )
        }
        toml::Value::Array(values) => {
            let values = values
                .iter()
                .map(|value| {
                    format!(
                        "        <string>{}</string>\n",
                        xml_escape(&value_to_string(value))
                    )
                })
                .collect::<String>();
            format!(
                "    <key>{}</key>\n    <array>\n{}    </array>\n",
                xml_escape(key),
                values
            )
        }
        _ => plist_entry(key, &value_to_string(value)),
    }
}

fn value_to_string(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
