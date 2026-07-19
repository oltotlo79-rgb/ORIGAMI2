use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, UNIX_EPOCH},
};

use ori_formats::{Ori2Limits, ProjectDocument, read_project_ori2_with_limits, write_project_ori2};
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
    DELETE, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE,
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
pub(super) enum RecoveryDocumentLoad {
    Missing,
    Available {
        document: Box<ProjectDocument>,
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

pub(super) fn load_document_from_path(path: &Path) -> Result<ProjectDocument, String> {
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

    let document = read_project_ori2_with_limits(&bytes, limits)
        .map_err(|_| PROJECT_FILE_INVALID_MESSAGE.to_owned())?;
    validate_document_instruction_poses(&document)
        .map_err(|_| PROJECT_INSTRUCTIONS_INVALID_MESSAGE.to_owned())?;
    Ok(document)
}

pub(super) fn inspect_recovery_document(path: &Path) -> RecoveryDocumentLoad {
    let limits = Ori2Limits::default();
    let entry_metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata_is_plain_regular_file(&metadata) => metadata,
        Ok(_) => return RecoveryDocumentLoad::Invalid,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return RecoveryDocumentLoad::Missing;
        }
        Err(_) => return RecoveryDocumentLoad::Invalid,
    };
    if entry_metadata.len() > limits.max_archive_size {
        return RecoveryDocumentLoad::Invalid;
    }

    let file = match open_recovery_regular_file_no_follow(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return RecoveryDocumentLoad::Missing;
        }
        Err(_) => return RecoveryDocumentLoad::Invalid,
    };
    let metadata = match file.metadata() {
        Ok(metadata) if metadata_is_plain_regular_file(&metadata) => metadata,
        Ok(_) | Err(_) => return RecoveryDocumentLoad::Invalid,
    };
    if metadata.len() > limits.max_archive_size {
        return RecoveryDocumentLoad::Invalid;
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
        return RecoveryDocumentLoad::Invalid;
    }
    let Ok(document) = read_project_ori2_with_limits(&bytes, limits) else {
        return RecoveryDocumentLoad::Invalid;
    };
    if validate_document_instruction_poses(&document).is_err() {
        return RecoveryDocumentLoad::Invalid;
    }
    RecoveryDocumentLoad::Available {
        document: Box::new(document),
        updated_at_unix_ms,
    }
}

#[cfg(unix)]
fn open_recovery_regular_file_no_follow(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    options.open(path)
}

#[cfg(target_os = "windows")]
fn open_recovery_regular_file_no_follow(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .access_mode(FILE_GENERIC_READ)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn open_recovery_regular_file_no_follow(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

fn metadata_is_plain_regular_file(metadata: &std::fs::Metadata) -> bool {
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
/// Callers pass a detached [`ProjectDocument`], so no live project mutex needs
/// to remain held while serialization, synchronization, verification, and
/// publication perform filesystem I/O.
pub(super) fn persist_recovery_document(
    path: &Path,
    document: &ProjectDocument,
) -> Result<(), RecoveryPersistenceError> {
    let mut staged = prepare_recovery_staged_file(path, document)?;
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
    document: &ProjectDocument,
) -> Result<StagedFile, RecoveryPersistenceError> {
    if path.file_name().is_none() {
        return Err(RecoveryPersistenceError);
    }
    let parent = containing_directory(path).ok_or(RecoveryPersistenceError)?;
    std::fs::create_dir_all(parent).map_err(|_| RecoveryPersistenceError)?;
    validate_document_instruction_poses(document).map_err(|_| RecoveryPersistenceError)?;
    let bytes = write_project_ori2(document).map_err(|_| RecoveryPersistenceError)?;
    prepare_staged_file(path, document, &bytes).map_err(|_| RecoveryPersistenceError)
}

#[cfg(test)]
pub(super) fn stage_recovery_document_for_test(
    path: &Path,
    document: &ProjectDocument,
) -> Result<StagedFile, RecoveryPersistenceError> {
    prepare_recovery_staged_file(path, document)
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
pub(super) fn persist_document(path: &Path, document: &ProjectDocument) -> Result<(), String> {
    persist_document_to_destination(
        &DialogSaveDestination::confirmed(path.to_path_buf()),
        document,
    )
}

pub(super) fn persist_document_to_destination(
    destination: &DialogSaveDestination,
    document: &ProjectDocument,
) -> Result<(), String> {
    let path = destination.path();
    if path.file_name().is_none() {
        return Err("選択された保存先はファイルパスではありません。".to_owned());
    }

    validate_document_instruction_poses(document)
        .map_err(|_| PROJECT_INSTRUCTIONS_SAVE_FAILED_MESSAGE.to_owned())?;
    let bytes = write_project_ori2(document)
        .map_err(|_| PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())?;
    let result = persist_document_atomically(
        path,
        document,
        &bytes,
        destination.existing_destination_policy(),
    );
    match destination.existing_destination_policy() {
        ExistingDestinationPolicy::RejectExisting => result.map_err(|_| {
            "拡張子を補正した保存先を安全に確定できなかったため、保存を中止しました。".to_owned()
        }),
        ExistingDestinationPolicy::ReplaceConfirmed => result.map_err(|_| {
            "プロジェクトを保存先へ安全に確定できなかったため、保存を中止しました。".to_owned()
        }),
    }
}

#[cfg(not(target_os = "windows"))]
fn persist_document_atomically(
    path: &Path,
    document: &ProjectDocument,
    bytes: &[u8],
    existing_destination_policy: ExistingDestinationPolicy,
) -> Result<(), String> {
    let mut staged = prepare_staged_file(path, document, bytes)?;
    let parent = containing_directory(path)
        .ok_or_else(|| format!("{} is not a file path", path.display()))?;
    let directory = File::open(parent).map_err(|error| {
        format!(
            "failed to open the project directory for {}: {error}",
            path.display()
        )
    })?;
    commit_unix_staged_project_file(&mut staged, path, existing_destination_policy, || {
        directory.sync_all()
    })
    .map_err(|error| {
        format!(
            "failed to commit and synchronize {} atomically: {error}",
            path.display()
        )
    })?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn persist_document_atomically(
    path: &Path,
    document: &ProjectDocument,
    bytes: &[u8],
    existing_destination_policy: ExistingDestinationPolicy,
) -> Result<(), String> {
    let mut staged = prepare_staged_file(path, document, bytes)?;
    super::rename_windows_staged_file_with_policy(
        staged.file(),
        path,
        existing_destination_policy,
    )?;
    staged.committed = true;
    Ok(())
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
    document: &ProjectDocument,
    bytes: &[u8],
) -> Result<StagedFile, String> {
    let mut staged = create_staged_file(path)?;
    staged.file_mut().write_all(bytes).map_err(|error| {
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
    verify_generated_ori2(document, &staged_bytes)?;
    Ok(staged)
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
    document: &ProjectDocument,
    bytes: &[u8],
) -> Result<(), String> {
    let verified = read_project_ori2_with_limits(bytes, Ori2Limits::default())
        .map_err(|error| format!("generated .ori2 data did not pass validation: {error}"))?;
    if verified != *document {
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
        let bytes = write_project_ori2(&document).expect("serialize recovery test project");
        fs::write(path, bytes).expect("write recovery test project");
    }

    #[test]
    fn missing_recovery_entry_clear_is_idempotent() {
        let directory = TestDirectory::new("missing");
        let slot = directory.slot();

        assert_eq!(
            inspect_recovery_document(&slot),
            RecoveryDocumentLoad::Missing
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
            inspect_recovery_document(&slot),
            RecoveryDocumentLoad::Invalid
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
            inspect_recovery_document(&slot),
            RecoveryDocumentLoad::Invalid
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
            inspect_recovery_document(&target),
            RecoveryDocumentLoad::Available { .. }
        ));
        symlink(&target, &slot).expect("create recovery file symlink");

        assert_eq!(
            inspect_recovery_document(&slot),
            RecoveryDocumentLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert!(matches!(
            inspect_recovery_document(&target),
            RecoveryDocumentLoad::Available { .. }
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
            inspect_recovery_document(&slot),
            RecoveryDocumentLoad::Invalid
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
            inspect_recovery_document(&slot),
            RecoveryDocumentLoad::Invalid
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
            inspect_recovery_document(&slot),
            RecoveryDocumentLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert!(matches!(
            inspect_recovery_document(&target),
            RecoveryDocumentLoad::Available { .. }
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
            inspect_recovery_document(&slot),
            RecoveryDocumentLoad::Invalid
        );
        assert_eq!(clear_recovery_document(&slot), Ok(()));
        assert_eq!(
            fs::read(target.join("sentinel")).expect("read target sentinel"),
            b"keep"
        );
    }
}
