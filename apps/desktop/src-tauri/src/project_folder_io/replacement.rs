use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use ori_formats::{
    MAX_EDITOR_HISTORY_JSON_BYTES, MAX_PROJECT_FOLDER_MANIFEST_BYTES,
    MAX_PROJECT_FOLDER_PREVIEW_BYTES, MAX_PROJECT_FOLDER_TOTAL_BYTES, MAX_PROJECT_JSON_BYTES,
    PROJECT_FOLDER_EDITOR_HISTORY_PATH, PROJECT_FOLDER_MANIFEST_PATH, PROJECT_FOLDER_PREVIEW_PATH,
    PROJECT_FOLDER_PROJECT_PATH, ProjectFolderArtifactV1, ProjectFolderLimits,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    DirectoryIdentity, FsResult, PinnedDirectory, PinnedFile, ProjectFolderFilesystemError,
    load_project_folder_artifact_from_pinned, populate_and_verify_staging, validate_names,
    validate_native_child_name,
};

const REGISTRY_RESERVED_NAME: &str = "reserved-v1.json";
const REGISTRY_RESERVED_TEMP_NAME: &str = "reserved-v1.tmp";
const REGISTRY_STAGED_NAME: &str = "staged-v1.json";
const REGISTRY_STAGED_TEMP_NAME: &str = "staged-v1.tmp";
#[cfg(test)]
const REGISTRY_RECORD_NAME: &str = REGISTRY_STAGED_NAME;
const REGISTRY_VERSION: u32 = 1;
const JOURNAL_VERSION: u32 = 1;
const MAX_REGISTRY_BYTES: u64 = 256 * 1024;
const MAX_JOURNAL_BYTES: u64 = 64 * 1024;
const MAX_REGISTRY_ENTRIES: usize = 4;
const MAX_RECOVERY_PARENT_ENTRIES: usize = 4_096;
const PHASE_COUNT: usize = 4;
const RECOVERY_NAMESPACE_PREFIX: &str = ".origami2-folder-";

#[derive(Clone)]
pub(super) struct ReplacementRegistry {
    root_path: Arc<PathBuf>,
}

impl ReplacementRegistry {
    pub(super) fn new(root_path: PathBuf) -> Self {
        Self {
            root_path: Arc::new(root_path),
        }
    }

    pub(super) fn recover_pending(&self) -> FsResult<()> {
        let Some(mut loaded) = self.load()? else {
            return Ok(());
        };
        let parent_path = decode_parent_path(&loaded.record)?;
        let parent = PinnedDirectory::open_selected(&parent_path)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        if parent.identity() != loaded.record.parent_identity {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        match loaded.record.state {
            RegistryPhase::Reserved => recover_reserved_transaction(&parent, &loaded.record)?,
            RegistryPhase::Staged => recover_registered_transaction(&parent, &loaded.record)?,
        }
        loaded.clear()
    }

    fn ensure_empty(&self) -> FsResult<()> {
        if self.load()?.is_some() {
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        } else {
            Ok(())
        }
    }

    fn reserve(&self, input: RegistryReservationInput<'_>) -> FsResult<RegistryReservation> {
        let root = self.open_or_create_root()?;
        let names = validate_registry_names(root.list_names(MAX_REGISTRY_ENTRIES + 1)?)?;
        if !names.is_empty() {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        let (path_encoding, parent_path_hex) = encode_parent_path(input.parent_path)?;
        let record = RegistryRecordV1 {
            version: REGISTRY_VERSION,
            state: RegistryPhase::Reserved,
            path_encoding,
            parent_path_hex,
            parent_identity: input.parent.identity(),
            target_name: input.target_name.to_owned(),
            transaction_id: input.transaction_id.to_owned(),
            old_manifest_sha256: input.old_manifest_sha256.to_owned(),
            new_manifest_sha256: input.new_manifest_sha256.to_owned(),
            old_entries: input.old_entries.clone(),
            new_entries: input.new_entries.clone(),
            old_directory_identity: input.old_directory_identity,
            stage_directory_identity: None,
        };
        let file = write_registry_record(&root, &record)?;
        Ok(RegistryReservation {
            root,
            file,
            record,
            cleared: false,
        })
    }

    fn load(&self) -> FsResult<Option<LoadedRegistry>> {
        let root = self.open_or_create_root()?;
        let names = validate_registry_names(root.list_names(MAX_REGISTRY_ENTRIES + 1)?)?;
        if names.is_empty() {
            return Ok(None);
        }
        let reserved = load_registry_record(&root, RegistryPhase::Reserved)?;
        let staged = load_registry_record(&root, RegistryPhase::Staged)?;
        if reserved.is_none() && staged.is_none() {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        if let (Some(reserved), Some(staged)) = (&reserved, &staged)
            && !registry_records_match(&reserved.record, &staged.record)
        {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        let record = staged
            .as_ref()
            .or(reserved.as_ref())
            .ok_or(ProjectFolderFilesystemError::RecoveryRequired)?
            .record
            .clone();
        let mut files = Vec::with_capacity(2);
        if let Some(reserved) = reserved {
            files.push(reserved);
        }
        if let Some(staged) = staged {
            files.push(staged);
        }
        Ok(Some(LoadedRegistry {
            root,
            files,
            record,
        }))
    }

    fn open_or_create_root(&self) -> FsResult<PinnedDirectory> {
        match create_private_registry_directory(&self.root_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ProjectFolderFilesystemError::RecoveryRequired),
        }
        validate_private_registry_directory(&self.root_path)?;
        PinnedDirectory::open_selected(&self.root_path)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)
    }
}

struct RegistryReservationInput<'a> {
    parent_path: &'a Path,
    parent: &'a PinnedDirectory,
    target_name: &'a str,
    transaction_id: &'a str,
    old_manifest_sha256: &'a str,
    new_manifest_sha256: &'a str,
    old_entries: &'a ArtifactFingerprintsV1,
    new_entries: &'a ArtifactFingerprintsV1,
    old_directory_identity: DirectoryIdentity,
}

struct RegistryReservation {
    root: PinnedDirectory,
    file: LoadedRegistryFile,
    record: RegistryRecordV1,
    cleared: bool,
}

impl RegistryReservation {
    fn record_stage(&mut self, identity: DirectoryIdentity) -> FsResult<LoadedRegistryFile> {
        let mut staged = self.record.clone();
        staged.state = RegistryPhase::Staged;
        staged.stage_directory_identity = Some(identity);
        write_registry_record(&self.root, &staged)
    }

    fn into_lease(self, staged: LoadedRegistryFile) -> RegistryLease {
        RegistryLease {
            root: self.root,
            files: vec![self.file, staged],
            cleared: false,
        }
    }

    fn clear(&mut self) -> FsResult<()> {
        if self.cleared {
            return Ok(());
        }
        self.root
            .remove_child_file_if_same(&self.file.name, &self.file.file)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        self.root
            .sync_directory()
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        self.cleared = true;
        Ok(())
    }
}

struct RegistryLease {
    root: PinnedDirectory,
    files: Vec<LoadedRegistryFile>,
    cleared: bool,
}

impl RegistryLease {
    fn clear(&mut self) -> FsResult<()> {
        if self.cleared {
            return Ok(());
        }
        remove_registry_files(&self.root, &mut self.files)?;
        self.cleared = true;
        Ok(())
    }
}

struct LoadedRegistry {
    root: PinnedDirectory,
    files: Vec<LoadedRegistryFile>,
    record: RegistryRecordV1,
}

impl LoadedRegistry {
    fn clear(&mut self) -> FsResult<()> {
        remove_registry_files(&self.root, &mut self.files)
    }
}

struct LoadedRegistryFile {
    state: RegistryPhase,
    name: String,
    file: PinnedFile,
    record: RegistryRecordV1,
}

fn write_registry_record(
    root: &PinnedDirectory,
    record: &RegistryRecordV1,
) -> FsResult<LoadedRegistryFile> {
    validate_registry_record(record)?;
    let (temp_name, final_name) = registry_file_names(record.state);
    if root.child_exists(temp_name)? || root.child_exists(final_name)? {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let bytes = serialize_json(record, MAX_REGISTRY_BYTES)?;
    let file = root.write_child_file_pinned(temp_name, &bytes)?;
    root.sync_directory()?;
    root.publish_child_file_no_replace(temp_name, &file, final_name)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    root.sync_directory()?;
    Ok(LoadedRegistryFile {
        state: record.state,
        name: final_name.to_owned(),
        file,
        record: record.clone(),
    })
}

fn load_registry_record(
    root: &PinnedDirectory,
    state: RegistryPhase,
) -> FsResult<Option<LoadedRegistryFile>> {
    let (temp_name, final_name) = registry_file_names(state);
    let temp_exists = root.child_exists(temp_name)?;
    let final_exists = root.child_exists(final_name)?;
    if temp_exists && final_exists {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let Some(name) = final_exists
        .then_some(final_name)
        .or_else(|| temp_exists.then_some(temp_name))
    else {
        return Ok(None);
    };
    let mut file = root
        .open_child_file_for_update(name, MAX_REGISTRY_BYTES)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    let bytes = file
        .read_bounded_and_revalidate(root, name, MAX_REGISTRY_BYTES)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    let record: RegistryRecordV1 = serde_json::from_slice(&bytes)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    validate_registry_record(&record)?;
    if record.state != state {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    if temp_exists {
        root.publish_child_file_no_replace(temp_name, &file, final_name)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        root.sync_directory()?;
    }
    Ok(Some(LoadedRegistryFile {
        state,
        name: final_name.to_owned(),
        file,
        record,
    }))
}

fn remove_registry_files(
    root: &PinnedDirectory,
    files: &mut Vec<LoadedRegistryFile>,
) -> FsResult<()> {
    files.sort_by_key(|file| file.state);
    for file in files.iter() {
        root.remove_child_file_if_same(&file.name, &file.file)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        root.sync_directory()?;
    }
    files.clear();
    Ok(())
}

fn registry_records_match(reserved: &RegistryRecordV1, staged: &RegistryRecordV1) -> bool {
    if reserved.state != RegistryPhase::Reserved || staged.state != RegistryPhase::Staged {
        return false;
    }
    let mut normalized = staged.clone();
    normalized.state = RegistryPhase::Reserved;
    normalized.stage_directory_identity = None;
    &normalized == reserved
}

const fn registry_file_names(state: RegistryPhase) -> (&'static str, &'static str) {
    match state {
        RegistryPhase::Reserved => (REGISTRY_RESERVED_TEMP_NAME, REGISTRY_RESERVED_NAME),
        RegistryPhase::Staged => (REGISTRY_STAGED_TEMP_NAME, REGISTRY_STAGED_NAME),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryRecordV1 {
    version: u32,
    state: RegistryPhase,
    path_encoding: ParentPathEncoding,
    parent_path_hex: String,
    parent_identity: DirectoryIdentity,
    target_name: String,
    transaction_id: String,
    old_manifest_sha256: String,
    new_manifest_sha256: String,
    old_entries: ArtifactFingerprintsV1,
    new_entries: ArtifactFingerprintsV1,
    old_directory_identity: DirectoryIdentity,
    stage_directory_identity: Option<DirectoryIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RegistryPhase {
    Reserved,
    Staged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ParentPathEncoding {
    WindowsUtf16Le,
    UnixBytes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalRecordV1 {
    version: u32,
    transaction_id: String,
    target_name: String,
    old_manifest_sha256: String,
    new_manifest_sha256: String,
    old_entries: ArtifactFingerprintsV1,
    new_entries: ArtifactFingerprintsV1,
    old_directory_identity: DirectoryIdentity,
    stage_directory_identity: DirectoryIdentity,
    state: ReplacementPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReplacementPhase {
    Prepared,
    OldMoved,
    NewPublished,
    CleanupComplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactFingerprintsV1 {
    entries: Vec<EntryFingerprintV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryFingerprintV1 {
    path: String,
    size: u64,
    sha256: String,
}

impl ReplacementPhase {
    const ALL: [Self; PHASE_COUNT] = [
        Self::Prepared,
        Self::OldMoved,
        Self::NewPublished,
        Self::CleanupComplete,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Prepared => 0,
            Self::OldMoved => 1,
            Self::NewPublished => 2,
            Self::CleanupComplete => 3,
        }
    }

    const fn file_component(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::OldMoved => "old-moved",
            Self::NewPublished => "new-published",
            Self::CleanupComplete => "cleanup-complete",
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReplacementCrashPoint {
    BeforePrepared,
    AfterPrepared,
    AfterOldRename,
    AfterOldMoved,
    AfterNewRename,
    AfterNewPublished,
    AfterBackupCleanup,
    AfterCleanupComplete,
}

pub(super) struct PreparedReplacementProjectFolder {
    parent: PinnedDirectory,
    staging: PinnedDirectory,
    old_target: PinnedDirectory,
    staging_name: String,
    backup_name: String,
    target_name: String,
    expected_new: ProjectFolderArtifactV1,
    expected_old: ProjectFolderArtifactV1,
    journal: JournalRecordV1,
    registry: RegistryLease,
    phase_records: Vec<LoadedPhase>,
    recovery_owned: bool,
    old_moved: bool,
    published_new: bool,
    finished: bool,
}

impl PreparedReplacementProjectFolder {
    pub(super) fn prepare(
        registry: &ReplacementRegistry,
        parent_path: &Path,
        target_name: &str,
        expected_new: ProjectFolderArtifactV1,
    ) -> FsResult<Self> {
        Self::prepare_with_hook(registry, parent_path, target_name, expected_new, |_| {})
    }

    fn prepare_with_hook<F>(
        registry: &ReplacementRegistry,
        parent_path: &Path,
        target_name: &str,
        expected_new: ProjectFolderArtifactV1,
        after_registry: F,
    ) -> FsResult<Self>
    where
        F: FnOnce(&PinnedDirectory),
    {
        Self::prepare_with_hooks(
            registry,
            parent_path,
            target_name,
            expected_new,
            |parent| parent.ensure_stable_replacement_identity(),
            |_, _| Ok(()),
            after_registry,
        )
    }

    #[cfg(test)]
    fn prepare_with_stage_registration_hook<F>(
        registry: &ReplacementRegistry,
        parent_path: &Path,
        target_name: &str,
        expected_new: ProjectFolderArtifactV1,
        before_stage_registration: F,
    ) -> FsResult<Self>
    where
        F: FnOnce(&PinnedDirectory, &PinnedDirectory) -> FsResult<()>,
    {
        Self::prepare_with_hooks(
            registry,
            parent_path,
            target_name,
            expected_new,
            |parent| parent.ensure_stable_replacement_identity(),
            before_stage_registration,
            |_| {},
        )
    }

    #[cfg(test)]
    fn prepare_with_replacement_admission_hook<H>(
        registry: &ReplacementRegistry,
        parent_path: &Path,
        target_name: &str,
        expected_new: ProjectFolderArtifactV1,
        replacement_admission: H,
    ) -> FsResult<Self>
    where
        H: FnOnce(&PinnedDirectory) -> FsResult<()>,
    {
        Self::prepare_with_hooks(
            registry,
            parent_path,
            target_name,
            expected_new,
            replacement_admission,
            |_, _| Ok(()),
            |_| {},
        )
    }

    fn prepare_with_hooks<H, F, G>(
        registry: &ReplacementRegistry,
        parent_path: &Path,
        target_name: &str,
        expected_new: ProjectFolderArtifactV1,
        replacement_admission: H,
        before_stage_registration: F,
        after_registry: G,
    ) -> FsResult<Self>
    where
        H: FnOnce(&PinnedDirectory) -> FsResult<()>,
        F: FnOnce(&PinnedDirectory, &PinnedDirectory) -> FsResult<()>,
        G: FnOnce(&PinnedDirectory),
    {
        validate_native_child_name(target_name)?;
        let parent = PinnedDirectory::open_selected(parent_path)?;
        replacement_admission(&parent)?;
        let old_target = parent
            .open_child_directory_for_rename(target_name)
            .map_err(|error| match error {
                ProjectFolderFilesystemError::OpenFailed => {
                    ProjectFolderFilesystemError::TargetExists
                }
                other => other,
            })?;
        let expected_old =
            load_project_folder_artifact_from_pinned(&old_target, ProjectFolderLimits::default())?;
        if expected_old.archive().document.project_id != expected_new.archive().document.project_id
        {
            return Err(ProjectFolderFilesystemError::TargetExists);
        }

        let transaction_id =
            transaction_id(expected_new.archive().document.project_id.canonical_bytes());
        let staging_name = staging_name(&transaction_id);
        let backup_name = backup_name(&transaction_id);
        registry.ensure_empty()?;
        ensure_transaction_namespace_clear(&parent, &transaction_id, target_name)?;
        let old_manifest_sha256 = artifact_manifest_sha256(&expected_old)?;
        let new_manifest_sha256 = artifact_manifest_sha256(&expected_new)?;
        let old_entries = artifact_fingerprints(&expected_old)?;
        let new_entries = artifact_fingerprints(&expected_new)?;
        let mut reservation = registry.reserve(RegistryReservationInput {
            parent_path,
            parent: &parent,
            target_name,
            transaction_id: &transaction_id,
            old_manifest_sha256: &old_manifest_sha256,
            new_manifest_sha256: &new_manifest_sha256,
            old_entries: &old_entries,
            new_entries: &new_entries,
            old_directory_identity: old_target.identity(),
        })?;
        let staging = match parent.create_child_directory(&staging_name, true) {
            Ok(staging) => staging,
            Err(error) => {
                let _ = reservation.clear();
                return Err(error);
            }
        };
        let staged_registry = match before_stage_registration(&staging, &reservation.root)
            .and_then(|()| reservation.record_stage(staging.identity()))
        {
            Ok(staged) => staged,
            Err(_) => {
                // `reserved-v1` was durable before stage creation. It remains
                // the locator whether exact stage cleanup succeeds or fails.
                let _ = cleanup_transaction_directory(
                    &parent,
                    &staging_name,
                    &staging,
                    staging.identity(),
                    &new_entries,
                    true,
                );
                return Err(ProjectFolderFilesystemError::RecoveryRequired);
            }
        };
        let registry_lease = reservation.into_lease(staged_registry);

        after_registry(&staging);
        if let Err(error) = populate_and_verify_staging(&staging, &expected_new) {
            let cleaned = cleanup_transaction_directory(
                &parent,
                &staging_name,
                &staging,
                staging.identity(),
                &new_entries,
                true,
            )
            .is_ok();
            let mut registry_lease = registry_lease;
            if cleaned {
                let _ = registry_lease.clear();
                return Err(error);
            }
            // The registry is the only durable locator for this exact stage.
            // Retain it whenever cleanup cannot prove that the stage vanished.
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }

        let journal = JournalRecordV1 {
            version: JOURNAL_VERSION,
            transaction_id,
            target_name: target_name.to_owned(),
            old_manifest_sha256,
            new_manifest_sha256,
            old_entries,
            new_entries,
            old_directory_identity: old_target.identity(),
            stage_directory_identity: staging.identity(),
            state: ReplacementPhase::Prepared,
        };
        Ok(Self {
            parent,
            staging,
            old_target,
            staging_name,
            backup_name,
            target_name: target_name.to_owned(),
            expected_new,
            expected_old,
            journal,
            registry: registry_lease,
            phase_records: Vec::with_capacity(PHASE_COUNT),
            recovery_owned: false,
            old_moved: false,
            published_new: false,
            finished: false,
        })
    }

    pub(super) fn publish(&mut self) -> FsResult<()> {
        let result = self.publish_inner(None);
        if result.is_err() && self.published_new {
            // The new authenticated tree is already the visible target.
            // Keep the journal/registry for startup cleanup, but do not invite
            // a duplicate save by reporting an ordinary write failure.
            self.finished = true;
            return Ok(());
        }
        if result.is_err() && self.old_moved {
            if matches!(self.parent.child_exists(&self.target_name), Ok(false))
                && self
                    .parent
                    .publish_child_directory_no_replace(
                        &self.backup_name,
                        &mut self.old_target,
                        &self.target_name,
                    )
                    .is_ok()
            {
                let _ = self.parent.sync_directory();
                self.old_moved = false;
            }
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        result
    }

    #[cfg(test)]
    pub(super) fn publish_until(&mut self, crash_point: ReplacementCrashPoint) -> FsResult<()> {
        self.publish_inner(Some(crash_point))
    }

    fn publish_inner(
        &mut self,
        #[cfg(test)] crash_point: Option<ReplacementCrashPoint>,
        #[cfg(not(test))] _crash_point: Option<()>,
    ) -> FsResult<()> {
        self.parent.revalidate_selected_path()?;
        assert_artifact(
            &self.old_target,
            &self.expected_old,
            &self.journal.old_manifest_sha256,
            self.journal.old_directory_identity,
        )?;
        assert_artifact(
            &self.staging,
            &self.expected_new,
            &self.journal.new_manifest_sha256,
            self.journal.stage_directory_identity,
        )?;

        #[cfg(test)]
        if crash_point == Some(ReplacementCrashPoint::BeforePrepared) {
            self.recovery_owned = true;
            return Err(ProjectFolderFilesystemError::InjectedCrash);
        }

        // From the first phase-file write attempt onward, even an error can
        // leave a durable temp record. Recovery, not Drop, owns all cleanup.
        self.recovery_owned = true;
        self.phase_records.push(write_phase_record(
            &self.parent,
            &self.journal,
            ReplacementPhase::Prepared,
        )?);
        crash_if_requested(
            #[cfg(test)]
            crash_point,
            #[cfg(test)]
            ReplacementCrashPoint::AfterPrepared,
        )?;

        self.parent.publish_child_directory_no_replace(
            &self.target_name,
            &mut self.old_target,
            &self.backup_name,
        )?;
        self.old_moved = true;
        crash_if_requested(
            #[cfg(test)]
            crash_point,
            #[cfg(test)]
            ReplacementCrashPoint::AfterOldRename,
        )?;
        self.parent.sync_directory()?;

        self.phase_records.push(write_phase_record(
            &self.parent,
            &self.journal,
            ReplacementPhase::OldMoved,
        )?);
        crash_if_requested(
            #[cfg(test)]
            crash_point,
            #[cfg(test)]
            ReplacementCrashPoint::AfterOldMoved,
        )?;

        self.parent.publish_child_directory_no_replace(
            &self.staging_name,
            &mut self.staging,
            &self.target_name,
        )?;
        self.old_moved = false;
        self.published_new = true;
        crash_if_requested(
            #[cfg(test)]
            crash_point,
            #[cfg(test)]
            ReplacementCrashPoint::AfterNewRename,
        )?;
        self.parent.sync_directory()?;

        self.phase_records.push(write_phase_record(
            &self.parent,
            &self.journal,
            ReplacementPhase::NewPublished,
        )?);
        crash_if_requested(
            #[cfg(test)]
            crash_point,
            #[cfg(test)]
            ReplacementCrashPoint::AfterNewPublished,
        )?;

        cleanup_transaction_directory(
            &self.parent,
            &self.backup_name,
            &self.old_target,
            self.journal.old_directory_identity,
            &self.journal.old_entries,
            false,
        )?;
        self.parent.sync_directory()?;
        crash_if_requested(
            #[cfg(test)]
            crash_point,
            #[cfg(test)]
            ReplacementCrashPoint::AfterBackupCleanup,
        )?;

        self.phase_records.push(write_phase_record(
            &self.parent,
            &self.journal,
            ReplacementPhase::CleanupComplete,
        )?);
        crash_if_requested(
            #[cfg(test)]
            crash_point,
            #[cfg(test)]
            ReplacementCrashPoint::AfterCleanupComplete,
        )?;

        remove_phase_records(&self.parent, &mut self.phase_records)?;
        self.registry.clear()?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for PreparedReplacementProjectFolder {
    fn drop(&mut self) {
        if self.finished || self.recovery_owned {
            return;
        }
        let cleaned = cleanup_transaction_directory(
            &self.parent,
            &self.staging_name,
            &self.staging,
            self.journal.stage_directory_identity,
            &self.journal.new_entries,
            true,
        )
        .is_ok();
        if cleaned {
            let _ = self.registry.clear();
        }
    }
}

struct LoadedPhase {
    state: ReplacementPhase,
    name: String,
    file: PinnedFile,
}

fn recover_reserved_transaction(
    parent: &PinnedDirectory,
    registry: &RegistryRecordV1,
) -> FsResult<()> {
    validate_recovery_namespace(parent, registry)?;
    if parent.child_exists(&backup_name(&registry.transaction_id))? {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    for state in ReplacementPhase::ALL {
        if parent.child_exists(&journal_name(&registry.transaction_id, state))?
            || parent.child_exists(&journal_temp_name(&registry.transaction_id, state))?
        {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
    }
    let target = parent
        .open_child_directory_for_rename(&registry.target_name)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    if !is_expected_directory(
        &target,
        &registry.old_manifest_sha256,
        registry.old_directory_identity,
    )? {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    if parent.child_exists(&staging_name(&registry.transaction_id))? {
        // The process may have died between stage creation and publication
        // of its object ID. Keep the durable reservation rather than deleting
        // an object whose identity cannot be authenticated.
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

fn recover_registered_transaction(
    parent: &PinnedDirectory,
    registry: &RegistryRecordV1,
) -> FsResult<()> {
    validate_recovery_namespace(parent, registry)?;
    let stage_directory_identity = registered_stage_identity(registry)?;
    let expected = JournalRecordV1 {
        version: JOURNAL_VERSION,
        transaction_id: registry.transaction_id.clone(),
        target_name: registry.target_name.clone(),
        old_manifest_sha256: registry.old_manifest_sha256.clone(),
        new_manifest_sha256: registry.new_manifest_sha256.clone(),
        old_entries: registry.old_entries.clone(),
        new_entries: registry.new_entries.clone(),
        old_directory_identity: registry.old_directory_identity,
        stage_directory_identity,
        state: ReplacementPhase::Prepared,
    };
    let mut phases = load_phase_records(parent, &expected)?;
    let highest = phases.iter().map(|phase| phase.state).max();
    validate_phase_continuity(&phases, highest)?;

    let stage_name = staging_name(&registry.transaction_id);
    let backup_name = backup_name(&registry.transaction_id);
    let target = open_optional_directory(parent, &registry.target_name)?;
    let stage = open_optional_directory(parent, &stage_name)?;
    let backup = open_optional_directory(parent, &backup_name)?;

    match highest {
        None => recover_without_phase(parent, registry, target, stage, backup)?,
        Some(ReplacementPhase::Prepared) => recover_prepared(
            parent,
            registry,
            &expected,
            target,
            stage,
            backup,
            &mut phases,
        )?,
        Some(ReplacementPhase::OldMoved) => recover_old_moved(
            parent,
            registry,
            &expected,
            target,
            stage,
            backup,
            &mut phases,
        )?,
        Some(ReplacementPhase::NewPublished) => recover_new_published(
            parent,
            registry,
            &expected,
            target,
            stage,
            backup,
            &mut phases,
        )?,
        Some(ReplacementPhase::CleanupComplete) => {
            assert_new_target_only(registry, target, stage, backup)?;
            remove_phase_records(parent, &mut phases)?;
        }
    }
    Ok(())
}

fn recover_without_phase(
    parent: &PinnedDirectory,
    registry: &RegistryRecordV1,
    target: Option<PinnedDirectory>,
    stage: Option<PinnedDirectory>,
    backup: Option<PinnedDirectory>,
) -> FsResult<()> {
    if backup.is_some() {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let Some(target) = target else {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    };
    if is_expected_directory(
        &target,
        &registry.old_manifest_sha256,
        registry.old_directory_identity,
    )? {
        if let Some(stage) = stage {
            if stage.identity() != registered_stage_identity(registry)? {
                return Err(ProjectFolderFilesystemError::RecoveryRequired);
            }
            cleanup_transaction_directory(
                parent,
                &staging_name(&registry.transaction_id),
                &stage,
                registered_stage_identity(registry)?,
                &registry.new_entries,
                true,
            )?;
            parent.sync_directory()?;
        }
        return Ok(());
    }
    if stage.is_none()
        && is_expected_directory(
            &target,
            &registry.new_manifest_sha256,
            registered_stage_identity(registry)?,
        )?
    {
        return Ok(());
    }
    Err(ProjectFolderFilesystemError::RecoveryRequired)
}

fn recover_prepared(
    parent: &PinnedDirectory,
    registry: &RegistryRecordV1,
    expected: &JournalRecordV1,
    target: Option<PinnedDirectory>,
    stage: Option<PinnedDirectory>,
    backup: Option<PinnedDirectory>,
    phases: &mut Vec<LoadedPhase>,
) -> FsResult<()> {
    match (target, stage, backup) {
        (Some(target), Some(stage), None)
            if is_expected_directory(
                &target,
                &registry.old_manifest_sha256,
                registry.old_directory_identity,
            )? && stage.identity() == registered_stage_identity(registry)? =>
        {
            cleanup_transaction_directory(
                parent,
                &staging_name(&registry.transaction_id),
                &stage,
                registered_stage_identity(registry)?,
                &registry.new_entries,
                true,
            )?;
            remove_phase_records(parent, phases)?;
            parent.sync_directory()
        }
        (Some(target), None, None)
            if is_expected_directory(
                &target,
                &registry.old_manifest_sha256,
                registry.old_directory_identity,
            )? =>
        {
            remove_phase_records(parent, phases)?;
            parent.sync_directory()
        }
        (None, Some(stage), Some(backup))
            if is_expected_directory(
                &stage,
                &registry.new_manifest_sha256,
                registered_stage_identity(registry)?,
            )? && is_expected_directory(
                &backup,
                &registry.old_manifest_sha256,
                registry.old_directory_identity,
            )? =>
        {
            phases.push(write_phase_record(
                parent,
                expected,
                ReplacementPhase::OldMoved,
            )?);
            publish_recovered_new(parent, registry, expected, stage, backup, phases)
        }
        _ => Err(ProjectFolderFilesystemError::RecoveryRequired),
    }
}

fn recover_old_moved(
    parent: &PinnedDirectory,
    registry: &RegistryRecordV1,
    expected: &JournalRecordV1,
    target: Option<PinnedDirectory>,
    stage: Option<PinnedDirectory>,
    backup: Option<PinnedDirectory>,
    phases: &mut Vec<LoadedPhase>,
) -> FsResult<()> {
    match (target, stage, backup) {
        (None, Some(stage), Some(backup))
            if is_expected_directory(
                &stage,
                &registry.new_manifest_sha256,
                registered_stage_identity(registry)?,
            )? && is_expected_directory(
                &backup,
                &registry.old_manifest_sha256,
                registry.old_directory_identity,
            )? =>
        {
            publish_recovered_new(parent, registry, expected, stage, backup, phases)
        }
        (Some(target), None, Some(backup))
            if is_expected_directory(
                &target,
                &registry.new_manifest_sha256,
                registered_stage_identity(registry)?,
            )? && is_expected_directory(
                &backup,
                &registry.old_manifest_sha256,
                registry.old_directory_identity,
            )? =>
        {
            phases.push(write_phase_record(
                parent,
                expected,
                ReplacementPhase::NewPublished,
            )?);
            finish_recovered_new(parent, registry, expected, backup, phases)
        }
        (Some(target), Some(stage), None)
            if is_expected_directory(
                &target,
                &registry.old_manifest_sha256,
                registry.old_directory_identity,
            )? && stage.identity() == registered_stage_identity(registry)? =>
        {
            cleanup_transaction_directory(
                parent,
                &staging_name(&registry.transaction_id),
                &stage,
                registered_stage_identity(registry)?,
                &registry.new_entries,
                true,
            )?;
            remove_phase_records(parent, phases)?;
            parent.sync_directory()
        }
        (Some(target), None, None)
            if is_expected_directory(
                &target,
                &registry.old_manifest_sha256,
                registry.old_directory_identity,
            )? =>
        {
            remove_phase_records(parent, phases)?;
            parent.sync_directory()
        }
        _ => Err(ProjectFolderFilesystemError::RecoveryRequired),
    }
}

fn recover_new_published(
    parent: &PinnedDirectory,
    registry: &RegistryRecordV1,
    expected: &JournalRecordV1,
    target: Option<PinnedDirectory>,
    stage: Option<PinnedDirectory>,
    backup: Option<PinnedDirectory>,
    phases: &mut Vec<LoadedPhase>,
) -> FsResult<()> {
    let Some(target) = target else {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    };
    if stage.is_some()
        || !is_expected_directory(
            &target,
            &registry.new_manifest_sha256,
            registered_stage_identity(registry)?,
        )?
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    if let Some(backup) = backup {
        if backup.identity() != registry.old_directory_identity {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        finish_recovered_new(parent, registry, expected, backup, phases)
    } else {
        phases.push(write_phase_record(
            parent,
            expected,
            ReplacementPhase::CleanupComplete,
        )?);
        remove_phase_records(parent, phases)?;
        parent.sync_directory()
    }
}

fn publish_recovered_new(
    parent: &PinnedDirectory,
    registry: &RegistryRecordV1,
    expected: &JournalRecordV1,
    mut stage: PinnedDirectory,
    mut backup: PinnedDirectory,
    phases: &mut Vec<LoadedPhase>,
) -> FsResult<()> {
    match parent.publish_child_directory_no_replace(
        &staging_name(&registry.transaction_id),
        &mut stage,
        &registry.target_name,
    ) {
        Ok(()) => {
            parent.sync_directory()?;
            phases.push(write_phase_record(
                parent,
                expected,
                ReplacementPhase::NewPublished,
            )?);
            finish_recovered_new(parent, registry, expected, backup, phases)
        }
        Err(_) => {
            if !parent.child_exists(&registry.target_name)? {
                parent.publish_child_directory_no_replace(
                    &backup_name(&registry.transaction_id),
                    &mut backup,
                    &registry.target_name,
                )?;
                parent.sync_directory()?;
            }
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        }
    }
}

fn finish_recovered_new(
    parent: &PinnedDirectory,
    registry: &RegistryRecordV1,
    expected: &JournalRecordV1,
    backup: PinnedDirectory,
    phases: &mut Vec<LoadedPhase>,
) -> FsResult<()> {
    cleanup_transaction_directory(
        parent,
        &backup_name(&registry.transaction_id),
        &backup,
        registry.old_directory_identity,
        &registry.old_entries,
        true,
    )?;
    parent.sync_directory()?;
    phases.push(write_phase_record(
        parent,
        expected,
        ReplacementPhase::CleanupComplete,
    )?);
    remove_phase_records(parent, phases)?;
    parent.sync_directory()
}

fn assert_new_target_only(
    registry: &RegistryRecordV1,
    target: Option<PinnedDirectory>,
    stage: Option<PinnedDirectory>,
    backup: Option<PinnedDirectory>,
) -> FsResult<()> {
    let Some(target) = target else {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    };
    if stage.is_some()
        || backup.is_some()
        || !is_expected_directory(
            &target,
            &registry.new_manifest_sha256,
            registered_stage_identity(registry)?,
        )?
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

fn load_phase_records(
    parent: &PinnedDirectory,
    expected: &JournalRecordV1,
) -> FsResult<Vec<LoadedPhase>> {
    let mut loaded = Vec::with_capacity(PHASE_COUNT);
    for state in ReplacementPhase::ALL {
        let final_name = journal_name(&expected.transaction_id, state);
        let temp_name = journal_temp_name(&expected.transaction_id, state);
        let final_exists = parent.child_exists(&final_name)?;
        let temp_exists = parent.child_exists(&temp_name)?;
        if final_exists && temp_exists {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        let Some(name) = final_exists
            .then_some(final_name.clone())
            .or_else(|| temp_exists.then_some(temp_name.clone()))
        else {
            continue;
        };
        let mut file = parent
            .open_child_file_for_update(&name, MAX_JOURNAL_BYTES)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        let bytes = file
            .read_bounded_and_revalidate(parent, &name, MAX_JOURNAL_BYTES)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        let record: JournalRecordV1 = serde_json::from_slice(&bytes)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        validate_journal_record(&record)?;
        let mut expected_record = expected.clone();
        expected_record.state = state;
        if record != expected_record {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        if temp_exists {
            parent
                .publish_child_file_no_replace(&temp_name, &file, &final_name)
                .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
            parent.sync_directory()?;
        }
        loaded.push(LoadedPhase {
            state,
            name: final_name,
            file,
        });
    }
    Ok(loaded)
}

fn write_phase_record(
    parent: &PinnedDirectory,
    base: &JournalRecordV1,
    state: ReplacementPhase,
) -> FsResult<LoadedPhase> {
    let mut record = base.clone();
    record.state = state;
    validate_journal_record(&record)?;
    let bytes = serialize_json(&record, MAX_JOURNAL_BYTES)?;
    let temp_name = journal_temp_name(&record.transaction_id, state);
    let final_name = journal_name(&record.transaction_id, state);
    if parent.child_exists(&temp_name)? || parent.child_exists(&final_name)? {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let file = parent.write_child_file_pinned(&temp_name, &bytes)?;
    parent.sync_directory()?;
    parent
        .publish_child_file_no_replace(&temp_name, &file, &final_name)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    parent.sync_directory()?;
    Ok(LoadedPhase {
        state,
        name: final_name,
        file,
    })
}

fn remove_phase_records(parent: &PinnedDirectory, phases: &mut Vec<LoadedPhase>) -> FsResult<()> {
    if phases
        .iter()
        .any(|phase| phase.state == ReplacementPhase::CleanupComplete)
    {
        // Once cleanup-complete is durable, retain it until last so any
        // interrupted lower-phase deletion remains unambiguously committed.
        phases.sort_by_key(|phase| phase.state);
    } else {
        // A rollback has no cleanup-complete sentinel. Delete newest first so
        // every process-kill residue remains a valid phase prefix.
        phases.sort_by_key(|phase| std::cmp::Reverse(phase.state));
    }
    for phase in phases.iter() {
        parent
            .remove_child_file_if_same(&phase.name, &phase.file)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        parent.sync_directory()?;
    }
    phases.clear();
    Ok(())
}

fn validate_phase_continuity(
    phases: &[LoadedPhase],
    highest: Option<ReplacementPhase>,
) -> FsResult<()> {
    let present = phases
        .iter()
        .map(|phase| phase.state)
        .collect::<HashSet<_>>();
    let Some(highest) = highest else {
        return Ok(());
    };
    if highest == ReplacementPhase::CleanupComplete {
        return Ok(());
    }
    if ReplacementPhase::ALL
        .iter()
        .take(highest.index() + 1)
        .any(|state| !present.contains(state))
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

fn validate_recovery_namespace(
    parent: &PinnedDirectory,
    registry: &RegistryRecordV1,
) -> FsResult<()> {
    let names = parent
        .list_names_with_ascii_prefix(RECOVERY_NAMESPACE_PREFIX, MAX_RECOVERY_PARENT_ENTRIES + 1)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    let allowed = recovery_names(registry);
    for name in names {
        let name = name
            .into_string()
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        if is_recovery_namespace_name(&name) && !allowed.contains(&name) {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
    }
    Ok(())
}

fn recovery_names(registry: &RegistryRecordV1) -> HashSet<String> {
    recovery_names_for_transaction(&registry.transaction_id)
}

fn recovery_names_for_transaction(transaction_id: &str) -> HashSet<String> {
    let mut names = HashSet::with_capacity(2 + PHASE_COUNT * 2);
    names.insert(staging_name(transaction_id));
    names.insert(backup_name(transaction_id));
    for state in ReplacementPhase::ALL {
        names.insert(journal_name(transaction_id, state));
        names.insert(journal_temp_name(transaction_id, state));
    }
    names
}

fn ensure_transaction_namespace_clear(
    parent: &PinnedDirectory,
    transaction_id: &str,
    target_name: &str,
) -> FsResult<()> {
    let names = parent
        .list_names_with_ascii_prefix(RECOVERY_NAMESPACE_PREFIX, MAX_RECOVERY_PARENT_ENTRIES + 1)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    let allowed = recovery_names_for_transaction(transaction_id);
    for name in names {
        let name = name
            .into_string()
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        if is_recovery_namespace_name(&name) && !allowed.contains(&name) {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
    }
    if parent.child_exists(&staging_name(transaction_id))?
        || parent.child_exists(&backup_name(transaction_id))?
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    for state in ReplacementPhase::ALL {
        if parent.child_exists(&journal_name(transaction_id, state))?
            || parent.child_exists(&journal_temp_name(transaction_id, state))?
        {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
    }
    if !parent.child_exists(target_name)? {
        return Err(ProjectFolderFilesystemError::ChangedDuringRead);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_recovery_namespace_name(name: &str) -> bool {
    name.get(..RECOVERY_NAMESPACE_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(RECOVERY_NAMESPACE_PREFIX))
}

#[cfg(unix)]
fn is_recovery_namespace_name(name: &str) -> bool {
    name.starts_with(RECOVERY_NAMESPACE_PREFIX)
}

fn open_optional_directory(
    parent: &PinnedDirectory,
    name: &str,
) -> FsResult<Option<PinnedDirectory>> {
    if !parent.child_exists(name)? {
        return Ok(None);
    }
    parent
        .open_child_directory_for_rename(name)
        .map(Some)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)
}

fn cleanup_transaction_directory(
    parent: &PinnedDirectory,
    directory_name: &str,
    directory: &PinnedDirectory,
    expected_identity: DirectoryIdentity,
    fingerprints: &ArtifactFingerprintsV1,
    allow_missing: bool,
) -> FsResult<()> {
    if directory.identity() != expected_identity {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    validate_artifact_fingerprints(fingerprints)?;
    parent
        .revalidate_child_directory(directory_name, directory)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    let root_names = validate_names(directory.list_names(5)?, 4)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    let expected_root_names = fingerprints
        .entries
        .iter()
        .map(|entry| {
            if entry.path == PROJECT_FOLDER_PREVIEW_PATH {
                "preview"
            } else {
                entry.path.as_str()
            }
        })
        .collect::<HashSet<_>>();
    if root_names
        .iter()
        .any(|name| !expected_root_names.contains(name.as_str()))
        || (!allow_missing
            && root_names
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>()
                != expected_root_names)
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }

    for path in [
        PROJECT_FOLDER_PROJECT_PATH,
        PROJECT_FOLDER_EDITOR_HISTORY_PATH,
    ] {
        remove_verified_transaction_file_if_present(directory, path, fingerprints, allow_missing)?;
    }

    let preview_expected = fingerprint_for_path(fingerprints, PROJECT_FOLDER_PREVIEW_PATH)?;
    if directory.child_exists("preview")? {
        let preview = directory
            .open_child_directory_for_rename("preview")
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        let preview_names = validate_names(preview.list_names(2)?, 1)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
        if preview_names
            .iter()
            .any(|name| name != "crease-pattern.svg")
            || (!allow_missing && preview_names != ["crease-pattern.svg"])
        {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        if preview.child_exists("crease-pattern.svg")? {
            verify_and_remove_transaction_file(&preview, "crease-pattern.svg", preview_expected)?;
        }
        if !preview.list_names(1)?.is_empty() {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        directory
            .remove_child_directory_if_same("preview", &preview)
            .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    } else if !allow_missing {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }

    // The manifest is deliberately deleted last. After a process kill during
    // cleanup it remains durable evidence until all payload entries are gone.
    remove_verified_transaction_file_if_present(
        directory,
        PROJECT_FOLDER_MANIFEST_PATH,
        fingerprints,
        allow_missing,
    )?;
    if !directory.list_names(1)?.is_empty() {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    parent
        .remove_child_directory_if_same(directory_name, directory)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)
}

fn remove_verified_transaction_file_if_present(
    parent: &PinnedDirectory,
    path: &str,
    fingerprints: &ArtifactFingerprintsV1,
    allow_missing: bool,
) -> FsResult<()> {
    let expected = match fingerprints.entries.iter().find(|entry| entry.path == path) {
        Some(expected) => expected,
        None => {
            if parent.child_exists(path)? {
                return Err(ProjectFolderFilesystemError::RecoveryRequired);
            }
            return Ok(());
        }
    };
    if !parent.child_exists(path)? {
        return if allow_missing {
            Ok(())
        } else {
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        };
    }
    verify_and_remove_transaction_file(parent, path, expected)
}

fn verify_and_remove_transaction_file(
    parent: &PinnedDirectory,
    name: &str,
    expected: &EntryFingerprintV1,
) -> FsResult<()> {
    let mut file = parent
        .open_child_file_for_update(name, fingerprint_limit(&expected.path))
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    let bytes = file
        .read_bounded_and_revalidate(parent, name, fingerprint_limit(&expected.path))
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    if bytes.len() as u64 != expected.size || hex_encode(&Sha256::digest(&bytes)) != expected.sha256
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    parent
        .remove_child_file_if_same(name, &file)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)
}

fn assert_artifact(
    directory: &PinnedDirectory,
    expected: &ProjectFolderArtifactV1,
    expected_hash: &str,
    expected_identity: DirectoryIdentity,
) -> FsResult<()> {
    if directory.identity() != expected_identity {
        return Err(ProjectFolderFilesystemError::ChangedDuringRead);
    }
    let actual =
        load_project_folder_artifact_from_pinned(directory, ProjectFolderLimits::default())?;
    if actual != *expected || artifact_manifest_sha256(&actual)? != expected_hash {
        return Err(ProjectFolderFilesystemError::ChangedDuringRead);
    }
    Ok(())
}

fn is_expected_directory(
    directory: &PinnedDirectory,
    expected_hash: &str,
    expected_identity: DirectoryIdentity,
) -> FsResult<bool> {
    if directory.identity() != expected_identity {
        return Ok(false);
    }
    let artifact =
        match load_project_folder_artifact_from_pinned(directory, ProjectFolderLimits::default()) {
            Ok(artifact) => artifact,
            Err(_) => return Ok(false),
        };
    Ok(artifact_manifest_sha256(&artifact)? == expected_hash)
}

fn artifact_manifest_sha256(artifact: &ProjectFolderArtifactV1) -> FsResult<String> {
    let manifest = artifact
        .entries()
        .iter()
        .find(|entry| entry.path == PROJECT_FOLDER_MANIFEST_PATH)
        .ok_or(ProjectFolderFilesystemError::InvalidTree)?;
    Ok(hex_encode(&Sha256::digest(&manifest.bytes)))
}

fn artifact_fingerprints(artifact: &ProjectFolderArtifactV1) -> FsResult<ArtifactFingerprintsV1> {
    let mut entries = artifact
        .entries()
        .iter()
        .map(|entry| EntryFingerprintV1 {
            path: entry.path.clone(),
            size: entry.bytes.len() as u64,
            sha256: hex_encode(&Sha256::digest(&entry.bytes)),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    let fingerprints = ArtifactFingerprintsV1 { entries };
    validate_artifact_fingerprints(&fingerprints)?;
    Ok(fingerprints)
}

fn validate_registry_record(record: &RegistryRecordV1) -> FsResult<()> {
    if record.version != REGISTRY_VERSION {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    if !matches!(
        (record.state, record.stage_directory_identity),
        (RegistryPhase::Reserved, None) | (RegistryPhase::Staged, Some(_))
    ) {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    validate_native_child_name(&record.target_name)?;
    validate_transaction_id(&record.transaction_id)?;
    validate_sha256(&record.old_manifest_sha256)?;
    validate_sha256(&record.new_manifest_sha256)?;
    validate_artifact_fingerprints(&record.old_entries)?;
    validate_artifact_fingerprints(&record.new_entries)?;
    if fingerprint_for_path(&record.old_entries, PROJECT_FOLDER_MANIFEST_PATH)?.sha256
        != record.old_manifest_sha256
        || fingerprint_for_path(&record.new_entries, PROJECT_FOLDER_MANIFEST_PATH)?.sha256
            != record.new_manifest_sha256
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let decoded = decode_parent_path(record)?;
    if !decoded.is_absolute() {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

fn registered_stage_identity(record: &RegistryRecordV1) -> FsResult<DirectoryIdentity> {
    if record.state != RegistryPhase::Staged {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    record
        .stage_directory_identity
        .ok_or(ProjectFolderFilesystemError::RecoveryRequired)
}

fn validate_journal_record(record: &JournalRecordV1) -> FsResult<()> {
    if record.version != JOURNAL_VERSION {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    validate_native_child_name(&record.target_name)?;
    validate_transaction_id(&record.transaction_id)?;
    validate_sha256(&record.old_manifest_sha256)?;
    validate_sha256(&record.new_manifest_sha256)?;
    validate_artifact_fingerprints(&record.old_entries)?;
    validate_artifact_fingerprints(&record.new_entries)?;
    if fingerprint_for_path(&record.old_entries, PROJECT_FOLDER_MANIFEST_PATH)?.sha256
        != record.old_manifest_sha256
        || fingerprint_for_path(&record.new_entries, PROJECT_FOLDER_MANIFEST_PATH)?.sha256
            != record.new_manifest_sha256
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

fn validate_artifact_fingerprints(fingerprints: &ArtifactFingerprintsV1) -> FsResult<()> {
    if !matches!(fingerprints.entries.len(), 3 | 4) {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let mut previous: Option<&str> = None;
    let mut paths = HashSet::with_capacity(fingerprints.entries.len());
    let mut total_size = 0_u64;
    for entry in &fingerprints.entries {
        if previous.is_some_and(|previous| previous >= entry.path.as_str())
            || !matches!(
                entry.path.as_str(),
                PROJECT_FOLDER_MANIFEST_PATH
                    | PROJECT_FOLDER_PROJECT_PATH
                    | PROJECT_FOLDER_EDITOR_HISTORY_PATH
                    | PROJECT_FOLDER_PREVIEW_PATH
            )
            || entry.size > fingerprint_limit(&entry.path)
        {
            return Err(ProjectFolderFilesystemError::RecoveryRequired);
        }
        total_size = total_size
            .checked_add(entry.size)
            .filter(|total| *total <= MAX_PROJECT_FOLDER_TOTAL_BYTES)
            .ok_or(ProjectFolderFilesystemError::RecoveryRequired)?;
        validate_sha256(&entry.sha256)?;
        paths.insert(entry.path.as_str());
        previous = Some(&entry.path);
    }
    if !paths.contains(PROJECT_FOLDER_MANIFEST_PATH)
        || !paths.contains(PROJECT_FOLDER_PROJECT_PATH)
        || !paths.contains(PROJECT_FOLDER_PREVIEW_PATH)
        || fingerprints.entries.len()
            != if paths.contains(PROJECT_FOLDER_EDITOR_HISTORY_PATH) {
                4
            } else {
                3
            }
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

fn fingerprint_for_path<'a>(
    fingerprints: &'a ArtifactFingerprintsV1,
    path: &str,
) -> FsResult<&'a EntryFingerprintV1> {
    fingerprints
        .entries
        .iter()
        .find(|entry| entry.path == path)
        .ok_or(ProjectFolderFilesystemError::RecoveryRequired)
}

fn fingerprint_limit(path: &str) -> u64 {
    match path {
        PROJECT_FOLDER_MANIFEST_PATH => MAX_PROJECT_FOLDER_MANIFEST_BYTES,
        PROJECT_FOLDER_PREVIEW_PATH => MAX_PROJECT_FOLDER_PREVIEW_BYTES,
        PROJECT_FOLDER_PROJECT_PATH => MAX_PROJECT_JSON_BYTES as u64,
        PROJECT_FOLDER_EDITOR_HISTORY_PATH => MAX_EDITOR_HISTORY_JSON_BYTES,
        _ => 0,
    }
}

fn validate_transaction_id(id: &str) -> FsResult<()> {
    if id.len() != 32
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

fn validate_sha256(hash: &str) -> FsResult<()> {
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

fn validate_registry_names(names: Vec<OsString>) -> FsResult<Vec<String>> {
    let names = validate_names(names, MAX_REGISTRY_ENTRIES)
        .map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    if names.iter().any(|name| {
        !matches!(
            name.as_str(),
            REGISTRY_RESERVED_NAME
                | REGISTRY_RESERVED_TEMP_NAME
                | REGISTRY_STAGED_NAME
                | REGISTRY_STAGED_TEMP_NAME
        )
    }) {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(names)
}

fn serialize_json<T: Serialize>(value: &T, limit: u64) -> FsResult<Vec<u8>> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|_| ProjectFolderFilesystemError::WriteFailed)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > limit {
        return Err(ProjectFolderFilesystemError::TooLarge);
    }
    Ok(bytes)
}

fn transaction_id(bytes: [u8; 16]) -> String {
    hex_encode(&bytes)
}

fn staging_name(transaction_id: &str) -> String {
    format!(".origami2-folder-stage-{transaction_id}")
}

fn backup_name(transaction_id: &str) -> String {
    format!(".origami2-folder-backup-{transaction_id}")
}

fn journal_name(transaction_id: &str, state: ReplacementPhase) -> String {
    format!(
        ".origami2-folder-txn-{transaction_id}-{}.json",
        state.file_component()
    )
}

fn journal_temp_name(transaction_id: &str, state: ReplacementPhase) -> String {
    format!(
        ".origami2-folder-txn-{transaction_id}-{}.tmp",
        state.file_component()
    )
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode(encoded: &str, maximum_bytes: usize) -> FsResult<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) || encoded.len() / 2 > maximum_bytes {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> FsResult<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(ProjectFolderFilesystemError::RecoveryRequired),
    }
}

#[cfg(target_os = "windows")]
fn encode_parent_path(path: &Path) -> FsResult<(ParentPathEncoding, String)> {
    use std::os::windows::ffi::OsStrExt;

    if !path.is_absolute() {
        return Err(ProjectFolderFilesystemError::InvalidRequest);
    }
    let mut bytes = Vec::new();
    for unit in path.as_os_str().encode_wide() {
        if unit == 0 {
            return Err(ProjectFolderFilesystemError::InvalidRequest);
        }
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    Ok((ParentPathEncoding::WindowsUtf16Le, hex_encode(&bytes)))
}

#[cfg(unix)]
fn encode_parent_path(path: &Path) -> FsResult<(ParentPathEncoding, String)> {
    use std::os::unix::ffi::OsStrExt;

    if !path.is_absolute() || path.as_os_str().as_bytes().contains(&0) {
        return Err(ProjectFolderFilesystemError::InvalidRequest);
    }
    Ok((
        ParentPathEncoding::UnixBytes,
        hex_encode(path.as_os_str().as_bytes()),
    ))
}

#[cfg(target_os = "windows")]
fn decode_parent_path(record: &RegistryRecordV1) -> FsResult<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    if record.path_encoding != ParentPathEncoding::WindowsUtf16Le {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let bytes = hex_decode(&record.parent_path_hex, 64 * 1024)?;
    if bytes.len() % 2 != 0 {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    if units.contains(&0) {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(PathBuf::from(OsString::from_wide(&units)))
}

#[cfg(unix)]
fn decode_parent_path(record: &RegistryRecordV1) -> FsResult<PathBuf> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    if record.path_encoding != ParentPathEncoding::UnixBytes {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    let bytes = hex_decode(&record.parent_path_hex, 64 * 1024)?;
    if bytes.contains(&0) {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(PathBuf::from(OsString::from_vec(bytes)))
}

#[cfg(unix)]
fn create_private_registry_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(target_os = "windows")]
fn create_private_registry_directory(path: &Path) -> std::io::Result<()> {
    fs::create_dir(path)
}

#[cfg(unix)]
fn validate_private_registry_directory(path: &Path) -> FsResult<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata =
        fs::symlink_metadata(path).map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_dir()
        || metadata.mode() & 0o077 != 0
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn validate_private_registry_directory(path: &Path) -> FsResult<()> {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    let metadata =
        fs::symlink_metadata(path).map_err(|_| ProjectFolderFilesystemError::RecoveryRequired)?;
    if !metadata.file_type().is_dir()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(ProjectFolderFilesystemError::RecoveryRequired);
    }
    Ok(())
}

#[cfg(test)]
fn crash_if_requested(
    requested: Option<ReplacementCrashPoint>,
    current: ReplacementCrashPoint,
) -> FsResult<()> {
    if requested == Some(current) {
        Err(ProjectFolderFilesystemError::InjectedCrash)
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
fn crash_if_requested() -> FsResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use ori_domain::CreasePattern;
    use ori_formats::{Ori2ProjectArchive, ProjectDocument, write_project_folder_v1};

    use super::*;
    use crate::project_folder_io::ProjectFolderIoState;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);
    const TARGET_NAME: &str = "replacement.origami2-folder";

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            for _ in 0..128 {
                let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "origami2-folder-replacement-{label}-{}-{id}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create test directory: {error}"),
                }
            }
            panic!("allocate replacement test directory");
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn artifacts() -> (ProjectFolderArtifactV1, ProjectFolderArtifactV1) {
        let old_archive = Ori2ProjectArchive {
            layer_evidence: None,
            document: ProjectDocument::new("Before", CreasePattern::empty()),
            editor_history: None,
        };
        let mut new_archive = old_archive.clone();
        new_archive.document.name = "After".to_owned();
        (
            write_project_folder_v1(&old_archive).expect("old artifact"),
            write_project_folder_v1(&new_archive).expect("new artifact"),
        )
    }

    fn write_fixture(root: &Path, artifact: &ProjectFolderArtifactV1) {
        fs::create_dir(root).expect("create project root");
        fs::create_dir(root.join("preview")).expect("create preview root");
        for entry in artifact.entries() {
            fs::write(root.join(&entry.path), &entry.bytes).expect("write fixture");
        }
    }

    fn setup_replacement(
        label: &str,
    ) -> (
        TestDirectory,
        PathBuf,
        PathBuf,
        ReplacementRegistry,
        ProjectFolderArtifactV1,
        ProjectFolderArtifactV1,
    ) {
        let directory = TestDirectory::new(label);
        let parent = directory.0.join("parent");
        let registry_root = directory.0.join("registry");
        fs::create_dir(&parent).expect("create parent");
        let (old, new) = artifacts();
        write_fixture(&parent.join(TARGET_NAME), &old);
        let registry = ReplacementRegistry::new(registry_root.clone());
        (directory, parent, registry_root, registry, old, new)
    }

    fn assert_target(parent: &Path, expected: &ProjectFolderArtifactV1) -> ProjectFolderArtifactV1 {
        let actual = super::super::load_project_folder_artifact(
            &parent.join(TARGET_NAME),
            ProjectFolderLimits::default(),
        )
        .expect("load recovered target");
        assert_eq!(&actual, expected);
        actual
    }

    fn assert_transaction_artifacts_absent(parent: &Path, registry_root: &Path) {
        let reserved = fs::read_dir(parent)
            .expect("enumerate parent")
            .map(|entry| entry.expect("entry").file_name())
            .filter_map(|name| name.into_string().ok())
            .filter(|name| name.starts_with(".origami2-folder-"))
            .collect::<Vec<_>>();
        assert!(
            reserved.is_empty(),
            "leftover transaction entries: {reserved:?}"
        );
        assert!(
            fs::read_dir(registry_root)
                .expect("enumerate registry")
                .next()
                .is_none()
        );
    }

    #[test]
    fn every_durable_crash_point_recovers_to_old_or_new_complete_tree() {
        let cases = [
            (ReplacementCrashPoint::BeforePrepared, false),
            (ReplacementCrashPoint::AfterPrepared, false),
            (ReplacementCrashPoint::AfterOldRename, true),
            (ReplacementCrashPoint::AfterOldMoved, true),
            (ReplacementCrashPoint::AfterNewRename, true),
            (ReplacementCrashPoint::AfterNewPublished, true),
            (ReplacementCrashPoint::AfterBackupCleanup, true),
            (ReplacementCrashPoint::AfterCleanupComplete, true),
        ];
        for (point, expects_new) in cases {
            let (_directory, parent, registry_root, registry, old, new) =
                setup_replacement(&format!("crash-{point:?}"));
            let mut prepared = PreparedReplacementProjectFolder::prepare(
                &registry,
                &parent,
                TARGET_NAME,
                new.clone(),
            )
            .expect("prepare replacement");
            assert_eq!(
                prepared.publish_until(point),
                Err(ProjectFolderFilesystemError::InjectedCrash)
            );
            drop(prepared);

            registry.recover_pending().expect("recover transaction");
            assert_target(&parent, if expects_new { &new } else { &old });
            assert_transaction_artifacts_absent(&parent, &registry_root);
        }
    }

    #[test]
    fn process_restart_recovery_matrix_converges_at_each_published_phase() {
        let cases = [
            (ReplacementCrashPoint::AfterPrepared, false),
            (ReplacementCrashPoint::AfterOldMoved, true),
            (ReplacementCrashPoint::AfterNewPublished, true),
        ];
        for (point, expects_new) in cases {
            let (_directory, parent, registry_root, registry, old, new) =
                setup_replacement(&format!("restart-matrix-{point:?}"));
            let mut prepared = PreparedReplacementProjectFolder::prepare(
                &registry,
                &parent,
                TARGET_NAME,
                new.clone(),
            )
            .expect("prepare replacement");
            assert_eq!(
                prepared.publish_until(point),
                Err(ProjectFolderFilesystemError::InjectedCrash)
            );
            drop(prepared);
            drop(registry);

            // A fresh value models a new process: no in-memory handle or phase state is reused.
            let restarted = ReplacementRegistry::new(registry_root.clone());
            restarted
                .recover_pending()
                .expect("recover after process restart");
            assert_target(&parent, if expects_new { &new } else { &old });
            assert_transaction_artifacts_absent(&parent, &registry_root);
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_readonly_cleanup_failure_preserves_complete_target_and_retries() {
        for (point, protects_backup, expects_new) in [
            (ReplacementCrashPoint::AfterPrepared, false, false),
            (ReplacementCrashPoint::AfterNewPublished, true, true),
        ] {
            let (_directory, parent, registry_root, registry, old, new) =
                setup_replacement(&format!("readonly-retry-{point:?}"));
            let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
            let mut prepared = PreparedReplacementProjectFolder::prepare(
                &registry,
                &parent,
                TARGET_NAME,
                new.clone(),
            )
            .expect("prepare replacement");
            assert_eq!(
                prepared.publish_until(point),
                Err(ProjectFolderFilesystemError::InjectedCrash)
            );
            drop(prepared);
            drop(registry);

            let protected = if protects_backup {
                parent
                    .join(backup_name(&transaction))
                    .join(PROJECT_FOLDER_PROJECT_PATH)
            } else {
                parent
                    .join(staging_name(&transaction))
                    .join(PROJECT_FOLDER_PROJECT_PATH)
            };
            let mut permissions = fs::metadata(&protected)
                .expect("protected payload metadata")
                .permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&protected, permissions).expect("make cleanup payload readonly");

            let restarted = ReplacementRegistry::new(registry_root.clone());
            assert_eq!(
                restarted.recover_pending(),
                Err(ProjectFolderFilesystemError::RecoveryRequired)
            );
            assert_target(&parent, if expects_new { &new } else { &old });

            let mut permissions = fs::metadata(&protected)
                .expect("retained payload metadata")
                .permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&protected, permissions).expect("allow bounded retry cleanup");
            restarted
                .recover_pending()
                .expect("retry cleanup after permission recovery");
            assert_target(&parent, if expects_new { &new } else { &old });
            assert_transaction_artifacts_absent(&parent, &registry_root);
        }
    }

    #[test]
    fn successful_replacement_publishes_new_and_removes_journal_registry_and_backup() {
        let (_directory, parent, registry_root, registry, _old, new) = setup_replacement("success");
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new.clone())
                .expect("prepare replacement");
        prepared.publish().expect("publish replacement");
        drop(prepared);

        assert_target(&parent, &new);
        assert_transaction_artifacts_absent(&parent, &registry_root);
    }

    #[test]
    fn unrelated_large_parent_directory_does_not_block_replacement_namespace_scan() {
        let (_directory, parent, _registry_root, registry, _old, new) =
            setup_replacement("large-unrelated-parent");
        for index in 0..(MAX_RECOVERY_PARENT_ENTRIES + 2) {
            fs::write(parent.join(format!("unrelated-{index:04}.txt")), b"x")
                .expect("write unrelated parent entry");
        }

        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new.clone())
                .expect("unrelated entries are outside the recovery namespace budget");
        prepared.publish().expect("publish replacement");
        drop(prepared);
        assert_target(&parent, &new);
    }

    #[test]
    fn different_project_id_never_replaces_the_existing_target() {
        let (_directory, parent, _registry_root, registry, old, new) =
            setup_replacement("different-project");
        let mut archive = new.archive().clone();
        archive.document.project_id = ori_domain::ProjectId::new();
        let foreign = write_project_folder_v1(&archive).expect("foreign artifact");

        assert!(matches!(
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, foreign,),
            Err(ProjectFolderFilesystemError::TargetExists)
        ));
        assert_target(&parent, &old);
    }

    #[test]
    fn unsupported_replacement_volume_creates_no_reservation_or_transaction_entry() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("unsupported-volume");
        let result = PreparedReplacementProjectFolder::prepare_with_replacement_admission_hook(
            &registry,
            &parent,
            TARGET_NAME,
            new,
            |_| Err(ProjectFolderFilesystemError::ReplacementUnsupported),
        );

        assert!(matches!(
            result,
            Err(ProjectFolderFilesystemError::ReplacementUnsupported)
        ));
        assert_target(&parent, &old);
        assert!(
            !registry_root.exists(),
            "unsupported replacement must not create the private reservation root"
        );
        assert!(
            fs::read_dir(&parent)
                .expect("enumerate parent")
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().into_string().ok())
                .all(|name| !name.starts_with(".origami2-folder-")),
            "unsupported replacement must not create stage, backup, or phase records"
        );
    }

    #[test]
    fn unrelated_transaction_namespace_blocks_before_a_new_replacement_starts() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("unrelated-transaction");
        let unrelated =
            parent.join(".origami2-folder-txn-00000000000000000000000000000000-prepared.json");
        fs::write(&unrelated, b"preserve unrelated recovery evidence")
            .expect("write unrelated transaction evidence");

        assert!(matches!(
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new,),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        ));
        assert_target(&parent, &old);
        assert_eq!(
            fs::read(&unrelated).expect("unrelated evidence retained"),
            b"preserve unrelated recovery evidence"
        );
        assert!(
            !registry_root.join(REGISTRY_RESERVED_NAME).exists()
                && !registry_root.join(REGISTRY_STAGED_NAME).exists(),
            "no transaction may be registered in a namespace that recovery would reject"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn case_variant_transaction_namespace_blocks_on_windows_before_reservation() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("case-variant-transaction");
        let unrelated =
            parent.join(".ORIGAMI2-FOLDER-TXN-00000000000000000000000000000000-PREPARED.JSON");
        fs::write(&unrelated, b"preserve case-variant recovery evidence")
            .expect("write case-variant transaction evidence");

        assert!(matches!(
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new,),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        ));
        assert_target(&parent, &old);
        assert_eq!(
            fs::read(&unrelated).expect("case-variant evidence retained"),
            b"preserve case-variant recovery evidence"
        );
        assert!(
            !registry_root.join(REGISTRY_RESERVED_NAME).exists()
                && !registry_root.join(REGISTRY_STAGED_NAME).exists(),
            "no transaction may be registered in a case-variant reserved namespace"
        );
    }

    #[test]
    fn prepared_recovery_resumes_partial_stage_cleanup() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("partial-prepared-stage-cleanup");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::AfterPrepared),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);

        fs::remove_file(
            parent
                .join(staging_name(&transaction))
                .join(PROJECT_FOLDER_PROJECT_PATH),
        )
        .expect("emulate process kill during stage cleanup");
        drop(registry);
        ReplacementRegistry::new(registry_root.clone())
            .recover_pending()
            .expect("resume authenticated partial stage cleanup");
        assert_target(&parent, &old);
        assert_transaction_artifacts_absent(&parent, &registry_root);
    }

    #[test]
    fn rolled_back_old_move_converges_after_stage_and_newest_phase_cleanup() {
        for newest_phase_removed in [false, true] {
            let label = if newest_phase_removed {
                "rollback-after-newest-phase-cleanup"
            } else {
                "rollback-after-stage-cleanup"
            };
            let (_directory, parent, registry_root, registry, old, new) = setup_replacement(label);
            let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
            let mut prepared =
                PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                    .expect("prepare replacement");
            assert_eq!(
                prepared.publish_until(ReplacementCrashPoint::AfterOldMoved),
                Err(ProjectFolderFilesystemError::InjectedCrash)
            );
            drop(prepared);

            fs::rename(
                parent.join(backup_name(&transaction)),
                parent.join(TARGET_NAME),
            )
            .expect("emulate exact old-target restoration");
            fs::remove_dir_all(parent.join(staging_name(&transaction)))
                .expect("emulate completed stage cleanup");
            if newest_phase_removed {
                fs::remove_file(
                    parent.join(journal_name(&transaction, ReplacementPhase::OldMoved)),
                )
                .expect("emulate newest-first rollback phase cleanup");
            }

            registry
                .recover_pending()
                .expect("settle authenticated rolled-back state");
            assert_target(&parent, &old);
            assert_transaction_artifacts_absent(&parent, &registry_root);
        }
    }

    #[test]
    fn valid_phase_temp_is_promoted_before_recovery_mutates_directories() {
        let (_directory, parent, registry_root, registry, _old, new) =
            setup_replacement("phase-temp");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new.clone())
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::AfterOldMoved),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);
        let final_name = journal_name(&transaction, ReplacementPhase::OldMoved);
        let temp_name = journal_temp_name(&transaction, ReplacementPhase::OldMoved);
        fs::rename(parent.join(final_name), parent.join(temp_name)).expect("restore temp phase");

        registry.recover_pending().expect("recover promoted temp");
        assert_target(&parent, &new);
        assert_transaction_artifacts_absent(&parent, &registry_root);
    }

    #[test]
    fn partial_backup_cleanup_resumes_only_for_authenticated_remaining_entries() {
        let (_directory, parent, registry_root, registry, _old, new) =
            setup_replacement("partial-backup");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new.clone())
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::AfterNewPublished),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);
        let backup = parent.join(backup_name(&transaction));
        fs::remove_file(backup.join("project.json")).expect("simulate completed payload cleanup");
        fs::remove_file(backup.join("preview/crease-pattern.svg"))
            .expect("simulate completed preview cleanup");
        fs::remove_dir(backup.join("preview")).expect("simulate preview directory cleanup");

        registry.recover_pending().expect("resume partial cleanup");
        assert_target(&parent, &new);
        assert_transaction_artifacts_absent(&parent, &registry_root);
    }

    #[test]
    fn altered_allowed_backup_payload_is_retained_and_fails_closed() {
        let (_directory, parent, registry_root, registry, _old, new) =
            setup_replacement("altered-backup");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new.clone())
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::AfterNewPublished),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);
        let altered = parent.join(backup_name(&transaction)).join("project.json");
        fs::write(&altered, b"attacker content").expect("alter allowed-name payload");

        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert_target(&parent, &new);
        assert_eq!(
            fs::read(&altered).expect("altered backup retained"),
            b"attacker content"
        );
        assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());
    }

    #[test]
    fn unknown_journal_field_fails_closed_without_changing_old_target() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("unknown-journal-field");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::AfterPrepared),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);

        let journal_path = parent.join(journal_name(&transaction, ReplacementPhase::Prepared));
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&journal_path).expect("read journal"))
                .expect("journal JSON");
        value["unknown"] = serde_json::Value::Bool(true);
        fs::write(
            &journal_path,
            serde_json::to_vec(&value).expect("mutated JSON"),
        )
        .expect("mutate journal");

        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert_target(&parent, &old);
        assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());
    }

    #[test]
    fn corrupt_and_oversized_journals_fail_closed_without_mutating_target() {
        for (label, replacement) in [
            ("corrupt-journal", vec![b'{']),
            (
                "oversized-journal",
                vec![b'x'; MAX_JOURNAL_BYTES as usize + 1],
            ),
        ] {
            let (_directory, parent, registry_root, registry, old, new) = setup_replacement(label);
            let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
            let mut prepared =
                PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                    .expect("prepare replacement");
            assert_eq!(
                prepared.publish_until(ReplacementCrashPoint::AfterPrepared),
                Err(ProjectFolderFilesystemError::InjectedCrash)
            );
            drop(prepared);
            fs::write(
                parent.join(journal_name(&transaction, ReplacementPhase::Prepared)),
                replacement,
            )
            .expect("replace journal bytes");

            assert_eq!(
                registry.recover_pending(),
                Err(ProjectFolderFilesystemError::RecoveryRequired)
            );
            assert_target(&parent, &old);
            assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());
        }
    }

    #[test]
    fn stage_directory_aba_is_not_deleted_or_published() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("stage-aba");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::BeforePrepared),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);

        let stage = parent.join(staging_name(&transaction));
        fs::rename(&stage, parent.join("retired-stage")).expect("retire admitted stage");
        fs::create_dir(&stage).expect("replacement stage");
        fs::write(stage.join("sentinel"), b"do not delete").expect("replacement sentinel");

        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert_target(&parent, &old);
        assert_eq!(
            fs::read(stage.join("sentinel")).expect("preserved replacement"),
            b"do not delete"
        );
        assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());
    }

    #[test]
    fn backup_directory_aba_is_not_deleted_after_new_publication() {
        let (_directory, parent, registry_root, registry, _old, new) =
            setup_replacement("backup-aba");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new.clone())
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::AfterNewPublished),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);

        let backup = parent.join(backup_name(&transaction));
        fs::rename(&backup, parent.join("retired-backup")).expect("retire admitted backup");
        fs::create_dir(&backup).expect("replacement backup");
        fs::write(backup.join("sentinel"), b"do not delete").expect("replacement sentinel");

        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert_target(&parent, &new);
        assert_eq!(
            fs::read(backup.join("sentinel")).expect("preserved replacement"),
            b"do not delete"
        );
        assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());
    }

    #[test]
    fn hard_linked_registry_is_rejected_and_retained() {
        let (directory, parent, registry_root, registry, old, new) =
            setup_replacement("registry-hard-link");
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::BeforePrepared),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);
        fs::hard_link(
            registry_root.join(REGISTRY_RECORD_NAME),
            directory.0.join("registry-second-name"),
        )
        .expect("hard link registry");

        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert_target(&parent, &old);
        assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());
    }

    #[test]
    fn hard_linked_phase_record_is_rejected_without_mutating_target() {
        let (directory, parent, registry_root, registry, old, new) =
            setup_replacement("journal-hard-link");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::AfterPrepared),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);
        fs::hard_link(
            parent.join(journal_name(&transaction, ReplacementPhase::Prepared)),
            directory.0.join("phase-second-name"),
        )
        .expect("hard link phase");

        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert_target(&parent, &old);
        assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());
    }

    #[test]
    fn external_parent_identity_aba_is_rejected_without_touching_replacement() {
        let (_directory, parent, registry_root, registry, _old, new) =
            setup_replacement("parent-aba");
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::BeforePrepared),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);

        let retired = parent.with_file_name("retired-parent");
        fs::rename(&parent, &retired).expect("retire admitted parent");
        fs::create_dir(&parent).expect("replacement parent");
        fs::write(parent.join("sentinel"), b"preserve").expect("replacement content");

        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert_eq!(
            fs::read(parent.join("sentinel")).expect("replacement preserved"),
            b"preserve"
        );
        assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());
    }

    #[cfg(unix)]
    #[test]
    fn registry_root_symlink_is_rejected_without_following() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("registry-root-symlink");
        let real = directory.0.join("real-registry");
        let link = directory.0.join("registry");
        fs::create_dir(&real).expect("real registry");
        symlink(&real, &link).expect("registry symlink");
        let registry = ReplacementRegistry::new(link);
        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert!(fs::read_dir(real).expect("real registry").next().is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn registry_root_reparse_point_is_rejected_without_following_when_supported() {
        use std::os::windows::fs::symlink_dir;

        let directory = TestDirectory::new("registry-root-reparse");
        let real = directory.0.join("real-registry");
        let link = directory.0.join("registry");
        fs::create_dir(&real).expect("real registry");
        if symlink_dir(&real, &link).is_err() {
            return;
        }
        let registry = ReplacementRegistry::new(link);
        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert!(fs::read_dir(real).expect("real registry").next().is_none());
    }

    #[test]
    fn unavailable_external_parent_blocks_only_folder_recovery_and_retry_succeeds() {
        let (_directory, parent, registry_root, registry, _old, new) =
            setup_replacement("offline-parent");
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new.clone())
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::AfterOldMoved),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);

        let offline = parent.with_file_name("offline-parent-held");
        fs::rename(&parent, &offline).expect("take external parent offline");
        let state = ProjectFolderIoState::new(registry_root.clone());
        assert_eq!(
            state.recover_pending_replacement(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert!(
            state
                .unresolved_replacement_recovery
                .load(Ordering::Acquire)
        );
        assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());

        fs::rename(&offline, &parent).expect("restore external parent");
        state
            .recover_pending_replacement()
            .expect("retry startup recovery");
        assert!(
            !state
                .unresolved_replacement_recovery
                .load(Ordering::Acquire)
        );
        assert_target(&parent, &new);
        assert_transaction_artifacts_absent(&parent, &registry_root);
    }

    #[test]
    fn failed_partial_stage_cleanup_retains_registry_locator_until_retry() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("retain-locator");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let result = PreparedReplacementProjectFolder::prepare_with_hook(
            &registry,
            &parent,
            TARGET_NAME,
            new,
            |stage| {
                stage
                    .write_child_file("unknown-sentinel", b"retain")
                    .expect("inject unknown");
            },
        );
        assert!(matches!(
            result,
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        ));
        assert!(registry_root.join(REGISTRY_RECORD_NAME).exists());
        assert_target(&parent, &old);
        let stage = parent.join(staging_name(&transaction));
        assert_eq!(
            fs::read(stage.join("unknown-sentinel")).expect("retained unknown"),
            b"retain"
        );

        fs::remove_file(stage.join("unknown-sentinel")).expect("remove injected unknown");
        registry.recover_pending().expect("retry cleanup");
        assert_transaction_artifacts_absent(&parent, &registry_root);
        assert_target(&parent, &old);
    }

    #[test]
    fn stage_registration_and_cleanup_failure_retains_pre_stage_reservation_locator() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("registration-and-cleanup-failure");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let result = PreparedReplacementProjectFolder::prepare_with_stage_registration_hook(
            &registry,
            &parent,
            TARGET_NAME,
            new,
            |stage, _registry_root| {
                stage
                    .write_child_file("unknown-sentinel", b"prevent cleanup")
                    .expect("inject cleanup failure");
                Err(ProjectFolderFilesystemError::InjectedCrash)
            },
        );
        assert!(matches!(
            result,
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        ));
        let stage = parent.join(staging_name(&transaction));
        assert_eq!(
            fs::read(stage.join("unknown-sentinel")).expect("orphan candidate retained"),
            b"prevent cleanup"
        );
        assert!(
            registry_root.join(REGISTRY_RESERVED_NAME).exists(),
            "stage must never outlive its durable native locator"
        );
        assert!(!registry_root.join(REGISTRY_STAGED_NAME).exists());
        assert_target(&parent, &old);
        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert!(registry_root.join(REGISTRY_RESERVED_NAME).exists());
    }

    #[test]
    fn crash_after_reservation_before_stage_creation_clears_only_the_reservation() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("reservation-before-stage");
        let transaction = transaction_id(new.archive().document.project_id.canonical_bytes());
        let parent_handle = PinnedDirectory::open_selected(&parent).expect("open parent");
        let old_target = parent_handle
            .open_child_directory_for_rename(TARGET_NAME)
            .expect("open old target");
        let old_manifest_sha256 = artifact_manifest_sha256(&old).expect("old manifest hash");
        let new_manifest_sha256 = artifact_manifest_sha256(&new).expect("new manifest hash");
        let old_entries = artifact_fingerprints(&old).expect("old fingerprints");
        let new_entries = artifact_fingerprints(&new).expect("new fingerprints");
        let reservation = registry
            .reserve(RegistryReservationInput {
                parent_path: &parent,
                parent: &parent_handle,
                target_name: TARGET_NAME,
                transaction_id: &transaction,
                old_manifest_sha256: &old_manifest_sha256,
                new_manifest_sha256: &new_manifest_sha256,
                old_entries: &old_entries,
                new_entries: &new_entries,
                old_directory_identity: old_target.identity(),
            })
            .expect("reserve replacement");
        drop(reservation);
        drop(old_target);
        drop(parent_handle);

        assert!(registry_root.join(REGISTRY_RESERVED_NAME).exists());
        assert!(!parent.join(staging_name(&transaction)).exists());
        registry
            .recover_pending()
            .expect("clear reservation without a stage");
        assert_target(&parent, &old);
        assert_transaction_artifacts_absent(&parent, &registry_root);
    }

    #[test]
    fn registry_unknown_field_and_oversize_content_fail_closed() {
        for (label, mutate) in [("unknown", false), ("oversize", true)] {
            let (_directory, parent, registry_root, registry, old, new) =
                setup_replacement(&format!("registry-{label}"));
            let mut prepared =
                PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                    .expect("prepare replacement");
            assert_eq!(
                prepared.publish_until(ReplacementCrashPoint::BeforePrepared),
                Err(ProjectFolderFilesystemError::InjectedCrash)
            );
            drop(prepared);
            let path = registry_root.join(REGISTRY_RECORD_NAME);
            if mutate {
                fs::write(&path, vec![b'x'; MAX_REGISTRY_BYTES as usize + 1])
                    .expect("oversize registry");
            } else {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).expect("registry"))
                        .expect("registry JSON");
                value["unknown"] = serde_json::Value::Bool(true);
                fs::write(&path, serde_json::to_vec(&value).expect("JSON")).expect("unknown field");
            }
            assert_eq!(
                registry.recover_pending(),
                Err(ProjectFolderFilesystemError::RecoveryRequired)
            );
            assert_target(&parent, &old);
            assert!(path.exists());
        }
    }

    #[test]
    fn legacy_truncated_directory_identity_is_isolated_without_migration() {
        let (_directory, parent, registry_root, registry, old, new) =
            setup_replacement("registry-legacy-identity");
        let mut prepared =
            PreparedReplacementProjectFolder::prepare(&registry, &parent, TARGET_NAME, new)
                .expect("prepare replacement");
        assert_eq!(
            prepared.publish_until(ReplacementCrashPoint::BeforePrepared),
            Err(ProjectFolderFilesystemError::InjectedCrash)
        );
        drop(prepared);

        let path = registry_root.join(REGISTRY_RECORD_NAME);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("registry")).expect("registry JSON");
        value["parent_identity"]
            .as_object_mut()
            .expect("directory identity object")
            .remove("third");
        fs::write(&path, serde_json::to_vec(&value).expect("legacy JSON"))
            .expect("write truncated identity");

        assert_eq!(
            registry.recover_pending(),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
        assert_target(&parent, &old);
        assert!(path.exists(), "unmigrated registry evidence must remain");
    }

    #[test]
    fn registry_fingerprints_enforce_each_role_hard_limit() {
        let (_old, new) = artifacts();
        let mut fingerprints = artifact_fingerprints(&new).expect("fingerprints");
        fingerprints
            .entries
            .iter_mut()
            .find(|entry| entry.path == PROJECT_FOLDER_PROJECT_PATH)
            .expect("project fingerprint")
            .size = MAX_PROJECT_JSON_BYTES as u64 + 1;

        assert_eq!(
            validate_artifact_fingerprints(&fingerprints),
            Err(ProjectFolderFilesystemError::RecoveryRequired)
        );
    }
}
