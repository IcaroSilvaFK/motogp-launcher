//! `DeployedRepository` adapter: the manifest stored in `deployed.json`.
use crate::store::{load_json, save_json};
use motogp_application::{DeployError, DeployedRepository};
use motogp_domain::Deployed;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Serialization shape of `deployed.json` (kept out of the domain model).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct DeployedDto {
    mods_dir: PathBuf,
    files: Vec<String>,
    pending: Vec<String>,
}

impl From<DeployedDto> for Deployed {
    fn from(dto: DeployedDto) -> Self {
        Deployed {
            mods_dir: dto.mods_dir,
            files: dto.files,
            pending: dto.pending,
        }
    }
}

impl From<&Deployed> for DeployedDto {
    fn from(deployed: &Deployed) -> Self {
        DeployedDto {
            mods_dir: deployed.mods_dir.clone(),
            files: deployed.files.clone(),
            pending: deployed.pending.clone(),
        }
    }
}

/// Manifest backed by a JSON file (atomic saves via the issue #2 store).
#[derive(Debug, Clone)]
pub struct JsonDeployedRepository {
    path: PathBuf,
}

impl JsonDeployedRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl DeployedRepository for JsonDeployedRepository {
    fn load(&self) -> Result<Deployed, DeployError> {
        Ok(load_json::<DeployedDto>(&self.path)?.into())
    }

    fn save(&self, deployed: &Deployed) -> Result<(), DeployError> {
        Ok(save_json(&self.path, &DeployedDto::from(deployed))?)
    }
}
