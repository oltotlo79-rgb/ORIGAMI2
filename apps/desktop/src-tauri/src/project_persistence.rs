use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, UNIX_EPOCH},
};

type ProjectFileOperationOwner = u64;

static ACTIVE_PROJECT_FILE_OPERATIONS: OnceLock<Mutex<HashMap<String, ProjectFileOperationOwner>>> =
    OnceLock::new();
static NEXT_PROJECT_FILE_OPERATION_OWNER: AtomicU64 = AtomicU64::new(1);

struct ProjectFileOperationGuard {
    keys: Vec<String>,
    owner: ProjectFileOperationOwner,
}

impl Drop for ProjectFileOperationGuard {
    fn drop(&mut self) {
        let mut active = lock_active_project_file_operations();
        for key in &self.keys {
            if active.get(key) == Some(&self.owner) {
                active.remove(key);
            }
        }
    }
}

fn acquire_project_file_operation(path: &Path) -> Result<ProjectFileOperationGuard, ()> {
    acquire_project_file_operation_with_pre_identity_open_hook(path, || Ok(()))
}

fn acquire_project_file_operation_with_pre_identity_open_hook(
    path: &Path,
    pre_identity_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> Result<ProjectFileOperationGuard, ()> {
    let keys =
        project_file_operation_keys_with_pre_identity_open_hook(path, pre_identity_open_hook)?;
    let mut active = lock_active_project_file_operations();
    if keys.iter().any(|key| active.contains_key(key)) {
        return Err(());
    }
    let owner = next_project_file_operation_owner()?;
    for key in &keys {
        active.insert(key.clone(), owner);
    }
    Ok(ProjectFileOperationGuard { keys, owner })
}

fn lock_active_project_file_operations()
-> std::sync::MutexGuard<'static, HashMap<String, ProjectFileOperationOwner>> {
    let mutex = ACTIVE_PROJECT_FILE_OPERATIONS.get_or_init(|| Mutex::new(HashMap::new()));
    match mutex.lock() {
        Ok(active) => active,
        Err(poisoned) => {
            let active = poisoned.into_inner();
            mutex.clear_poison();
            active
        }
    }
}

fn next_project_file_operation_owner() -> Result<ProjectFileOperationOwner, ()> {
    NEXT_PROJECT_FILE_OPERATION_OWNER
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |owner| {
            owner.checked_add(1)
        })
        .map_err(|_| ())
}

fn project_file_operation_keys_with_pre_identity_open_hook(
    path: &Path,
    pre_identity_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> Result<Vec<String>, ()> {
    let path_key = format!("path:{}", target_path_fingerprint(path)?);
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            let identity_key = project_file_identity_key_with_pre_open_hook(
                path,
                &metadata,
                pre_identity_open_hook,
            )?;
            Ok(vec![path_key, identity_key])
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(vec![path_key]),
        Err(_) => Err(()),
    }
}

fn project_file_identity_key_with_pre_open_hook(
    path: &Path,
    entry_metadata: &std::fs::Metadata,
    pre_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> Result<String, ()> {
    let (file, opened_metadata) = open_inspected_regular_file_no_follow_with_pre_open_hook(
        path,
        entry_metadata,
        false,
        pre_open_hook,
    )
    .map_err(|_| ())?;
    let FileSystemObjectIdentityV1(first, second, third) =
        regular_file_identity_v1(&file, &opened_metadata).ok_or(())?;
    Ok(format!("file:{first}:{second}:{third}"))
}

use ori_domain::ProjectId;
#[cfg(test)]
use ori_formats::ProjectDocument;
use ori_formats::{
    Ori2Limits, Ori2ProjectArchive, read_project_archive_ori2_with_limits,
    write_project_archive_ori2,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
const SINGLE_FILE_JOURNAL_SCHEMA_V1: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SingleFileJournalPhaseV1 {
    Prepared,
    OldMoved,
    NewPublished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SingleFileJournalPayloadV1 {
    schema_version: u32,
    project_id: ProjectId,
    target_path_sha256: String,
    transaction_id: String,
    temp_object_id: String,
    temp_sha256: String,
    backup_object_id: String,
    old_sha256: Option<String>,
    phase: SingleFileJournalPhaseV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthenticatedSingleFileJournalV1 {
    payload: SingleFileJournalPayloadV1,
    payload_sha256: String,
}

fn encode_single_file_journal_v1(
    payload: SingleFileJournalPayloadV1,
) -> Result<Vec<u8>, serde_json::Error> {
    let canonical = serde_json::to_vec(&payload)?;
    serde_json::to_vec_pretty(&AuthenticatedSingleFileJournalV1 {
        payload,
        payload_sha256: sha256_hex_bytes(&canonical),
    })
}

fn decode_single_file_journal_v1(
    bytes: &[u8],
    expected_project_id: ProjectId,
    expected_target_path_sha256: &str,
) -> Result<SingleFileJournalPayloadV1, ()> {
    let journal: AuthenticatedSingleFileJournalV1 =
        serde_json::from_slice(bytes).map_err(|_| ())?;
    let canonical = serde_json::to_vec(&journal.payload).map_err(|_| ())?;
    let payload = journal.payload;
    let valid = payload.schema_version == SINGLE_FILE_JOURNAL_SCHEMA_V1
        && payload.project_id == expected_project_id
        && payload.target_path_sha256 == expected_target_path_sha256
        && journal.payload_sha256 == sha256_hex_bytes(&canonical)
        && is_lowercase_sha256(&payload.target_path_sha256)
        && is_lowercase_sha256(&payload.temp_sha256)
        && payload
            .old_sha256
            .as_deref()
            .is_none_or(is_lowercase_sha256)
        && is_safe_transaction_component(&payload.transaction_id)
        && is_safe_recovery_object_component(&payload.temp_object_id)
        && is_safe_recovery_object_component(&payload.backup_object_id)
        && !payload
            .temp_object_id
            .eq_ignore_ascii_case(&payload.backup_object_id);
    valid.then_some(payload).ok_or(())
}

fn sha256_hex_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_safe_transaction_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
}

fn is_windows_reserved_device_component(value: &str) -> bool {
    let stem = value
        .trim_end_matches(['.', ' '])
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

fn is_safe_recovery_object_component(value: &str) -> bool {
    is_safe_transaction_component(value) && !is_windows_reserved_device_component(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SingleFileRecoveryObject {
    Target,
    Temp,
    Backup,
    Journal,
}

trait SingleFileRecoveryFs {
    fn object_sha256(&self, object: SingleFileRecoveryObject) -> Result<Option<String>, ()>;
    fn rename_object(
        &mut self,
        from: SingleFileRecoveryObject,
        to: SingleFileRecoveryObject,
    ) -> Result<(), ()>;
    fn remove_object(&mut self, object: SingleFileRecoveryObject) -> Result<(), ()>;
    fn sync_directory(&mut self) -> Result<(), ()>;
}

fn recover_authenticated_single_file_v1(
    fs: &mut impl SingleFileRecoveryFs,
    journal: &SingleFileJournalPayloadV1,
) -> Result<(), ()> {
    let target = fs.object_sha256(SingleFileRecoveryObject::Target)?;
    let temp = fs.object_sha256(SingleFileRecoveryObject::Temp)?;
    let backup = fs.object_sha256(SingleFileRecoveryObject::Backup)?;
    let expected_old = journal.old_sha256.as_ref();
    let old_matches = |actual: &Option<String>| actual.as_ref() == expected_old;

    let before_old_move =
        old_matches(&target) && temp.as_ref() == Some(&journal.temp_sha256) && backup.is_none();
    let rollback_complete = old_matches(&target) && temp.is_none() && backup.is_none();
    let before_publish =
        target.is_none() && temp.as_ref() == Some(&journal.temp_sha256) && old_matches(&backup);
    let after_publish = target.as_ref() == Some(&journal.temp_sha256)
        && temp.is_none()
        && (old_matches(&backup) || backup.is_none());
    if before_old_move {
        fs.remove_object(SingleFileRecoveryObject::Temp)?;
        fs.sync_directory()?;
    } else if rollback_complete {
        // A previous recovery removed the private stage and was interrupted
        // before unlinking the journal.
    } else if before_publish {
        fs.rename_object(
            SingleFileRecoveryObject::Temp,
            SingleFileRecoveryObject::Target,
        )?;
        fs.sync_directory()?;
        fs.remove_object(SingleFileRecoveryObject::Backup)?;
    } else if after_publish {
        if backup.is_some() {
            fs.remove_object(SingleFileRecoveryObject::Backup)?;
        }
    } else {
        return Err(());
    }
    fs.remove_object(SingleFileRecoveryObject::Journal)?;
    fs.sync_directory()
}

struct DiskSingleFileRecoveryFs {
    directory: PathBuf,
    directory_identity: ProjectDirectoryIdentity,
    authenticated_objects:
        RefCell<HashMap<SingleFileRecoveryObject, AuthenticatedRecoveryObjectV1>>,
    target: PathBuf,
    temp: PathBuf,
    backup: PathBuf,
    journal: PathBuf,
}

impl DiskSingleFileRecoveryFs {
    fn path(&self, object: SingleFileRecoveryObject) -> &Path {
        match object {
            SingleFileRecoveryObject::Target => &self.target,
            SingleFileRecoveryObject::Temp => &self.temp,
            SingleFileRecoveryObject::Backup => &self.backup,
            SingleFileRecoveryObject::Journal => &self.journal,
        }
    }

    fn object_sha256_with_pre_open_hook(
        &self,
        object: SingleFileRecoveryObject,
        pre_open_hook: impl FnOnce() -> std::io::Result<()>,
    ) -> Result<Option<String>, ()> {
        self.verify_directory_identity()?;
        self.authenticated_objects.borrow_mut().remove(&object);
        let path = self.path(object);
        let entry_metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(()),
        };
        let (hash, file, opened_metadata) =
            hash_inspected_regular_file_no_follow_with_pre_open_hook(
                path,
                &entry_metadata,
                object != SingleFileRecoveryObject::Target,
                pre_open_hook,
            )
            .map_err(|_| ())?;
        let authenticated = authenticated_regular_file_v1(&file, &opened_metadata).ok_or(())?;
        self.authenticated_objects.borrow_mut().insert(
            object,
            AuthenticatedRecoveryObjectV1 {
                authenticated,
                sha256: hash.clone(),
            },
        );
        Ok(Some(hash))
    }

    fn rename_object_with_pre_rename_hook(
        &mut self,
        from: SingleFileRecoveryObject,
        to: SingleFileRecoveryObject,
        pre_rename_hook: impl FnOnce() -> std::io::Result<()>,
    ) -> Result<(), ()> {
        if from == to {
            return Err(());
        }
        self.verify_directory_identity()?;
        let authenticated_source = self
            .authenticated_objects
            .borrow()
            .get(&from)
            .cloned()
            .ok_or(())?;
        let source = self.path(from);
        let destination = self.path(to);
        let entry_metadata = std::fs::symlink_metadata(source).map_err(|_| ())?;
        let require_single_link = from != SingleFileRecoveryObject::Target;
        if require_single_link && authenticated_source.authenticated.link_count != 1 {
            return Err(());
        }
        let mut source_file =
            open_regular_file_for_authenticated_rename_no_follow_v1(source).map_err(|_| ())?;
        let opened_metadata = source_file.metadata().map_err(|_| ())?;
        if !regular_file_metadata_matches_v1(&entry_metadata, &opened_metadata, require_single_link)
            || authenticated_regular_file_v1(&source_file, &opened_metadata)
                != Some(authenticated_source.authenticated)
        {
            return Err(());
        }
        revalidate_opened_regular_file_content_v1(
            &mut source_file,
            authenticated_source.authenticated,
            &authenticated_source.sha256,
            require_single_link,
        )
        .map_err(|_| ())?;

        pre_rename_hook().map_err(|_| ())?;
        self.verify_directory_identity()?;
        revalidate_opened_regular_file_content_v1(
            &mut source_file,
            authenticated_source.authenticated,
            &authenticated_source.sha256,
            require_single_link,
        )
        .map_err(|_| ())?;
        revalidate_path_against_authenticated_regular_file_v1(
            source,
            authenticated_source.authenticated,
            require_single_link,
        )
        .map_err(|_| ())?;
        // Destination no-replace is atomic on supported platforms. Windows
        // renames the validated handle itself; Unix exposes only a path-based
        // no-replace primitive here, so the immediate source recheck and the
        // postconditions detect, but cannot roll back, a last-instant source
        // name race.
        rename_opened_regular_file_no_replace_v1(
            source,
            destination,
            &source_file,
            self.directory_identity,
        )?;

        match std::fs::symlink_metadata(source) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(()),
        }
        revalidate_opened_regular_file_content_v1(
            &mut source_file,
            authenticated_source.authenticated,
            &authenticated_source.sha256,
            require_single_link,
        )
        .map_err(|_| ())?;
        revalidate_path_against_authenticated_regular_file_v1(
            destination,
            authenticated_source.authenticated,
            require_single_link,
        )
        .map_err(|_| ())?;
        drop(source_file);
        let mut authenticated_objects = self.authenticated_objects.borrow_mut();
        authenticated_objects.remove(&from);
        authenticated_objects.insert(to, authenticated_source);
        drop(authenticated_objects);
        self.verify_directory_identity()
    }

    fn remove_object_with_pre_delete_hook(
        &mut self,
        object: SingleFileRecoveryObject,
        pre_delete_hook: impl FnOnce() -> std::io::Result<()>,
    ) -> Result<(), ()> {
        self.verify_directory_identity()?;
        let path = self.path(object);
        let entry_metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(()),
        };
        let require_single_link = object != SingleFileRecoveryObject::Target;
        let (mut opened_file, opened_metadata) =
            open_inspected_regular_file_no_follow_with_pre_open_hook(
                path,
                &entry_metadata,
                require_single_link,
                || Ok(()),
            )
            .map_err(|_| ())?;
        let (opened_sha256, hashed_metadata) = hash_opened_regular_file_from_start_v1(
            &mut opened_file,
            &opened_metadata,
            require_single_link,
        )
        .map_err(|_| ())?;
        let authenticated =
            authenticated_regular_file_v1(&opened_file, &hashed_metadata).ok_or(())?;
        let cached_authenticated = self.authenticated_objects.borrow().get(&object).cloned();
        if cached_authenticated.as_ref().is_some_and(|cached| {
            cached.authenticated != authenticated || cached.sha256 != opened_sha256
        }) {
            return Err(());
        }
        let authenticated = cached_authenticated.unwrap_or(AuthenticatedRecoveryObjectV1 {
            authenticated,
            sha256: opened_sha256,
        });
        pre_delete_hook().map_err(|_| ())?;
        self.verify_directory_identity()?;
        revalidate_opened_regular_file_content_v1(
            &mut opened_file,
            authenticated.authenticated,
            &authenticated.sha256,
            require_single_link,
        )
        .map_err(|_| ())?;
        revalidate_path_against_authenticated_regular_file_v1(
            path,
            authenticated.authenticated,
            require_single_link,
        )
        .map_err(|_| ())?;

        // Rust has no portable handle-bound unlink operation. Keep the
        // no-follow handle alive, revalidate the named entry immediately
        // before unlinking it, and fail unless the name is absent afterward.
        // This narrows but does not claim to eliminate the final path race.
        std::fs::remove_file(path).map_err(|_| ())?;
        match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(()),
        }
        drop(opened_file);
        self.authenticated_objects.borrow_mut().remove(&object);
        self.verify_directory_identity()
    }

    fn verify_directory_identity(&self) -> Result<(), ()> {
        (project_directory_identity(&self.directory)? == self.directory_identity)
            .then_some(())
            .ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileSystemObjectIdentityV1(u64, u64, u64);

type ProjectDirectoryIdentity = FileSystemObjectIdentityV1;

#[cfg(any(target_os = "windows", test))]
fn windows_file_identity_from_full_file_id_v1(
    volume_serial_number: u64,
    identifier: [u8; 16],
) -> Option<FileSystemObjectIdentityV1> {
    if volume_serial_number == 0
        || identifier.iter().all(|byte| *byte == 0)
        || identifier.iter().all(|byte| *byte == u8::MAX)
    {
        return None;
    }
    let low = u64::from_le_bytes(identifier[..8].try_into().ok()?);
    let high = u64::from_le_bytes(identifier[8..].try_into().ok()?);
    Some(FileSystemObjectIdentityV1(volume_serial_number, low, high))
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy)]
struct WindowsHandleInformationV1 {
    identity: FileSystemObjectIdentityV1,
    attributes: u32,
    link_count: u64,
}

#[cfg(target_os = "windows")]
fn windows_handle_information_v1(file: &File) -> Option<WindowsHandleInformationV1> {
    use std::{
        mem::{MaybeUninit, size_of},
        os::windows::io::AsRawHandle,
        ptr,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ID_INFO, FileIdInfo, GetFileInformationByHandle,
        GetFileInformationByHandleEx,
    };

    let mut legacy_information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the output storage and retained handle are valid for the call.
    if unsafe {
        GetFileInformationByHandle(file.as_raw_handle() as _, legacy_information.as_mut_ptr())
    } == 0
    {
        return None;
    }
    // SAFETY: success initialized the structure.
    let legacy_information = unsafe { legacy_information.assume_init() };

    let mut full_identity = FILE_ID_INFO::default();
    let full_identity_size = u32::try_from(size_of::<FILE_ID_INFO>()).ok()?;
    // SAFETY: the retained handle is valid and `full_identity` provides
    // writable storage of exactly the size passed to the API.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle() as _,
            FileIdInfo,
            ptr::addr_of_mut!(full_identity).cast(),
            full_identity_size,
        )
    } == 0
    {
        return None;
    }
    let identity = windows_file_identity_from_full_file_id_v1(
        full_identity.VolumeSerialNumber,
        full_identity.FileId.Identifier,
    )?;
    Some(WindowsHandleInformationV1 {
        identity,
        attributes: legacy_information.dwFileAttributes,
        link_count: u64::from(legacy_information.nNumberOfLinks),
    })
}

#[cfg(unix)]
fn project_directory_identity(path: &Path) -> Result<ProjectDirectoryIdentity, ()> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
    let metadata = options
        .open(path)
        .map_err(|_| ())?
        .metadata()
        .map_err(|_| ())?;
    if !metadata.file_type().is_dir() {
        return Err(());
    }
    Ok(FileSystemObjectIdentityV1(
        metadata.dev(),
        metadata.ino(),
        0,
    ))
}

#[cfg(target_os = "windows")]
fn project_directory_identity(path: &Path) -> Result<ProjectDirectoryIdentity, ()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(());
    }
    let mut options = OpenOptions::new();
    options
        .access_mode(FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    let directory = options.open(path).map_err(|_| ())?;
    project_directory_identity_from_handle(&directory)
}

#[cfg(target_os = "windows")]
fn project_directory_identity_from_handle(
    directory: &File,
) -> Result<ProjectDirectoryIdentity, ()> {
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;

    let information = windows_handle_information_v1(directory).ok_or(())?;
    if information.attributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || information.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(());
    }
    Ok(information.identity)
}

impl SingleFileRecoveryFs for DiskSingleFileRecoveryFs {
    fn object_sha256(&self, object: SingleFileRecoveryObject) -> Result<Option<String>, ()> {
        self.object_sha256_with_pre_open_hook(object, || Ok(()))
    }

    fn rename_object(
        &mut self,
        from: SingleFileRecoveryObject,
        to: SingleFileRecoveryObject,
    ) -> Result<(), ()> {
        self.rename_object_with_pre_rename_hook(from, to, || Ok(()))
    }

    fn remove_object(&mut self, object: SingleFileRecoveryObject) -> Result<(), ()> {
        self.remove_object_with_pre_delete_hook(object, || Ok(()))
    }

    fn sync_directory(&mut self) -> Result<(), ()> {
        self.verify_directory_identity()?;
        sync_project_directory(&self.directory).map_err(|_| ())
    }
}

fn target_path_fingerprint(path: &Path) -> Result<String, ()> {
    let parent = containing_directory(path).ok_or(())?;
    let canonical_parent = std::fs::canonicalize(parent).map_err(|_| ())?;
    let file_name = path.file_name().and_then(|name| name.to_str()).ok_or(())?;
    let normalized = canonical_parent
        .join(file_name)
        .to_string_lossy()
        .nfc()
        .collect::<String>();
    #[cfg(windows)]
    let normalized = normalized.to_lowercase();
    Ok(sha256_hex_bytes(normalized.as_bytes()))
}

fn journal_path_for_target(path: &Path, target_fingerprint: &str) -> Result<PathBuf, ()> {
    let parent = containing_directory(path).ok_or(())?;
    Ok(parent.join(format!(
        ".origami2-journal-{}.json",
        &target_fingerprint[..32]
    )))
}

fn recover_single_file_journal_for_target(
    target: &Path,
    expected_project_id: ProjectId,
) -> Result<(), ()> {
    recover_single_file_journal_for_target_inner(target, Some(expected_project_id))
}

fn recover_single_file_journal_for_open(target: &Path) -> Result<(), ()> {
    recover_single_file_journal_for_target_inner(target, None)
}

fn recover_single_file_journal_for_target_inner(
    target: &Path,
    expected_project_id: Option<ProjectId>,
) -> Result<(), ()> {
    recover_single_file_journal_for_target_inner_with_pre_open_hook(
        target,
        expected_project_id,
        || Ok(()),
    )
}

fn recover_single_file_journal_for_target_inner_with_pre_open_hook(
    target: &Path,
    expected_project_id: Option<ProjectId>,
    pre_journal_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> Result<(), ()> {
    const MAX_JOURNAL_BYTES: u64 = 64 * 1024;

    let fingerprint = target_path_fingerprint(target)?;
    let journal_path = journal_path_for_target(target, &fingerprint)?;
    let entry_metadata = match std::fs::symlink_metadata(&journal_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(()),
    };
    if entry_metadata.len() > MAX_JOURNAL_BYTES {
        return Err(());
    }
    let (mut journal_file, opened_metadata) =
        open_inspected_regular_file_no_follow_for_stable_read_with_pre_open_hook(
            &journal_path,
            &entry_metadata,
            true,
            pre_journal_open_hook,
        )
        .map_err(|_| ())?;
    let bytes = read_opened_regular_file_bounded_stably_with_post_read_hook_v1(
        &journal_path,
        &mut journal_file,
        &opened_metadata,
        true,
        MAX_JOURNAL_BYTES,
        || Ok(()),
    )
    .map_err(|_| ())?;
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(());
    }
    let untrusted: AuthenticatedSingleFileJournalV1 =
        serde_json::from_slice(&bytes).map_err(|_| ())?;
    let project_id = expected_project_id.unwrap_or(untrusted.payload.project_id);
    let payload = decode_single_file_journal_v1(&bytes, project_id, &fingerprint)?;
    let directory = containing_directory(target).ok_or(())?.to_path_buf();
    let directory_identity = project_directory_identity(&directory)?;
    let mut fs = DiskSingleFileRecoveryFs {
        authenticated_objects: Default::default(),
        temp: directory.join(&payload.temp_object_id),
        backup: directory.join(&payload.backup_object_id),
        journal: journal_path,
        target: target.to_path_buf(),
        directory_identity,
        directory,
    };
    recover_authenticated_single_file_v1(&mut fs, &payload)
}

fn persist_single_file_journal_phase(
    target: &Path,
    payload: &SingleFileJournalPayloadV1,
    create_new: bool,
) -> Result<PathBuf, ()> {
    let directory = containing_directory(target).ok_or(())?;
    let directory_identity = project_directory_identity(directory)?;
    let journal = journal_path_for_target(target, &payload.target_path_sha256)?;
    if !create_new {
        let metadata = std::fs::symlink_metadata(&journal).map_err(|_| ())?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(());
        }
    }
    let bytes = encode_single_file_journal_v1(payload.clone()).map_err(|_| ())?;
    let write_path = if create_new {
        journal.clone()
    } else {
        containing_directory(target).ok_or(())?.join(format!(
            ".origami2-journal-update-{}-{}",
            std::process::id(),
            NEXT_STAGED_FILE_ID.fetch_add(1, Ordering::Relaxed)
        ))
    };
    let mut write_path_created = false;
    let write_result = (|| {
        if project_directory_identity(directory)? != directory_identity {
            return Err(());
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options.open(&write_path).map_err(|_| ())?;
        write_path_created = true;
        write_complete_staged_payload(&mut file, &bytes).map_err(|_| ())?;
        file.sync_all().map_err(|_| ())?;
        drop(file);
        if !create_new {
            if project_directory_identity(directory)? != directory_identity {
                return Err(());
            }
            std::fs::rename(&write_path, &journal).map_err(|_| ())?;
        }
        if project_directory_identity(directory)? != directory_identity {
            return Err(());
        }
        sync_project_directory(directory).map_err(|_| ())
    })();
    if write_result.is_err()
        && write_path_created
        && project_directory_identity(directory).is_ok_and(|current| current == directory_identity)
    {
        let _ = std::fs::remove_file(&write_path);
    }
    write_result?;
    Ok(journal)
}

#[cfg(not(target_os = "windows"))]
fn sync_project_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(target_os = "windows")]
fn sync_project_directory(path: &Path) -> std::io::Result<()> {
    let expected_identity = project_directory_identity(path)
        .map_err(|()| std::io::Error::other("directory identity is unavailable"))?;
    let mut options = OpenOptions::new();
    options
        .access_mode(FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | FILE_ADD_FILE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    let directory = options.open(path)?;
    if project_directory_identity_from_handle(&directory)
        .map_err(|()| std::io::Error::other("opened directory identity is unavailable"))?
        != expected_identity
    {
        return Err(std::io::Error::other("save directory changed"));
    }
    super::project_folder_io::platform::flush_directory_handle(&directory)?;
    if project_directory_identity(path)
        .map_err(|()| std::io::Error::other("directory identity changed after synchronization"))?
        != expected_identity
    {
        return Err(std::io::Error::other("save directory changed"));
    }
    Ok(())
}

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::{ffi::OsStrExt, fs::OpenOptionsExt};
#[cfg(target_os = "windows")]
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

use super::{
    save_path::{DialogSaveDestination, ExistingDestinationPolicy},
    validate_document_instruction_poses,
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_ADD_FILE, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_LIST_DIRECTORY,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

pub(super) const PROJECT_FILE_OPEN_FAILED_MESSAGE: &str =
    "選択されたプロジェクトファイルを開けませんでした。";
pub(super) const PROJECT_FILE_INSPECTION_FAILED_MESSAGE: &str =
    "選択されたプロジェクトファイルのサイズを確認できませんでした。";
pub(super) const PROJECT_FILE_TOO_LARGE_MESSAGE: &str =
    "選択されたプロジェクトファイルはサイズ上限を超えています。";
pub(super) const PROJECT_FILE_READ_FAILED_MESSAGE: &str =
    "選択されたプロジェクトファイルを読み込めませんでした。";
pub(super) const PROJECT_FILE_INVALID_MESSAGE: &str =
    "選択されたプロジェクトファイルが破損しているか、対応していない形式です。";
pub(super) const PROJECT_INSTRUCTIONS_INVALID_MESSAGE: &str =
    "プロジェクト内の折り手順データを検証できませんでした。";
pub(super) const PROJECT_INSTRUCTIONS_SAVE_FAILED_MESSAGE: &str =
    "プロジェクト内の折り手順データを安全に保存できませんでした。";
pub(super) const PROJECT_SERIALIZATION_FAILED_MESSAGE: &str =
    "プロジェクトの保存データを作成できませんでした。";
const PROJECT_REPLACE_SAVE_FAILED_MESSAGE: &str =
    "プロジェクトを保存先へ安全に確定できなかったため、保存を中止しました。";

static NEXT_STAGED_FILE_ID: AtomicU64 = AtomicU64::new(0);
pub(super) const FRONTEND_MAX_SAFE_INTEGER_U64: u64 = (1_u64 << 53) - 1;
const RECOVERY_QUARANTINE_NAMES: [&str; 8] = [
    ".origami2-recovery-invalid-00",
    ".origami2-recovery-invalid-01",
    ".origami2-recovery-invalid-02",
    ".origami2-recovery-invalid-03",
    ".origami2-recovery-invalid-04",
    ".origami2-recovery-invalid-05",
    ".origami2-recovery-invalid-06",
    ".origami2-recovery-invalid-07",
];

/// Bounded, redacted result used by the private crash-recovery slot.
///
/// This type deliberately carries neither a path nor an underlying I/O or
/// parser error. Recovery diagnostics must not accidentally expose app-data
/// locations or raw operating-system details to the WebView.
#[derive(Debug, PartialEq)]
pub(super) enum RecoveryProjectLoad {
    Missing,
    Available {
        project: Box<Ori2ProjectArchive>,
        updated_at_unix_ms: Option<u64>,
    },
    Invalid,
}

/// Opaque persistence failure for crash-recovery storage.
///
/// The ordinary Save As path returns localized user-facing errors. Recovery
/// runs in the background, so its boundary intentionally erases raw errors and
/// lets the caller expose one fixed status instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RecoveryPersistenceError;

pub(super) fn load_project_archive_from_path(path: &Path) -> Result<Ori2ProjectArchive, String> {
    load_project_archive_from_path_with_pre_open_hook(path, || Ok(()))
}

fn load_project_archive_from_path_with_pre_open_hook(
    path: &Path,
    pre_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> Result<Ori2ProjectArchive, String> {
    let _operation = acquire_project_file_operation(path)
        .map_err(|()| PROJECT_FILE_OPEN_FAILED_MESSAGE.to_owned())?;
    recover_single_file_journal_for_open(path)
        .map_err(|_| PROJECT_FILE_INVALID_MESSAGE.to_owned())?;
    let limits = Ori2Limits::default();
    let entry_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PROJECT_FILE_OPEN_FAILED_MESSAGE.to_owned()
        } else {
            PROJECT_FILE_INSPECTION_FAILED_MESSAGE.to_owned()
        }
    })?;
    if !metadata_is_plain_regular_file(&entry_metadata) {
        return Err(PROJECT_FILE_OPEN_FAILED_MESSAGE.to_owned());
    }
    let declared_size = entry_metadata.len();
    if declared_size > limits.max_archive_size {
        return Err(PROJECT_FILE_TOO_LARGE_MESSAGE.to_owned());
    }
    let (mut file, opened_metadata) =
        open_inspected_regular_file_no_follow_for_stable_read_with_pre_open_hook(
            path,
            &entry_metadata,
            false,
            pre_open_hook,
        )
        .map_err(|_| PROJECT_FILE_OPEN_FAILED_MESSAGE.to_owned())?;
    if opened_metadata.len() > limits.max_archive_size {
        return Err(PROJECT_FILE_TOO_LARGE_MESSAGE.to_owned());
    }

    let bytes = read_opened_regular_file_bounded_stably_with_post_read_hook_v1(
        path,
        &mut file,
        &opened_metadata,
        false,
        limits.max_archive_size,
        || Ok(()),
    )
    .map_err(|_| PROJECT_FILE_READ_FAILED_MESSAGE.to_owned())?;
    if bytes.len() as u64 > limits.max_archive_size {
        return Err(PROJECT_FILE_TOO_LARGE_MESSAGE.to_owned());
    }

    let project = read_project_archive_ori2_with_limits(&bytes, limits)
        .map_err(|_| PROJECT_FILE_INVALID_MESSAGE.to_owned())?;
    validate_document_instruction_poses(&project.document)
        .map_err(|_| PROJECT_INSTRUCTIONS_INVALID_MESSAGE.to_owned())?;
    Ok(project)
}

#[cfg(test)]
pub(super) fn load_document_from_path(path: &Path) -> Result<ProjectDocument, String> {
    load_project_archive_from_path(path).map(|project| project.document)
}

pub(super) fn inspect_recovery_project(path: &Path) -> RecoveryProjectLoad {
    inspect_recovery_project_with_pre_open_hook(path, || Ok(()))
}

fn inspect_recovery_project_with_pre_open_hook(
    path: &Path,
    pre_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> RecoveryProjectLoad {
    let limits = Ori2Limits::default();
    let entry_metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata_is_plain_regular_file(&metadata) => metadata,
        Ok(_) => return RecoveryProjectLoad::Invalid,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return RecoveryProjectLoad::Missing;
        }
        Err(_) => return RecoveryProjectLoad::Invalid,
    };
    if entry_metadata.len() > limits.max_archive_size {
        return RecoveryProjectLoad::Invalid;
    }

    let (mut file, metadata) =
        match open_inspected_regular_file_no_follow_for_stable_read_with_pre_open_hook(
            path,
            &entry_metadata,
            false,
            pre_open_hook,
        ) {
            Ok(opened) => opened,
            Err(_) => return RecoveryProjectLoad::Invalid,
        };
    if metadata.len() > limits.max_archive_size {
        return RecoveryProjectLoad::Invalid;
    }
    let updated_at_unix_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(frontend_safe_unix_millis);
    let bytes = match read_opened_regular_file_bounded_stably_with_post_read_hook_v1(
        path,
        &mut file,
        &metadata,
        false,
        limits.max_archive_size,
        || Ok(()),
    ) {
        Ok(bytes) if bytes.len() as u64 <= limits.max_archive_size => bytes,
        Ok(_) | Err(_) => return RecoveryProjectLoad::Invalid,
    };
    let Ok(project) = read_project_archive_ori2_with_limits(&bytes, limits) else {
        return RecoveryProjectLoad::Invalid;
    };
    if validate_document_instruction_poses(&project.document).is_err() {
        return RecoveryProjectLoad::Invalid;
    }
    RecoveryProjectLoad::Available {
        project: Box::new(project),
        updated_at_unix_ms,
    }
}

#[cfg(unix)]
pub(super) fn open_regular_file_no_follow(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    options.open(path)
}

#[cfg(target_os = "windows")]
pub(super) fn open_regular_file_no_follow(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .access_mode(FILE_GENERIC_READ)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

#[cfg(not(any(unix, target_os = "windows")))]
pub(super) fn open_regular_file_no_follow(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(not(target_os = "windows"))]
fn open_regular_file_no_follow_for_stable_read_v1(path: &Path) -> std::io::Result<File> {
    open_regular_file_no_follow(path)
}

#[cfg(target_os = "windows")]
fn open_regular_file_no_follow_for_stable_read_v1(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .access_mode(FILE_GENERIC_READ)
        // Keep delete sharing so identity-swap detection remains testable and
        // fail-closed, but deny writer sharing for the complete read window.
        // Save/rename handles continue to use `open_regular_file_no_follow`.
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

pub(super) fn metadata_is_plain_regular_file(metadata: &std::fs::Metadata) -> bool {
    if !metadata.file_type().is_file() {
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    true
}

pub(super) fn frontend_safe_unix_millis(duration: Duration) -> Option<u64> {
    let milliseconds = u64::try_from(duration.as_millis()).ok()?;
    (milliseconds <= FRONTEND_MAX_SAFE_INTEGER_U64).then_some(milliseconds)
}

/// Atomically replaces the private recovery slot with a verified `.ori2`.
///
/// Callers pass a detached [`Ori2ProjectArchive`], so no live project mutex needs
/// to remain held while serialization, synchronization, verification, and
/// publication perform filesystem I/O.
pub(super) fn persist_recovery_project(
    path: &Path,
    project: &Ori2ProjectArchive,
) -> Result<(), RecoveryPersistenceError> {
    let mut staged = prepare_recovery_staged_file(path, project)?;
    publish_recovery_staged_file(&mut staged, path)
}

pub(super) fn clear_recovery_document(path: &Path) -> Result<(), RecoveryPersistenceError> {
    // The recovery directory is private application storage. This boundary
    // deliberately guarantees no-follow behavior for the final component;
    // it does not claim to pin or authenticate every ancestor directory.
    if path.file_name().is_none() {
        return Err(RecoveryPersistenceError);
    }
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(RecoveryPersistenceError),
    }

    let parent = containing_directory(path).ok_or(RecoveryPersistenceError)?;
    sync_recovery_directory(parent)?;

    for quarantine_name in RECOVERY_QUARANTINE_NAMES {
        let quarantine = parent.join(quarantine_name);
        if quarantine == path {
            continue;
        }
        match remove_known_quarantine_entry(&quarantine) {
            QuarantineCleanup::Vacant => {}
            QuarantineCleanup::Removed => sync_recovery_directory(parent)?,
            QuarantineCleanup::Occupied => continue,
        }

        match rename_recovery_entry_no_replace(path, &quarantine)? {
            RecoveryRename::Renamed => {
                sync_recovery_directory(parent)?;
                if matches!(
                    remove_known_quarantine_entry(&quarantine),
                    QuarantineCleanup::Removed
                ) {
                    sync_recovery_directory(parent)?;
                }
                return Ok(());
            }
            RecoveryRename::SourceMissing => return Ok(()),
            RecoveryRename::DestinationExists => continue,
        }
    }

    Err(RecoveryPersistenceError)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryRename {
    Renamed,
    SourceMissing,
    DestinationExists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuarantineCleanup {
    Vacant,
    Removed,
    Occupied,
}

fn remove_known_quarantine_entry(path: &Path) -> QuarantineCleanup {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return QuarantineCleanup::Vacant;
        }
        Err(_) => return QuarantineCleanup::Occupied,
    };

    let result = if entry_requires_directory_unlink(&metadata) {
        // `remove_dir` maps to an entry-only rmdir operation. It cannot remove
        // a non-empty real directory and removes a Windows junction itself,
        // never the directory to which that junction redirects.
        std::fs::remove_dir(path)
    } else {
        // `remove_file` unlinks the named regular/link/special entry. The
        // final component is never traversed.
        std::fs::remove_file(path)
    };
    match result {
        Ok(()) => QuarantineCleanup::Removed,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => QuarantineCleanup::Vacant,
        Err(_) => QuarantineCleanup::Occupied,
    }
}

fn entry_requires_directory_unlink(metadata: &std::fs::Metadata) -> bool {
    #[cfg(target_os = "windows")]
    {
        metadata.is_dir()
    }
    #[cfg(not(target_os = "windows"))]
    {
        metadata.file_type().is_dir()
    }
}

#[cfg(unix)]
fn rename_recovery_entry_no_replace(
    source: &Path,
    destination: &Path,
) -> Result<RecoveryRename, RecoveryPersistenceError> {
    let parent = containing_directory(source).ok_or(RecoveryPersistenceError)?;
    let expected_directory_identity =
        project_directory_identity(parent).map_err(|_| RecoveryPersistenceError)?;
    rename_recovery_entry_no_replace_in_directory(source, destination, expected_directory_identity)
}

#[cfg(unix)]
fn rename_recovery_entry_no_replace_in_directory(
    source: &Path,
    destination: &Path,
    expected_directory_identity: ProjectDirectoryIdentity,
) -> Result<RecoveryRename, RecoveryPersistenceError> {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

    let source_parent = containing_directory(source).ok_or(RecoveryPersistenceError)?;
    let destination_parent = containing_directory(destination).ok_or(RecoveryPersistenceError)?;
    if source_parent != destination_parent
        || project_directory_identity(source_parent).map_err(|_| RecoveryPersistenceError)?
            != expected_directory_identity
    {
        return Err(RecoveryPersistenceError);
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let directory = options
        .open(source_parent)
        .map_err(|_| RecoveryPersistenceError)?;
    let metadata = directory.metadata().map_err(|_| RecoveryPersistenceError)?;
    if FileSystemObjectIdentityV1(metadata.dev(), metadata.ino(), 0) != expected_directory_identity
    {
        return Err(RecoveryPersistenceError);
    }
    let source = CString::new(
        source
            .file_name()
            .ok_or(RecoveryPersistenceError)?
            .as_bytes(),
    )
    .map_err(|_| RecoveryPersistenceError)?;
    let destination = CString::new(
        destination
            .file_name()
            .ok_or(RecoveryPersistenceError)?
            .as_bytes(),
    )
    .map_err(|_| RecoveryPersistenceError)?;
    let directory_fd = directory.as_raw_fd();

    #[cfg(any(target_os = "linux", target_os = "android"))]
    let renamed = unsafe {
        // SAFETY: both C strings remain live for the syscall. RENAME_NOREPLACE
        // makes destination reservation and source retirement one operation,
        // and both names are resolved beneath the authenticated directory fd.
        libc::syscall(
            libc::SYS_renameat2,
            directory_fd,
            source.as_ptr(),
            directory_fd,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let renamed = unsafe {
        // SAFETY: both C strings remain live for the call. RENAME_EXCL is the
        // macOS no-replace counterpart of Linux RENAME_NOREPLACE.
        libc::renameatx_np(
            directory_fd,
            source.as_ptr(),
            directory_fd,
            destination.as_ptr(),
            libc::RENAME_EXCL,
        ) as libc::c_long
    };
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    let renamed: libc::c_long = return Err(RecoveryPersistenceError);

    if renamed == 0 {
        return Ok(RecoveryRename::Renamed);
    }
    classify_recovery_rename_error(std::io::Error::last_os_error())
}

#[cfg(target_os = "windows")]
fn rename_recovery_entry_no_replace(
    source: &Path,
    destination: &Path,
) -> Result<RecoveryRename, RecoveryPersistenceError> {
    let source_file = match open_recovery_entry_for_exclusive_retirement(source) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RecoveryRename::SourceMissing);
        }
        Err(_) => return Err(RecoveryPersistenceError),
    };
    let renamed = super::rename_windows_staged_file_with_policy(
        &source_file,
        destination,
        ExistingDestinationPolicy::RejectExisting,
    );
    drop(source_file);
    if renamed.is_ok() {
        return Ok(RecoveryRename::Renamed);
    }

    match std::fs::symlink_metadata(destination) {
        Ok(_) => Ok(RecoveryRename::DestinationExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::symlink_metadata(source) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(RecoveryRename::SourceMissing)
                }
                _ => Err(RecoveryPersistenceError),
            }
        }
        Err(_) => Err(RecoveryPersistenceError),
    }
}

#[cfg(target_os = "windows")]
fn open_recovery_entry_for_exclusive_retirement(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .access_mode(DELETE | FILE_READ_ATTRIBUTES)
        // Deliberately withhold FILE_SHARE_DELETE while this handle owns the
        // name retirement. A concurrent path-based rename/replacement must
        // fail rather than let us quarantine an old object and report success
        // while a replacement remains in the active recovery slot.
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    options.open(path)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn rename_recovery_entry_no_replace(
    _source: &Path,
    _destination: &Path,
) -> Result<RecoveryRename, RecoveryPersistenceError> {
    Err(RecoveryPersistenceError)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn rename_recovery_entry_no_replace_in_directory(
    _source: &Path,
    _destination: &Path,
    _expected_directory_identity: ProjectDirectoryIdentity,
) -> Result<RecoveryRename, RecoveryPersistenceError> {
    Err(RecoveryPersistenceError)
}

#[cfg(unix)]
fn classify_recovery_rename_error(
    error: std::io::Error,
) -> Result<RecoveryRename, RecoveryPersistenceError> {
    if error.raw_os_error() == Some(libc::ENOENT) {
        return Ok(RecoveryRename::SourceMissing);
    }
    if matches!(
        error.raw_os_error(),
        Some(code) if code == libc::EEXIST || code == libc::ENOTEMPTY
    ) {
        return Ok(RecoveryRename::DestinationExists);
    }
    Err(RecoveryPersistenceError)
}

#[cfg(unix)]
fn sync_recovery_directory(path: &Path) -> Result<(), RecoveryPersistenceError> {
    let directory = File::open(path).map_err(|_| RecoveryPersistenceError)?;
    directory.sync_all().map_err(|_| RecoveryPersistenceError)
}

#[cfg(not(unix))]
fn sync_recovery_directory(_path: &Path) -> Result<(), RecoveryPersistenceError> {
    Ok(())
}

fn prepare_recovery_staged_file(
    path: &Path,
    project: &Ori2ProjectArchive,
) -> Result<StagedFile, RecoveryPersistenceError> {
    if path.file_name().is_none() {
        return Err(RecoveryPersistenceError);
    }
    let parent = containing_directory(path).ok_or(RecoveryPersistenceError)?;
    std::fs::create_dir_all(parent).map_err(|_| RecoveryPersistenceError)?;
    validate_document_instruction_poses(&project.document).map_err(|_| RecoveryPersistenceError)?;
    crate::restore_archive_editor(project).map_err(|_| RecoveryPersistenceError)?;
    let bytes = write_project_archive_ori2(project).map_err(|_| RecoveryPersistenceError)?;
    prepare_staged_file(path, project, &bytes).map_err(|_| RecoveryPersistenceError)
}

#[cfg(test)]
pub(super) fn stage_recovery_project_for_test(
    path: &Path,
    project: &Ori2ProjectArchive,
) -> Result<StagedFile, RecoveryPersistenceError> {
    prepare_recovery_staged_file(path, project)
}

#[cfg(not(target_os = "windows"))]
fn publish_recovery_staged_file(
    staged: &mut StagedFile,
    destination: &Path,
) -> Result<(), RecoveryPersistenceError> {
    let parent = containing_directory(destination).ok_or(RecoveryPersistenceError)?;
    let directory = File::open(parent).map_err(|_| RecoveryPersistenceError)?;

    // The first barrier ensures an error is reported before the visible slot
    // changes. A post-publish barrier failure leaves the verified document
    // visible but reports failure so the generation remains retryable.
    directory.sync_all().map_err(|_| RecoveryPersistenceError)?;
    publish_unix_staged_file(
        staged,
        destination,
        ExistingDestinationPolicy::ReplaceConfirmed,
    )
    .map_err(|_| RecoveryPersistenceError)?;
    directory.sync_all().map_err(|_| RecoveryPersistenceError)
}

#[cfg(target_os = "windows")]
fn publish_recovery_staged_file(
    staged: &mut StagedFile,
    destination: &Path,
) -> Result<(), RecoveryPersistenceError> {
    super::rename_windows_staged_file_with_policy(
        staged.file(),
        destination,
        ExistingDestinationPolicy::ReplaceConfirmed,
    )
    .map_err(|_| RecoveryPersistenceError)?;
    staged.committed = true;
    Ok(())
}

#[cfg(test)]
pub(super) fn persist_project_archive(
    path: &Path,
    project: &Ori2ProjectArchive,
) -> Result<(), String> {
    persist_project_archive_to_destination(
        &DialogSaveDestination::confirmed(path.to_path_buf()),
        project,
    )
}

#[cfg(test)]
pub(super) fn persist_document(path: &Path, document: &ProjectDocument) -> Result<(), String> {
    persist_project_archive(path, &Ori2ProjectArchive::document_only(document.clone()))
}

pub(super) fn persist_project_archive_to_destination(
    destination: &DialogSaveDestination,
    project: &Ori2ProjectArchive,
) -> Result<(), String> {
    let path = destination.path();
    if path.file_name().is_none() {
        return Err("選択された保存先はファイルパスではありません。".to_owned());
    }

    validate_document_instruction_poses(&project.document)
        .map_err(|_| PROJECT_INSTRUCTIONS_SAVE_FAILED_MESSAGE.to_owned())?;
    crate::restore_archive_editor(project)
        .map_err(|_| PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())?;
    let bytes = write_project_archive_ori2(project)
        .map_err(|_| PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())?;
    let result = (|| {
        let _operation = acquire_project_file_operation(path)
            .map_err(|()| "project file operation is already active".to_owned())?;
        persist_document_atomically(
            path,
            project,
            &bytes,
            destination.existing_destination_policy(),
        )
    })();
    match destination.existing_destination_policy() {
        ExistingDestinationPolicy::RejectExisting => result.map_err(|_| {
            "拡張子を補正した保存先を安全に確定できなかったため、保存を中止しました。".to_owned()
        }),
        ExistingDestinationPolicy::ReplaceConfirmed => {
            result.map_err(|_| PROJECT_REPLACE_SAVE_FAILED_MESSAGE.to_owned())
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn persist_document_atomically(
    path: &Path,
    project: &Ori2ProjectArchive,
    bytes: &[u8],
    existing_destination_policy: ExistingDestinationPolicy,
) -> Result<(), String> {
    persist_document_atomically_with_pre_publish_hook(
        path,
        project,
        bytes,
        existing_destination_policy,
        || Ok(()),
    )
}

#[cfg(not(target_os = "windows"))]
fn persist_document_atomically_with_pre_publish_hook(
    path: &Path,
    project: &Ori2ProjectArchive,
    bytes: &[u8],
    existing_destination_policy: ExistingDestinationPolicy,
    pre_publish_hook: impl FnOnce() -> std::io::Result<()>,
) -> Result<(), String> {
    recover_single_file_journal_for_target(path, project.document.project_id).map_err(|_| {
        format!(
            "failed to recover an interrupted save for {}",
            path.display()
        )
    })?;
    let mut staged = prepare_staged_file(path, project, bytes)?;
    let parent = containing_directory(path)
        .ok_or_else(|| format!("{} is not a file path", path.display()))?;
    let directory = File::open(parent).map_err(|error| {
        format!(
            "failed to open the project directory for {}: {error}",
            path.display()
        )
    })?;
    let commit = match existing_destination_policy {
        ExistingDestinationPolicy::RejectExisting => match pre_publish_hook() {
            Ok(()) => commit_unix_staged_project_file(
                &mut staged,
                path,
                ExistingDestinationPolicy::RejectExisting,
                || directory.sync_all(),
            ),
            Err(error) => Err(error),
        },
        ExistingDestinationPolicy::ReplaceConfirmed => match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata_is_plain_regular_file(&metadata) => {
                commit_unix_staged_project_file_with_journal_and_hooks(
                    &mut staged,
                    path,
                    project.document.project_id,
                    || directory.sync_all(),
                    pre_publish_hook,
                    || Ok(()),
                )
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match pre_publish_hook() {
                    Ok(()) => {
                        // Replace confirmation applies only to a destination that
                        // was observed and authenticated as a regular file. A
                        // missing destination is published create-only, so a path
                        // introduced after this observation is preserved rather
                        // than replaced.
                        commit_unix_staged_project_file(
                            &mut staged,
                            path,
                            ExistingDestinationPolicy::RejectExisting,
                            || directory.sync_all(),
                        )
                    }
                    Err(error) => Err(error),
                }
            }
            Ok(_) => Err(std::io::Error::other(
                "replacement destination is not a regular file",
            )),
            Err(error) => Err(error),
        },
    };
    commit.map_err(|error| {
        format!(
            "failed to commit and synchronize {} atomically: {error}",
            path.display()
        )
    })?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn commit_unix_staged_project_file_with_journal_and_hooks<F, H1, H2>(
    staged: &mut StagedFile,
    destination: &Path,
    project_id: ProjectId,
    mut sync_directory: F,
    pre_old_move_hook: H1,
    pre_new_publish_hook: H2,
) -> std::io::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
    H1: FnOnce() -> std::io::Result<()>,
    H2: FnOnce() -> std::io::Result<()>,
{
    let fingerprint = target_path_fingerprint(destination)
        .map_err(|()| std::io::Error::other("target fingerprint failed"))?;
    let temp_name = staged
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| std::io::Error::other("staged name is not portable"))?
        .to_owned();
    let transaction_id = format!(
        "{}-{}",
        std::process::id(),
        NEXT_STAGED_FILE_ID.fetch_add(1, Ordering::Relaxed)
    );
    let backup_name = format!(".origami2-backup-{transaction_id}");
    let parent =
        containing_directory(destination).ok_or_else(|| std::io::Error::other("missing parent"))?;
    let directory_identity = project_directory_identity(parent)
        .map_err(|()| std::io::Error::other("save directory identity failed"))?;
    let backup = parent.join(&backup_name);
    let old_entry_metadata = std::fs::symlink_metadata(destination)?;
    let (old_sha256, mut old_file, old_metadata) =
        hash_inspected_regular_file_no_follow_with_pre_open_hook(
            destination,
            &old_entry_metadata,
            false,
            || Ok(()),
        )?;
    let old_authenticated = authenticated_regular_file_v1(&old_file, &old_metadata)
        .ok_or_else(|| std::io::Error::other("old destination identity is unavailable"))?;
    if old_authenticated.link_count != 1 {
        return Err(std::io::Error::other(
            "old destination is not exclusively linked",
        ));
    }
    let (temp_sha256, staged_authenticated) = hash_staged_file_with_retained_seal_v1(staged)?;
    let mut payload = SingleFileJournalPayloadV1 {
        schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
        project_id,
        target_path_sha256: fingerprint,
        transaction_id,
        temp_object_id: temp_name,
        temp_sha256,
        backup_object_id: backup_name,
        old_sha256: Some(old_sha256),
        phase: SingleFileJournalPhaseV1::Prepared,
    };
    persist_single_file_journal_phase(destination, &payload, true)
        .map_err(|()| std::io::Error::other("journal prepare failed"))?;
    #[cfg(test)]
    abort_at_single_file_save_failpoint("journal_prepared");
    staged.committed = true;

    let result = (|| {
        pre_old_move_hook()?;
        if project_directory_identity(parent).ok() != Some(directory_identity) {
            return Err(std::io::Error::other("save directory changed"));
        }
        if authenticated_regular_file_v1(&old_file, &old_file.metadata()?)
            != Some(old_authenticated)
        {
            return Err(std::io::Error::other(
                "old destination handle changed before backup",
            ));
        }
        revalidate_opened_regular_file_content_v1(
            &mut old_file,
            old_authenticated,
            payload
                .old_sha256
                .as_deref()
                .ok_or_else(|| std::io::Error::other("old destination digest is unavailable"))?,
            true,
        )?;
        revalidate_path_against_authenticated_regular_file_v1(
            destination,
            old_authenticated,
            true,
        )?;
        rename_opened_regular_file_no_replace_v1(
            destination,
            &backup,
            &old_file,
            directory_identity,
        )
        .map_err(|()| std::io::Error::other("old destination backup rename failed"))?;
        match std::fs::symlink_metadata(destination) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(std::io::Error::other(
                    "old destination name remained after backup",
                ));
            }
        }
        revalidate_opened_regular_file_content_v1(
            &mut old_file,
            old_authenticated,
            payload
                .old_sha256
                .as_deref()
                .ok_or_else(|| std::io::Error::other("old destination digest is unavailable"))?,
            true,
        )?;
        revalidate_path_against_authenticated_regular_file_v1(&backup, old_authenticated, true)?;
        sync_directory()?;
        payload.phase = SingleFileJournalPhaseV1::OldMoved;
        persist_single_file_journal_phase(destination, &payload, false)
            .map_err(|()| std::io::Error::other("old-moved journal failed"))?;
        #[cfg(test)]
        abort_at_single_file_save_failpoint("old_moved");
        pre_new_publish_hook()?;
        if project_directory_identity(parent).ok() != Some(directory_identity) {
            return Err(std::io::Error::other("save directory changed"));
        }
        if authenticated_regular_file_v1(staged.file(), &staged.file().metadata()?)
            != Some(staged_authenticated)
        {
            return Err(std::io::Error::other(
                "staged handle changed before publication",
            ));
        }
        revalidate_opened_regular_file_content_v1(
            staged.file_mut(),
            staged_authenticated,
            &payload.temp_sha256,
            true,
        )?;
        revalidate_path_against_authenticated_regular_file_v1(
            &staged.path,
            staged_authenticated,
            true,
        )?;
        rename_opened_regular_file_no_replace_v1(
            &staged.path,
            destination,
            staged.file(),
            directory_identity,
        )
        .map_err(|()| std::io::Error::other("staged publication rename failed"))?;
        match std::fs::symlink_metadata(&staged.path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(std::io::Error::other(
                    "staged name remained after publication",
                ));
            }
        }
        revalidate_opened_regular_file_content_v1(
            staged.file_mut(),
            staged_authenticated,
            &payload.temp_sha256,
            true,
        )?;
        revalidate_path_against_authenticated_regular_file_v1(
            destination,
            staged_authenticated,
            true,
        )?;
        sync_directory()?;
        payload.phase = SingleFileJournalPhaseV1::NewPublished;
        persist_single_file_journal_phase(destination, &payload, false)
            .map_err(|()| std::io::Error::other("new-published journal failed"))?;
        #[cfg(test)]
        abort_at_single_file_save_failpoint("new_published");
        revalidate_path_against_authenticated_regular_file_v1(&backup, old_authenticated, true)?;
        std::fs::remove_file(&backup)?;
        match std::fs::symlink_metadata(&backup) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(std::io::Error::other(
                    "old destination backup remained after cleanup",
                ));
            }
        }
        let journal = journal_path_for_target(destination, &payload.target_path_sha256)
            .map_err(|()| std::io::Error::other("journal path failed"))?;
        std::fs::remove_file(journal)?;
        sync_directory()
    })();
    if result.is_err() {
        let _ = recover_single_file_journal_for_target(destination, project_id);
    }
    result
}

#[cfg(test)]
fn abort_at_single_file_save_failpoint(expected: &str) {
    if std::env::var_os("ORIGAMI2_TEST_SINGLE_FILE_SAVE_ABORT_AT")
        .is_some_and(|value| value == expected)
    {
        std::process::abort();
    }
}

type RegularFileIdentityV1 = FileSystemObjectIdentityV1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuthenticatedRegularFileV1 {
    identity: RegularFileIdentityV1,
    length: u64,
    link_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthenticatedRecoveryObjectV1 {
    authenticated: AuthenticatedRegularFileV1,
    sha256: String,
}

#[cfg(unix)]
fn regular_file_identity_v1(
    _file: &File,
    metadata: &std::fs::Metadata,
) -> Option<RegularFileIdentityV1> {
    use std::os::unix::fs::MetadataExt;

    Some(FileSystemObjectIdentityV1(
        metadata.dev(),
        metadata.ino(),
        0,
    ))
}

#[cfg(target_os = "windows")]
fn regular_file_identity_v1(
    file: &File,
    _metadata: &std::fs::Metadata,
) -> Option<RegularFileIdentityV1> {
    regular_file_information_from_handle_v1(file).map(|(identity, _)| identity)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn regular_file_identity_v1(
    _file: &File,
    _metadata: &std::fs::Metadata,
) -> Option<RegularFileIdentityV1> {
    None
}

#[cfg(unix)]
fn regular_file_link_count_v1(_file: &File, metadata: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;

    Some(metadata.nlink())
}

#[cfg(target_os = "windows")]
fn regular_file_link_count_v1(file: &File, _metadata: &std::fs::Metadata) -> Option<u64> {
    regular_file_information_from_handle_v1(file).map(|(_, link_count)| link_count)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn regular_file_link_count_v1(_file: &File, _metadata: &std::fs::Metadata) -> Option<u64> {
    None
}

fn authenticated_regular_file_v1(
    file: &File,
    metadata: &std::fs::Metadata,
) -> Option<AuthenticatedRegularFileV1> {
    metadata_is_plain_regular_file(metadata).then_some(())?;
    Some(AuthenticatedRegularFileV1 {
        identity: regular_file_identity_v1(file, metadata)?,
        length: metadata.len(),
        link_count: regular_file_link_count_v1(file, metadata)?,
    })
}

fn authenticated_regular_file_with_link_policy_v1(
    file: &File,
    metadata: &std::fs::Metadata,
    require_single_link: bool,
) -> std::io::Result<AuthenticatedRegularFileV1> {
    let authenticated = authenticated_regular_file_v1(file, metadata)
        .ok_or_else(|| std::io::Error::other("regular file identity is unavailable"))?;
    if require_single_link && authenticated.link_count != 1 {
        return Err(std::io::Error::other(
            "private file is not exclusively linked",
        ));
    }
    Ok(authenticated)
}

#[cfg(target_os = "windows")]
fn regular_file_information_from_handle_v1(file: &File) -> Option<(RegularFileIdentityV1, u64)> {
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;

    let information = windows_handle_information_v1(file)?;
    if information.attributes & FILE_ATTRIBUTE_DIRECTORY != 0
        || information.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return None;
    }
    Some((information.identity, information.link_count))
}

#[cfg(unix)]
fn regular_file_metadata_matches_v1(
    expected: &std::fs::Metadata,
    actual: &std::fs::Metadata,
    require_single_link: bool,
) -> bool {
    use std::os::unix::fs::MetadataExt;

    metadata_is_plain_regular_file(expected)
        && metadata_is_plain_regular_file(actual)
        && expected.dev() == actual.dev()
        && expected.ino() == actual.ino()
        && expected.len() == actual.len()
        && expected.nlink() == actual.nlink()
        && (!require_single_link || expected.nlink() == 1)
}

#[cfg(target_os = "windows")]
fn regular_file_metadata_matches_v1(
    expected: &std::fs::Metadata,
    actual: &std::fs::Metadata,
    _require_single_link: bool,
) -> bool {
    metadata_is_plain_regular_file(expected)
        && metadata_is_plain_regular_file(actual)
        && expected.len() == actual.len()
        && expected.file_attributes() == actual.file_attributes()
        && expected.creation_time() == actual.creation_time()
        && expected.last_write_time() == actual.last_write_time()
}

#[cfg(not(any(unix, target_os = "windows")))]
fn regular_file_metadata_matches_v1(
    _expected: &std::fs::Metadata,
    _actual: &std::fs::Metadata,
    _require_single_link: bool,
) -> bool {
    false
}

#[cfg(unix)]
fn stable_read_metadata_matches_v1(
    expected: &std::fs::Metadata,
    actual: &std::fs::Metadata,
    require_single_link: bool,
) -> bool {
    use std::os::unix::fs::MetadataExt;

    regular_file_metadata_matches_v1(expected, actual, require_single_link)
        && expected.mode() == actual.mode()
        && expected.uid() == actual.uid()
        && expected.gid() == actual.gid()
        && expected.mtime() == actual.mtime()
        && expected.mtime_nsec() == actual.mtime_nsec()
        && expected.ctime() == actual.ctime()
        && expected.ctime_nsec() == actual.ctime_nsec()
}

#[cfg(target_os = "windows")]
fn stable_read_metadata_matches_v1(
    expected: &std::fs::Metadata,
    actual: &std::fs::Metadata,
    require_single_link: bool,
) -> bool {
    regular_file_metadata_matches_v1(expected, actual, require_single_link)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn stable_read_metadata_matches_v1(
    _expected: &std::fs::Metadata,
    _actual: &std::fs::Metadata,
    _require_single_link: bool,
) -> bool {
    false
}

fn read_opened_regular_file_bounded_stably_with_post_read_hook_v1(
    path: &Path,
    file: &mut File,
    opened_metadata: &std::fs::Metadata,
    require_single_link: bool,
    max_bytes: u64,
    post_read_hook: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<Vec<u8>> {
    let opened_authenticated =
        authenticated_regular_file_with_link_policy_v1(file, opened_metadata, require_single_link)?;
    file.seek(SeekFrom::Start(0))?;
    let capacity = usize::try_from(opened_metadata.len())
        .unwrap_or(0)
        .min(usize::try_from(max_bytes).unwrap_or(usize::MAX));
    let mut bytes = Vec::with_capacity(capacity);
    {
        let mut bounded_reader = (&mut *file).take(max_bytes.saturating_add(1));
        bounded_reader.read_to_end(&mut bytes)?;
    }
    post_read_hook()?;

    // Timestamp resolution is not a sufficient mutation detector on every
    // supported Unix filesystem. Re-read through the retained handle and
    // compare the exact bytes without allocating a second archive-sized
    // buffer, then retain the metadata/identity checks below as an independent
    // seal against replacement and size/link changes.
    file.seek(SeekFrom::Start(0))?;
    let mut verified_bytes = 0_usize;
    let mut verification_buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut verification_buffer)?;
        if read == 0 {
            break;
        }
        let verified_end = verified_bytes
            .checked_add(read)
            .ok_or_else(|| std::io::Error::other("regular file verification overflow"))?;
        if verified_end > bytes.len()
            || bytes[verified_bytes..verified_end] != verification_buffer[..read]
        {
            return Err(std::io::Error::other(
                "regular file changed while being read",
            ));
        }
        verified_bytes = verified_end;
    }
    if verified_bytes != bytes.len() {
        return Err(std::io::Error::other(
            "regular file changed while being read",
        ));
    }

    let final_metadata = file.metadata()?;
    let final_authenticated =
        authenticated_regular_file_with_link_policy_v1(file, &final_metadata, require_single_link)?;
    let bytes_read = u64::try_from(bytes.len())
        .map_err(|_| std::io::Error::other("regular file read length overflow"))?;
    if bytes_read != opened_metadata.len()
        || bytes_read != final_metadata.len()
        || !stable_read_metadata_matches_v1(opened_metadata, &final_metadata, require_single_link)
        || final_authenticated != opened_authenticated
    {
        return Err(std::io::Error::other(
            "regular file changed while being read",
        ));
    }
    revalidate_path_against_authenticated_regular_file_v1(
        path,
        opened_authenticated,
        require_single_link,
    )
    .map_err(|_| std::io::Error::other("regular file path changed while being read"))?;
    Ok(bytes)
}

#[cfg(test)]
fn hash_regular_file_no_follow_with_pre_open_hook(
    path: &Path,
    pre_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<String> {
    let entry_metadata = std::fs::symlink_metadata(path)?;
    hash_inspected_regular_file_no_follow_with_pre_open_hook(
        path,
        &entry_metadata,
        false,
        pre_open_hook,
    )
    .map(|(hash, _, _)| hash)
}

fn hash_inspected_regular_file_no_follow_with_pre_open_hook(
    path: &Path,
    entry_metadata: &std::fs::Metadata,
    require_single_link: bool,
    pre_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<(String, File, std::fs::Metadata)> {
    let (mut file, opened_metadata) = open_inspected_regular_file_no_follow_with_pre_open_hook(
        path,
        entry_metadata,
        require_single_link,
        pre_open_hook,
    )?;
    let (hash, final_metadata) =
        hash_opened_regular_file_from_start_v1(&mut file, &opened_metadata, require_single_link)?;
    Ok((hash, file, final_metadata))
}

fn hash_opened_regular_file_from_start_v1(
    file: &mut File,
    opened_metadata: &std::fs::Metadata,
    require_single_link: bool,
) -> std::io::Result<(String, std::fs::Metadata)> {
    let opened_authenticated =
        authenticated_regular_file_with_link_policy_v1(file, opened_metadata, require_single_link)?;
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total_read = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total_read = total_read
            .checked_add(
                u64::try_from(read)
                    .map_err(|_| std::io::Error::other("regular file length overflow"))?,
            )
            .ok_or_else(|| std::io::Error::other("regular file length overflow"))?;
        hasher.update(&buffer[..read]);
    }
    let final_metadata = file.metadata()?;
    let final_authenticated =
        authenticated_regular_file_with_link_policy_v1(file, &final_metadata, require_single_link)?;
    if total_read != final_metadata.len()
        || !regular_file_metadata_matches_v1(opened_metadata, &final_metadata, require_single_link)
        || final_authenticated != opened_authenticated
    {
        return Err(std::io::Error::other("regular file changed while hashing"));
    }
    Ok((
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        final_metadata,
    ))
}

fn revalidate_opened_regular_file_content_v1(
    file: &mut File,
    expected_authenticated: AuthenticatedRegularFileV1,
    expected_sha256: &str,
    require_single_link: bool,
) -> std::io::Result<()> {
    let opened_metadata = file.metadata()?;
    if authenticated_regular_file_with_link_policy_v1(file, &opened_metadata, require_single_link)?
        != expected_authenticated
    {
        return Err(std::io::Error::other(
            "regular file handle changed before content validation",
        ));
    }
    let (actual_sha256, final_metadata) =
        hash_opened_regular_file_from_start_v1(file, &opened_metadata, require_single_link)?;
    if actual_sha256 != expected_sha256
        || authenticated_regular_file_v1(file, &final_metadata) != Some(expected_authenticated)
    {
        return Err(std::io::Error::other(
            "regular file content changed before use",
        ));
    }
    Ok(())
}

fn hash_staged_file_with_retained_seal_v1(
    staged: &mut StagedFile,
) -> std::io::Result<(String, AuthenticatedRegularFileV1)> {
    let opened_metadata = staged.file().metadata()?;
    let opened_authenticated =
        authenticated_regular_file_with_link_policy_v1(staged.file(), &opened_metadata, true)?;
    revalidate_path_against_authenticated_regular_file_v1(
        &staged.path,
        opened_authenticated,
        true,
    )?;
    let (hash, final_metadata) =
        hash_opened_regular_file_from_start_v1(staged.file_mut(), &opened_metadata, true)?;
    let authenticated = authenticated_regular_file_v1(staged.file(), &final_metadata)
        .ok_or_else(|| std::io::Error::other("staged file identity is unavailable"))?;
    Ok((hash, authenticated))
}

fn revalidate_path_against_authenticated_regular_file_v1(
    path: &Path,
    authenticated: AuthenticatedRegularFileV1,
    require_single_link: bool,
) -> std::io::Result<()> {
    if require_single_link && authenticated.link_count != 1 {
        return Err(std::io::Error::other(
            "private file is not exclusively linked",
        ));
    }
    let entry_metadata = std::fs::symlink_metadata(path)?;
    let (file, opened_metadata) = open_inspected_regular_file_no_follow_with_pre_open_hook(
        path,
        &entry_metadata,
        require_single_link,
        || Ok(()),
    )?;
    if authenticated_regular_file_v1(&file, &opened_metadata) != Some(authenticated) {
        return Err(std::io::Error::other("regular file identity changed"));
    }
    Ok(())
}

fn open_inspected_regular_file_no_follow_with_pre_open_hook(
    path: &Path,
    entry_metadata: &std::fs::Metadata,
    require_single_link: bool,
    pre_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<(File, std::fs::Metadata)> {
    open_inspected_regular_file_no_follow_with_opener_and_pre_open_hook(
        path,
        entry_metadata,
        require_single_link,
        open_regular_file_no_follow,
        pre_open_hook,
    )
}

fn open_inspected_regular_file_no_follow_for_stable_read_with_pre_open_hook(
    path: &Path,
    entry_metadata: &std::fs::Metadata,
    require_single_link: bool,
    pre_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<(File, std::fs::Metadata)> {
    open_inspected_regular_file_no_follow_with_opener_and_pre_open_hook(
        path,
        entry_metadata,
        require_single_link,
        open_regular_file_no_follow_for_stable_read_v1,
        pre_open_hook,
    )
}

fn open_inspected_regular_file_no_follow_with_opener_and_pre_open_hook(
    path: &Path,
    entry_metadata: &std::fs::Metadata,
    require_single_link: bool,
    open_file: fn(&Path) -> std::io::Result<File>,
    pre_open_hook: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<(File, std::fs::Metadata)> {
    if !regular_file_metadata_matches_v1(entry_metadata, entry_metadata, require_single_link) {
        return Err(std::io::Error::other("not a regular no-follow file"));
    }
    let expected_file = open_file(path)?;
    let expected_metadata = expected_file.metadata()?;
    if !regular_file_metadata_matches_v1(entry_metadata, &expected_metadata, require_single_link) {
        return Err(std::io::Error::other(
            "regular file changed before inspection",
        ));
    }
    let expected_authenticated = authenticated_regular_file_with_link_policy_v1(
        &expected_file,
        &expected_metadata,
        require_single_link,
    )?;
    pre_open_hook()?;
    let file = open_file(path)?;
    let opened_metadata = file.metadata()?;
    let opened_authenticated = authenticated_regular_file_with_link_policy_v1(
        &file,
        &opened_metadata,
        require_single_link,
    )?;
    if !regular_file_metadata_matches_v1(&expected_metadata, &opened_metadata, require_single_link)
        || opened_authenticated != expected_authenticated
    {
        return Err(std::io::Error::other("regular file changed before use"));
    }
    Ok((file, opened_metadata))
}

#[cfg(unix)]
fn open_regular_file_for_authenticated_rename_no_follow_v1(path: &Path) -> std::io::Result<File> {
    open_regular_file_no_follow(path)
}

#[cfg(target_os = "windows")]
fn open_regular_file_for_authenticated_rename_no_follow_v1(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        // Content is re-hashed immediately before and after publication, so
        // the retained handle needs read-data access in addition to DELETE.
        .access_mode(FILE_GENERIC_READ | DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn open_regular_file_for_authenticated_rename_no_follow_v1(_path: &Path) -> std::io::Result<File> {
    Err(std::io::Error::other(
        "authenticated no-follow rename is unsupported",
    ))
}

#[cfg(target_os = "windows")]
fn rename_opened_regular_file_no_replace_v1(
    _source: &Path,
    destination: &Path,
    source_file: &File,
    _expected_directory_identity: ProjectDirectoryIdentity,
) -> Result<(), ()> {
    super::rename_windows_staged_file_with_policy(
        source_file,
        destination,
        ExistingDestinationPolicy::RejectExisting,
    )
    .map_err(|_| ())
}

#[cfg(not(target_os = "windows"))]
fn rename_opened_regular_file_no_replace_v1(
    source: &Path,
    destination: &Path,
    _source_file: &File,
    expected_directory_identity: ProjectDirectoryIdentity,
) -> Result<(), ()> {
    match rename_recovery_entry_no_replace_in_directory(
        source,
        destination,
        expected_directory_identity,
    )
    .map_err(|_| ())?
    {
        RecoveryRename::Renamed => Ok(()),
        RecoveryRename::SourceMissing | RecoveryRename::DestinationExists => Err(()),
    }
}

#[cfg(target_os = "windows")]
fn persist_document_atomically(
    path: &Path,
    project: &Ori2ProjectArchive,
    bytes: &[u8],
    existing_destination_policy: ExistingDestinationPolicy,
) -> Result<(), String> {
    persist_document_atomically_with_pre_publish_hook(
        path,
        project,
        bytes,
        existing_destination_policy,
        || Ok(()),
    )
}

#[cfg(target_os = "windows")]
fn persist_document_atomically_with_pre_publish_hook(
    path: &Path,
    project: &Ori2ProjectArchive,
    bytes: &[u8],
    existing_destination_policy: ExistingDestinationPolicy,
    pre_publish_hook: impl FnOnce() -> std::io::Result<()>,
) -> Result<(), String> {
    recover_single_file_journal_for_target(path, project.document.project_id).map_err(|_| {
        format!(
            "failed to recover an interrupted save for {}",
            path.display()
        )
    })?;
    let mut staged = prepare_staged_file(path, project, bytes)?;
    match existing_destination_policy {
        ExistingDestinationPolicy::RejectExisting => {
            pre_publish_hook().map_err(|error| error.to_string())?;
            super::rename_windows_staged_file_with_policy(
                staged.file(),
                path,
                ExistingDestinationPolicy::RejectExisting,
            )?;
        }
        ExistingDestinationPolicy::ReplaceConfirmed => match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata_is_plain_regular_file(&metadata) => {
                commit_windows_staged_project_file_with_journal_and_hook(
                    &mut staged,
                    path,
                    project.document.project_id,
                    pre_publish_hook,
                )?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                pre_publish_hook().map_err(|error| error.to_string())?;
                // A target that was absent when replacement was classified
                // remains a create-only publication. The handle-bound
                // no-replace rename fails if any entry wins this race.
                super::rename_windows_staged_file_with_policy(
                    staged.file(),
                    path,
                    ExistingDestinationPolicy::RejectExisting,
                )?;
            }
            Ok(_) => return Err("replacement destination is not a regular file".to_owned()),
            Err(error) => return Err(error.to_string()),
        },
    }
    staged.committed = true;
    Ok(())
}

#[cfg(target_os = "windows")]
fn commit_windows_staged_project_file_with_journal_and_hook(
    staged: &mut StagedFile,
    destination: &Path,
    project_id: ProjectId,
    pre_publish_hook: impl FnOnce() -> std::io::Result<()>,
) -> Result<(), String> {
    let parent = containing_directory(destination).ok_or_else(|| "missing parent".to_owned())?;
    let directory_identity = project_directory_identity(parent)
        .map_err(|()| "failed to identify the save directory".to_owned())?;
    let fingerprint = target_path_fingerprint(destination)
        .map_err(|()| "failed to fingerprint the save target".to_owned())?;
    let temp_name = staged
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "staged name is not portable".to_owned())?
        .to_owned();
    let transaction_id = format!(
        "{}-{}",
        std::process::id(),
        NEXT_STAGED_FILE_ID.fetch_add(1, Ordering::Relaxed)
    );
    let old_entry_metadata =
        std::fs::symlink_metadata(destination).map_err(|error| error.to_string())?;
    let (old_sha256, mut old_file, old_metadata) =
        hash_inspected_regular_file_no_follow_with_pre_open_hook(
            destination,
            &old_entry_metadata,
            false,
            || Ok(()),
        )
        .map_err(|error| error.to_string())?;
    let old_authenticated = authenticated_regular_file_v1(&old_file, &old_metadata)
        .ok_or_else(|| "old destination identity is unavailable".to_owned())?;
    let (temp_sha256, staged_authenticated) =
        hash_staged_file_with_retained_seal_v1(staged).map_err(|error| error.to_string())?;
    let payload = SingleFileJournalPayloadV1 {
        schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
        project_id,
        target_path_sha256: fingerprint,
        transaction_id: transaction_id.clone(),
        temp_object_id: temp_name,
        temp_sha256,
        backup_object_id: format!(".origami2-backup-{transaction_id}"),
        old_sha256: Some(old_sha256),
        phase: SingleFileJournalPhaseV1::Prepared,
    };
    persist_single_file_journal_phase(destination, &payload, true)
        .map_err(|()| "failed to prepare the save journal".to_owned())?;
    #[cfg(test)]
    abort_at_single_file_save_failpoint("journal_prepared");
    pre_publish_hook().map_err(|error| error.to_string())?;
    if project_directory_identity(parent).ok() != Some(directory_identity) {
        return Err("the save directory changed before commit".to_owned());
    }
    if authenticated_regular_file_v1(
        &old_file,
        &old_file.metadata().map_err(|error| error.to_string())?,
    ) != Some(old_authenticated)
    {
        return Err("the old destination handle changed before commit".to_owned());
    }
    revalidate_opened_regular_file_content_v1(
        &mut old_file,
        old_authenticated,
        payload
            .old_sha256
            .as_deref()
            .ok_or_else(|| "the old destination digest is unavailable".to_owned())?,
        false,
    )
    .map_err(|error| error.to_string())?;
    revalidate_path_against_authenticated_regular_file_v1(destination, old_authenticated, false)
        .map_err(|error| error.to_string())?;
    if authenticated_regular_file_v1(
        staged.file(),
        &staged
            .file()
            .metadata()
            .map_err(|error| error.to_string())?,
    ) != Some(staged_authenticated)
    {
        return Err("the staged handle changed before commit".to_owned());
    }
    revalidate_opened_regular_file_content_v1(
        staged.file_mut(),
        staged_authenticated,
        &payload.temp_sha256,
        true,
    )
    .map_err(|error| error.to_string())?;
    revalidate_path_against_authenticated_regular_file_v1(&staged.path, staged_authenticated, true)
        .map_err(|error| error.to_string())?;
    super::rename_windows_staged_file_with_policy(
        staged.file(),
        destination,
        ExistingDestinationPolicy::ReplaceConfirmed,
    )?;
    staged.committed = true;
    match std::fs::symlink_metadata(&staged.path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err("the staged name remained after commit".to_owned()),
    }
    revalidate_opened_regular_file_content_v1(
        staged.file_mut(),
        staged_authenticated,
        &payload.temp_sha256,
        true,
    )
    .map_err(|error| error.to_string())?;
    revalidate_path_against_authenticated_regular_file_v1(destination, staged_authenticated, true)
        .map_err(|error| error.to_string())?;
    drop(old_file);
    #[cfg(test)]
    abort_at_single_file_save_failpoint("new_published");
    let journal = journal_path_for_target(destination, &payload.target_path_sha256)
        .map_err(|()| "failed to locate the save journal".to_owned())?;
    std::fs::remove_file(journal).map_err(|error| error.to_string())?;
    sync_project_directory(
        containing_directory(destination).ok_or_else(|| "missing parent".to_owned())?,
    )
    .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "windows"))]
pub(super) fn publish_unix_staged_file(
    staged: &mut StagedFile,
    destination: &Path,
    existing_destination_policy: ExistingDestinationPolicy,
) -> std::io::Result<()> {
    publish_unix_staged_file_with_pre_publish_hook_v1(
        staged,
        destination,
        existing_destination_policy,
        || Ok(()),
    )
}

#[cfg(not(target_os = "windows"))]
fn publish_unix_staged_file_with_pre_publish_hook_v1(
    staged: &mut StagedFile,
    destination: &Path,
    existing_destination_policy: ExistingDestinationPolicy,
    pre_publish_hook: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<()> {
    let staged_metadata = staged.file().metadata()?;
    let opened_authenticated =
        authenticated_regular_file_with_link_policy_v1(staged.file(), &staged_metadata, true)?;
    let (staged_sha256, hashed_metadata) =
        hash_opened_regular_file_from_start_v1(staged.file_mut(), &staged_metadata, true)?;
    let staged_authenticated =
        authenticated_regular_file_with_link_policy_v1(staged.file(), &hashed_metadata, true)?;
    if staged_authenticated != opened_authenticated {
        return Err(std::io::Error::other(
            "staged file changed while its content was sealed",
        ));
    }
    revalidate_path_against_authenticated_regular_file_v1(
        &staged.path,
        staged_authenticated,
        true,
    )?;
    pre_publish_hook()?;
    revalidate_opened_regular_file_content_v1(
        staged.file_mut(),
        staged_authenticated,
        &staged_sha256,
        true,
    )?;
    revalidate_path_against_authenticated_regular_file_v1(
        &staged.path,
        staged_authenticated,
        true,
    )?;

    match existing_destination_policy {
        ExistingDestinationPolicy::ReplaceConfirmed => {
            std::fs::rename(&staged.path, destination)?;
            match std::fs::symlink_metadata(&staged.path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                _ => {
                    return Err(std::io::Error::other(
                        "staged name remained after publication",
                    ));
                }
            }
            revalidate_opened_regular_file_content_v1(
                staged.file_mut(),
                staged_authenticated,
                &staged_sha256,
                true,
            )?;
            revalidate_path_against_authenticated_regular_file_v1(
                destination,
                staged_authenticated,
                true,
            )?;
            staged.committed = true;
        }
        ExistingDestinationPolicy::RejectExisting => {
            // The staged file is in the destination directory, so creating a
            // hard link is an atomic create-new publish on the same file
            // system. Unlike a preflight existence check followed by rename,
            // this cannot replace a path created by another process in the
            // intervening window.
            std::fs::hard_link(&staged.path, destination)?;
            let published_authenticated = AuthenticatedRegularFileV1 {
                // The retained stage was authenticated with the single-link
                // policy immediately above. Publication adds exactly one name.
                link_count: 2,
                ..staged_authenticated
            };
            if authenticated_regular_file_v1(staged.file(), &staged.file().metadata()?)
                != Some(published_authenticated)
            {
                return Err(std::io::Error::other(
                    "published file does not match the retained staged handle",
                ));
            }
            revalidate_opened_regular_file_content_v1(
                staged.file_mut(),
                published_authenticated,
                &staged_sha256,
                false,
            )?;
            revalidate_path_against_authenticated_regular_file_v1(
                destination,
                published_authenticated,
                false,
            )?;

            // The destination now names the authenticated staged inode. From
            // this point cleanup must not turn a successful publication into
            // a retryable save error. Mark it committed before best-effort
            // retirement so Drop cannot later unlink a swapped staging name.
            staged.committed = true;
            if revalidate_path_against_authenticated_regular_file_v1(
                &staged.path,
                published_authenticated,
                false,
            )
            .is_ok()
            {
                let _ = std::fs::remove_file(&staged.path);
            }
            let final_authenticated =
                authenticated_regular_file_v1(staged.file(), &staged.file().metadata()?)
                    .ok_or_else(|| {
                        std::io::Error::other(
                            "published file identity is unavailable after staging cleanup",
                        )
                    })?;
            revalidate_opened_regular_file_content_v1(
                staged.file_mut(),
                final_authenticated,
                &staged_sha256,
                false,
            )?;
            revalidate_path_against_authenticated_regular_file_v1(
                destination,
                final_authenticated,
                false,
            )?;
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(super) fn commit_unix_staged_project_file<F>(
    staged: &mut StagedFile,
    destination: &Path,
    existing_destination_policy: ExistingDestinationPolicy,
    mut sync_directory: F,
) -> std::io::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
{
    // Any reported error must occur before the visible destination changes.
    // Once publish succeeds, a directory durability failure cannot be
    // reported as an ordinary save failure because callers would retain the
    // old saved baseline and may retry despite the new file being visible.
    sync_directory()?;
    publish_unix_staged_file(staged, destination, existing_destination_policy)?;
    let _ = sync_directory();
    Ok(())
}

pub(super) struct StagedFile {
    file: Option<File>,
    pub(super) path: PathBuf,
    pub(super) committed: bool,
}

impl StagedFile {
    pub(super) fn file(&self) -> &File {
        self.file
            .as_ref()
            .expect("a staged file handle remains present until drop")
    }

    pub(super) fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("a staged file handle remains present until drop")
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        // Windows sharing deliberately denies deletion while this handle is
        // open. Closing first is harmless and makes cleanup consistent on all
        // platforms.
        drop(self.file.take());
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub(super) fn prepare_staged_file(
    path: &Path,
    project: &Ori2ProjectArchive,
    bytes: &[u8],
) -> Result<StagedFile, String> {
    let mut staged = create_staged_file(path)?;
    write_complete_staged_payload(staged.file_mut(), bytes).map_err(|error| {
        format!(
            "failed to write staged project data for {}: {error}",
            path.display()
        )
    })?;
    staged.file_mut().sync_all().map_err(|error| {
        format!(
            "failed to synchronize staged project data for {}: {error}",
            path.display()
        )
    })?;

    // Re-read the staged file through the same handle before its same-directory
    // rename. Windows additionally denies writer/delete sharing for the life
    // of this handle.
    staged
        .file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|error| {
            format!(
                "failed to rewind staged project data for {}: {error}",
                path.display()
            )
        })?;
    let mut staged_bytes = Vec::with_capacity(bytes.len());
    staged
        .file_mut()
        .read_to_end(&mut staged_bytes)
        .map_err(|error| {
            format!(
                "failed to verify staged project data for {}: {error}",
                path.display()
            )
        })?;
    if staged_bytes != bytes {
        return Err(format!(
            "staged project data for {} changed before commit",
            path.display()
        ));
    }
    verify_generated_ori2(project, &staged_bytes)?;
    Ok(staged)
}

fn write_complete_staged_payload(writer: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    writer.write_all(bytes)
}

#[cfg(test)]
#[path = "project_persistence/staged_payload_adapter_tests.rs"]
mod staged_payload_adapter_tests;

pub(super) fn create_staged_file(path: &Path) -> Result<StagedFile, String> {
    let parent = containing_directory(path)
        .ok_or_else(|| format!("{} is not a file path", path.display()))?;
    path.file_name()
        .ok_or_else(|| format!("{} is not a file path", path.display()))?;

    for _ in 0..128 {
        let id = NEXT_STAGED_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let mut staged_name = OsString::from(".origami2-");
        staged_name.push(format!("{}-{id}.tmp", std::process::id()));
        let staged_path = parent.join(staged_name);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(target_os = "windows")]
        options
            .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE)
            .share_mode(FILE_SHARE_READ);
        match options.open(&staged_path) {
            Ok(file) => {
                let staged = StagedFile {
                    file: Some(file),
                    path: staged_path,
                    committed: false,
                };
                #[cfg(not(target_os = "windows"))]
                match std::fs::symlink_metadata(path) {
                    Ok(metadata) if metadata.file_type().is_file() => staged
                        .file()
                        .set_permissions(metadata.permissions())
                        .map_err(|error| {
                            format!(
                                "failed to preserve permissions for {}: {error}",
                                path.display()
                            )
                        })?,
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!(
                            "failed to inspect permissions for {}: {error}",
                            path.display()
                        ));
                    }
                }
                return Ok(staged);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "failed to prepare atomic save for {}: {error}",
                    path.display()
                ));
            }
        }
    }

    Err(format!(
        "failed to prepare atomic save for {}: could not allocate a unique staged file",
        path.display()
    ))
}

pub(super) fn containing_directory(path: &Path) -> Option<&Path> {
    path.parent().map(|parent| {
        if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        }
    })
}

pub(super) fn verify_generated_ori2(
    project: &Ori2ProjectArchive,
    bytes: &[u8],
) -> Result<(), String> {
    let verified = read_project_archive_ori2_with_limits(bytes, Ori2Limits::default())
        .map_err(|error| format!("generated .ori2 data did not pass validation: {error}"))?;
    if verified != *project {
        return Err("generated .ori2 data did not round-trip exactly".to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[path = "project_persistence/recovery_entry_tests.rs"]
mod recovery_entry_tests;

#[cfg(all(test, target_os = "windows"))]
mod windows_large_archive_budget_tests {
    use std::{fs, time::Instant};

    use ori_core::EditorState;
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Point2, ProjectId, Vertex, VertexId};

    use super::*;

    #[test]
    fn ten_thousand_edge_archive_save_open_and_history_stay_bounded() {
        let namespace = ProjectId::new();
        let vertices = (0..=10_000)
            .map(|index| Vertex {
                id: VertexId::derive_v5(namespace, format!("v-{index}").as_bytes()),
                position: Point2::new((index % 101) as f64, (index / 101) as f64),
            })
            .collect::<Vec<_>>();
        let edges = (0..10_000)
            .map(|index| Edge {
                id: EdgeId::derive_v5(namespace, format!("e-{index}").as_bytes()),
                start: vertices[index].id,
                end: vertices[index + 1].id,
                kind: if index % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            })
            .collect::<Vec<_>>();
        let pattern = CreasePattern { vertices, edges };
        let document = ProjectDocument::new("Windows 10k archive budget", pattern.clone());
        let mut editor = EditorState::new(pattern);
        editor
            .set_history_entry_limit(64)
            .expect("set bounded non-default history limit");
        let history = editor
            .export_history_v1(document.project_id)
            .expect("export bounded history metadata");
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            document,
            editor_history: Some(history),
        };
        let directory = std::env::temp_dir().join(format!(
            "origami2-windows-10k-budget-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).expect("create bounded archive test directory");
        let path = directory.join("target-file.ori2");

        let save_started = Instant::now();
        persist_project_archive(&path, &archive).expect("save 10k archive atomically");
        let save_elapsed = save_started.elapsed();
        let bytes = fs::metadata(&path).expect("saved metadata").len();
        assert!(
            bytes > 0 && bytes <= 16 * 1024 * 1024,
            "archive bytes: {bytes}"
        );
        assert!(
            save_elapsed <= Duration::from_secs(10),
            "save elapsed: {save_elapsed:?}"
        );

        let open_started = Instant::now();
        let reopened = load_project_archive_from_path(&path).expect("open 10k archive");
        let open_elapsed = open_started.elapsed();
        assert_eq!(reopened.document.crease_pattern.edges.len(), 10_000);
        let reopened_history = reopened
            .editor_history
            .expect("history entry remains authenticated");
        assert_eq!(reopened_history.undo_len(), 0);
        assert_eq!(reopened_history.redo_len(), 0);
        assert_eq!(reopened_history.history_entry_limit(), 64);
        assert!(
            open_elapsed <= Duration::from_secs(10),
            "open elapsed: {open_elapsed:?}"
        );
        fs::remove_file(&path).expect("remove archive fixture");
        fs::remove_dir(&directory).expect("remove archive test directory");
    }
}
