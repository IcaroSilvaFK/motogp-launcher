use crate::{ArchiveReader, ContentHasher, LibraryError, LibraryFiles, ModRepository};
use motogp_domain::{
    base_name, display_name, extension_of, group_pak_files, is_mod_file, is_valid_mod_id,
    mod_id_from_hash, FileConflict, ModEntry, ModGroup,
};
use std::path::Path;

/// Use cases of the local mod library: list, import and remove.
pub struct ModService<R, F, A, H> {
    repository: R,
    files: F,
    archive: A,
    hasher: H,
}

impl<R, F, A, H> ModService<R, F, A, H>
where
    R: ModRepository,
    F: LibraryFiles,
    A: ArchiveReader,
    H: ContentHasher,
{
    pub fn new(repository: R, files: F, archive: A, hasher: H) -> Self {
        Self {
            repository,
            files,
            archive,
            hasher,
        }
    }

    /// File conflicts of a candidate load order against the library.
    pub fn list_conflicts(&self, order: &[String]) -> Result<Vec<FileConflict>, LibraryError> {
        crate::list_conflicts(&self.repository, order)
    }

    pub fn list(&self) -> Result<Vec<ModEntry>, LibraryError> {
        self.repository.list()
    }

    /// Imports a `.pak/.utoc/.ucas` (only its same-name siblings) or a `.zip`
    /// (every valid group). Re-importing the same content returns the existing entry.
    pub fn import(&self, source: &Path) -> Result<Vec<ModEntry>, LibraryError> {
        let file_name = source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let extension = extension_of(&file_name).unwrap_or_default();
        match extension.as_str() {
            "zip" | "pak" | "utoc" | "ucas" => {}
            _ => return Err(LibraryError::UnsupportedFormat { extension }),
        }

        let staging = self.files.create_staging()?;
        let result = self.stage_and_register(source, &file_name, &extension, &staging);
        // Best effort: a cleanup failure must not hide the import result.
        let _ = self.files.discard(&staging);
        result
    }

    /// Removes the mod folder and its index entry.
    pub fn remove(&self, id: &str) -> Result<(), LibraryError> {
        if !is_valid_mod_id(id) {
            return Err(LibraryError::InvalidId { id: id.to_string() });
        }
        if !self.repository.list()?.iter().any(|entry| entry.id == id) {
            return Err(LibraryError::NotFound { id: id.to_string() });
        }
        // Files first: if the folder cannot be deleted (for example a Windows
        // sharing violation) the entry stays and the removal can be retried.
        // Deleting a missing folder is not an error.
        self.files.remove_mod_dir(id)?;
        if !self.repository.remove(id)? {
            return Err(LibraryError::NotFound { id: id.to_string() });
        }
        Ok(())
    }

    fn stage_and_register(
        &self,
        source: &Path,
        file_name: &str,
        extension: &str,
        staging: &Path,
    ) -> Result<Vec<ModEntry>, LibraryError> {
        let groups = if extension == "zip" {
            let names = self.archive.extract_flat(source, staging)?;
            let groups = group_pak_files(&names)?;
            if groups.is_empty() {
                return Err(LibraryError::NoPakFound);
            }
            groups
        } else {
            self.stage_siblings(source, file_name, staging)?
        };

        self.publish_groups(&groups, staging)
    }

    /// Publishes every new group, then registers them all with one index save.
    /// On any failure the folders published by this import are removed and the
    /// index stays unchanged.
    fn publish_groups(
        &self,
        groups: &[ModGroup],
        staging: &Path,
    ) -> Result<Vec<ModEntry>, LibraryError> {
        let mut known = self.repository.list()?;
        let mut entries = Vec::new();
        let mut created: Vec<ModEntry> = Vec::new();
        for group in groups {
            match self.publish_group(group, staging, &known) {
                Ok((entry, is_new)) => {
                    if is_new {
                        known.push(entry.clone());
                        created.push(entry.clone());
                    }
                    entries.push(entry);
                }
                Err(error) => {
                    self.rollback(&created);
                    return Err(error);
                }
            }
        }
        if !created.is_empty() {
            if let Err(error) = self.repository.add_all(&created) {
                self.rollback(&created);
                return Err(error);
            }
        }
        Ok(entries)
    }

    /// Best effort: removes only the folders this import published.
    fn rollback(&self, created: &[ModEntry]) {
        for entry in created {
            let _ = self.files.remove_mod_dir(&entry.id);
        }
    }

    /// Copies the chosen file and its same-base-name siblings into `staging`.
    fn stage_siblings(
        &self,
        source: &Path,
        file_name: &str,
        staging: &Path,
    ) -> Result<Vec<ModGroup>, LibraryError> {
        let folder = source.parent().unwrap_or_else(|| Path::new(""));
        let wanted = base_name(file_name).to_lowercase();
        let siblings: Vec<String> = self
            .files
            .list_dir(folder)?
            .into_iter()
            .filter(|name| is_mod_file(name) && base_name(name).to_lowercase() == wanted)
            .collect();
        let groups = group_pak_files(&siblings)?;
        for group in &groups {
            for name in &group.files {
                self.files.copy_file(&folder.join(name), staging)?;
            }
        }
        Ok(groups)
    }

    /// Publishes one staged group unless the library already has its ID.
    /// Returns the entry and whether it is new (not yet indexed).
    fn publish_group(
        &self,
        group: &ModGroup,
        staging: &Path,
        known: &[ModEntry],
    ) -> Result<(ModEntry, bool), LibraryError> {
        let pak = group.pak_file().unwrap_or_default();
        let id = mod_id_from_hash(&self.hasher.sha256_hex(&staging.join(pak))?);
        if let Some(existing) = known.iter().find(|entry| entry.id == id) {
            return Ok((existing.clone(), false));
        }

        let mut size_bytes = 0;
        for name in &group.files {
            size_bytes += self.files.file_size(&staging.join(name))?;
        }
        let entry = ModEntry {
            id,
            name: display_name(&group.base_name),
            files: group.files.clone(),
            size_bytes,
        };
        self.files.publish(staging, &group.files, &entry.id)?;
        Ok((entry, true))
    }
}
