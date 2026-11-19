//! `DeployedRepository` adapter: the manifest stored in `deployed.json`.
use crate::store::{load_json, save_json};
use motogp_application::{DeployError, DeployedRepository};
use motogp_domain::{Deployed, DeployedFile};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct DeployedFileDto {
    name: String,
    source: PathBuf,
}

/// Serialization shape of `deployed.json` (kept out of the domain model).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct DeployedDto {
    mods_dir: PathBuf,
    files: Vec<DeployedFileDto>,
    pending: Vec<DeployedFileDto>,
}

impl From<DeployedFileDto> for DeployedFile {
    fn from(dto: DeployedFileDto) -> Self {
        DeployedFile::new(dto.name, dto.source)
    }
}

impl From<&DeployedFile> for DeployedFileDto {
    fn from(file: &DeployedFile) -> Self {
        DeployedFileDto {
            name: file.name.clone(),
            source: file.source.clone(),
        }
    }
}

impl From<DeployedDto> for Deployed {
    fn from(dto: DeployedDto) -> Self {
        Deployed {
            mods_dir: dto.mods_dir,
            files: dto.files.into_iter().map(Into::into).collect(),
            pending: dto.pending.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<&Deployed> for DeployedDto {
    fn from(deployed: &Deployed) -> Self {
        DeployedDto {
            mods_dir: deployed.mods_dir.clone(),
            files: deployed.files.iter().map(Into::into).collect(),
            pending: deployed.pending.iter().map(Into::into).collect(),
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
