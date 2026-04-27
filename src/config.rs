use crate::{BriskError, Result};
use console::style;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct BriskConfig {
    #[serde(default)]
    pub package: PackageConfig,
    #[serde(default)]
    pub app: AppConfig,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub dependencies: Vec<SwiftPackageDependency>,
    #[serde(default)]
    pub signing: SigningConfig,
    #[serde(default)]
    pub test: TestConfig,
    #[serde(default)]
    pub archive: ArchiveConfig,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub bundle_id: Option<String>,
    #[serde(default)]
    pub deployment_target: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub bundle_id: String,
    #[serde(default = "default_deployment_target")]
    pub deployment_target: String,
    #[serde(default = "default_sources")]
    pub sources: PathBuf,
    #[serde(default)]
    pub resources: Vec<PathBuf>,
    #[serde(default)]
    pub asset_catalogs: Vec<PathBuf>,
    #[serde(default)]
    pub app_icon: Option<String>,
    #[serde(default)]
    pub entitlements: Option<PathBuf>,
    #[serde(default)]
    pub frameworks: Vec<String>,
    #[serde(default)]
    pub linker_flags: Vec<String>,
    #[serde(default)]
    pub swift_flags: Vec<String>,
    #[serde(default)]
    pub info: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default = "default_architectures")]
    pub architectures: Vec<String>,
    #[serde(default = "default_platform")]
    pub platform: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwiftPackageDependency {
    pub url: String,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(default)]
    pub requirement: SwiftPackageRequirement,
    #[serde(default)]
    pub products: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwiftPackageRequirement {
    #[serde(default)]
    pub exact: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub revision: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SigningConfig {
    #[serde(default = "default_signing_identity")]
    pub identity: String,
    #[serde(default)]
    pub hardened_runtime: bool,
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub notarize: bool,
    #[serde(default)]
    pub keychain_profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestConfig {
    #[serde(default = "default_tests")]
    pub sources: PathBuf,
    #[serde(default)]
    pub xctest: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ArchiveConfig {
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub export_path: Option<PathBuf>,
    #[serde(default)]
    pub zip: bool,
}

impl Default for PackageConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: default_version(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bundle_id: String::new(),
            deployment_target: default_deployment_target(),
            sources: default_sources(),
            resources: Vec::new(),
            asset_catalogs: Vec::new(),
            app_icon: None,
            entitlements: None,
            frameworks: Vec::new(),
            linker_flags: Vec::new(),
            swift_flags: Vec::new(),
            info: BTreeMap::new(),
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            architectures: default_architectures(),
            platform: default_platform(),
        }
    }
}

impl Default for SwiftPackageRequirement {
    fn default() -> Self {
        Self {
            exact: None,
            from: Some("1.0.0".to_string()),
            branch: None,
            revision: None,
        }
    }
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            identity: default_signing_identity(),
            hardened_runtime: false,
            team_id: None,
            notarize: false,
            keychain_profile: None,
        }
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            sources: default_tests(),
            xctest: false,
        }
    }
}

impl BriskConfig {
    pub fn load(root: &Path) -> Result<Self> {
        let path = manifest_path(root).ok_or_else(|| {
            let hint = if root.join("Package.swift").exists() {
                format!(
                    "found Package.swift but no Brisk manifest\nrun {} to create one, or use brisk in a directory with an .xcodeproj/.xcworkspace",
                    style("brisk init").cyan()
                )
            } else {
                format!(
                    "run {} first, or use brisk in a directory with an .xcodeproj/.xcworkspace",
                    style("brisk new <name>").cyan()
                )
            };
            BriskError::Message(format!(
                "could not find .brisk.toml or brisk.toml in {}\n{}",
                root.display(),
                hint
            ))
        })?;
        let raw = fs::read_to_string(&path).map_err(|e| {
            BriskError::Message(format!("could not read {}: {}", path.display(), e))
        })?;
        let mut project: toml::Value = toml::from_str(&raw)?;
        if let Some(mut global) = load_global_config()? {
            normalize_global_config(&mut global);
            merge_toml(&mut global, project);
            project = global;
        }
        let mut config: Self = project.try_into()?;
        config.normalize();
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        fs::write(root.join(".brisk.toml"), toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn app_name(&self) -> &str {
        &self.package.name
    }

    pub fn bundle_id(&self) -> &str {
        &self.app.bundle_id
    }

    pub fn deployment_target(&self) -> &str {
        &self.app.deployment_target
    }

    fn normalize(&mut self) {
        if self.package.name.is_empty()
            && let Some(name) = self.name.take()
        {
            self.package.name = name;
        }
        if self.app.bundle_id.is_empty()
            && let Some(bundle_id) = self.bundle_id.take()
        {
            self.app.bundle_id = bundle_id;
        }
        if let Some(deployment_target) = self.deployment_target.take() {
            self.app.deployment_target = deployment_target;
        }
        if self.build.architectures.is_empty() {
            self.build.architectures = default_architectures();
        }
    }

    fn validate(&self) -> Result<()> {
        if self.package.name.is_empty() {
            return Err(BriskError::Message("package.name is required".to_string()));
        }
        if self.app.bundle_id.is_empty() {
            return Err(BriskError::Message("app.bundle_id is required".to_string()));
        }
        for arch in &self.build.architectures {
            match arch.as_str() {
                "arm64" | "x86_64" => {}
                _ => {
                    return Err(BriskError::Message(format!(
                        "unsupported architecture {arch}; expected arm64 or x86_64"
                    )));
                }
            }
        }
        Ok(())
    }
}

pub fn has_manifest(root: &Path) -> bool {
    manifest_path(root).is_some()
}

pub fn manifest_path(root: &Path) -> Option<PathBuf> {
    let dotfile = root.join(".brisk.toml");
    if dotfile.exists() {
        Some(dotfile)
    } else {
        let legacy = root.join("brisk.toml");
        legacy.exists().then_some(legacy)
    }
}

pub fn global_default_organization_id() -> Result<Option<String>> {
    let Some(global) = load_global_config()? else {
        return Ok(None);
    };
    Ok(global
        .get("defaults")
        .and_then(|defaults| defaults.get("organization_id"))
        .and_then(toml::Value::as_str)
        .map(str::to_string))
}

fn load_global_config() -> Result<Option<toml::Value>> {
    let Some(home) = std::env::var_os("HOME") else {
        return Ok(None);
    };
    let path = PathBuf::from(home)
        .join(".config")
        .join("brisk")
        .join("config.toml");
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(toml::from_str(&fs::read_to_string(path)?)?))
}

fn normalize_global_config(config: &mut toml::Value) {
    let Some(table) = config.as_table_mut() else {
        return;
    };
    if let Some(defaults) = table.remove("defaults")
        && let Some(defaults) = defaults.as_table()
    {
        let app = table
            .entry("app")
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if let Some(app) = app.as_table_mut()
            && let Some(value) = defaults.get("deployment_target")
        {
            app.entry("deployment_target")
                .or_insert_with(|| value.clone());
        }
        let build = table
            .entry("build")
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if let Some(build) = build.as_table_mut()
            && let Some(value) = defaults.get("architectures")
        {
            build
                .entry("architectures")
                .or_insert_with(|| value.clone());
        }
    }
}

fn merge_toml(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base), toml::Value::Table(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(base_value) => merge_toml(base_value, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

pub fn new_config(name: &str, bundle_id: String) -> BriskConfig {
    BriskConfig {
        package: PackageConfig {
            name: name.to_string(),
            version: default_version(),
        },
        app: AppConfig {
            bundle_id,
            deployment_target: default_deployment_target(),
            sources: default_sources(),
            resources: vec![PathBuf::from("Resources")],
            asset_catalogs: Vec::new(),
            app_icon: None,
            entitlements: None,
            frameworks: Vec::new(),
            linker_flags: Vec::new(),
            swift_flags: Vec::new(),
            info: BTreeMap::new(),
        },
        build: BuildConfig::default(),
        dependencies: Vec::new(),
        signing: SigningConfig::default(),
        test: TestConfig::default(),
        archive: ArchiveConfig::default(),
        name: None,
        bundle_id: None,
        deployment_target: None,
    }
}

fn default_version() -> String {
    "0.1.0".to_string()
}

fn default_deployment_target() -> String {
    "13.0".to_string()
}

fn default_sources() -> PathBuf {
    PathBuf::from("Sources")
}

fn default_tests() -> PathBuf {
    PathBuf::from("Tests")
}

fn default_signing_identity() -> String {
    "-".to_string()
}

fn default_architectures() -> Vec<String> {
    vec!["arm64".to_string()]
}

fn default_platform() -> String {
    "macos".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_config_has_required_defaults() {
        let config = new_config("Demo", "com.example.demo".to_string());
        assert_eq!(config.app_name(), "Demo");
        assert_eq!(config.bundle_id(), "com.example.demo");
        assert_eq!(config.deployment_target(), "13.0");
        assert_eq!(config.build.architectures, vec!["arm64".to_string()]);
        assert_eq!(config.signing.identity, "-");
    }

    #[test]
    fn legacy_manifest_normalizes() {
        let mut config: BriskConfig = toml::from_str(
            r#"
name = "Legacy"
bundle_id = "com.example.legacy"
deployment_target = "14.0"
"#,
        )
        .unwrap();
        config.normalize();
        assert_eq!(config.app_name(), "Legacy");
        assert_eq!(config.bundle_id(), "com.example.legacy");
        assert_eq!(config.deployment_target(), "14.0");
    }

    #[test]
    fn rejects_unknown_architecture() {
        let mut config = new_config("Demo", "com.example.demo".to_string());
        config.build.architectures = vec!["ppc".to_string()];
        assert!(config.validate().is_err());
    }

    #[test]
    fn dotfile_manifest_takes_priority() {
        let root = std::env::temp_dir().join(format!("brisk-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("brisk.toml"), "").unwrap();
        fs::write(root.join(".brisk.toml"), "").unwrap();
        assert_eq!(manifest_path(&root), Some(root.join(".brisk.toml")));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn project_values_override_global_values() {
        let mut global: toml::Value = toml::from_str(
            r#"
[app]
deployment_target = "14.0"

[build]
architectures = ["arm64", "x86_64"]
"#,
        )
        .unwrap();
        let project: toml::Value = toml::from_str(
            r#"
[app]
deployment_target = "15.0"
"#,
        )
        .unwrap();
        merge_toml(&mut global, project);
        assert_eq!(global["app"]["deployment_target"].as_str(), Some("15.0"));
        assert_eq!(
            global["build"]["architectures"].as_array().unwrap().len(),
            2
        );
    }
}
