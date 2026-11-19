use crate::{DeployError, DeployFiles, DeployedRepository};
use motogp_domain::{deployed_name, is_safe_file_name, is_valid_mod_id, Deployed};
use std::path::{Path, PathBuf};

/// One already-ordered mod of the profile, ready to be deployed. The position
/// in the slice given to [`DeployService::deploy`] is its index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployMod {
    pub id: String,
    /// Library folder holding the files.
    pub source_dir: PathBuf,
    /// File names (never paths) inside `source_dir`.
    pub files: Vec<String>,
}

/// Use cases: deploy a session into `~mods` and clean it afterwards.
///
/// Protocol (write-ahead journal): for each file the destination name is first
/// verified absent and persisted in `Deployed::pending`, then created
/// exclusively, then moved to `Deployed::files`. A process death at any point
/// leaves every possibly-created file recorded, so [`clean`](Self::clean) can
/// recover it without scanning by name or prefix.
pub struct DeployService<R, F> {
    repository: R,
    files: F,
}

impl<R: DeployedRepository, F: DeployFiles> DeployService<R, F> {
    pub fn new(repository: R, files: F) -> Self {
        Self { repository, files }
    }

    /// Cleans the previous session (aborting on failure), then deploys `mods`
    /// in order. Returns the destination names. An empty profile touches nothing.
    pub fn deploy(&self, mods_dir: &Path, mods: &[DeployMod]) -> Result<Vec<String>, DeployError> {
        self.clean()?;
        let plan = plan(mods)?;
        if plan.is_empty() {
            return Ok(vec![]);
        }
        for (name, _) in &plan {
            if self.files.exists(mods_dir, name)? {
                return Err(DeployError::AlreadyExists { name: name.clone() });
            }
        }
        self.files.create_dir_all(mods_dir)?;
        match self.deploy_plan(mods_dir, &plan) {
            Ok(()) => Ok(plan.into_iter().map(|(name, _)| name).collect()),
            Err(error) => {
                // Best-effort rollback; whatever it cannot remove stays tracked.
                let _ = self.clean();
                Err(error)
            }
        }
    }

    fn deploy_plan(&self, mods_dir: &Path, plan: &[(String, PathBuf)]) -> Result<(), DeployError> {
        let mut manifest = Deployed {
            mods_dir: mods_dir.to_path_buf(),
            ..Deployed::default()
        };
        for (name, source) in plan {
            // Ownership proof: the name is absent right before it is journaled.
            if self.files.exists(mods_dir, name)? {
                return Err(DeployError::AlreadyExists { name: name.clone() });
            }
            manifest.pending.push(name.clone());
            self.repository.save(&manifest)?;
            match self.files.materialize(source, mods_dir, name) {
                Ok(()) => {
                    manifest.pending.retain(|n| n != name);
                    manifest.files.push(name.clone());
                    self.repository.save(&manifest)?;
                }
                Err(error @ DeployError::AlreadyExists { .. }) => {
                    // Lost a race to a foreign file: it is not ours, untrack it.
                    manifest.pending.retain(|n| n != name);
                    self.repository.save(&manifest)?;
                    return Err(error);
                }
                // Anything else may have left a partial file: keep it journaled.
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Removes exactly the tracked files (deployed and journaled), tolerating
    /// missing ones, and returns how many were actually removed. On a partial
    /// failure the files still present stay tracked and an error is returned.
    pub fn clean(&self) -> Result<usize, DeployError> {
        let manifest = self.repository.load()?;
        if manifest.is_empty() {
            return Ok(0);
        }
        // Validate everything first so a tampered manifest removes nothing.
        if manifest.mods_dir.as_os_str().is_empty() {
            return Err(DeployError::InvalidPath {
                path: String::new(),
            });
        }
        if let Some(bad) = manifest.tracked().find(|name| !is_safe_file_name(name)) {
            return Err(DeployError::InvalidPath { path: bad.into() });
        }

        let mut removed = 0;
        let mut first_error: Option<DeployError> = None;
        let mut remaining = Deployed {
            mods_dir: manifest.mods_dir.clone(),
            ..Deployed::default()
        };
        let groups = [
            (&manifest.files, &mut remaining.files),
            (&manifest.pending, &mut remaining.pending),
        ];
        for (tracked, keep) in groups {
            for name in tracked {
                match self.files.remove(&manifest.mods_dir, name) {
                    Ok(true) => removed += 1,
                    Ok(false) => {}
                    Err(error) => {
                        keep.push(name.clone());
                        first_error.get_or_insert(error);
                    }
                }
            }
        }

        match first_error {
            None => {
                self.repository.save(&Deployed::default())?;
                Ok(removed)
            }
            Some(error) => {
                self.repository.save(&remaining)?;
                Err(DeployError::CleanIncomplete {
                    removed,
                    remaining: remaining.files.len() + remaining.pending.len(),
                    detail: error.to_string(),
                })
            }
        }
    }
}

/// Validates the profile and computes `(destination name, source path)`.
fn plan(mods: &[DeployMod]) -> Result<Vec<(String, PathBuf)>, DeployError> {
    let mut plan = Vec::new();
    for (index, item) in mods.iter().enumerate() {
        if !is_valid_mod_id(&item.id) {
            return Err(DeployError::InvalidPath {
                path: item.id.clone(),
            });
        }
        for file in &item.files {
            if !is_safe_file_name(file) {
                return Err(DeployError::InvalidPath { path: file.clone() });
            }
            plan.push((
                deployed_name(index, &item.id, file),
                item.source_dir.join(file),
            ));
        }
    }
    Ok(plan)
}
