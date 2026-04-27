mod cmd;
mod config;
mod direct;
mod ui;
mod version;
mod xcode;

use clap::{Parser, Subcommand, ValueEnum};
use cmd::command;
use config::{BriskConfig, has_manifest};
use console::style;
use direct::{archive_direct_app, build_direct_app, init_app, new_app, test_direct_app};
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;
use ui::status;
use version::{BRISK_VERSION, is_newer};
use xcode::{
    archive_xcode_app, build_xcode_app, has_xcode_project, list_xcode_project, test_xcode_app,
};

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
    #[error("json decode error: {0}")]
    JsonDecode(#[from] serde_json::Error),
}

#[derive(Parser)]
#[command(name = "brisk")]
#[command(version = BRISK_VERSION)]
#[command(about = "brisk - native builds for Swift macOS apps", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true, help = "Show the commands brisk runs")]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Create a new SwiftUI macOS app")]
    New {
        name: String,
        #[arg(long, help = "Bundle identifier, defaults to com.example.<app>")]
        bundle_id: Option<String>,
    },
    #[command(about = "Create .brisk.toml for an existing Swift app")]
    Init {
        #[arg(long, help = "Bundle identifier, inferred when possible")]
        bundle_id: Option<String>,
        #[arg(long, help = "Overwrite an existing Brisk manifest")]
        force: bool,
    },
    #[command(about = "Build the app bundle")]
    #[command(visible_alias = "b")]
    Build(XcodeBuildArgs),
    #[command(about = "Build and launch the app")]
    #[command(visible_alias = "r")]
    Run(XcodeBuildArgs),
    #[command(about = "Print the built .app path")]
    Path(XcodeBuildArgs),
    #[command(about = "Run tests")]
    Test(XcodeBuildArgs),
    #[command(about = "Archive the app")]
    Archive {
        #[command(flatten)]
        args: XcodeBuildArgs,
        #[arg(long, help = "Archive output path")]
        archive_path: Option<PathBuf>,
    },
    #[command(about = "List Xcode schemes and targets")]
    List(XcodeContainerArgs),
    #[command(about = "Update brisk")]
    Update {
        #[arg(help = "Optional compatibility alias: self or self-nightly")]
        action: Option<String>,
        #[arg(short = 's', long = "self", hide = true)]
        update_self: bool,
        #[arg(short, long, help = "Update from GitHub HEAD")]
        nightly: bool,
        #[arg(
            short,
            long,
            help = "Force reinstall even when current version is latest"
        )]
        force: bool,
        #[arg(
            long,
            help = "After nightly self-update, clean Cargo git cache for brisk"
        )]
        clean: bool,
        #[arg(long, help = "After nightly self-update, keep Cargo git cache")]
        no_clean: bool,
    },
    #[command(about = "Check required Apple CLI tools")]
    Doctor,
    #[command(about = "Remove build output")]
    Clean,
}

#[derive(clap::Args, Clone, Debug)]
struct XcodeContainerArgs {
    #[arg(long, help = "Xcode workspace")]
    workspace: Option<PathBuf>,
    #[arg(long, help = "Xcode project")]
    project: Option<PathBuf>,
}

#[derive(clap::Args, Clone, Debug)]
struct XcodeArgs {
    #[command(flatten)]
    container: XcodeContainerArgs,
    #[arg(long, help = "Xcode scheme")]
    scheme: Option<String>,
    #[arg(long, help = "Xcode build configuration")]
    configuration: Option<String>,
    #[arg(long, help = "Xcode destination specifier")]
    destination: Option<String>,
    #[arg(long, help = "Xcode SDK")]
    sdk: Option<String>,
    #[arg(last = true, help = "Additional xcodebuild arguments")]
    xcode_args: Vec<String>,
}

#[derive(clap::Args, Clone, Debug)]
struct XcodeBuildArgs {
    #[arg(short, long, help = "Release build")]
    release: bool,
    #[arg(long, value_enum, default_value_t = Backend::Auto, help = "Build backend")]
    backend: Backend,
    #[command(flatten)]
    xcode: XcodeArgs,
}

#[derive(Clone, Debug)]
struct BuildOptions {
    release: bool,
    verbose: bool,
    backend: Backend,
    scheme: Option<String>,
    workspace: Option<PathBuf>,
    project: Option<PathBuf>,
    configuration: Option<String>,
    destination: Option<String>,
    sdk: Option<String>,
    xcode_args: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Backend {
    Auto,
    Direct,
    Xcode,
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
        Commands::Init { bundle_id, force } => init_app(&cwd()?, bundle_id, force),
        Commands::Build(args) => build_app(build_options(args, cli.verbose)).map(|_| ()),
        Commands::Run(args) => {
            let app = build_app(build_options(args, cli.verbose))?;
            status("launch", app.display());
            command("open").arg(&app).run()?;
            Ok(())
        }
        Commands::Path(args) => {
            let opts = build_options(args, cli.verbose);
            println!("{}", app_path_for_current_project(&opts)?.display());
            Ok(())
        }
        Commands::Test(args) => test_app(build_options(args, cli.verbose)),
        Commands::Archive { args, archive_path } => {
            archive_app(build_options(args, cli.verbose), archive_path).map(|_| ())
        }
        Commands::List(args) => list_xcode_project(&cwd()?, args.workspace, args.project),
        Commands::Update {
            action,
            update_self,
            nightly,
            force,
            clean,
            no_clean,
        } => update(action, update_self, nightly, force, clean, no_clean),
        Commands::Doctor => doctor(),
        Commands::Clean => clean(),
    }
}

fn build_options(args: XcodeBuildArgs, verbose: bool) -> BuildOptions {
    BuildOptions {
        release: args.release,
        verbose,
        backend: args.backend,
        scheme: args.xcode.scheme,
        workspace: args.xcode.container.workspace,
        project: args.xcode.container.project,
        configuration: args.xcode.configuration,
        destination: args.xcode.destination,
        sdk: args.xcode.sdk,
        xcode_args: args.xcode.xcode_args,
    }
}

fn build_app(opts: BuildOptions) -> Result<PathBuf> {
    let root = cwd()?;
    if should_use_xcode(&root, &opts)? {
        doctor_quiet(true)?;
        build_xcode_app(&root, &opts)
    } else {
        doctor_quiet(false)?;
        build_direct_app(&root, opts.release, opts.verbose)
    }
}

fn test_app(opts: BuildOptions) -> Result<()> {
    let root = cwd()?;
    if should_use_xcode(&root, &opts)? {
        doctor_quiet(true)?;
        test_xcode_app(&root, &opts)
    } else {
        doctor_quiet(false)?;
        test_direct_app(&root, opts.verbose)
    }
}

fn archive_app(opts: BuildOptions, archive_path: Option<PathBuf>) -> Result<PathBuf> {
    let root = cwd()?;
    if should_use_xcode(&root, &opts)? {
        doctor_quiet(true)?;
        archive_xcode_app(&root, &opts, archive_path)
    } else {
        doctor_quiet(false)?;
        archive_direct_app(&root, opts.release, opts.verbose, archive_path)
    }
}

fn app_path_for_current_project(opts: &BuildOptions) -> Result<PathBuf> {
    let root = cwd()?;
    if should_use_xcode(&root, opts)? {
        xcode::xcode_app_path(&root, opts)
    } else {
        let config = BriskConfig::load(&root)?;
        Ok(direct::app_path(&root, &config, profile(opts.release)))
    }
}

fn should_use_xcode(root: &Path, opts: &BuildOptions) -> Result<bool> {
    match opts.backend {
        Backend::Xcode => Ok(true),
        Backend::Direct => {
            if has_xcode_only_options(opts) {
                Err(BriskError::Message(
                    "--backend direct cannot be combined with Xcode-only flags".to_string(),
                ))
            } else {
                Ok(false)
            }
        }
        Backend::Auto => {
            if has_manifest(root) {
                Ok(false)
            } else {
                Ok(has_xcode_only_options(opts) || has_xcode_project(root))
            }
        }
    }
}

fn has_xcode_only_options(opts: &BuildOptions) -> bool {
    opts.workspace.is_some()
        || opts.project.is_some()
        || opts.scheme.is_some()
        || opts.configuration.is_some()
        || opts.destination.is_some()
        || opts.sdk.is_some()
        || !opts.xcode_args.is_empty()
}

fn update(
    action: Option<String>,
    mut update_self: bool,
    mut nightly: bool,
    force: bool,
    clean: bool,
    no_clean: bool,
) -> Result<()> {
    if let Some(action) = action {
        match action.as_str() {
            "s" | "self" => update_self = true,
            "sn" | "self-nightly" => {
                update_self = true;
                nightly = true;
            }
            other => {
                return Err(BriskError::Message(format!(
                    "unknown update alias '{other}' (use self or self-nightly)"
                )));
            }
        }
    }
    if clean && no_clean {
        return Err(BriskError::Message(
            "cannot specify both --clean and --no-clean".to_string(),
        ));
    }
    let _ = update_self;
    if nightly {
        update_from_source(force, clean, no_clean)
    } else {
        update_from_crates(force)
    }
}

fn update_from_crates(force: bool) -> Result<()> {
    let latest = latest_crate_version()?;
    println!(
        "  {} {}",
        style("current:").dim(),
        style(BRISK_VERSION).cyan()
    );
    println!("  {} {}", style("latest: ").dim(), style(&latest).cyan());
    if !is_newer(BRISK_VERSION, &latest) && !force {
        println!("{} already up to date", style("✓").green());
        println!(
            "  {} use {} to reinstall anyway",
            style("hint:").dim(),
            style("-f / --force").yellow()
        );
        return Ok(());
    }
    let mut args = vec!["install", "brisk", "--bin", "brisk"];
    if force || is_newer(BRISK_VERSION, &latest) {
        args.push("--force");
    }
    run_cargo_install(&args, "install")?;
    println!(
        "{} updated to {}",
        style("✓").green(),
        style(format!("v{latest}")).cyan()
    );
    Ok(())
}

fn latest_crate_version() -> Result<String> {
    let output = command("curl")
        .arg("-fsSL")
        .arg("https://crates.io/api/v1/crates/brisk")
        .output()
        .map_err(|err| BriskError::Message(format!("crates.io API request failed: {err}")))?;
    let text = String::from_utf8_lossy(&output);
    let needle = "\"max_stable_version\":\"";
    let start = text
        .find(needle)
        .ok_or_else(|| BriskError::Message("failed to parse crates.io API response".to_string()))?
        + needle.len();
    let rest = &text[start..];
    let end = rest
        .find('"')
        .ok_or_else(|| BriskError::Message("failed to parse crates.io API response".to_string()))?;
    Ok(rest[..end].to_string())
}

fn update_from_source(force: bool, clean: bool, no_clean: bool) -> Result<()> {
    println!(
        "  {} {}",
        style("current:").dim(),
        style(BRISK_VERSION).cyan()
    );
    println!(
        "  {} {}",
        style("channel:").dim(),
        style("nightly (GitHub HEAD)").yellow()
    );
    let mut args = vec![
        "install",
        "--git",
        "https://github.com/plyght/brisk",
        "--bin",
        "brisk",
    ];
    if force {
        args.push("--force");
    }
    run_cargo_install(&args, "build")?;
    if clean {
        let removed = cleanup_nightly_artifacts()?;
        println!(
            "{} cleaned {} nightly cache entr{}",
            style("✓").green(),
            removed,
            if removed == 1 { "y" } else { "ies" }
        );
    } else if !no_clean {
        println!(
            "  {} use {} or {} to control nightly cache cleanup",
            style("hint:").dim(),
            style("--clean").yellow(),
            style("--no-clean").yellow()
        );
    }
    println!("{} installed nightly build from HEAD", style("✓").green());
    Ok(())
}

fn run_cargo_install(args: &[&str], label: &str) -> Result<()> {
    println!(
        "  {} running {} (live output below)",
        style(format!("{label}:")).dim(),
        style(format!("cargo {}", args.join(" "))).yellow()
    );
    let status = std::process::Command::new("cargo")
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|err| BriskError::Message(format!("failed to run cargo: {err}")))?;
    if !status.success() {
        return Err(BriskError::Message("cargo install failed".to_string()));
    }
    Ok(())
}

fn cleanup_nightly_artifacts() -> Result<usize> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| BriskError::Message("HOME is not set".to_string()))?;
    let mut removed = 0usize;
    for root in [
        home.join(".cargo/git/checkouts"),
        home.join(".cargo/git/db"),
    ] {
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("brisk-") && path.is_dir() && std::fs::remove_dir_all(&path).is_ok()
            {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

fn doctor() -> Result<()> {
    ui::section("toolchain");
    let checks = [
        ("swiftc", "Swift compiler"),
        ("xcodebuild", "Xcode builder"),
        ("codesign", "Code signing"),
        ("open", "App launcher"),
        ("xcrun", "Xcode tool runner"),
    ];
    for (bin, label) in checks {
        let path = which(bin)?;
        println!(
            "{} {:<16} {}",
            style("✓").green().bold(),
            label,
            style(path.display()).dim()
        );
    }
    if let Ok(output) = command("xcode-select").arg("-p").output() {
        let path = String::from_utf8_lossy(&output).trim().to_string();
        if !path.is_empty() {
            println!(
                "{} {:<16} {}",
                style("✓").green().bold(),
                "Developer dir",
                style(path).dim()
            );
        }
    }
    ui::success("ready");
    Ok(())
}

fn doctor_quiet(needs_xcodebuild: bool) -> Result<()> {
    for bin in ["swiftc", "codesign", "open", "xcrun"] {
        ensure_tool(bin)?;
    }
    if needs_xcodebuild {
        ensure_tool("xcodebuild")?;
    }
    Ok(())
}

fn ensure_tool(bin: &str) -> Result<()> {
    which(bin).map(|_| ())
}

fn which(bin: &str) -> Result<PathBuf> {
    let output = command("/usr/bin/which").arg(bin).output().map_err(|_| {
        BriskError::Message(format!(
            "missing {bin}; install Xcode Command Line Tools with xcode-select --install"
        ))
    })?;
    let path = String::from_utf8_lossy(&output).trim().to_string();
    Ok(PathBuf::from(path))
}

fn clean() -> Result<()> {
    let root = cwd()?;
    let direct_dir = root.join(".build");
    if direct_dir.exists() {
        std::fs::remove_dir_all(&direct_dir)?;
        status("clean", direct_dir.display());
    }
    let xcode_dir = root.join(".brisk").join("DerivedData");
    if xcode_dir.exists() {
        std::fs::remove_dir_all(&xcode_dir)?;
        status("clean", xcode_dir.display());
    }
    Ok(())
}

fn cwd() -> Result<PathBuf> {
    Ok(std::env::current_dir()?)
}

fn profile(release: bool) -> &'static str {
    if release { "release" } else { "debug" }
}
