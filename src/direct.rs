use crate::cmd::command;
use crate::config::BriskConfig;
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

    let config = BriskConfig {
        name: name.to_string(),
        bundle_id: bundle_id
            .unwrap_or_else(|| format!("com.example.{}", sanitize_bundle_part(name))),
        deployment_target: "13.0".to_string(),
    };

    fs::create_dir_all(root.join("Sources"))?;
    config.save(&root)?;
    fs::write(root.join("Sources").join("App.swift"), app_swift(name))?;
    fs::write(
        root.join("Sources").join("ContentView.swift"),
        content_view_swift(name),
    )?;

    status("create", root.display());
    status_dim("write", root.join("brisk.toml").display());
    status_dim("write", root.join("Sources/App.swift").display());
    status_dim("write", root.join("Sources/ContentView.swift").display());
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
    let bin_path = build_dir.join(&config.name);
    let app = app_path(root, &config, profile);

    fs::create_dir_all(&build_dir)?;

    let swift_files = collect_swift_files(&root.join("Sources"))?;
    if swift_files.is_empty() {
        return Err(BriskError::Message(
            "no Swift files found in Sources".to_string(),
        ));
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
        .arg(format!("arm64-apple-macos{}", config.deployment_target))
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

    create_bundle(&config, &bin_path, &app)?;
    status("bundle", app.display());

    let signing_spinner = spinner("ad-hoc signing");
    let mut codesign = command("codesign");
    codesign
        .arg("--force")
        .arg("--deep")
        .arg("--sign")
        .arg("-")
        .arg(&app);
    if verbose {
        status_dim("run", codesign.display());
    }
    codesign
        .run_silent()
        .inspect_err(|_| signing_spinner.finish_and_clear())?;
    signing_spinner.finish_and_clear();
    status("sign", "ad-hoc");
    success(format!(
        "built {} in {:.1}s",
        app.display(),
        started.elapsed().as_secs_f32()
    ));

    Ok(app)
}

fn create_bundle(config: &BriskConfig, bin_path: &Path, app: &Path) -> Result<()> {
    if app.exists() {
        fs::remove_dir_all(app)?;
    }
    let contents = app.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos)?;
    fs::create_dir_all(&resources)?;
    fs::copy(bin_path, macos.join(&config.name))?;
    fs::write(contents.join("Info.plist"), info_plist(config))?;
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

pub fn app_path(root: &Path, config: &BriskConfig, profile: &str) -> PathBuf {
    root.join(".build")
        .join(profile)
        .join(format!("{}.app", config.name))
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

fn info_plist(config: &BriskConfig) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>{name}</string>
    <key>CFBundleIdentifier</key>
    <string>{bundle_id}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>{deployment_target}</string>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
</dict>
</plist>
"#,
        name = config.name,
        bundle_id = config.bundle_id,
        deployment_target = config.deployment_target,
    )
}
