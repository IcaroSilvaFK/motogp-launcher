use crate::{DeployError, DeployFiles, DeploySession, DeployedRepository, DirCheck};
use motogp_domain::{
    deployed_name, is_mods_dir, is_safe_file_name, is_valid_mod_id, Deployed, DeployedFile,
};
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
            manifest
                .pending
                .push(DeployedFile::new(name.clone(), source.clone()));
            self.repository.save(&manifest)?;
            match self.files.materialize(source, mods_dir, name) {
                Ok(()) => {
                    manifest.pending.retain(|f| f.name != *name);
                    manifest
                        .files
                        .push(DeployedFile::new(name.clone(), source.clone()));
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

    /// `true` when the manifest tracks nothing (nothing to clean).
    pub fn is_clean(&self) -> Result<bool, DeployError> {
        Ok(self.repository.load()?.is_empty())
    }

    /// Removes the tracked files that are provably the launcher's own and
    /// returns how many were actually removed. Tracked entries whose file is
    /// missing or no longer ours (replaced by the user) are untracked without
    /// touching the file. On a partial failure the entries that could not be
    /// processed stay tracked and an error is returned. `authorized_mods_dir`
    /// must come from the configured game install, not from the manifest.
    pub fn clean(&self, authorized_mods_dir: &Path) -> Result<usize, DeployError> {
        let manifest = self.load_validated()?;
        if manifest.is_empty() {
            return Ok(0);
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
        self.remove_tracked(&manifest)
    }

    /// Read-only: how the manifest relates to `authorized_mods_dir` (derived
    /// from the configured game install). Never saves, removes or creates.
    pub fn session(&self, authorized_mods_dir: &Path) -> Result<DeploySession, DeployError> {
        let manifest = self.repository.load()?;
        if manifest.is_empty() {
            return Ok(DeploySession::None);
        }
        let tracked = manifest.files.len() + manifest.pending.len();
        Ok(
            match self
                .files
                .check_dir(&manifest.mods_dir, authorized_mods_dir)?
            {
                DirCheck::Authorized => DeploySession::Current { tracked },
                DirCheck::Foreign => DeploySession::Stale {
                    mods_dir: manifest.mods_dir,
                    tracked,
                },
                DirCheck::Missing => DeploySession::Vanished {
                    mods_dir: manifest.mods_dir,
                },
            },
        )
    }

    /// Recovery: drops a manifest of a previous game install without touching
    /// any file (no ownership proof is needed because nothing is removed).
    /// Refuses the current session (`SessionNotStale`): forgetting it would
    /// orphan files in the live `~mods`; use [`clean`](Self::clean) instead.
    pub fn forget_stale_session(&self, authorized_mods_dir: &Path) -> Result<(), DeployError> {
        let manifest = self.repository.load()?;
        if manifest.is_empty() {
            return Ok(());
        }
        match self
            .files
            .check_dir(&manifest.mods_dir, authorized_mods_dir)?
        {
            DirCheck::Authorized => Err(DeployError::SessionNotStale {
                path: manifest.mods_dir.display().to_string(),
            }),
            DirCheck::Foreign | DirCheck::Missing => {
                self.repository.save(&Deployed::default())?;
                Ok(())
            }
        }
    }

    /// Recovery: cleans the folder of a previous game install after the user
    /// explicitly confirmed it. `confirmed` must resolve to exactly the folder
    /// recorded in the manifest and that folder must be named `~mods`,
    /// otherwise `UnauthorizedDirectory` and nothing is removed. Removal is the
    /// same as [`clean`](Self::clean): only files `DeployFiles::owns` proves
    /// are ours. A manifest of the current install is handled by `clean`.
    pub fn clean_stale_session(
        &self,
        authorized_mods_dir: &Path,
        confirmed: &Path,
    ) -> Result<usize, DeployError> {
        let manifest = self.load_validated()?;
        if manifest.is_empty() {
            return Ok(0);
        }
        match self
            .files
            .check_dir(&manifest.mods_dir, authorized_mods_dir)?
        {
            DirCheck::Authorized => return self.remove_tracked(&manifest),
            DirCheck::Missing => {
                self.repository.save(&Deployed::default())?;
                return Ok(0);
            }
            DirCheck::Foreign => {}
        }
        let unauthorized = || DeployError::UnauthorizedDirectory {
            path: manifest.mods_dir.display().to_string(),
        };
        if self.files.check_dir(&manifest.mods_dir, confirmed)? != DirCheck::Authorized
            || !is_mods_dir(&manifest.mods_dir)
        {
            return Err(unauthorized());
        }
        self.remove_tracked(&manifest)
    }

    /// Loads the manifest and validates it before anything is removed, so a
    /// tampered manifest removes nothing. An empty manifest is returned as is.
    fn load_validated(&self) -> Result<Deployed, DeployError> {
        let manifest = self.repository.load()?;
        if manifest.is_empty() {
            return Ok(manifest);
        }
        if manifest.mods_dir.as_os_str().is_empty() {
            return Err(DeployError::InvalidPath {
                path: String::new(),
            });
        }
        if let Some(bad) = manifest.tracked().find(|name| !is_safe_file_name(name)) {
            return Err(DeployError::InvalidPath { path: bad.into() });
        }
        Ok(manifest)
    }

    /// The single removal loop: the manifest's directory has already been
    /// authorized by the caller. Entries that fail stay tracked.
    fn remove_tracked(&self, manifest: &Deployed) -> Result<usize, DeployError> {
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
