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
    pub entitlements: Option<PathBuf>,
    #[serde(default)]
    pub info: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SigningConfig {
    #[serde(default = "default_signing_identity")]
    pub identity: String,
    #[serde(default)]
    pub hardened_runtime: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestConfig {
    #[serde(default = "default_tests")]
    pub sources: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ArchiveConfig {
    #[serde(default)]
    pub path: Option<PathBuf>,
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
            entitlements: None,
            info: BTreeMap::new(),
        }
    }
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            identity: default_signing_identity(),
            hardened_runtime: false,
        }
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            sources: default_tests(),
        }
    }
}

impl BriskConfig {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("brisk.toml");
        let raw = fs::read_to_string(&path).map_err(|e| {
            BriskError::Message(format!(
                "could not read {}: {}\nrun {} first, or use brisk in a directory with an .xcodeproj/.xcworkspace",
                path.display(),
                e,
                style("brisk new <name>").cyan()
            ))
        })?;
        let mut config: Self = toml::from_str(&raw)?;
        config.normalize();
        Ok(config)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        fs::write(root.join("brisk.toml"), toml::to_string_pretty(self)?)?;
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
            entitlements: None,
            info: BTreeMap::new(),
        },
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
