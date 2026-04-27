use crate::cmd::command;
use crate::ui::{section, spinner, status, status_dim, success};
use crate::{BriskError, BuildOptions, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug)]
pub(crate) enum XcodeContainer {
    Workspace(PathBuf),
    Project(PathBuf),
}

#[derive(Deserialize)]
struct BuildSettingsResponse {
    project: BuildSettingsProject,
}

#[derive(Deserialize)]
struct BuildSettingsProject {
    targets: Vec<BuildSettingsTarget>,
}

#[derive(Deserialize)]
struct BuildSettingsTarget {
    #[serde(rename = "buildSettings")]
    build_settings: BuildSettings,
}

#[derive(Deserialize)]
struct BuildSettings {
    #[serde(rename = "BUILT_PRODUCTS_DIR")]
    built_products_dir: Option<String>,
    #[serde(rename = "FULL_PRODUCT_NAME")]
    full_product_name: Option<String>,
}

pub fn build_xcode_app(root: &Path, opts: &BuildOptions) -> Result<PathBuf> {
    let started = Instant::now();
    let plan = XcodePlan::resolve(root, opts)?;
    let derived_data = derived_data_path(root);
    fs::create_dir_all(&derived_data)?;

    status("backend", "xcodebuild");
    status("profile", plan.configuration.as_str());
    status("scheme", &plan.scheme);
    status("derived", derived_data.display());

    let build_spinner = spinner("building Xcode project");
    let mut build = command("xcodebuild");
    plan.add_build_args(&mut build, &derived_data);
    build.arg("build");
    for arg in &opts.xcode_args {
        build.arg(arg);
    }
    if opts.verbose {
        status_dim("run", build.display());
    }
    build
        .run_silent()
        .inspect_err(|_| build_spinner.finish_and_clear())?;
    build_spinner.finish_and_clear();

    let app = xcode_app_path_with_plan(root, &plan, opts)?;
    status("bundle", app.display());
    success(format!(
        "built {} in {:.1}s",
        app.display(),
        started.elapsed().as_secs_f32()
    ));
    Ok(app)
}

pub fn xcode_app_path(root: &Path, opts: &BuildOptions) -> Result<PathBuf> {
    let plan = XcodePlan::resolve(root, opts)?;
    xcode_app_path_with_plan(root, &plan, opts)
}

pub fn test_xcode_app(root: &Path, opts: &BuildOptions) -> Result<()> {
    let plan = XcodePlan::resolve(root, opts)?;
    let derived_data = derived_data_path(root);
    fs::create_dir_all(&derived_data)?;
    status("backend", "xcodebuild");
    status("scheme", &plan.scheme);
    let test_spinner = spinner("running Xcode tests");
    let mut test = command("xcodebuild");
    plan.add_build_args(&mut test, &derived_data);
    test.arg("test");
    for arg in &opts.xcode_args {
        test.arg(arg);
    }
    if opts.verbose {
        status_dim("run", test.display());
    }
    test.run_silent()
        .inspect_err(|_| test_spinner.finish_and_clear())?;
    test_spinner.finish_and_clear();
    success("tests passed");
    Ok(())
}

pub fn archive_xcode_app(
    root: &Path,
    opts: &BuildOptions,
    archive_path: Option<PathBuf>,
) -> Result<PathBuf> {
    let mut archive_opts = opts.clone();
    if archive_opts.configuration.is_none() && !archive_opts.release {
        archive_opts.configuration = Some("Release".to_string());
    }
    let plan = XcodePlan::resolve(root, &archive_opts)?;
    let derived_data = derived_data_path(root);
    fs::create_dir_all(&derived_data)?;
    let archive = archive_path.unwrap_or_else(|| {
        root.join(".brisk")
            .join("Archives")
            .join(format!("{}.xcarchive", plan.scheme))
    });
    if let Some(parent) = archive.parent() {
        fs::create_dir_all(parent)?;
    }
    status("backend", "xcodebuild");
    status("scheme", &plan.scheme);
    status("archive", archive.display());
    let archive_spinner = spinner("archiving Xcode project");
    let mut command = command("xcodebuild");
    plan.add_build_args(&mut command, &derived_data);
    command.arg("-archivePath").arg(&archive).arg("archive");
    for arg in &archive_opts.xcode_args {
        command.arg(arg);
    }
    if archive_opts.verbose {
        status_dim("run", command.display());
    }
    command
        .run_silent()
        .inspect_err(|_| archive_spinner.finish_and_clear())?;
    archive_spinner.finish_and_clear();
    success(format!("archived {}", archive.display()));
    Ok(archive)
}

pub fn list_xcode_project(
    root: &Path,
    workspace: Option<PathBuf>,
    project: Option<PathBuf>,
) -> Result<()> {
    let opts = BuildOptions {
        release: false,
        verbose: false,
        scheme: None,
        workspace,
        project,
        configuration: None,
        destination: None,
        backend: crate::Backend::Xcode,
        sdk: None,
        xcode_args: Vec::new(),
    };
    let container = resolve_container(root, &opts)?;
    let mut list = command("xcodebuild");
    match &container {
        XcodeContainer::Workspace(path) => {
            list.arg("-workspace").arg(path);
        }
        XcodeContainer::Project(path) => {
            list.arg("-project").arg(path);
        }
    }
    list.arg("-list");
    let output = list.output()?;
    section("xcode");
    print!("{}", String::from_utf8_lossy(&output));
    Ok(())
}

fn xcode_app_path_with_plan(root: &Path, plan: &XcodePlan, opts: &BuildOptions) -> Result<PathBuf> {
    let derived_data = derived_data_path(root);
    let mut settings = command("xcodebuild");
    plan.add_build_args(&mut settings, &derived_data);
    settings.arg("-showBuildSettings").arg("-json");
    for arg in &opts.xcode_args {
        settings.arg(arg);
    }
    if opts.verbose {
        status_dim("run", settings.display());
    }
    let output = settings.output()?;
    let parsed: Vec<BuildSettingsResponse> = serde_json::from_slice(&output)?;
    for project in parsed {
        for target in project.project.targets {
            let Some(dir) = target.build_settings.built_products_dir else {
                continue;
            };
            let Some(name) = target.build_settings.full_product_name else {
                continue;
            };
            if name.ends_with(".app") {
                return Ok(PathBuf::from(dir).join(name));
            }
        }
    }
    Err(BriskError::Message(
        "xcodebuild did not report an app product for this scheme".to_string(),
    ))
}

struct XcodePlan {
    container: XcodeContainer,
    scheme: String,
    configuration: String,
    destination: Option<String>,
    sdk: Option<String>,
}

impl XcodePlan {
    fn resolve(root: &Path, opts: &BuildOptions) -> Result<Self> {
        let container = resolve_container(root, opts)?;
        let scheme = match &opts.scheme {
            Some(scheme) => scheme.clone(),
            None => infer_scheme(&container)?,
        };
        let configuration = opts
            .configuration
            .clone()
            .unwrap_or_else(|| if opts.release { "Release" } else { "Debug" }.to_string());
        Ok(Self {
            container,
            scheme,
            configuration,
            destination: opts.destination.clone(),
            sdk: opts.sdk.clone(),
        })
    }

    fn add_args(&self, cmd: &mut crate::cmd::Cmd) {
        match &self.container {
            XcodeContainer::Workspace(path) => {
                cmd.arg("-workspace").arg(path);
            }
            XcodeContainer::Project(path) => {
                cmd.arg("-project").arg(path);
            }
        }
        cmd.arg("-scheme").arg(&self.scheme);
        if let Some(destination) = &self.destination {
            cmd.arg("-destination").arg(destination);
        }
        if let Some(sdk) = &self.sdk {
            cmd.arg("-sdk").arg(sdk);
        }
    }

    fn add_build_args(&self, cmd: &mut crate::cmd::Cmd, derived_data: &Path) {
        self.add_args(cmd);
        cmd.arg("-configuration")
            .arg(&self.configuration)
            .arg("-derivedDataPath")
            .arg(derived_data);
    }
}

fn resolve_container(root: &Path, opts: &BuildOptions) -> Result<XcodeContainer> {
    if let Some(workspace) = &opts.workspace {
        return Ok(XcodeContainer::Workspace(absolutize(root, workspace)));
    }
    if let Some(project) = &opts.project {
        return Ok(XcodeContainer::Project(absolutize(root, project)));
    }
    discover_xcode_project(root)
}

pub fn has_xcode_project(root: &Path) -> bool {
    discover_xcode_project(root).is_ok()
}

pub(crate) fn discover_xcode_project(root: &Path) -> Result<XcodeContainer> {
    let mut workspaces = Vec::new();
    let mut projects = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("xcworkspace") => workspaces.push(path),
            Some("xcodeproj") => projects.push(path),
            _ => {}
        }
    }
    workspaces.sort();
    projects.sort();
    if let Some(workspace) = workspaces.into_iter().next() {
        Ok(XcodeContainer::Workspace(workspace))
    } else if let Some(project) = projects.into_iter().next() {
        Ok(XcodeContainer::Project(project))
    } else {
        Err(BriskError::Message(
            "no .xcodeproj or .xcworkspace found".to_string(),
        ))
    }
}

fn infer_scheme(container: &XcodeContainer) -> Result<String> {
    let path = match container {
        XcodeContainer::Workspace(path) | XcodeContainer::Project(path) => path,
    };
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToString::to_string)
        .ok_or_else(|| {
            BriskError::Message(format!("could not infer scheme from {}", path.display()))
        })
}

fn derived_data_path(root: &Path) -> PathBuf {
    root.join(".brisk").join("DerivedData")
}

fn absolutize(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}
