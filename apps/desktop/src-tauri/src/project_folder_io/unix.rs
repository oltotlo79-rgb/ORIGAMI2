use std::{
    ffi::{CStr, CString, OsString},
    fs::{File, Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::{
            ffi::OsStringExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Path, PathBuf},
};

use super::{DirectoryIdentity, FsResult, ProjectFolderFilesystemError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjectIdentity {
    device: u64,
    inode: u64,
}

impl ObjectIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

pub(super) struct PinnedDirectory {
    path: PathBuf,
    file: File,
    identity: ObjectIdentity,
}

impl PinnedDirectory {
    pub(super) fn open_selected(path: &Path) -> FsResult<Self> {
        let before = std::fs::symlink_metadata(path).map_err(map_open_error)?;
        if before.file_type().is_symlink() || !before.file_type().is_dir() {
            return Err(ProjectFolderFilesystemError::LinkOrSpecialEntry);
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let file = options.open(path).map_err(map_open_error)?;
        let metadata = file.metadata().map_err(map_read_error)?;
        if !metadata.file_type().is_dir()
            || ObjectIdentity::from_metadata(&before) != ObjectIdentity::from_metadata(&metadata)
        {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        Ok(Self {
            path: path.to_path_buf(),
            file,
            identity: ObjectIdentity::from_metadata(&metadata),
        })
    }

    pub(super) fn list_names(&self, maximum: usize) -> FsResult<Vec<OsString>> {
        list_directory_names(self.file.as_raw_fd(), maximum, None)
    }

    pub(super) fn list_names_with_ascii_prefix(
        &self,
        prefix: &str,
        maximum: usize,
    ) -> FsResult<Vec<OsString>> {
        list_directory_names(self.file.as_raw_fd(), maximum, Some(prefix.as_bytes()))
    }

    pub(super) fn open_child_directory(&self, name: &str) -> FsResult<Self> {
        open_directory_at(self, name, false, false)
    }

    pub(super) fn open_child_directory_for_rename(&self, name: &str) -> FsResult<Self> {
        open_directory_at(self, name, false, true)
    }

    pub(super) fn create_child_directory(&self, name: &str, _deletable: bool) -> FsResult<Self> {
        let name_c = c_name(name)?;
        let created = unsafe {
            // SAFETY: the parent fd and C string remain live for the call.
            libc::mkdirat(self.file.as_raw_fd(), name_c.as_ptr(), 0o700)
        };
        if created != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EEXIST) {
                return Err(ProjectFolderFilesystemError::TargetExists);
            }
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        open_directory_at(self, name, true, true)
    }

    pub(super) fn open_child_file(&self, name: &str, limit: u64) -> FsResult<PinnedFile> {
        self.open_child_file_with_delete(name, limit, false)
    }

    pub(super) fn open_child_file_for_update(
        &self,
        name: &str,
        limit: u64,
    ) -> FsResult<PinnedFile> {
        self.open_child_file_with_delete(name, limit, true)
    }

    fn open_child_file_with_delete(
        &self,
        name: &str,
        limit: u64,
        deletable: bool,
    ) -> FsResult<PinnedFile> {
        let name_c = c_name(name)?;
        let fd = unsafe {
            // SAFETY: the parent fd and C string remain live for the call.
            libc::openat(
                self.file.as_raw_fd(),
                name_c.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(map_open_error(std::io::Error::last_os_error()));
        }
        let file = unsafe {
            // SAFETY: `openat` returned a new owned descriptor.
            File::from_raw_fd(fd)
        };
        PinnedFile::admit(file, limit, deletable)
    }

    pub(super) fn write_child_file(&self, name: &str, bytes: &[u8]) -> FsResult<()> {
        let pinned = self.write_child_file_pinned(name, bytes)?;
        let identity = pinned.identity;
        drop(pinned);
        let reopened = self.open_child_file(name, bytes.len() as u64)?;
        if reopened.identity != identity || reopened.declared_size != bytes.len() as u64 {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        Ok(())
    }

    pub(super) fn write_child_file_pinned(&self, name: &str, bytes: &[u8]) -> FsResult<PinnedFile> {
        let name_c = c_name(name)?;
        let fd = unsafe {
            // SAFETY: the parent fd and C string remain live for the call.
            libc::openat(
                self.file.as_raw_fd(),
                name_c.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EEXIST) {
                return Err(ProjectFolderFilesystemError::TargetExists);
            }
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        let mut file = unsafe {
            // SAFETY: `openat` returned a new owned descriptor.
            File::from_raw_fd(fd)
        };
        let initial = validate_plain_file(&file.metadata().map_err(map_write_error)?, None)?;
        file.write_all(bytes).map_err(map_write_error)?;
        file.sync_all().map_err(map_write_error)?;
        file.seek(SeekFrom::Start(0)).map_err(map_write_error)?;
        let mut readback = Vec::with_capacity(bytes.len());
        file.read_to_end(&mut readback).map_err(map_write_error)?;
        if readback != bytes {
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        let after = validate_plain_file(&file.metadata().map_err(map_write_error)?, None)?;
        if after.identity != initial.identity || after.size != bytes.len() as u64 {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        Ok(PinnedFile {
            file,
            identity: initial.identity,
            declared_size: bytes.len() as u64,
            deletable: true,
        })
    }

    pub(super) fn child_exists(&self, name: &str) -> FsResult<bool> {
        let name_c = c_name(name)?;
        let mut status = std::mem::MaybeUninit::<libc::stat>::uninit();
        let result = unsafe {
            // SAFETY: status points to writable storage and the inputs remain
            // live throughout this no-follow metadata query.
            libc::fstatat(
                self.file.as_raw_fd(),
                name_c.as_ptr(),
                status.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result == 0 {
            return Ok(true);
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOENT) {
            Ok(false)
        } else {
            Err(ProjectFolderFilesystemError::ReadFailed)
        }
    }

    pub(super) fn revalidate_selected_path(&self) -> FsResult<()> {
        let metadata = std::fs::symlink_metadata(&self.path).map_err(map_read_error)?;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_dir()
            || ObjectIdentity::from_metadata(&metadata) != self.identity
        {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        let handle_metadata = self.file.metadata().map_err(map_read_error)?;
        if !handle_metadata.file_type().is_dir()
            || ObjectIdentity::from_metadata(&handle_metadata) != self.identity
        {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        Ok(())
    }

    pub(super) fn revalidate_child_directory(&self, name: &str, child: &Self) -> FsResult<()> {
        let reopened = open_directory_at(self, name, false, false)?;
        if reopened.identity != child.identity {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        child.revalidate_selected_path()
    }

    pub(super) fn identity(&self) -> DirectoryIdentity {
        DirectoryIdentity {
            first: self.identity.device,
            second: self.identity.inode,
            third: 0,
        }
    }

    pub(super) fn ensure_stable_replacement_identity(&self) -> FsResult<()> {
        self.revalidate_selected_path()
    }

    pub(super) fn sync_directory(&self) -> FsResult<()> {
        self.file
            .sync_all()
            .map_err(|_| ProjectFolderFilesystemError::WriteFailed)
    }

    pub(super) fn remove_child_file_if_same(&self, name: &str, child: &PinnedFile) -> FsResult<()> {
        self.revalidate_child_file(name, child)?;
        if !child.deletable {
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        unlink_at(self.file.as_raw_fd(), name, 0)
    }

    pub(super) fn remove_child_directory_if_same(&self, name: &str, child: &Self) -> FsResult<()> {
        self.revalidate_child_directory(name, child)?;
        unlink_at(self.file.as_raw_fd(), name, libc::AT_REMOVEDIR)
    }

    pub(super) fn publish_child_directory_no_replace(
        &self,
        source_name: &str,
        source: &mut Self,
        target_name: &str,
    ) -> FsResult<()> {
        self.revalidate_child_directory(source_name, source)?;
        let source_c = c_name(source_name)?;
        let target_c = c_name(target_name)?;

        #[cfg(any(target_os = "linux", target_os = "android"))]
        let renamed = unsafe {
            // SAFETY: names and the pinned parent fd remain live. Both names
            // are relative to the same directory, and RENAME_NOREPLACE makes
            // destination reservation part of the rename.
            libc::syscall(
                libc::SYS_renameat2,
                self.file.as_raw_fd(),
                source_c.as_ptr(),
                self.file.as_raw_fd(),
                target_c.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        let renamed = unsafe {
            // SAFETY: inputs remain live and RENAME_EXCL is the macOS
            // no-replace counterpart.
            libc::renameatx_np(
                self.file.as_raw_fd(),
                source_c.as_ptr(),
                self.file.as_raw_fd(),
                target_c.as_ptr(),
                libc::RENAME_EXCL,
            ) as libc::c_long
        };
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        let renamed: libc::c_long = return Err(ProjectFolderFilesystemError::WriteFailed);

        if renamed == 0 {
            let reopened = self.open_child_directory(target_name)?;
            if reopened.identity != source.identity {
                return Err(ProjectFolderFilesystemError::ChangedDuringRead);
            }
            source.path = self.path.join(target_name);
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if matches!(
            error.raw_os_error(),
            Some(code) if code == libc::EEXIST || code == libc::ENOTEMPTY
        ) {
            Err(ProjectFolderFilesystemError::TargetExists)
        } else {
            Err(ProjectFolderFilesystemError::WriteFailed)
        }
    }

    pub(super) fn publish_child_file_no_replace(
        &self,
        source_name: &str,
        source: &PinnedFile,
        target_name: &str,
    ) -> FsResult<()> {
        self.revalidate_child_file(source_name, source)?;
        rename_child_no_replace(self.file.as_raw_fd(), source_name, target_name)?;
        let reopened = self.open_child_file(target_name, source.declared_size)?;
        if reopened.identity != source.identity || reopened.declared_size != source.declared_size {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        Ok(())
    }

    fn revalidate_child_file(&self, name: &str, child: &PinnedFile) -> FsResult<()> {
        let reopened = self.open_child_file(name, child.declared_size)?;
        if reopened.identity != child.identity || reopened.declared_size != child.declared_size {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        Ok(())
    }
}

pub(super) struct PinnedFile {
    file: File,
    identity: ObjectIdentity,
    declared_size: u64,
    deletable: bool,
}

impl PinnedFile {
    fn admit(file: File, limit: u64, deletable: bool) -> FsResult<Self> {
        let admitted = validate_plain_file(&file.metadata().map_err(map_read_error)?, Some(limit))?;
        Ok(Self {
            file,
            identity: admitted.identity,
            declared_size: admitted.size,
            deletable,
        })
    }

    pub(super) const fn declared_size(&self) -> u64 {
        self.declared_size
    }

    pub(super) fn read_bounded_and_revalidate(
        &mut self,
        parent: &PinnedDirectory,
        name: &str,
        limit: u64,
    ) -> FsResult<Vec<u8>> {
        if self.declared_size > limit {
            return Err(ProjectFolderFilesystemError::TooLarge);
        }
        let capacity = usize::try_from(self.declared_size)
            .unwrap_or(0)
            .min(usize::try_from(limit).unwrap_or(usize::MAX));
        let mut bytes = Vec::with_capacity(capacity);
        (&mut self.file)
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(map_read_error)?;
        if bytes.len() as u64 > limit {
            return Err(ProjectFolderFilesystemError::TooLarge);
        }
        let after =
            validate_plain_file(&self.file.metadata().map_err(map_read_error)?, Some(limit))?;
        if after.identity != self.identity
            || after.size != self.declared_size
            || after.size != bytes.len() as u64
        {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        let reopened = parent.open_child_file(name, limit)?;
        if reopened.identity != self.identity || reopened.declared_size != self.declared_size {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        Ok(bytes)
    }
}

struct AdmittedFile {
    identity: ObjectIdentity,
    size: u64,
}

fn validate_plain_file(metadata: &Metadata, limit: Option<u64>) -> FsResult<AdmittedFile> {
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(ProjectFolderFilesystemError::LinkOrSpecialEntry);
    }
    if metadata.nlink() != 1 {
        return Err(ProjectFolderFilesystemError::LinkOrSpecialEntry);
    }
    if limit.is_some_and(|limit| metadata.len() > limit) {
        return Err(ProjectFolderFilesystemError::TooLarge);
    }
    Ok(AdmittedFile {
        identity: ObjectIdentity::from_metadata(metadata),
        size: metadata.len(),
    })
}

fn open_directory_at(
    parent: &PinnedDirectory,
    name: &str,
    created_by_us: bool,
    require_current_owner: bool,
) -> FsResult<PinnedDirectory> {
    let name_c = c_name(name)?;
    let fd = unsafe {
        // SAFETY: parent fd and C string remain live.
        libc::openat(
            parent.file.as_raw_fd(),
            name_c.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        if created_by_us && error.raw_os_error() == Some(libc::ENOENT) {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        return Err(map_open_error(error));
    }
    let file = unsafe {
        // SAFETY: `openat` returned a new owned descriptor.
        File::from_raw_fd(fd)
    };
    let metadata = file.metadata().map_err(map_read_error)?;
    if !metadata.file_type().is_dir() {
        return Err(ProjectFolderFilesystemError::LinkOrSpecialEntry);
    }
    if require_current_owner && metadata.uid() != unsafe { libc::geteuid() } {
        return Err(ProjectFolderFilesystemError::WriteFailed);
    }
    Ok(PinnedDirectory {
        path: parent.path.join(name),
        file,
        identity: ObjectIdentity::from_metadata(&metadata),
    })
}

fn list_directory_names(
    directory_fd: RawFd,
    maximum: usize,
    prefix: Option<&[u8]>,
) -> FsResult<Vec<OsString>> {
    let dot = b".\0";
    let independent = unsafe {
        // SAFETY: `directory_fd` is live and `dot` is NUL terminated.
        // Opening "." creates an independent open-file description. `dup`
        // would share the directory offset, making a second enumeration
        // incorrectly begin at EOF.
        libc::openat(
            directory_fd,
            dot.as_ptr().cast(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if independent < 0 {
        return Err(ProjectFolderFilesystemError::ReadFailed);
    }
    let stream = unsafe {
        // SAFETY: fdopendir takes ownership of the independent descriptor.
        libc::fdopendir(independent)
    };
    if stream.is_null() {
        unsafe {
            // SAFETY: fdopendir failed and did not take ownership.
            libc::close(independent);
        }
        return Err(ProjectFolderFilesystemError::ReadFailed);
    }

    let mut names = Vec::new();
    loop {
        clear_errno();
        let entry = unsafe {
            // SAFETY: stream remains live until closed below.
            libc::readdir(stream)
        };
        if entry.is_null() {
            if current_errno() != 0 {
                unsafe {
                    // SAFETY: stream is live and closed exactly once.
                    libc::closedir(stream);
                }
                return Err(ProjectFolderFilesystemError::ReadFailed);
            }
            break;
        }
        let name = unsafe {
            // SAFETY: readdir guarantees a NUL-terminated d_name for this
            // entry until the next readdir call.
            CStr::from_ptr((*entry).d_name.as_ptr())
        }
        .to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if prefix.is_some_and(|prefix| !name.starts_with(prefix)) {
            continue;
        }
        names.push(OsString::from_vec(name.to_vec()));
        if names.len() > maximum {
            unsafe {
                // SAFETY: stream is live and closed exactly once.
                libc::closedir(stream);
            }
            return Err(ProjectFolderFilesystemError::InvalidTree);
        }
    }
    let closed = unsafe {
        // SAFETY: stream is live and closed exactly once.
        libc::closedir(stream)
    };
    if closed != 0 {
        return Err(ProjectFolderFilesystemError::ReadFailed);
    }
    Ok(names)
}

fn unlink_at(parent_fd: RawFd, name: &str, flags: i32) -> FsResult<()> {
    let name_c = c_name(name)?;
    let removed = unsafe {
        // SAFETY: parent fd and C string remain live.
        libc::unlinkat(parent_fd, name_c.as_ptr(), flags)
    };
    if removed == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ENOENT) {
        Ok(())
    } else {
        Err(ProjectFolderFilesystemError::WriteFailed)
    }
}

fn rename_child_no_replace(parent_fd: RawFd, source: &str, target: &str) -> FsResult<()> {
    let source_c = c_name(source)?;
    let target_c = c_name(target)?;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    let renamed = unsafe {
        // SAFETY: both names and the pinned parent fd remain live.
        libc::syscall(
            libc::SYS_renameat2,
            parent_fd,
            source_c.as_ptr(),
            parent_fd,
            target_c.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let renamed = unsafe {
        // SAFETY: both names and the pinned parent fd remain live.
        libc::renameatx_np(
            parent_fd,
            source_c.as_ptr(),
            parent_fd,
            target_c.as_ptr(),
            libc::RENAME_EXCL,
        ) as libc::c_long
    };
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    let renamed: libc::c_long = return Err(ProjectFolderFilesystemError::WriteFailed);

    if renamed == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if matches!(
        error.raw_os_error(),
        Some(code) if code == libc::EEXIST || code == libc::ENOTEMPTY
    ) {
        Err(ProjectFolderFilesystemError::TargetExists)
    } else {
        Err(ProjectFolderFilesystemError::WriteFailed)
    }
}

fn c_name(name: &str) -> FsResult<CString> {
    if name.is_empty() || name.contains('/') || name.as_bytes().contains(&0) {
        return Err(ProjectFolderFilesystemError::InvalidRequest);
    }
    CString::new(name).map_err(|_| ProjectFolderFilesystemError::InvalidRequest)
}

fn map_open_error(error: std::io::Error) -> ProjectFolderFilesystemError {
    match error.raw_os_error() {
        Some(code) if code == libc::ELOOP => ProjectFolderFilesystemError::LinkOrSpecialEntry,
        Some(code) if code == libc::ENOENT => ProjectFolderFilesystemError::OpenFailed,
        _ => ProjectFolderFilesystemError::OpenFailed,
    }
}

fn map_read_error(_error: std::io::Error) -> ProjectFolderFilesystemError {
    ProjectFolderFilesystemError::ReadFailed
}

fn map_write_error(_error: std::io::Error) -> ProjectFolderFilesystemError {
    ProjectFolderFilesystemError::WriteFailed
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn clear_errno() {
    unsafe {
        // SAFETY: errno storage is thread-local and valid for this thread.
        *libc::__errno_location() = 0;
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn current_errno() -> i32 {
    unsafe {
        // SAFETY: errno storage is thread-local and valid for this thread.
        *libc::__errno_location()
    }
}

#[cfg(target_os = "macos")]
fn clear_errno() {
    unsafe {
        // SAFETY: errno storage is thread-local and valid for this thread.
        *libc::__error() = 0;
    }
}

#[cfg(target_os = "macos")]
fn current_errno() -> i32 {
    unsafe {
        // SAFETY: errno storage is thread-local and valid for this thread.
        *libc::__error()
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
fn clear_errno() {}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
fn current_errno() -> i32 {
    libc::ENOSYS
}
