use clap::{Parser, Subcommand};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use thiserror::Error;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SPINNER_TICK_CHARS: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";

type Result<T> = std::result::Result<T, BriskError>;

#[derive(Error, Debug)]
enum BriskError {
    #[error("{0}")]
    Message(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("toml decode error: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error("toml encode error: {0}")]
    TomlEncode(#[from] toml::ser::Error),
}

#[derive(Parser)]
#[command(name = "brisk")]
#[command(version = VERSION)]
#[command(about = "brisk - Cargo-like SwiftUI macOS app builds", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Create a new SwiftUI macOS app")]
    New {
        name: String,
        #[arg(long, help = "Bundle identifier, defaults to com.example.<app>")]
        bundle_id: Option<String>,
    },
    #[command(about = "Build the app bundle  [alias: b]")]
    #[command(visible_alias = "b")]
    Build {
        #[arg(short, long, help = "Release build")]
        release: bool,
    },
    #[command(about = "Build and launch the app  [alias: r]")]
    #[command(visible_alias = "r")]
    Run {
        #[arg(short, long, help = "Release build")]
        release: bool,
    },
    #[command(about = "Print the built .app path")]
    Path {
        #[arg(short, long, help = "Release build")]
        release: bool,
    },
    #[command(about = "Check required Apple CLI tools")]
    Doctor,
    #[command(about = "Remove build output")]
    Clean,
}

#[derive(Debug, Serialize, Deserialize)]
struct BriskConfig {
    name: String,
    bundle_id: String,
    deployment_target: String,
}

impl BriskConfig {
    fn load(root: &Path) -> Result<Self> {
        let path = root.join("brisk.toml");
        let raw = fs::read_to_string(&path).map_err(|e| {
            BriskError::Message(format!(
                "could not read {}: {}\nrun {} first",
                path.display(),
                e,
                style("brisk new <name>").cyan()
            ))
        })?;
        Ok(toml::from_str(&raw)?)
    }

    fn save(&self, root: &Path) -> Result<()> {
        fs::write(root.join("brisk.toml"), toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{} {}", style("error:").red().bold(), err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::New { name, bundle_id } => new_app(&name, bundle_id),
        Commands::Build { release } => build_app(release).map(|_| ()),
        Commands::Run { release } => {
            let app = build_app(release)?;
            status("launch", app.display());
            command("open").arg(&app).run()?;
            Ok(())
        }
        Commands::Path { release } => {
            let config = BriskConfig::load(&cwd()?)?;
            println!("{}", app_path(&cwd()?, &config, profile(release)).display());
            Ok(())
        }
        Commands::Doctor => doctor(),
        Commands::Clean => clean(),
    }
}

fn new_app(name: &str, bundle_id: Option<String>) -> Result<()> {
    validate_app_name(name)?;
    let root = cwd()?.join(name);
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
    println!("\n{}", style("next:").bold());
    println!("  cd {name}");
    println!("  brisk run");
    Ok(())
}

fn build_app(release: bool) -> Result<PathBuf> {
    doctor_quiet()?;
    let root = cwd()?;
    let config = BriskConfig::load(&root)?;
    let profile = profile(release);
    let build_dir = root.join(".build").join(profile);
    let bin_path = build_dir.join(&config.name);
    let app = app_path(&root, &config, profile);

    fs::create_dir_all(&build_dir)?;

    let swift_files = collect_swift_files(&root.join("Sources"))?;
    if swift_files.is_empty() {
        return Err(BriskError::Message(
            "no Swift files found in Sources".to_string(),
        ));
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
    swiftc
        .run_silent()
        .inspect_err(|_| compile_spinner.finish_and_clear())?;
    compile_spinner.finish_and_clear();
    status("compile", bin_path.display());

    create_bundle(&config, &bin_path, &app)?;
    status("bundle", app.display());

    let spinner = spinner("ad-hoc signing");
    command("codesign")
        .arg("--force")
        .arg("--deep")
        .arg("--sign")
        .arg("-")
        .arg(&app)
        .run_silent()
        .inspect_err(|_| spinner.finish_and_clear())?;
    spinner.finish_and_clear();
    status("sign", "ad-hoc");

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

fn doctor() -> Result<()> {
    let checks = [
        ("swiftc", "Swift compiler"),
        ("codesign", "Code signing"),
        ("open", "App launcher"),
    ];
    for (bin, label) in checks {
        ensure_tool(bin)?;
        println!("{} {}", style("✓").green().bold(), label);
    }
    Ok(())
}

fn doctor_quiet() -> Result<()> {
    for bin in ["swiftc", "codesign", "open"] {
        ensure_tool(bin)?;
    }
    Ok(())
}

fn ensure_tool(bin: &str) -> Result<()> {
    command("/usr/bin/which")
        .arg(bin)
        .run_silent()
        .map_err(|_| {
            BriskError::Message(format!(
                "missing {bin}; install Xcode Command Line Tools with xcode-select --install"
            ))
        })
}

fn clean() -> Result<()> {
    let dir = cwd()?.join(".build");
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    status("clean", dir.display());
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

struct Cmd {
    inner: Command,
}

fn command(program: &str) -> Cmd {
    Cmd {
        inner: Command::new(program),
    }
}

impl Cmd {
    fn arg<T: AsRef<OsStr>>(&mut self, arg: T) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    fn run(&mut self) -> Result<()> {
        let status = self.inner.status()?;
        if status.success() {
            Ok(())
        } else {
            Err(BriskError::Message(format!(
                "command failed with status {status}"
            )))
        }
    }

    fn run_silent(&mut self) -> Result<()> {
        let output = self
            .inner
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            Err(BriskError::Message(if detail.is_empty() {
                format!("command failed with status {}", output.status)
            } else {
                detail
            }))
        }
    }
}

fn spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars(SPINNER_TICK_CHARS),
    );
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner.set_message(message.to_string());
    spinner
}

fn status(action: &str, message: impl std::fmt::Display) {
    println!(
        "{} {}",
        style(format!("{action:>8}")).green().bold(),
        message
    );
}

fn cwd() -> Result<PathBuf> {
    Ok(std::env::current_dir()?)
}

fn profile(release: bool) -> &'static str {
    if release { "release" } else { "debug" }
}

fn app_path(root: &Path, config: &BriskConfig, profile: &str) -> PathBuf {
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
