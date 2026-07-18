use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use ori_formats::{Ori2Limits, ProjectDocument, read_project_ori2_with_limits, write_project_ori2};

use super::{
    save_path::{DialogSaveDestination, ExistingDestinationPolicy},
    validate_document_instruction_poses,
};

#[cfg(target_os = "windows")]
use std::os::windows::fs::OpenOptionsExt;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
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
fn publish_unix_staged_file(
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
                match std::fs::metadata(path) {
                    Ok(metadata) if metadata.is_file() => staged
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
