use crate::{BriskError, Result};
use console::style;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct BriskConfig {
    pub name: String,
    pub bundle_id: String,
    pub deployment_target: String,
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
        Ok(toml::from_str(&raw)?)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        fs::write(root.join("brisk.toml"), toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
