use crate::cmd::command;
use crate::config::{
    BriskConfig, SwiftPackageDependency, global_default_organization_id, manifest_path, new_config,
};
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

    let config = new_config(name, bundle_id.unwrap_or(default_bundle_id(name)?));

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
    status_dim(
        "write",
        manifest_path(&root)
            .unwrap_or_else(|| root.join(".brisk.toml"))
            .display(),
    );
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

    status(
        "backend",
        if config.dependencies.is_empty() {
            "swiftc"
        } else {
            "swiftpm"
        },
    );
    status("profile", profile);
    status("sources", swift_files.len());
    status("arch", config.build.architectures.join(","));
    if verbose {
        for file in &swift_files {
            status_dim("source", file.display());
        }
    }

    if config.dependencies.is_empty() {
        compile_with_swiftc(&config, &swift_files, &bin_path, release, verbose)?;
    } else {
        compile_with_swiftpm(root, &config, release, verbose)?;
        copy_swiftpm_binary(root, &config, profile, &bin_path)?;
    }

    create_bundle(root, &config, &bin_path, &app, verbose)?;
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
    if !config.dependencies.is_empty() || config.test.xctest {
        test_with_swiftpm(root, &config, verbose)?;
        success("tests passed");
        return Ok(());
    }
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
    add_common_swiftc_args(&mut swiftc, &config, &config.build.architectures[0], false);
    swiftc.arg("-o").arg(&test_bin);
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
    if config.archive.zip {
        zip_archive(root, &archive, &config, verbose)?;
    }
    if config.signing.notarize {
        notarize_archive(&archive, &config, verbose)?;
    }
    success(format!("archived {}", archive.display()));
    Ok(archive)
}

fn compile_with_swiftc(
    config: &BriskConfig,
    swift_files: &[PathBuf],
    bin_path: &Path,
    release: bool,
    verbose: bool,
) -> Result<()> {
    let compile_spinner = spinner("compiling Swift");
    if config.build.architectures.len() == 1 {
        let arch = &config.build.architectures[0];
        let mut swiftc = command("swiftc");
        add_common_swiftc_args(&mut swiftc, config, arch, release);
        swiftc.arg("-parse-as-library").arg("-o").arg(bin_path);
        for file in swift_files {
            swiftc.arg(file);
        }
        if verbose {
            status_dim("run", swiftc.display());
        }
        swiftc
            .run_silent()
            .inspect_err(|_| compile_spinner.finish_and_clear())?;
    } else {
        let parent = bin_path.parent().ok_or_else(|| {
            BriskError::Message(format!("invalid binary path {}", bin_path.display()))
        })?;
        let arch_dir = parent.join("arch");
        if arch_dir.exists() {
            fs::remove_dir_all(&arch_dir)?;
        }
        fs::create_dir_all(&arch_dir)?;
        let mut arch_bins = Vec::new();
        for arch in &config.build.architectures {
            let arch_bin = arch_dir.join(format!("{}-{arch}", config.app_name()));
            let mut swiftc = command("swiftc");
            add_common_swiftc_args(&mut swiftc, config, arch, release);
            swiftc.arg("-parse-as-library").arg("-o").arg(&arch_bin);
            for file in swift_files {
                swiftc.arg(file);
            }
            if verbose {
                status_dim("run", swiftc.display());
            }
            swiftc
                .run_silent()
                .inspect_err(|_| compile_spinner.finish_and_clear())?;
            arch_bins.push(arch_bin);
        }
        let mut lipo = command("lipo");
        lipo.arg("-create");
        for arch_bin in &arch_bins {
            lipo.arg(arch_bin);
        }
        lipo.arg("-output").arg(bin_path);
        if verbose {
            status_dim("run", lipo.display());
        }
        lipo.run_silent()
            .inspect_err(|_| compile_spinner.finish_and_clear())?;
    }
    compile_spinner.finish_and_clear();
    status("compile", bin_path.display());
    Ok(())
}

fn add_common_swiftc_args(
    swiftc: &mut crate::cmd::Cmd,
    config: &BriskConfig,
    arch: &str,
    release: bool,
) {
    swiftc.arg("-target").arg(format!(
        "{}-apple-macos{}",
        arch,
        config.deployment_target()
    ));
    swiftc.arg("-framework").arg("SwiftUI");
    for framework in &config.app.frameworks {
        swiftc.arg("-framework").arg(framework);
    }
    for flag in &config.app.swift_flags {
        swiftc.arg(flag);
    }
    for flag in &config.app.linker_flags {
        swiftc.arg(flag);
    }
    if release {
        swiftc.arg("-O");
    } else {
        swiftc.arg("-Onone").arg("-g");
    }
}

fn compile_with_swiftpm(
    root: &Path,
    config: &BriskConfig,
    release: bool,
    verbose: bool,
) -> Result<()> {
    write_package_swift(root, config)?;
    let swiftpm_spinner = spinner("building Swift package");
    let mut build = command("swift");
    build.arg("build").arg("--package-path").arg(root);
    if release {
        build.arg("-c").arg("release");
    }
    for arch in &config.build.architectures {
        build.arg("--arch").arg(arch);
    }
    if verbose {
        status_dim("run", build.display());
    }
    build
        .run_silent()
        .inspect_err(|_| swiftpm_spinner.finish_and_clear())?;
    swiftpm_spinner.finish_and_clear();
    Ok(())
}

fn test_with_swiftpm(root: &Path, config: &BriskConfig, verbose: bool) -> Result<()> {
    write_package_swift(root, config)?;
    let test_spinner = spinner("running XCTest");
    let mut test = command("swift");
    test.arg("test").arg("--package-path").arg(root);
    for arch in &config.build.architectures {
        test.arg("--arch").arg(arch);
    }
    if verbose {
        status_dim("run", test.display());
    }
    test.run_silent()
        .inspect_err(|_| test_spinner.finish_and_clear())?;
    test_spinner.finish_and_clear();
    Ok(())
}

fn copy_swiftpm_binary(
    root: &Path,
    config: &BriskConfig,
    profile: &str,
    bin_path: &Path,
) -> Result<()> {
    let swiftpm_profile = if profile == "release" {
        "release"
    } else {
        "debug"
    };
    let source = root
        .join(".build")
        .join(swiftpm_profile)
        .join(config.app_name());
    if !source.exists() {
        return Err(BriskError::Message(format!(
            "SwiftPM did not produce {}",
            source.display()
        )));
    }
    fs::copy(&source, bin_path)?;
    status("compile", bin_path.display());
    Ok(())
}

fn write_package_swift(root: &Path, config: &BriskConfig) -> Result<()> {
    let package = package_swift(config);
    fs::write(root.join("Package.swift"), package)?;
    status_dim("write", root.join("Package.swift").display());
    Ok(())
}

fn package_swift(config: &BriskConfig) -> String {
    let package_deps = config
        .dependencies
        .iter()
        .map(package_dependency)
        .collect::<Vec<_>>()
        .join(",\n        ");
    let products = config
        .dependencies
        .iter()
        .flat_map(|dependency| {
            let package = dependency
                .package
                .clone()
                .unwrap_or_else(|| package_name_from_url(&dependency.url));
            dependency.products.iter().map(move |product| {
                format!(".product(name: \"{product}\", package: \"{package}\")")
            })
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "// swift-tools-version: 5.9\nimport PackageDescription\n\nlet package = Package(\n    name: \"{}\",\n    platforms: [.macOS(.v{})],\n    products: [.executable(name: \"{}\", targets: [\"{}\"])],\n    dependencies: [{}],\n    targets: [\n        .executableTarget(name: \"{}\", dependencies: [{}], path: \"{}\"{}),\n        .testTarget(name: \"{}Tests\", dependencies: [\"{}\"], path: \"{}\")\n    ]\n)\n",
        config.app_name(),
        swiftpm_platform_version(config.deployment_target()),
        config.app_name(),
        config.app_name(),
        package_deps,
        config.app_name(),
        products,
        config.app.sources.display(),
        swift_settings(config),
        config.app_name(),
        config.app_name(),
        config.test.sources.display()
    )
}

fn swiftpm_platform_version(version: &str) -> String {
    version.split('.').next().unwrap_or(version).to_string()
}

fn package_dependency(dependency: &SwiftPackageDependency) -> String {
    let requirement = if let Some(exact) = &dependency.requirement.exact {
        format!("exact: \"{exact}\"")
    } else if let Some(branch) = &dependency.requirement.branch {
        format!("branch: \"{branch}\"")
    } else if let Some(revision) = &dependency.requirement.revision {
        format!("revision: \"{revision}\"")
    } else {
        format!(
            "from: \"{}\"",
            dependency.requirement.from.as_deref().unwrap_or("1.0.0")
        )
    };
    format!(".package(url: \"{}\", {requirement})", dependency.url)
}

fn package_name_from_url(url: &str) -> String {
    url.trim_end_matches(".git")
        .rsplit('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

fn swift_settings(config: &BriskConfig) -> String {
    if config.app.swift_flags.is_empty() {
        return String::new();
    }
    let flags = config
        .app
        .swift_flags
        .iter()
        .map(|flag| format!(".unsafeFlags([\"{flag}\"])"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(", swiftSettings: [{flags}]")
}

fn create_bundle(
    root: &Path,
    config: &BriskConfig,
    bin_path: &Path,
    app: &Path,
    verbose: bool,
) -> Result<()> {
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
    copy_resources(root, config, &resources)?;
    compile_assets(root, config, &resources, verbose)?;
    Ok(())
}

fn copy_resources(root: &Path, config: &BriskConfig, resources: &Path) -> Result<()> {
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

fn compile_assets(
    root: &Path,
    config: &BriskConfig,
    resources: &Path,
    verbose: bool,
) -> Result<()> {
    for catalog in &config.app.asset_catalogs {
        let source = root.join(catalog);
        if !source.exists() {
            continue;
        }
        let asset_spinner = spinner("compiling assets");
        let mut actool = command("xcrun");
        actool
            .arg("actool")
            .arg("--compile")
            .arg(resources)
            .arg("--platform")
            .arg("macosx")
            .arg("--minimum-deployment-target")
            .arg(config.deployment_target());
        if let Some(icon) = &config.app.app_icon {
            actool.arg("--app-icon").arg(icon);
        }
        actool.arg(&source);
        if verbose {
            status_dim("run", actool.display());
        }
        actool
            .run_silent()
            .inspect_err(|_| asset_spinner.finish_and_clear())?;
        asset_spinner.finish_and_clear();
        status_dim("assets", source.display());
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

fn zip_archive(root: &Path, archive: &Path, config: &BriskConfig, verbose: bool) -> Result<()> {
    let export = config.archive.export_path.clone().unwrap_or_else(|| {
        root.join(".brisk")
            .join("Archives")
            .join(format!("{}.zip", config.app_name()))
    });
    if let Some(parent) = export.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut ditto = command("ditto");
    ditto
        .arg("-c")
        .arg("-k")
        .arg("--keepParent")
        .arg(archive)
        .arg(&export);
    if verbose {
        status_dim("run", ditto.display());
    }
    ditto.run_silent()?;
    status("export", export.display());
    Ok(())
}

fn notarize_archive(archive: &Path, config: &BriskConfig, verbose: bool) -> Result<()> {
    let profile = config.signing.keychain_profile.as_ref().ok_or_else(|| {
        BriskError::Message("signing.notarize requires signing.keychain_profile".to_string())
    })?;
    let mut notary = command("xcrun");
    notary
        .arg("notarytool")
        .arg("submit")
        .arg(archive)
        .arg("--keychain-profile")
        .arg(profile)
        .arg("--wait");
    if verbose {
        status_dim("run", notary.display());
    }
    notary.run_silent()?;
    status("notarize", archive.display());
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

fn default_bundle_id(name: &str) -> Result<String> {
    let organization_id =
        global_default_organization_id()?.unwrap_or_else(|| "com.example".to_string());
    Ok(format!(
        "{}.{}",
        organization_id.trim_end_matches('.'),
        sanitize_bundle_part(name)
    ))
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

@main
struct SmokeTests {
    static func main() {
        print("Smoke tests passed")
    }
}
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
    if let Some(icon) = &config.app.app_icon {
        entries.push(plist_entry("CFBundleIconName", icon));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::new_config;

    #[test]
    fn escapes_xml_values() {
        assert_eq!(xml_escape("a&b<c>\"'"), "a&amp;b&lt;c&gt;&quot;&apos;");
    }

    #[test]
    fn generated_plist_includes_icon() {
        let mut config = new_config("Demo", "com.example.demo".to_string());
        config.app.app_icon = Some("AppIcon".to_string());
        let plist = info_plist(&config);
        assert!(plist.contains("CFBundleIconName"));
        assert!(plist.contains("AppIcon"));
    }

    #[test]
    fn app_path_uses_profile_and_app_name() {
        let config = new_config("Demo", "com.example.demo".to_string());
        assert_eq!(
            app_path(Path::new("/tmp/demo"), &config, "release"),
            PathBuf::from("/tmp/demo/.build/release/Demo.app")
        );
    }

    #[test]
    fn swiftpm_platform_version_drops_minor_version() {
        assert_eq!(swiftpm_platform_version("13.3"), "13");
    }

    #[test]
    fn package_name_handles_git_urls() {
        assert_eq!(
            package_name_from_url("https://github.com/apple/swift-argument-parser.git"),
            "swift-argument-parser"
        );
    }

    #[test]
    fn generated_package_uses_explicit_package_identity() {
        let mut config = new_config("Demo", "com.example.demo".to_string());
        config
            .dependencies
            .push(crate::config::SwiftPackageDependency {
                url: "https://github.com/example/RepoName.git".to_string(),
                package: Some("custom-identity".to_string()),
                requirement: crate::config::SwiftPackageRequirement::default(),
                products: vec!["LibraryProduct".to_string()],
            });
        let manifest = package_swift(&config);
        assert!(manifest.contains("package: \"custom-identity\""));
        assert!(manifest.contains(".product(name: \"LibraryProduct\""));
    }
}
