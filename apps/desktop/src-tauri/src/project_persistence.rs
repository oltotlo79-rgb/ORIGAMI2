use std::{
    collections::HashSet,
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

static ACTIVE_PROJECT_FILE_OPERATIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

struct ProjectFileOperationGuard {
    keys: Vec<String>,
}

impl Drop for ProjectFileOperationGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE_PROJECT_FILE_OPERATIONS
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
        {
            for key in &self.keys {
                active.remove(key);
            }
        }
    }
}

fn acquire_project_file_operation(path: &Path) -> Result<ProjectFileOperationGuard, ()> {
    let keys = project_file_operation_keys(path)?;
    let mut active = ACTIVE_PROJECT_FILE_OPERATIONS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .map_err(|_| ())?;
    if keys.iter().any(|key| active.contains(key)) {
        return Err(());
    }
    active.extend(keys.iter().cloned());
    Ok(ProjectFileOperationGuard { keys })
}

fn project_file_operation_keys(path: &Path) -> Result<Vec<String>, ()> {
    let path_key = format!("path:{}", target_path_fingerprint(path)?);
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                return Err(());
            }
            Ok(vec![path_key, project_file_identity_key(path)?])
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(vec![path_key]),
        Err(_) => Err(()),
    }
}

#[cfg(unix)]
fn project_file_identity_key(path: &Path) -> Result<String, ()> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let mut options = OpenOptions::new();
    options.read(true).custom_flags(libc::O_NOFOLLOW);
    let metadata = options
        .open(path)
        .map_err(|_| ())?
        .metadata()
        .map_err(|_| ())?;
    if !metadata.file_type().is_file() {
        return Err(());
    }
    Ok(format!("file:{}:{}", metadata.dev(), metadata.ino()))
}

#[cfg(target_os = "windows")]
fn project_file_identity_key(path: &Path) -> Result<String, ()> {
    use std::{mem::MaybeUninit, os::windows::io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut options = OpenOptions::new();
    options
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    let file = options.open(path).map_err(|_| ())?;
    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the output points to writable storage and the file handle stays
    // valid for the duration of the call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, information.as_mut_ptr()) }
        == 0
    {
        return Err(());
    }
    // SAFETY: a successful call initialized the complete structure.
    let information = unsafe { information.assume_init() };
    if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(());
    }
    let index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    Ok(format!("file:{}:{index}", information.dwVolumeSerialNumber))
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

    fn verify_directory_identity(&self) -> Result<(), ()> {
        (project_directory_identity(&self.directory)? == self.directory_identity)
            .then_some(())
            .ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectDirectoryIdentity(u64, u64);

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
    Ok(ProjectDirectoryIdentity(metadata.dev(), metadata.ino()))
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
    use std::{mem::MaybeUninit, os::windows::io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the output storage and directory handle are valid for the call.
    if unsafe {
        GetFileInformationByHandle(directory.as_raw_handle() as _, information.as_mut_ptr())
    } == 0
    {
        return Err(());
    }
    // SAFETY: success initialized the structure.
    let information = unsafe { information.assume_init() };
    if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(());
    }
    let index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    Ok(ProjectDirectoryIdentity(
        u64::from(information.dwVolumeSerialNumber),
        index,
    ))
}

#[cfg(unix)]
fn recovery_private_object_is_exclusive(path: &Path) -> Result<bool, ()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(path).map_err(|_| ())?;
    Ok(metadata.file_type().is_file()
        && !metadata.file_type().is_symlink()
        && metadata.nlink() == 1)
}

#[cfg(target_os = "windows")]
fn recovery_private_object_is_exclusive(path: &Path) -> Result<bool, ()> {
    use std::{mem::MaybeUninit, os::windows::io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let metadata = std::fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let mut options = OpenOptions::new();
    options
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    let file = options.open(path).map_err(|_| ())?;
    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the output storage and handle are valid for the call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, information.as_mut_ptr()) }
        == 0
    {
        return Err(());
    }
    // SAFETY: success initialized the structure.
    let information = unsafe { information.assume_init() };
    Ok(
        information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
            && information.nNumberOfLinks == 1,
    )
}

impl SingleFileRecoveryFs for DiskSingleFileRecoveryFs {
    fn object_sha256(&self, object: SingleFileRecoveryObject) -> Result<Option<String>, ()> {
        self.verify_directory_identity()?;
        let path = self.path(object);
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(()),
        };
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(());
        }
        if object != SingleFileRecoveryObject::Target
            && !recovery_private_object_is_exclusive(path)?
        {
            return Err(());
        }
        let mut file = File::open(path).map_err(|_| ())?;
        let opened = file.metadata().map_err(|_| ())?;
        if !opened.is_file() || opened.len() != metadata.len() {
            return Err(());
        }
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer).map_err(|_| ())?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(Some(
            hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        ))
    }

    fn rename_object(
        &mut self,
        from: SingleFileRecoveryObject,
        to: SingleFileRecoveryObject,
    ) -> Result<(), ()> {
        self.verify_directory_identity()?;
        if std::fs::symlink_metadata(self.path(to)).is_ok() {
            return Err(());
        }
        std::fs::rename(self.path(from), self.path(to)).map_err(|_| ())
    }

    fn remove_object(&mut self, object: SingleFileRecoveryObject) -> Result<(), ()> {
        self.verify_directory_identity()?;
        let path = self.path(object);
        match std::fs::symlink_metadata(path) {
            Ok(metadata)
                if metadata.file_type().is_file() && !metadata.file_type().is_symlink() =>
            {
                if object != SingleFileRecoveryObject::Target
                    && !recovery_private_object_is_exclusive(path)?
                {
                    return Err(());
                }
                std::fs::remove_file(path).map_err(|_| ())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(()),
        }
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
    let fingerprint = target_path_fingerprint(target)?;
    let journal_path = journal_path_for_target(target, &fingerprint)?;
    let metadata = match std::fs::symlink_metadata(&journal_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(()),
    };
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 64 * 1024
    {
        return Err(());
    }
    let bytes = std::fs::read(&journal_path).map_err(|_| ())?;
    let untrusted: AuthenticatedSingleFileJournalV1 =
        serde_json::from_slice(&bytes).map_err(|_| ())?;
    let project_id = expected_project_id.unwrap_or(untrusted.payload.project_id);
    let payload = decode_single_file_journal_v1(&bytes, project_id, &fingerprint)?;
    let directory = containing_directory(target).ok_or(())?.to_path_buf();
    let directory_identity = project_directory_identity(&directory)?;
    let mut fs = DiskSingleFileRecoveryFs {
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
    let _operation = acquire_project_file_operation(path)
        .map_err(|()| PROJECT_FILE_OPEN_FAILED_MESSAGE.to_owned())?;
    recover_single_file_journal_for_open(path)
        .map_err(|_| PROJECT_FILE_INVALID_MESSAGE.to_owned())?;
    let limits = Ori2Limits::default();
    let file = File::open(path).map_err(|_| PROJECT_FILE_OPEN_FAILED_MESSAGE.to_owned())?;
    let declared_size = file
        .metadata()
        .map_err(|_| PROJECT_FILE_INSPECTION_FAILED_MESSAGE.to_owned())?
        .len();
    if declared_size > limits.max_archive_size {
        return Err(PROJECT_FILE_TOO_LARGE_MESSAGE.to_owned());
    }

    let capacity = usize::try_from(declared_size)
        .unwrap_or(0)
        .min(usize::try_from(limits.max_archive_size).unwrap_or(usize::MAX));
    let mut bytes = Vec::with_capacity(capacity);
    let mut bounded_reader = file.take(limits.max_archive_size.saturating_add(1));
    bounded_reader
        .read_to_end(&mut bytes)
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

    let file = match open_regular_file_no_follow(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return RecoveryProjectLoad::Missing;
        }
        Err(_) => return RecoveryProjectLoad::Invalid,
    };
    let metadata = match file.metadata() {
        Ok(metadata) if metadata_is_plain_regular_file(&metadata) => metadata,
        Ok(_) | Err(_) => return RecoveryProjectLoad::Invalid,
    };
    if metadata.len() > limits.max_archive_size {
        return RecoveryProjectLoad::Invalid;
    }
    let updated_at_unix_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(frontend_safe_unix_millis);
    let capacity = usize::try_from(metadata.len())
        .unwrap_or(0)
        .min(usize::try_from(limits.max_archive_size).unwrap_or(usize::MAX));
    let mut bytes = Vec::with_capacity(capacity);
    let mut bounded_reader = file.take(limits.max_archive_size.saturating_add(1));
    if bounded_reader.read_to_end(&mut bytes).is_err()
        || bytes.len() as u64 > limits.max_archive_size
    {
        return RecoveryProjectLoad::Invalid;
    }
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
    let source =
        CString::new(source.as_os_str().as_bytes()).map_err(|_| RecoveryPersistenceError)?;
    let destination =
        CString::new(destination.as_os_str().as_bytes()).map_err(|_| RecoveryPersistenceError)?;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    let renamed = unsafe {
        // SAFETY: both C strings remain live for the syscall. RENAME_NOREPLACE
        // makes destination reservation and source retirement one operation.
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let renamed = unsafe {
        // SAFETY: both C strings remain live for the call. RENAME_EXCL is the
        // macOS no-replace counterpart of Linux RENAME_NOREPLACE.
        libc::renameatx_np(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
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
    let commit = if existing_destination_policy == ExistingDestinationPolicy::ReplaceConfirmed
        && std::fs::symlink_metadata(path).is_ok_and(|metadata| {
            metadata.file_type().is_file() && !metadata.file_type().is_symlink()
        }) {
        commit_unix_staged_project_file_with_journal(
            &mut staged,
            path,
            project.document.project_id,
            || directory.sync_all(),
        )
    } else {
        commit_unix_staged_project_file(&mut staged, path, existing_destination_policy, || {
            directory.sync_all()
        })
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
fn commit_unix_staged_project_file_with_journal<F>(
    staged: &mut StagedFile,
    destination: &Path,
    project_id: ProjectId,
    mut sync_directory: F,
) -> std::io::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
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
    let old_sha256 = hash_regular_file_no_follow(destination)?;
    let temp_sha256 = hash_regular_file_no_follow(&staged.path)?;
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
    staged.close_for_journal_commit();

    let result = (|| {
        if project_directory_identity(parent).ok() != Some(directory_identity) {
            return Err(std::io::Error::other("save directory changed"));
        }
        std::fs::rename(destination, &backup)?;
        sync_directory()?;
        payload.phase = SingleFileJournalPhaseV1::OldMoved;
        persist_single_file_journal_phase(destination, &payload, false)
            .map_err(|()| std::io::Error::other("old-moved journal failed"))?;
        #[cfg(test)]
        abort_at_single_file_save_failpoint("old_moved");
        if project_directory_identity(parent).ok() != Some(directory_identity) {
            return Err(std::io::Error::other("save directory changed"));
        }
        std::fs::rename(&staged.path, destination)?;
        sync_directory()?;
        payload.phase = SingleFileJournalPhaseV1::NewPublished;
        persist_single_file_journal_phase(destination, &payload, false)
            .map_err(|()| std::io::Error::other("new-published journal failed"))?;
        #[cfg(test)]
        abort_at_single_file_save_failpoint("new_published");
        std::fs::remove_file(&backup)?;
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

fn hash_regular_file_no_follow(path: &Path) -> std::io::Result<String> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other("not a regular no-follow file"));
    }
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(target_os = "windows")]
fn persist_document_atomically(
    path: &Path,
    project: &Ori2ProjectArchive,
    bytes: &[u8],
    existing_destination_policy: ExistingDestinationPolicy,
) -> Result<(), String> {
    recover_single_file_journal_for_target(path, project.document.project_id).map_err(|_| {
        format!(
            "failed to recover an interrupted save for {}",
            path.display()
        )
    })?;
    let mut staged = prepare_staged_file(path, project, bytes)?;
    if existing_destination_policy == ExistingDestinationPolicy::ReplaceConfirmed
        && std::fs::symlink_metadata(path).is_ok_and(|metadata| {
            metadata.file_type().is_file() && !metadata.file_type().is_symlink()
        })
    {
        commit_windows_staged_project_file_with_journal(
            &mut staged,
            path,
            project.document.project_id,
        )?;
    } else {
        super::rename_windows_staged_file_with_policy(
            staged.file(),
            path,
            existing_destination_policy,
        )?;
    }
    staged.committed = true;
    Ok(())
}

#[cfg(target_os = "windows")]
fn commit_windows_staged_project_file_with_journal(
    staged: &mut StagedFile,
    destination: &Path,
    project_id: ProjectId,
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
    let payload = SingleFileJournalPayloadV1 {
        schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
        project_id,
        target_path_sha256: fingerprint,
        transaction_id: transaction_id.clone(),
        temp_object_id: temp_name,
        temp_sha256: hash_regular_file_no_follow(&staged.path)
            .map_err(|error| error.to_string())?,
        backup_object_id: format!(".origami2-backup-{transaction_id}"),
        old_sha256: Some(
            hash_regular_file_no_follow(destination).map_err(|error| error.to_string())?,
        ),
        phase: SingleFileJournalPhaseV1::Prepared,
    };
    persist_single_file_journal_phase(destination, &payload, true)
        .map_err(|()| "failed to prepare the save journal".to_owned())?;
    #[cfg(test)]
    abort_at_single_file_save_failpoint("journal_prepared");
    if project_directory_identity(parent).ok() != Some(directory_identity) {
        return Err("the save directory changed before commit".to_owned());
    }
    super::rename_windows_staged_file_with_policy(
        staged.file(),
        destination,
        ExistingDestinationPolicy::ReplaceConfirmed,
    )?;
    staged.committed = true;
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
    match existing_destination_policy {
        ExistingDestinationPolicy::ReplaceConfirmed => {
            std::fs::rename(&staged.path, destination)?;
            staged.committed = true;
        }
        ExistingDestinationPolicy::RejectExisting => {
            // The staged file is in the destination directory, so creating a
            // hard link is an atomic create-new publish on the same file
            // system. Unlike a preflight existence check followed by rename,
            // this cannot replace a path created by another process in the
            // intervening window.
            std::fs::hard_link(&staged.path, destination)?;
            if std::fs::remove_file(&staged.path).is_ok() {
                staged.committed = true;
            }
            // If unlinking the staging name failed, Drop retries it. The
            // destination is already the verified inode and must be reported
            // as committed rather than encouraging a duplicate save.
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

    #[cfg(not(target_os = "windows"))]
    fn close_for_journal_commit(&mut self) {
        drop(self.file.take());
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
mod staged_payload_adapter_tests {
    use std::{
        collections::HashMap,
        fs,
        io::{self, Write},
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        DialogSaveDestination, DiskSingleFileRecoveryFs, Ori2ProjectArchive,
        PROJECT_REPLACE_SAVE_FAILED_MESSAGE, ProjectDocument, SINGLE_FILE_JOURNAL_SCHEMA_V1,
        SingleFileJournalPayloadV1, SingleFileJournalPhaseV1, SingleFileRecoveryFs,
        SingleFileRecoveryObject, acquire_project_file_operation, decode_single_file_journal_v1,
        encode_single_file_journal_v1, journal_path_for_target, load_project_archive_from_path,
        persist_project_archive_to_destination, project_directory_identity,
        recover_authenticated_single_file_v1, recover_single_file_journal_for_target,
        sha256_hex_bytes, target_path_fingerprint, write_complete_staged_payload,
        write_project_archive_ori2,
    };
    use ori_core::{Command, EditorState};
    use ori_domain::{CreasePattern, ProjectId};
    use ori_domain::{Point2, VertexId};
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command as ProcessCommand;

    static NEXT_JOURNAL_TEST_ID: AtomicU64 = AtomicU64::new(0);

    fn journal_test_directory(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "origami2-single-journal-{label}-{}-{}",
            std::process::id(),
            NEXT_JOURNAL_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create journal test directory");
        path
    }

    struct InjectedWriter {
        bytes: Vec<u8>,
        maximum_chunk: usize,
        fail_after: Option<usize>,
    }

    #[derive(Clone)]
    struct RecoveryFsModel {
        objects: HashMap<SingleFileRecoveryObject, String>,
        fail_at: Option<usize>,
        calls: usize,
    }

    impl RecoveryFsModel {
        fn step(&mut self) -> Result<(), ()> {
            let current = self.calls;
            self.calls += 1;
            if self.fail_at == Some(current) {
                self.fail_at = None;
                Err(())
            } else {
                Ok(())
            }
        }
    }

    impl SingleFileRecoveryFs for RecoveryFsModel {
        fn object_sha256(&self, object: SingleFileRecoveryObject) -> Result<Option<String>, ()> {
            Ok(self.objects.get(&object).cloned())
        }

        fn rename_object(
            &mut self,
            from: SingleFileRecoveryObject,
            to: SingleFileRecoveryObject,
        ) -> Result<(), ()> {
            self.step()?;
            let value = self.objects.remove(&from).ok_or(())?;
            if self.objects.insert(to, value).is_some() {
                return Err(());
            }
            Ok(())
        }

        fn remove_object(&mut self, object: SingleFileRecoveryObject) -> Result<(), ()> {
            self.step()?;
            self.objects.remove(&object);
            Ok(())
        }

        fn sync_directory(&mut self) -> Result<(), ()> {
            self.step()
        }
    }

    impl Write for InjectedWriter {
        fn write(&mut self, source: &[u8]) -> io::Result<usize> {
            if self
                .fail_after
                .is_some_and(|limit| self.bytes.len() >= limit)
            {
                return Err(io::Error::new(
                    io::ErrorKind::StorageFull,
                    "injected disk full",
                ));
            }
            let remaining = self
                .fail_after
                .map_or(source.len(), |limit| limit.saturating_sub(self.bytes.len()));
            let count = source.len().min(self.maximum_chunk).min(remaining);
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "injected write zero",
                ));
            }
            self.bytes.extend_from_slice(&source[..count]);
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn short_writes_are_completed_and_disk_full_never_reports_success() {
        let payload = b"complete authenticated ori2 payload";
        let mut short = InjectedWriter {
            bytes: Vec::new(),
            maximum_chunk: 3,
            fail_after: None,
        };
        write_complete_staged_payload(&mut short, payload).expect("complete short writes");
        assert_eq!(short.bytes, payload);

        let mut full = InjectedWriter {
            bytes: Vec::new(),
            maximum_chunk: 4,
            fail_after: Some(9),
        };
        let error = write_complete_staged_payload(&mut full, payload)
            .expect_err("disk full must abort staging");
        assert!(matches!(
            error.kind(),
            io::ErrorKind::StorageFull | io::ErrorKind::WriteZero
        ));
        assert_ne!(full.bytes, payload);
    }

    #[test]
    fn journal_v1_is_content_authenticated_and_bound_to_project_and_target() {
        let project_id = ProjectId::new();
        let target = sha256_hex_bytes(b"canonical target path");
        let payload = SingleFileJournalPayloadV1 {
            schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
            project_id,
            target_path_sha256: target.clone(),
            transaction_id: "transaction-1".to_owned(),
            temp_object_id: "temp-1".to_owned(),
            temp_sha256: sha256_hex_bytes(b"new ori2"),
            backup_object_id: "backup-1".to_owned(),
            old_sha256: Some(sha256_hex_bytes(b"old ori2")),
            phase: SingleFileJournalPhaseV1::Prepared,
        };
        let encoded = encode_single_file_journal_v1(payload.clone()).expect("encode journal");
        assert_eq!(
            decode_single_file_journal_v1(&encoded, project_id, &target),
            Ok(payload)
        );
        assert!(
            decode_single_file_journal_v1(&encoded, ProjectId::new(), &target).is_err(),
            "a different project must not adopt the transaction"
        );
        assert!(
            decode_single_file_journal_v1(&encoded, project_id, &sha256_hex_bytes(b"other target"))
                .is_err(),
            "a different target path must not adopt the transaction"
        );

        let mut tampered: serde_json::Value =
            serde_json::from_slice(&encoded).expect("journal JSON");
        tampered["payload"]["phase"] = serde_json::json!("new_published");
        assert!(
            decode_single_file_journal_v1(
                &serde_json::to_vec(&tampered).expect("tampered journal"),
                project_id,
                &target
            )
            .is_err(),
            "phase tampering must fail authentication"
        );
    }

    #[test]
    fn every_recovery_phase_is_idempotent_across_injected_operation_failures() {
        let old = sha256_hex_bytes(b"old complete ori2");
        let new = sha256_hex_bytes(b"new complete ori2");
        for phase in [
            SingleFileJournalPhaseV1::Prepared,
            SingleFileJournalPhaseV1::OldMoved,
            SingleFileJournalPhaseV1::NewPublished,
        ] {
            let journal = SingleFileJournalPayloadV1 {
                schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
                project_id: ProjectId::new(),
                target_path_sha256: sha256_hex_bytes(b"target"),
                transaction_id: "transaction-2".to_owned(),
                temp_object_id: "temp-2".to_owned(),
                temp_sha256: new.clone(),
                backup_object_id: "backup-2".to_owned(),
                old_sha256: Some(old.clone()),
                phase,
            };
            let mut initial =
                HashMap::from([(SingleFileRecoveryObject::Journal, "journal".to_owned())]);
            match phase {
                SingleFileJournalPhaseV1::Prepared => {
                    initial.insert(SingleFileRecoveryObject::Target, old.clone());
                    initial.insert(SingleFileRecoveryObject::Temp, new.clone());
                }
                SingleFileJournalPhaseV1::OldMoved => {
                    initial.insert(SingleFileRecoveryObject::Backup, old.clone());
                    initial.insert(SingleFileRecoveryObject::Temp, new.clone());
                }
                SingleFileJournalPhaseV1::NewPublished => {
                    initial.insert(SingleFileRecoveryObject::Target, new.clone());
                    initial.insert(SingleFileRecoveryObject::Backup, old.clone());
                }
            }
            for fail_at in 0..8 {
                let mut fs = RecoveryFsModel {
                    objects: initial.clone(),
                    fail_at: Some(fail_at),
                    calls: 0,
                };
                let _ = recover_authenticated_single_file_v1(&mut fs, &journal);
                fs.fail_at = None;
                recover_authenticated_single_file_v1(&mut fs, &journal)
                    .expect("restart recovery converges idempotently");
                let expected = if phase == SingleFileJournalPhaseV1::Prepared {
                    &old
                } else {
                    &new
                };
                assert_eq!(
                    fs.objects.get(&SingleFileRecoveryObject::Target),
                    Some(expected)
                );
                for private in [
                    SingleFileRecoveryObject::Temp,
                    SingleFileRecoveryObject::Backup,
                    SingleFileRecoveryObject::Journal,
                ] {
                    assert!(!fs.objects.contains_key(&private));
                }
            }
        }
    }

    #[test]
    fn subprocess_crash_save_helper() {
        let Some(path) = std::env::var_os("ORIGAMI2_TEST_SINGLE_FILE_SAVE_PATH") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        #[cfg(target_os = "windows")]
        if std::env::var_os("ORIGAMI2_TEST_SINGLE_FILE_SAVE_MODE").as_deref()
            == Some(std::ffi::OsStr::new("hold_lock"))
        {
            use std::os::windows::fs::OpenOptionsExt;
            let ready = std::path::PathBuf::from(
                std::env::var_os("ORIGAMI2_TEST_SINGLE_FILE_SAVE_READY")
                    .expect("ready marker path"),
            );
            let release = std::path::PathBuf::from(
                std::env::var_os("ORIGAMI2_TEST_SINGLE_FILE_SAVE_RELEASE")
                    .expect("release marker path"),
            );
            let _locked = fs::OpenOptions::new()
                .read(true)
                .share_mode(super::FILE_SHARE_READ)
                .open(&path)
                .expect("cross-process non-delete-sharing handle");
            fs::write(&ready, b"ready").expect("publish ready marker");
            for _ in 0..1_000 {
                if release.exists() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            panic!("lock helper timed out");
        }
        if std::env::var_os("ORIGAMI2_TEST_SINGLE_FILE_SAVE_MODE").as_deref()
            == Some(std::ffi::OsStr::new("recover"))
        {
            load_project_archive_from_path(&path).expect("recover in a fresh subprocess");
            return;
        }
        let mut archive = load_project_archive_from_path(&path).expect("load crash source");
        archive.document.name = "new archive after crash".to_owned();
        persist_project_archive_to_destination(&DialogSaveDestination::confirmed(path), &archive)
            .expect("the configured failpoint must abort before save returns");
        panic!("configured save failpoint did not abort");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn separate_process_sharing_violation_preserves_old_archive_and_allows_retry() {
        let directory = journal_test_directory("cross-process-sharing");
        let target = directory.join("project.ori2");
        let ready = directory.join("lock-ready");
        let release = directory.join("lock-release");
        let old_archive = Ori2ProjectArchive::document_only(ProjectDocument::new(
            "old archive under lock",
            CreasePattern::empty(),
        ));
        let mut new_archive = old_archive.clone();
        new_archive.document.name = "new archive after retry".to_owned();
        let old_bytes = write_project_archive_ori2(&old_archive).expect("old archive bytes");
        fs::write(&target, &old_bytes).expect("old target");

        let mut lock_child = ProcessCommand::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg("project_persistence::staged_payload_adapter_tests::subprocess_crash_save_helper")
            .env("ORIGAMI2_TEST_SINGLE_FILE_SAVE_PATH", &target)
            .env("ORIGAMI2_TEST_SINGLE_FILE_SAVE_MODE", "hold_lock")
            .env("ORIGAMI2_TEST_SINGLE_FILE_SAVE_READY", &ready)
            .env("ORIGAMI2_TEST_SINGLE_FILE_SAVE_RELEASE", &release)
            .spawn()
            .expect("spawn cross-process lock holder");
        for _ in 0..1_000 {
            if ready.exists() {
                break;
            }
            assert!(lock_child.try_wait().expect("poll lock child").is_none());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(ready.exists(), "lock child must publish readiness");

        let error = persist_project_archive_to_destination(
            &DialogSaveDestination::confirmed(target.clone()),
            &new_archive,
        )
        .expect_err("cross-process sharing violation must fail closed");
        assert_eq!(error, PROJECT_REPLACE_SAVE_FAILED_MESSAGE);
        assert!(!error.contains(&directory.to_string_lossy().to_string()));
        assert!(!error.contains("project.ori2"));
        assert_eq!(
            fs::read(&target).expect("old target remains complete"),
            old_bytes
        );
        let fingerprint = target_path_fingerprint(&target).expect("target fingerprint");
        let journal = journal_path_for_target(&target, &fingerprint).expect("journal path");
        let entries_after_failure = fs::read_dir(&directory)
            .expect("failure directory")
            .map(|entry| entry.expect("failure entry").path())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            entries_after_failure,
            [target.clone(), ready.clone(), journal]
                .into_iter()
                .collect(),
            "only the complete old target, readiness marker, and authenticated retry journal may remain"
        );

        fs::write(&release, b"release").expect("release lock child");
        assert!(lock_child.wait().expect("join lock child").success());
        persist_project_archive_to_destination(
            &DialogSaveDestination::confirmed(target.clone()),
            &new_archive,
        )
        .expect("retry after cross-process lock release");
        assert_eq!(
            load_project_archive_from_path(&target).expect("load retried archive"),
            new_archive
        );
        let entries_after_retry = fs::read_dir(&directory)
            .expect("retry directory")
            .map(|entry| entry.expect("retry entry").file_name())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            entries_after_retry,
            ["project.ori2", "lock-ready", "lock-release"]
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect()
        );
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[test]
    fn separate_process_crash_and_recovery_preserve_authenticated_archive_and_history() {
        #[cfg(unix)]
        let cases = [
            ("journal_prepared", "old archive before crash"),
            ("old_moved", "new archive after crash"),
            ("new_published", "new archive after crash"),
        ];
        #[cfg(target_os = "windows")]
        let cases = [
            ("journal_prepared", "old archive before crash"),
            ("new_published", "new archive after crash"),
        ];
        for (failpoint, expected_name) in cases {
            let directory = journal_test_directory(failpoint);
            let target = directory.join("project.ori2");
            let first = VertexId::new();
            let second = VertexId::new();
            let mut editor = EditorState::new(CreasePattern::empty());
            editor
                .set_history_entry_limit(7)
                .expect("non-default limit");
            editor
                .execute(
                    0,
                    Command::AddVertex {
                        id: first,
                        position: Point2::new(1.0, 2.0),
                    },
                )
                .expect("first history command");
            editor
                .execute(
                    1,
                    Command::AddVertex {
                        id: second,
                        position: Point2::new(3.0, 4.0),
                    },
                )
                .expect("second history command");
            editor.undo(2).expect("create non-empty Redo stack");
            let document =
                ProjectDocument::new("old archive before crash", editor.pattern().clone());
            let history = editor
                .export_history_v1(document.project_id)
                .expect("authenticated non-empty history");
            assert_eq!(
                (
                    history.undo_len(),
                    history.redo_len(),
                    history.history_entry_limit()
                ),
                (1, 1, 7)
            );
            let old_archive = Ori2ProjectArchive {
                document,
                editor_history: Some(history.clone()),
                layer_evidence: None,
            };
            fs::write(
                &target,
                write_project_archive_ori2(&old_archive).expect("old archive bytes"),
            )
            .expect("old target");

            let status = ProcessCommand::new(std::env::current_exe().expect("test executable"))
                .arg("--exact")
                .arg("project_persistence::staged_payload_adapter_tests::subprocess_crash_save_helper")
                .arg("--nocapture")
                .env("ORIGAMI2_TEST_SINGLE_FILE_SAVE_PATH", &target)
                .env("ORIGAMI2_TEST_SINGLE_FILE_SAVE_ABORT_AT", failpoint)
                .status()
                .expect("run crash subprocess");
            assert!(
                !status.success(),
                "failpoint {failpoint} must terminate the child"
            );
            #[cfg(unix)]
            assert_eq!(status.signal(), Some(6), "child must terminate via SIGABRT");
            #[cfg(target_os = "windows")]
            assert!(
                status.code().is_some(),
                "aborted Windows child must expose a status code"
            );

            let recovery_status = ProcessCommand::new(
                std::env::current_exe().expect("test executable"),
            )
            .arg("--exact")
            .arg("project_persistence::staged_payload_adapter_tests::subprocess_crash_save_helper")
            .env("ORIGAMI2_TEST_SINGLE_FILE_SAVE_PATH", &target)
            .env("ORIGAMI2_TEST_SINGLE_FILE_SAVE_MODE", "recover")
            .status()
            .expect("run recovery subprocess");
            assert!(
                recovery_status.success(),
                "fresh recovery subprocess must succeed"
            );
            let recovered = load_project_archive_from_path(&target).expect("second recovery");
            assert_eq!(recovered.document.name, expected_name);
            assert_eq!(
                recovered.document.project_id,
                old_archive.document.project_id
            );
            assert_eq!(recovered.editor_history, Some(history));
            let remaining = fs::read_dir(&directory)
                .expect("recovery directory")
                .map(|entry| entry.expect("directory entry").file_name())
                .collect::<Vec<_>>();
            assert_eq!(remaining, vec![std::ffi::OsString::from("project.ori2")]);
            fs::remove_dir_all(directory).expect("cleanup test directory");
        }
    }

    #[test]
    fn disk_adapter_recovers_every_phase_and_removes_private_objects() {
        let old_bytes = b"old complete ori2";
        let new_bytes = b"new complete ori2";
        for phase in [
            SingleFileJournalPhaseV1::Prepared,
            SingleFileJournalPhaseV1::OldMoved,
            SingleFileJournalPhaseV1::NewPublished,
        ] {
            let directory = journal_test_directory("phases");
            let target = directory.join("project.ori2");
            let project_id = ProjectId::new();
            let fingerprint = target_path_fingerprint(&target).expect("target fingerprint");
            let temp_name = "temp-transaction";
            let backup_name = "backup-transaction";
            let payload = SingleFileJournalPayloadV1 {
                schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
                project_id,
                target_path_sha256: fingerprint.clone(),
                transaction_id: "transaction-3".to_owned(),
                temp_object_id: temp_name.to_owned(),
                temp_sha256: sha256_hex_bytes(new_bytes),
                backup_object_id: backup_name.to_owned(),
                old_sha256: Some(sha256_hex_bytes(old_bytes)),
                phase,
            };
            match phase {
                SingleFileJournalPhaseV1::Prepared => {
                    fs::write(&target, old_bytes).expect("old target");
                    fs::write(directory.join(temp_name), new_bytes).expect("temp");
                }
                SingleFileJournalPhaseV1::OldMoved => {
                    fs::write(directory.join(backup_name), old_bytes).expect("backup");
                    fs::write(directory.join(temp_name), new_bytes).expect("temp");
                }
                SingleFileJournalPhaseV1::NewPublished => {
                    fs::write(&target, new_bytes).expect("new target");
                    fs::write(directory.join(backup_name), old_bytes).expect("backup");
                }
            }
            let journal = journal_path_for_target(&target, &fingerprint).expect("journal path");
            fs::write(
                &journal,
                encode_single_file_journal_v1(payload).expect("journal bytes"),
            )
            .expect("write journal");
            recover_single_file_journal_for_target(&target, project_id).expect("recover phase");
            let expected: &[u8] = if phase == SingleFileJournalPhaseV1::Prepared {
                old_bytes
            } else {
                new_bytes
            };
            assert_eq!(fs::read(&target).expect("public target"), expected);
            assert!(!directory.join(temp_name).exists());
            assert!(!directory.join(backup_name).exists());
            assert!(!journal.exists());
            recover_single_file_journal_for_target(&target, project_id)
                .expect("recovery is idempotent after cleanup");
            fs::remove_dir_all(directory).expect("cleanup test directory");
        }
    }

    #[test]
    fn disk_adapter_rejects_parent_directory_swap_before_rename_and_cleanup() {
        let root = journal_test_directory("directory-swap");
        let active = root.join("active");
        let retired = root.join("retired");
        fs::create_dir(&active).expect("active directory");
        let target = active.join("project.ori2");
        let temp = active.join("temp-transaction");
        let backup = active.join("backup-transaction");
        let journal = active.join("journal.json");
        fs::write(&temp, b"owned staged bytes").expect("owned temp");
        fs::write(&journal, b"owned journal bytes").expect("owned journal");
        let identity = project_directory_identity(&active).expect("directory identity");
        let mut adapter = DiskSingleFileRecoveryFs {
            directory: active.clone(),
            directory_identity: identity,
            target: target.clone(),
            temp: temp.clone(),
            backup,
            journal: journal.clone(),
        };

        fs::rename(&active, &retired).expect("retire verified directory");
        fs::create_dir(&active).expect("replacement directory");
        let external_target = active.join("project.ori2");
        let external_temp = active.join("temp-transaction");
        let sentinel = b"external sentinel";
        fs::write(&external_target, sentinel).expect("external target");
        fs::write(&external_temp, sentinel).expect("external temp");

        assert!(
            adapter
                .rename_object(
                    SingleFileRecoveryObject::Temp,
                    SingleFileRecoveryObject::Target,
                )
                .is_err()
        );
        assert!(
            adapter
                .remove_object(SingleFileRecoveryObject::Journal)
                .is_err()
        );
        assert_eq!(
            fs::read(&external_target).expect("target unchanged"),
            sentinel
        );
        assert_eq!(fs::read(&external_temp).expect("temp unchanged"), sentinel);
        assert_eq!(
            fs::read(retired.join("temp-transaction")).expect("owned temp retained"),
            b"owned staged bytes"
        );
        assert_eq!(
            fs::read(retired.join("journal.json")).expect("owned journal retained"),
            b"owned journal bytes"
        );
        fs::remove_dir_all(root).expect("cleanup test directory");
    }

    #[test]
    fn open_recovers_interrupted_old_moved_transaction_before_reading() {
        let directory = journal_test_directory("open-recovery");
        let target = directory.join("project.ori2");
        let mut old_document = ProjectDocument::new("old", CreasePattern::empty());
        let project_id = old_document.project_id;
        let old_bytes =
            write_project_archive_ori2(&Ori2ProjectArchive::document_only(old_document.clone()))
                .expect("old archive");
        old_document.name = "new".to_owned();
        let new_archive = Ori2ProjectArchive::document_only(old_document);
        let new_bytes = write_project_archive_ori2(&new_archive).expect("new archive");
        let fingerprint = target_path_fingerprint(&target).expect("fingerprint");
        let temp_name = "temp-open-transaction";
        let backup_name = "backup-open-transaction";
        fs::write(directory.join(temp_name), &new_bytes).expect("temp archive");
        fs::write(directory.join(backup_name), &old_bytes).expect("backup archive");
        let payload = SingleFileJournalPayloadV1 {
            schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
            project_id,
            target_path_sha256: fingerprint.clone(),
            transaction_id: "open-transaction".to_owned(),
            temp_object_id: temp_name.to_owned(),
            temp_sha256: sha256_hex_bytes(&new_bytes),
            backup_object_id: backup_name.to_owned(),
            old_sha256: Some(sha256_hex_bytes(&old_bytes)),
            phase: SingleFileJournalPhaseV1::OldMoved,
        };
        let journal = journal_path_for_target(&target, &fingerprint).expect("journal path");
        fs::write(
            &journal,
            encode_single_file_journal_v1(payload).expect("journal"),
        )
        .expect("write journal");

        assert_eq!(
            load_project_archive_from_path(&target).expect("open recovers first"),
            new_archive
        );
        assert_eq!(fs::read(&target).expect("published target"), new_bytes);
        assert!(!journal.exists());
        assert!(!directory.join(temp_name).exists());
        assert!(!directory.join(backup_name).exists());
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[test]
    fn open_rejects_tampered_journal_without_changing_any_object() {
        let directory = journal_test_directory("open-tamper");
        let target = directory.join("project.ori2");
        let document = ProjectDocument::new("preserve", CreasePattern::empty());
        let project_id = document.project_id;
        let old_bytes = write_project_archive_ori2(&Ori2ProjectArchive::document_only(document))
            .expect("old archive");
        let temp_bytes = old_bytes.clone();
        fs::write(&target, &old_bytes).expect("target");
        let fingerprint = target_path_fingerprint(&target).expect("fingerprint");
        let temp_name = "temp-tampered-transaction";
        fs::write(directory.join(temp_name), &temp_bytes).expect("temp");
        let payload = SingleFileJournalPayloadV1 {
            schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
            project_id,
            target_path_sha256: fingerprint.clone(),
            transaction_id: "tampered-transaction".to_owned(),
            temp_object_id: temp_name.to_owned(),
            temp_sha256: sha256_hex_bytes(&temp_bytes),
            backup_object_id: "backup-tampered-transaction".to_owned(),
            old_sha256: Some(sha256_hex_bytes(&old_bytes)),
            phase: SingleFileJournalPhaseV1::Prepared,
        };
        let journal = journal_path_for_target(&target, &fingerprint).expect("journal path");
        let encoded = encode_single_file_journal_v1(payload).expect("journal");
        let mut value: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON");
        value["payload"]["phase"] = serde_json::json!("new_published");
        let tampered = serde_json::to_vec(&value).expect("tampered JSON");
        fs::write(&journal, &tampered).expect("write tampered journal");

        assert!(load_project_archive_from_path(&target).is_err());
        assert_eq!(fs::read(&target).expect("target preserved"), old_bytes);
        assert_eq!(
            fs::read(directory.join(temp_name)).expect("temp preserved"),
            temp_bytes
        );
        assert_eq!(fs::read(&journal).expect("journal preserved"), tampered);
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[test]
    fn recovery_rejects_hardlinked_private_object_and_preserves_sentinel() {
        let directory = journal_test_directory("private-hardlink");
        let target = directory.join("project.ori2");
        let sentinel = directory.join("sentinel");
        let temp_name = "temp-hardlink-transaction";
        let backup_name = "backup-hardlink-transaction";
        let old_bytes = b"old complete ori2";
        let new_bytes = b"sentinel new bytes";
        fs::write(&sentinel, new_bytes).expect("sentinel");
        fs::hard_link(&sentinel, directory.join(temp_name)).expect("hardlinked temp");
        fs::write(directory.join(backup_name), old_bytes).expect("backup");
        let project_id = ProjectId::new();
        let fingerprint = target_path_fingerprint(&target).expect("fingerprint");
        let payload = SingleFileJournalPayloadV1 {
            schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
            project_id,
            target_path_sha256: fingerprint.clone(),
            transaction_id: "hardlink-transaction".to_owned(),
            temp_object_id: temp_name.to_owned(),
            temp_sha256: sha256_hex_bytes(new_bytes),
            backup_object_id: backup_name.to_owned(),
            old_sha256: Some(sha256_hex_bytes(old_bytes)),
            phase: SingleFileJournalPhaseV1::OldMoved,
        };
        let journal = journal_path_for_target(&target, &fingerprint).expect("journal");
        fs::write(
            &journal,
            encode_single_file_journal_v1(payload).expect("journal bytes"),
        )
        .expect("write journal");

        assert!(recover_single_file_journal_for_target(&target, project_id).is_err());
        assert_eq!(fs::read(&sentinel).expect("sentinel preserved"), new_bytes);
        assert_eq!(
            fs::read(directory.join(temp_name)).expect("hardlink preserved"),
            new_bytes
        );
        assert!(directory.join(backup_name).exists());
        assert!(journal.exists());
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[test]
    fn save_rejects_preexisting_hardlinked_journal_without_unlinking_it() {
        let directory = journal_test_directory("journal-hardlink");
        let target = directory.join("project.ori2");
        let sentinel = directory.join("sentinel");
        let sentinel_bytes = b"attacker sentinel";
        fs::write(&sentinel, sentinel_bytes).expect("sentinel");
        let fingerprint = target_path_fingerprint(&target).expect("fingerprint");
        let journal = journal_path_for_target(&target, &fingerprint).expect("journal");
        fs::hard_link(&sentinel, &journal).expect("hardlinked journal");
        let archive = Ori2ProjectArchive::document_only(ProjectDocument::new(
            "must not save",
            CreasePattern::empty(),
        ));

        assert!(
            persist_project_archive_to_destination(
                &DialogSaveDestination::confirmed(target.clone()),
                &archive,
            )
            .is_err()
        );
        assert!(!target.exists());
        assert_eq!(
            fs::read(&sentinel).expect("sentinel preserved"),
            sentinel_bytes
        );
        assert_eq!(
            fs::read(&journal).expect("journal link preserved"),
            sentinel_bytes
        );
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_real_fs_faults_preserve_complete_target_and_redact_reason() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = journal_test_directory("windows-real-faults");
        let target = directory.join("project.ori2");
        let sentinel = directory.join("unowned-sentinel");
        let old_archive = Ori2ProjectArchive::document_only(ProjectDocument::new(
            "old complete",
            CreasePattern::empty(),
        ));
        let new_archive = Ori2ProjectArchive::document_only(ProjectDocument::new(
            "new complete",
            CreasePattern::empty(),
        ));
        let old_bytes = write_project_archive_ori2(&old_archive).expect("old archive");
        let sentinel_bytes = b"unowned bytes";
        fs::write(&target, &old_bytes).expect("old target");
        fs::write(&sentinel, sentinel_bytes).expect("sentinel");

        let mut permissions = fs::metadata(&target).expect("metadata").permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&target, permissions).expect("read-only target");
        let read_only_error = persist_project_archive_to_destination(
            &DialogSaveDestination::confirmed(target.clone()),
            &new_archive,
        )
        .expect_err("read-only replacement must fail");
        assert_eq!(fs::read(&target).expect("complete old target"), old_bytes);
        assert_eq!(
            fs::read(&sentinel).expect("sentinel unchanged"),
            sentinel_bytes
        );

        let mut permissions = fs::metadata(&target).expect("metadata").permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&target, permissions).expect("writable target");
        let blocking_handle = fs::OpenOptions::new()
            .read(true)
            .share_mode(super::FILE_SHARE_READ)
            .open(&target)
            .expect("non-delete-sharing handle");
        let sharing_error = persist_project_archive_to_destination(
            &DialogSaveDestination::confirmed(target.clone()),
            &new_archive,
        )
        .expect_err("sharing violation must fail");
        assert_eq!(
            sharing_error, read_only_error,
            "OS reasons must be redacted"
        );
        assert_eq!(fs::read(&target).expect("complete old target"), old_bytes);
        assert_eq!(
            fs::read(&sentinel).expect("sentinel unchanged"),
            sentinel_bytes
        );

        drop(blocking_handle);
        persist_project_archive_to_destination(
            &DialogSaveDestination::confirmed(target.clone()),
            &new_archive,
        )
        .expect("journal remains retryable after fault removal");
        assert_eq!(
            load_project_archive_from_path(&target).expect("complete new target"),
            new_archive
        );
        assert_eq!(
            fs::read(&sentinel).expect("sentinel unchanged"),
            sentinel_bytes
        );
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_recovery_phase_fault_matrix_remains_retryable() {
        use std::os::windows::fs::OpenOptionsExt;

        let old = b"old complete bytes";
        let new = b"new complete bytes";
        for phase in [
            SingleFileJournalPhaseV1::Prepared,
            SingleFileJournalPhaseV1::OldMoved,
            SingleFileJournalPhaseV1::NewPublished,
        ] {
            let directory = journal_test_directory("windows-phase-fault");
            let target = directory.join("project.ori2");
            let temp = directory.join("temp-phase-fault");
            let backup = directory.join("backup-phase-fault");
            let sentinel = directory.join("sentinel");
            fs::write(&sentinel, b"unowned").expect("sentinel");
            match phase {
                SingleFileJournalPhaseV1::Prepared => {
                    fs::write(&target, old).expect("target");
                    fs::write(&temp, new).expect("temp");
                }
                SingleFileJournalPhaseV1::OldMoved => {
                    fs::write(&temp, new).expect("temp");
                    fs::write(&backup, old).expect("backup");
                }
                SingleFileJournalPhaseV1::NewPublished => {
                    fs::write(&target, new).expect("target");
                    fs::write(&backup, old).expect("backup");
                }
            }
            let project_id = ProjectId::new();
            let fingerprint = target_path_fingerprint(&target).expect("fingerprint");
            let payload = SingleFileJournalPayloadV1 {
                schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
                project_id,
                target_path_sha256: fingerprint.clone(),
                transaction_id: "phase-fault".to_owned(),
                temp_object_id: "temp-phase-fault".to_owned(),
                temp_sha256: sha256_hex_bytes(new),
                backup_object_id: "backup-phase-fault".to_owned(),
                old_sha256: Some(sha256_hex_bytes(old)),
                phase,
            };
            let journal = journal_path_for_target(&target, &fingerprint).expect("journal");
            fs::write(
                &journal,
                encode_single_file_journal_v1(payload).expect("journal bytes"),
            )
            .expect("journal");

            let fault_path = if phase == SingleFileJournalPhaseV1::NewPublished {
                &backup
            } else {
                &temp
            };
            let blocker = fs::OpenOptions::new()
                .read(true)
                .share_mode(super::FILE_SHARE_READ)
                .open(fault_path)
                .expect("sharing fault handle");
            assert!(recover_single_file_journal_for_target(&target, project_id).is_err());
            assert!(journal.exists(), "journal must remain retryable");
            assert_eq!(fs::read(&sentinel).expect("sentinel"), b"unowned");
            if phase == SingleFileJournalPhaseV1::Prepared {
                assert_eq!(fs::read(&target).expect("old target"), old);
            } else if phase == SingleFileJournalPhaseV1::NewPublished {
                assert_eq!(fs::read(&target).expect("new target"), new);
            } else {
                assert!(!target.exists());
                assert_eq!(fs::read(&backup).expect("old backup"), old);
            }

            drop(blocker);
            recover_single_file_journal_for_target(&target, project_id).expect("retry recovery");
            let expected: &[u8] = if phase == SingleFileJournalPhaseV1::Prepared {
                old
            } else {
                new
            };
            assert_eq!(fs::read(&target).expect("complete target"), expected);
            assert!(!journal.exists());
            assert_eq!(fs::read(&sentinel).expect("sentinel"), b"unowned");
            fs::remove_dir_all(directory).expect("cleanup test directory");
        }
    }

    #[cfg(unix)]
    #[test]
    fn unix_read_only_parent_redacts_errors_and_retries_after_permission_restore() {
        use std::os::unix::fs::PermissionsExt;

        let directory = journal_test_directory("unix-permission-save");
        let target = directory.join("project.ori2");
        let sentinel = directory.join("sentinel");
        let old_archive = Ori2ProjectArchive::document_only(ProjectDocument::new(
            "old complete",
            CreasePattern::empty(),
        ));
        let new_archive = Ori2ProjectArchive::document_only(ProjectDocument::new(
            "new complete",
            CreasePattern::empty(),
        ));
        let old_bytes = write_project_archive_ori2(&old_archive).expect("old archive");
        fs::write(&target, &old_bytes).expect("old target");
        fs::write(&sentinel, b"unowned").expect("sentinel");

        fs::set_permissions(&directory, fs::Permissions::from_mode(0o555))
            .expect("read-only parent");
        let read_only_error = persist_project_archive_to_destination(
            &DialogSaveDestination::confirmed(target.clone()),
            &new_archive,
        )
        .expect_err("read-only parent must reject save");
        assert_eq!(fs::read(&target).expect("old target"), old_bytes);
        assert_eq!(fs::read(&sentinel).expect("sentinel"), b"unowned");

        fs::set_permissions(&directory, fs::Permissions::from_mode(0o500))
            .expect("owner-only parent");
        let denied_error = persist_project_archive_to_destination(
            &DialogSaveDestination::confirmed(target.clone()),
            &new_archive,
        )
        .expect_err("permission denied must reject save");
        assert_eq!(
            denied_error, read_only_error,
            "raw reasons must be redacted"
        );
        assert_eq!(fs::read(&target).expect("old target"), old_bytes);

        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("restore parent");
        persist_project_archive_to_destination(
            &DialogSaveDestination::confirmed(target.clone()),
            &new_archive,
        )
        .expect("retry after permission restore");
        assert_eq!(
            load_project_archive_from_path(&target).expect("new complete target"),
            new_archive
        );
        assert_eq!(fs::read(&sentinel).expect("sentinel"), b"unowned");
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[cfg(unix)]
    #[test]
    fn unix_recovery_permission_fault_matrix_remains_retryable() {
        use std::os::unix::fs::PermissionsExt;

        let old = b"old complete bytes";
        let new = b"new complete bytes";
        for phase in [
            SingleFileJournalPhaseV1::Prepared,
            SingleFileJournalPhaseV1::OldMoved,
            SingleFileJournalPhaseV1::NewPublished,
        ] {
            let directory = journal_test_directory("unix-phase-permission");
            let target = directory.join("project.ori2");
            let temp = directory.join("temp-phase-permission");
            let backup = directory.join("backup-phase-permission");
            let sentinel = directory.join("sentinel");
            fs::write(&sentinel, b"unowned").expect("sentinel");
            match phase {
                SingleFileJournalPhaseV1::Prepared => {
                    fs::write(&target, old).expect("target");
                    fs::write(&temp, new).expect("temp");
                }
                SingleFileJournalPhaseV1::OldMoved => {
                    fs::write(&temp, new).expect("temp");
                    fs::write(&backup, old).expect("backup");
                }
                SingleFileJournalPhaseV1::NewPublished => {
                    fs::write(&target, new).expect("target");
                    fs::write(&backup, old).expect("backup");
                }
            }
            let project_id = ProjectId::new();
            let fingerprint = target_path_fingerprint(&target).expect("fingerprint");
            let payload = SingleFileJournalPayloadV1 {
                schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
                project_id,
                target_path_sha256: fingerprint.clone(),
                transaction_id: "phase-permission".to_owned(),
                temp_object_id: "temp-phase-permission".to_owned(),
                temp_sha256: sha256_hex_bytes(new),
                backup_object_id: "backup-phase-permission".to_owned(),
                old_sha256: Some(sha256_hex_bytes(old)),
                phase,
            };
            let journal = journal_path_for_target(&target, &fingerprint).expect("journal");
            fs::write(
                &journal,
                encode_single_file_journal_v1(payload).expect("journal bytes"),
            )
            .expect("journal");
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o555))
                .expect("read-only parent");

            assert!(recover_single_file_journal_for_target(&target, project_id).is_err());
            assert!(journal.exists());
            assert_eq!(fs::read(&sentinel).expect("sentinel"), b"unowned");
            if phase == SingleFileJournalPhaseV1::Prepared {
                assert_eq!(fs::read(&target).expect("old target"), old);
            } else if phase == SingleFileJournalPhaseV1::NewPublished {
                assert_eq!(fs::read(&target).expect("new target"), new);
            } else {
                assert!(!target.exists());
                assert_eq!(fs::read(&backup).expect("old backup"), old);
            }

            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .expect("restore parent");
            recover_single_file_journal_for_target(&target, project_id).expect("retry recovery");
            let expected: &[u8] = if phase == SingleFileJournalPhaseV1::Prepared {
                old
            } else {
                new
            };
            assert_eq!(fs::read(&target).expect("complete target"), expected);
            assert!(!journal.exists());
            assert_eq!(fs::read(&sentinel).expect("sentinel"), b"unowned");
            fs::remove_dir_all(directory).expect("cleanup test directory");
        }
    }

    #[test]
    fn journal_decoder_rejects_reserved_and_casefold_colliding_private_names() {
        let project_id = ProjectId::new();
        let fingerprint = "1".repeat(64);
        let base = SingleFileJournalPayloadV1 {
            schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
            project_id,
            target_path_sha256: fingerprint.clone(),
            transaction_id: "transaction".to_owned(),
            temp_object_id: "temp-object".to_owned(),
            temp_sha256: "2".repeat(64),
            backup_object_id: "backup-object".to_owned(),
            old_sha256: Some("3".repeat(64)),
            phase: SingleFileJournalPhaseV1::Prepared,
        };
        for reserved in ["CON", "con.txt", "AUX", "COM1.log", "lpt9"] {
            let mut payload = base.clone();
            payload.temp_object_id = reserved.to_owned();
            let bytes = encode_single_file_journal_v1(payload).expect("encoded journal");
            assert!(decode_single_file_journal_v1(&bytes, project_id, &fingerprint).is_err());
        }
        let mut collision = base;
        collision.temp_object_id = "Private-Object".to_owned();
        collision.backup_object_id = "private-object".to_owned();
        let bytes = encode_single_file_journal_v1(collision).expect("encoded collision");
        assert!(decode_single_file_journal_v1(&bytes, project_id, &fingerprint).is_err());
    }

    #[test]
    fn same_target_single_flight_rejects_double_writer_open_and_aba() {
        let directory = journal_test_directory("single-flight");
        let target = directory.join("project.ori2");
        let old_archive =
            Ori2ProjectArchive::document_only(ProjectDocument::new("old", CreasePattern::empty()));
        let old_bytes = write_project_archive_ori2(&old_archive).expect("old archive");
        fs::write(&target, &old_bytes).expect("old target");
        let owner = acquire_project_file_operation(&target).expect("first owner");
        assert!(acquire_project_file_operation(&target).is_err());
        assert!(load_project_archive_from_path(&target).is_err());

        let other_project = Ori2ProjectArchive::document_only(ProjectDocument::new(
            "other project",
            CreasePattern::empty(),
        ));
        assert!(
            persist_project_archive_to_destination(
                &DialogSaveDestination::confirmed(target.clone()),
                &other_project,
            )
            .is_err()
        );
        assert_eq!(fs::read(&target).expect("target preserved"), old_bytes);
        assert_eq!(fs::read_dir(&directory).expect("directory").count(), 1);

        drop(owner);
        let next_owner = acquire_project_file_operation(&directory.join("./project.ori2"))
            .expect("canonical alias acquires only after release");
        assert!(acquire_project_file_operation(&target).is_err());
        drop(next_owner);
        assert_eq!(
            load_project_archive_from_path(&target).expect("open after release"),
            old_archive
        );
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[test]
    fn single_flight_guard_is_released_during_panic_unwind() {
        let directory = journal_test_directory("single-flight-panic");
        let target = directory.join("project.ori2");
        let unwind = std::panic::catch_unwind(|| {
            let _owner = acquire_project_file_operation(&target).expect("first owner");
            panic!("simulate writer panic");
        });
        assert!(unwind.is_err());
        let recovered = acquire_project_file_operation(&target)
            .expect("panic drop must release the target operation");
        drop(recovered);
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[test]
    fn single_flight_normalizes_unicode_aliases_and_allows_distinct_targets() {
        let directory = journal_test_directory("single-flight-unicode");
        let composed = directory.join("caf\u{e9}.ori2");
        let decomposed = directory.join("cafe\u{301}.ori2");
        let other = directory.join("other.ori2");
        let owner = acquire_project_file_operation(&composed).expect("composed owner");
        assert!(acquire_project_file_operation(&decomposed).is_err());
        let other_owner = acquire_project_file_operation(&other)
            .expect("an unrelated target may proceed concurrently");
        drop(other_owner);
        drop(owner);
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[test]
    fn single_flight_rejects_hardlink_alias_to_owned_target() {
        let directory = journal_test_directory("single-flight-hardlink");
        let target = directory.join("project.ori2");
        let alias = directory.join("alias.ori2");
        fs::write(&target, b"same object").expect("target");
        fs::hard_link(&target, &alias).expect("hardlink alias");
        let owner = acquire_project_file_operation(&target).expect("target owner");
        assert!(acquire_project_file_operation(&alias).is_err());
        drop(owner);
        drop(acquire_project_file_operation(&alias).expect("released alias"));
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[cfg(unix)]
    #[test]
    fn single_flight_rejects_symlink_target_without_following_it() {
        use std::os::unix::fs::symlink;

        let directory = journal_test_directory("single-flight-symlink");
        let target = directory.join("project.ori2");
        let alias = directory.join("alias.ori2");
        fs::write(&target, b"preserve").expect("target");
        symlink(&target, &alias).expect("symlink alias");
        assert!(acquire_project_file_operation(&alias).is_err());
        assert_eq!(fs::read(&target).expect("target preserved"), b"preserve");
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[test]
    fn single_flight_ownership_set_returns_to_baseline_after_many_paths() {
        let directory = journal_test_directory("single-flight-bounded");
        let baseline = super::ACTIVE_PROJECT_FILE_OPERATIONS
            .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
            .lock()
            .expect("operation set")
            .len();
        for index in 0..512 {
            drop(
                acquire_project_file_operation(
                    &directory.join(format!("distinct-{index:04}.ori2")),
                )
                .expect("distinct target"),
            );
        }
        assert_eq!(
            super::ACTIVE_PROJECT_FILE_OPERATIONS
                .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
                .lock()
                .expect("operation set")
                .len(),
            baseline
        );
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[cfg(windows)]
    #[test]
    fn single_flight_rejects_windows_case_alias() {
        let directory = journal_test_directory("single-flight-case");
        let owner = acquire_project_file_operation(&directory.join("Project.ori2"))
            .expect("mixed-case owner");
        assert!(acquire_project_file_operation(&directory.join("project.ORI2")).is_err());
        drop(owner);
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[cfg(unix)]
    #[test]
    fn single_flight_keeps_unix_case_sensitive_targets_distinct() {
        let directory = journal_test_directory("single-flight-case");
        let owner = acquire_project_file_operation(&directory.join("Project.ori2"))
            .expect("mixed-case owner");
        let lower_owner = acquire_project_file_operation(&directory.join("project.ori2"))
            .expect("Unix case-sensitive target");
        drop(lower_owner);
        drop(owner);
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }

    #[cfg(unix)]
    #[test]
    fn journal_symlink_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let directory = journal_test_directory("nofollow");
        let target = directory.join("project.ori2");
        let sentinel = directory.join("outside-sentinel");
        fs::write(&sentinel, b"preserve").expect("sentinel");
        let fingerprint = target_path_fingerprint(&target).expect("fingerprint");
        let journal = journal_path_for_target(&target, &fingerprint).expect("journal path");
        symlink(&sentinel, &journal).expect("journal symlink");
        assert!(recover_single_file_journal_for_target(&target, ProjectId::new()).is_err());
        assert_eq!(
            fs::read(&sentinel).expect("sentinel preserved"),
            b"preserve"
        );
        fs::remove_file(&journal).expect("remove symlink");
        fs::remove_dir_all(directory).expect("cleanup test directory");
    }
}

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
mod recovery_entry_tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use ori_domain::CreasePattern;

    use super::*;

    static NEXT_TEST_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            for _ in 0..128 {
                let id = NEXT_TEST_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "origami2-recovery-entry-{label}-{}-{id}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create recovery entry test directory: {error}"),
                }
            }
            panic!("allocate recovery entry test directory");
        }

        fn slot(&self) -> PathBuf {
            self.0.join("current-project.ori2")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            remove_test_entry(&self.slot());
            for name in RECOVERY_QUARANTINE_NAMES {
                remove_test_entry(&self.0.join(name));
            }
            remove_test_entry(&self.0.join("target-file.ori2"));
            remove_test_entry(&self.0.join("target-directory"));
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn remove_test_entry(path: &Path) {
        let _ = fs::remove_file(path.join("sentinel"));
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(path);
    }

    fn write_valid_document(path: &Path) {
        let document = ProjectDocument::new("recovery test", CreasePattern::empty());
        let bytes = write_project_archive_ori2(&Ori2ProjectArchive::document_only(document))
            .expect("serialize recovery test project");
        fs::write(path, bytes).expect("write recovery test project");
    }

    #[test]
    fn missing_recovery_entry_clear_is_idempotent() {
        let directory = TestDirectory::new("missing");
        let slot = directory.slot();

        assert_eq!(
            inspect_recovery_project(&slot),
            RecoveryProjectLoad::Missing
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert_eq!(clear_recovery_document(&slot), Ok(()));
    }

    #[test]
    fn corrupt_regular_recovery_entry_is_retired_and_unlinked() {
        let directory = TestDirectory::new("corrupt-file");
        let slot = directory.slot();
        fs::write(&slot, b"not an ori2 archive").expect("write corrupt recovery entry");

        assert_eq!(
            inspect_recovery_project(&slot),
            RecoveryProjectLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert!(matches!(
            fs::symlink_metadata(&slot),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        assert!(RECOVERY_QUARANTINE_NAMES.iter().all(|name| {
            matches!(
                fs::symlink_metadata(directory.0.join(name)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound
            )
        }));
    }

    #[test]
    fn empty_real_directory_recovery_entry_is_retired_then_removed() {
        let directory = TestDirectory::new("empty-directory");
        let slot = directory.slot();
        fs::create_dir(&slot).expect("create empty recovery directory");

        assert_eq!(
            inspect_recovery_project(&slot),
            RecoveryProjectLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert!(!slot.exists());
    }

    #[test]
    fn nonempty_real_directory_is_moved_out_of_the_active_slot_without_recursion() {
        let directory = TestDirectory::new("nonempty-directory");
        let slot = directory.slot();
        fs::create_dir(&slot).expect("create nonempty recovery directory");
        fs::write(slot.join("sentinel"), b"keep").expect("write recovery sentinel");

        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert!(matches!(
            fs::symlink_metadata(&slot),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        let occupied = RECOVERY_QUARANTINE_NAMES
            .iter()
            .map(|name| directory.0.join(name))
            .filter(|path| fs::symlink_metadata(path).is_ok())
            .collect::<Vec<_>>();
        assert_eq!(occupied.len(), 1);
        assert_eq!(
            fs::read(occupied[0].join("sentinel")).expect("read quarantined sentinel"),
            b"keep"
        );
    }

    #[test]
    fn eight_nonempty_quarantine_directories_fail_closed_without_moving_active_entry() {
        let directory = TestDirectory::new("quarantine-full");
        let slot = directory.slot();
        fs::write(&slot, b"active").expect("write active recovery entry");
        for name in RECOVERY_QUARANTINE_NAMES {
            let quarantine = directory.0.join(name);
            fs::create_dir(&quarantine).expect("create occupied quarantine");
            fs::write(quarantine.join("sentinel"), b"keep").expect("write quarantine sentinel");
        }

        assert_eq!(
            clear_recovery_document(&slot),
            Err(RecoveryPersistenceError)
        );
        assert_eq!(fs::read(&slot).expect("read active entry"), b"active");
        for name in RECOVERY_QUARANTINE_NAMES {
            assert_eq!(
                fs::read(directory.0.join(name).join("sentinel"))
                    .expect("read retained quarantine sentinel"),
                b"keep"
            );
        }
    }

    #[test]
    fn removable_stale_quarantine_entry_is_unlinked_before_exclusive_retirement() {
        let directory = TestDirectory::new("stale-quarantine");
        let slot = directory.slot();
        fs::write(&slot, b"active").expect("write active recovery entry");
        let first_quarantine = directory.0.join(RECOVERY_QUARANTINE_NAMES[0]);
        fs::write(&first_quarantine, b"stale").expect("write stale quarantine entry");

        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert!(!slot.exists());
        assert!(!first_quarantine.exists());
    }

    #[cfg(unix)]
    #[test]
    fn final_component_file_symlink_is_invalid_and_clear_never_follows_target() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("file-symlink");
        let target = directory.0.join("target-file.ori2");
        let slot = directory.slot();
        write_valid_document(&target);
        assert!(matches!(
            inspect_recovery_project(&target),
            RecoveryProjectLoad::Available { .. }
        ));
        symlink(&target, &slot).expect("create recovery file symlink");

        assert_eq!(
            inspect_recovery_project(&slot),
            RecoveryProjectLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert!(matches!(
            inspect_recovery_project(&target),
            RecoveryProjectLoad::Available { .. }
        ));
        assert!(matches!(
            fs::symlink_metadata(&slot),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[cfg(unix)]
    #[test]
    fn final_component_directory_symlink_is_unlinked_without_touching_target_contents() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("directory-symlink");
        let target = directory.0.join("target-directory");
        let slot = directory.slot();
        fs::create_dir(&target).expect("create symlink target directory");
        fs::write(target.join("sentinel"), b"keep").expect("write target sentinel");
        symlink(&target, &slot).expect("create recovery directory symlink");

        assert_eq!(
            inspect_recovery_project(&slot),
            RecoveryProjectLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert_eq!(
            fs::read(target.join("sentinel")).expect("read target sentinel"),
            b"keep"
        );
    }

    #[cfg(unix)]
    #[test]
    fn final_component_fifo_is_retired_as_a_special_entry_without_opening_it() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};

        let directory = TestDirectory::new("fifo");
        let slot = directory.slot();
        let slot_c = CString::new(slot.as_os_str().as_bytes()).expect("fifo path has no NUL");
        let created = unsafe {
            // SAFETY: `slot_c` is a live NUL-terminated path and the mode is a
            // conventional owner-only FIFO mode.
            libc::mkfifo(slot_c.as_ptr(), 0o600)
        };
        assert_eq!(created, 0, "create recovery FIFO");

        assert_eq!(
            inspect_recovery_project(&slot),
            RecoveryProjectLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert!(matches!(
            fs::symlink_metadata(&slot),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn exclusive_retirement_handle_blocks_path_replacement_until_it_is_released() {
        let directory = TestDirectory::new("windows-exclusive-retirement");
        let slot = directory.slot();
        let retired = directory.0.join(RECOVERY_QUARANTINE_NAMES[0]);
        fs::write(&slot, b"active").expect("write active recovery entry");
        let handle = open_recovery_entry_for_exclusive_retirement(&slot)
            .expect("open exclusive retirement handle");

        assert!(
            fs::rename(&slot, &retired).is_err(),
            "another path handle must not rename the active slot"
        );
        assert!(
            fs::remove_file(&slot).is_err(),
            "another path handle must not delete the active slot"
        );
        assert_eq!(
            fs::read(&slot).expect("read retained active slot"),
            b"active"
        );

        drop(handle);
        fs::rename(&slot, &retired).expect("rename succeeds after ownership is released");
        assert_eq!(fs::read(&retired).expect("read retired entry"), b"active");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn final_component_file_reparse_point_is_invalid_and_clear_preserves_target() {
        use std::os::windows::fs::symlink_file;

        let directory = TestDirectory::new("windows-file-link");
        let target = directory.0.join("target-file.ori2");
        let slot = directory.slot();
        write_valid_document(&target);
        if symlink_file(&target, &slot).is_err() {
            // Symlink creation still requires Developer Mode or a privilege on
            // some supported Windows configurations.
            return;
        }

        assert_eq!(
            inspect_recovery_project(&slot),
            RecoveryProjectLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert!(matches!(
            inspect_recovery_project(&target),
            RecoveryProjectLoad::Available { .. }
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn final_component_directory_reparse_point_clear_preserves_target_contents() {
        use std::os::windows::fs::symlink_dir;

        let directory = TestDirectory::new("windows-directory-link");
        let target = directory.0.join("target-directory");
        let slot = directory.slot();
        fs::create_dir(&target).expect("create directory link target");
        fs::write(target.join("sentinel"), b"keep").expect("write target sentinel");
        if symlink_dir(&target, &slot).is_err() {
            return;
        }

        assert_eq!(
            inspect_recovery_project(&slot),
            RecoveryProjectLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert_eq!(
            fs::read(target.join("sentinel")).expect("read target sentinel"),
            b"keep"
        );
    }
}

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
