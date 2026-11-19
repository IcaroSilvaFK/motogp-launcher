use crate::{DeployError, DeployFiles, DeployedRepository, DirCheck};
use motogp_domain::{deployed_name, is_safe_file_name, is_valid_mod_id, Deployed, DeployedFile};
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
/// Protocol (write-ahead journal): for each file the destination name and its
/// library source are first verified absent and persisted in
/// `Deployed::pending`, then created exclusively, then moved to
/// `Deployed::files`. A process death at any point leaves every
/// possibly-created file recorded, so [`clean`](Self::clean) can recover it
/// without scanning by name or prefix.
///
/// Safety properties of removal: a file is deleted only if (1) the manifest's
/// directory resolves to the independently authorized `~mods` and (2)
/// `DeployFiles::owns` proves that exact file is the launcher's (same file as
/// its library source, or a byte-identical copy). A journal entry alone never
/// grants ownership: if the proof fails the entry is untracked and the file is
/// left untouched.
pub struct DeployService<R, F> {
    repository: R,
    files: F,
}

impl<R: DeployedRepository, F: DeployFiles> DeployService<R, F> {
    pub fn new(repository: R, files: F) -> Self {
        Self { repository, files }
    }

    /// Cleans the previous session in `mods_dir` (aborting on failure), then
    /// deploys `mods` in order. `mods_dir` is the authorized `~mods` derived
    /// from the configured game install, never from the manifest. Returns the destination names. An empty profile touches nothing.
    pub fn deploy(&self, mods_dir: &Path, mods: &[DeployMod]) -> Result<Vec<String>, DeployError> {
        self.clean(mods_dir)?;
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
                let _ = self.clean(mods_dir);
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
            // The name is absent right before it is journaled (ownership is
            // still proven again at removal time, never assumed).
            if self.files.exists(mods_dir, name)? {
                return Err(DeployError::AlreadyExists { name: name.clone() });
            }
            manifest.pending.push(DeployedFile::new(name.clone(), source.clone()));
            self.repository.save(&manifest)?;
            match self.files.materialize(source, mods_dir, name) {
                Ok(()) => {
                    manifest.pending.retain(|f| f.name != *name);
                    manifest.files.push(DeployedFile::new(name.clone(), source.clone()));
                    self.repository.save(&manifest)?;
                }
                Err(error @ DeployError::AlreadyExists { .. }) => {
                    // Lost a race to a foreign file: it is not ours, untrack it.
                    // (If this save fails, `clean` re-verifies ownership.)
                    manifest.pending.retain(|f| f.name != *name);
                    self.repository.save(&manifest)?;
                    return Err(error);
                }
                // Anything else may have left a partial file: keep it journaled.
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Removes the tracked files that are provably the launcher's own and
    /// returns how many were actually removed. Tracked entries whose file is
    /// missing or no longer ours (replaced by the user) are untracked without
    /// touching the file. On a partial failure the entries that could not be
    /// processed stay tracked and an error is returned. `authorized_mods_dir`
    /// must come from the configured game install, not from the manifest.
    pub fn clean(&self, authorized_mods_dir: &Path) -> Result<usize, DeployError> {
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
        match self
            .files
            .check_dir(&manifest.mods_dir, authorized_mods_dir)?
        {
            DirCheck::Authorized => {}
            DirCheck::Missing => {
                self.repository.save(&Deployed::default())?;
                return Ok(0);
            }
            DirCheck::Foreign => {
                return Err(DeployError::UnauthorizedDirectory {
                    path: manifest.mods_dir.display().to_string(),
                })
            }
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
            for entry in tracked {
                match self.remove_if_owned(&manifest.mods_dir, entry) {
                    Ok(true) => removed += 1,
                    Ok(false) => {}
                    Err(error) => {
                        keep.push(entry.clone());
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

    /// `true` when the file was ours and got removed; `false` when it is missing
    /// or not provably ours (left untouched).
    fn remove_if_owned(&self, dir: &Path, entry: &DeployedFile) -> Result<bool, DeployError> {
        if self.files.owns(dir, &entry.name, &entry.source)? {
            self.files.remove(dir, &entry.name)
        } else {
            Ok(false)
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
