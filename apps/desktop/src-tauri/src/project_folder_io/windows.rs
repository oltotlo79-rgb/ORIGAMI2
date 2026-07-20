use std::{
    ffi::{OsStr, OsString},
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, RawHandle},
    },
    path::{Path, PathBuf},
    ptr,
};

use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_FLAG_SEQUENTIAL_SCAN, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_LIST_DIRECTORY,
    FILE_READ_ATTRIBUTES, FILE_RENAME_INFO, FILE_SHARE_READ, FILE_SHARE_WRITE, FileDispositionInfo,
    FileRenameInfo, GetFileInformationByHandle, SetFileInformationByHandle,
};

use super::{FsResult, ProjectFolderFilesystemError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjectIdentity {
    volume: u32,
    index: u64,
}

#[derive(Debug, Clone, Copy)]
struct HandleInformation {
    identity: ObjectIdentity,
    attributes: u32,
    links: u32,
    size: u64,
}

pub(super) struct PinnedDirectory {
    path: PathBuf,
    file: File,
    identity: ObjectIdentity,
    deletable: bool,
}

impl PinnedDirectory {
    pub(super) fn open_selected(path: &Path) -> FsResult<Self> {
        open_directory(path, false)
    }

    pub(super) fn list_names(&self, maximum: usize) -> FsResult<Vec<OsString>> {
        self.revalidate_selected_path()?;
        let entries =
            std::fs::read_dir(&self.path).map_err(|_| ProjectFolderFilesystemError::ReadFailed)?;
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|_| ProjectFolderFilesystemError::ReadFailed)?;
            names.push(entry.file_name());
            if names.len() > maximum {
                return Err(ProjectFolderFilesystemError::InvalidTree);
            }
        }
        self.revalidate_selected_path()?;
        Ok(names)
    }

    pub(super) fn open_child_directory(&self, name: &str) -> FsResult<Self> {
        validate_child_name(name)?;
        self.revalidate_selected_path()?;
        open_directory(&self.path.join(name), false)
    }

    pub(super) fn create_child_directory(&self, name: &str, deletable: bool) -> FsResult<Self> {
        validate_child_name(name)?;
        self.revalidate_selected_path()?;
        let path = self.path.join(name);
        match std::fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(ProjectFolderFilesystemError::TargetExists);
            }
            Err(_) => return Err(ProjectFolderFilesystemError::WriteFailed),
        }
        open_directory(&path, deletable).map_err(|error| {
            if matches!(error, ProjectFolderFilesystemError::OpenFailed) {
                ProjectFolderFilesystemError::ChangedDuringRead
            } else {
                error
            }
        })
    }

    pub(super) fn open_child_file(&self, name: &str, limit: u64) -> FsResult<PinnedFile> {
        validate_child_name(name)?;
        self.revalidate_selected_path()?;
        let path = self.path.join(name);
        let entry = std::fs::symlink_metadata(&path).map_err(map_open_error)?;
        if !entry.file_type().is_file()
            || entry.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(ProjectFolderFilesystemError::LinkOrSpecialEntry);
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .access_mode(FILE_GENERIC_READ | FILE_READ_ATTRIBUTES)
            // Withhold write/delete sharing while the admitted bytes are
            // consumed. A replacement or mutation must fail or be observed
            // by the identity/size checks below.
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_SEQUENTIAL_SCAN);
        let file = options.open(&path).map_err(map_open_error)?;
        PinnedFile::admit(file, limit)
    }

    pub(super) fn write_child_file(&self, name: &str, bytes: &[u8]) -> FsResult<()> {
        validate_child_name(name)?;
        self.revalidate_selected_path()?;
        let path = self.path.join(name);
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create_new(true)
            .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_SEQUENTIAL_SCAN);
        let mut file = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(ProjectFolderFilesystemError::TargetExists);
            }
            Err(_) => return Err(ProjectFolderFilesystemError::WriteFailed),
        };
        let initial = validate_plain_file(&file, None)?;
        file.write_all(bytes)
            .map_err(|_| ProjectFolderFilesystemError::WriteFailed)?;
        file.sync_all()
            .map_err(|_| ProjectFolderFilesystemError::WriteFailed)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| ProjectFolderFilesystemError::WriteFailed)?;
        let mut readback = Vec::with_capacity(bytes.len());
        file.read_to_end(&mut readback)
            .map_err(|_| ProjectFolderFilesystemError::WriteFailed)?;
        if readback != bytes {
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        let after = validate_plain_file(&file, None)?;
        if after.identity != initial.identity || after.size != bytes.len() as u64 {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        drop(file);
        let reopened = self.open_child_file(name, bytes.len() as u64)?;
        if reopened.identity != initial.identity || reopened.declared_size != bytes.len() as u64 {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        Ok(())
    }

    pub(super) fn child_exists(&self, name: &str) -> FsResult<bool> {
        validate_child_name(name)?;
        self.revalidate_selected_path()?;
        match std::fs::symlink_metadata(self.path.join(name)) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(ProjectFolderFilesystemError::ReadFailed),
        }
    }

    pub(super) fn revalidate_selected_path(&self) -> FsResult<()> {
        let held = directory_information(&self.file)?;
        if held.identity != self.identity
            || held.attributes & FILE_ATTRIBUTE_DIRECTORY == 0
            || held.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        if self.deletable {
            // This handle requests DELETE and withholds delete sharing. The
            // selected final component therefore cannot be renamed, deleted,
            // or replaced while the handle is live.
            return Ok(());
        }
        let reopened = open_directory(&self.path, false)?;
        if reopened.identity != self.identity {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        Ok(())
    }

    pub(super) fn revalidate_child_directory(&self, name: &str, child: &Self) -> FsResult<()> {
        validate_child_name(name)?;
        self.revalidate_selected_path()?;
        if !child.deletable {
            let reopened = open_directory(&self.path.join(name), false)?;
            if reopened.identity != child.identity {
                return Err(ProjectFolderFilesystemError::ChangedDuringRead);
            }
        }
        child.revalidate_selected_path()
    }

    pub(super) fn sync_directory(&self) -> FsResult<()> {
        // Windows does not expose a portable directory fsync through
        // std::fs. Every payload is flushed before publication, while the
        // exact directory rename is performed by a held handle.
        Ok(())
    }

    pub(super) fn remove_child_file(&self, name: &str) -> FsResult<()> {
        validate_child_name(name)?;
        match std::fs::remove_file(self.path.join(name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(ProjectFolderFilesystemError::WriteFailed),
        }
    }

    pub(super) fn remove_child_directory(&self, name: &str) -> FsResult<()> {
        validate_child_name(name)?;
        match std::fs::remove_dir(self.path.join(name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(ProjectFolderFilesystemError::WriteFailed),
        }
    }

    pub(super) fn remove_child_directory_if_same(&self, name: &str, child: &Self) -> FsResult<()> {
        validate_child_name(name)?;
        self.revalidate_child_directory(name, child)?;
        if !child.deletable {
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        let removed = unsafe {
            // SAFETY: `child.file` is a live directory handle opened with
            // DELETE access, and `disposition` remains valid for the call.
            // Deletion therefore targets the exact admitted object, not a
            // potentially swapped pathname.
            SetFileInformationByHandle(
                child.file.as_raw_handle() as RawHandle,
                FileDispositionInfo,
                ptr::addr_of!(disposition).cast(),
                u32::try_from(size_of::<FILE_DISPOSITION_INFO>())
                    .map_err(|_| ProjectFolderFilesystemError::WriteFailed)?,
            )
        };
        if removed == 0 {
            Err(ProjectFolderFilesystemError::WriteFailed)
        } else {
            Ok(())
        }
    }

    pub(super) fn publish_child_directory_no_replace(
        &self,
        source_name: &str,
        source: &Self,
        target_name: &str,
    ) -> FsResult<()> {
        validate_child_name(source_name)?;
        validate_child_name(target_name)?;
        self.revalidate_selected_path()?;
        self.revalidate_child_directory(source_name, source)?;
        if !source.deletable {
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        rename_directory_handle_no_replace(&source.file, &self.path.join(target_name)).map_err(
            |_| match std::fs::symlink_metadata(self.path.join(target_name)) {
                Ok(_) => ProjectFolderFilesystemError::TargetExists,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    ProjectFolderFilesystemError::WriteFailed
                }
                Err(_) => ProjectFolderFilesystemError::WriteFailed,
            },
        )
    }
}

pub(super) struct PinnedFile {
    file: File,
    identity: ObjectIdentity,
    declared_size: u64,
}

impl PinnedFile {
    fn admit(file: File, limit: u64) -> FsResult<Self> {
        let admitted = validate_plain_file(&file, Some(limit))?;
        Ok(Self {
            file,
            identity: admitted.identity,
            declared_size: admitted.size,
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
            .map_err(|_| ProjectFolderFilesystemError::ReadFailed)?;
        if bytes.len() as u64 > limit {
            return Err(ProjectFolderFilesystemError::TooLarge);
        }
        let after = validate_plain_file(&self.file, Some(limit))?;
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

fn open_directory(path: &Path, deletable: bool) -> FsResult<PinnedDirectory> {
    let entry = std::fs::symlink_metadata(path).map_err(map_open_error)?;
    if !entry.file_type().is_dir() || entry.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(ProjectFolderFilesystemError::LinkOrSpecialEntry);
    }
    let mut options = OpenOptions::new();
    let mut access = FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES;
    if deletable {
        access |= DELETE;
    }
    options
        .read(true)
        .access_mode(access)
        // Withholding FILE_SHARE_DELETE pins the final directory name for the
        // handle lifetime. A `deletable` staging handle also owns DELETE
        // access so the exact verified directory can be published by handle.
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    let file = options.open(path).map_err(map_open_error)?;
    let information = directory_information(&file)?;
    Ok(PinnedDirectory {
        path: path.to_path_buf(),
        file,
        identity: information.identity,
        deletable,
    })
}

fn directory_information(file: &File) -> FsResult<HandleInformation> {
    let information = handle_information(file)?;
    if information.attributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || information.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(ProjectFolderFilesystemError::LinkOrSpecialEntry);
    }
    Ok(information)
}

fn validate_plain_file(file: &File, limit: Option<u64>) -> FsResult<HandleInformation> {
    let information = handle_information(file)?;
    if information.attributes & FILE_ATTRIBUTE_DIRECTORY != 0
        || information.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(ProjectFolderFilesystemError::LinkOrSpecialEntry);
    }
    if information.links != 1 {
        return Err(ProjectFolderFilesystemError::LinkOrSpecialEntry);
    }
    if limit.is_some_and(|limit| information.size > limit) {
        return Err(ProjectFolderFilesystemError::TooLarge);
    }
    Ok(information)
}

fn handle_information(file: &File) -> FsResult<HandleInformation> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    let succeeded = unsafe {
        // SAFETY: the file handle remains live and `information` points to
        // valid writable storage for the complete call.
        GetFileInformationByHandle(file.as_raw_handle() as RawHandle, &mut information)
    };
    if succeeded == 0 {
        return Err(ProjectFolderFilesystemError::ReadFailed);
    }
    Ok(HandleInformation {
        identity: ObjectIdentity {
            volume: information.dwVolumeSerialNumber,
            index: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        },
        attributes: information.dwFileAttributes,
        links: information.nNumberOfLinks,
        size: (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow),
    })
}

fn rename_directory_handle_no_replace(file: &File, destination: &Path) -> std::io::Result<()> {
    let destination_wide = destination.as_os_str().encode_wide().collect::<Vec<_>>();
    if destination_wide.contains(&0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "NUL in destination",
        ));
    }
    let file_name_bytes = destination_wide
        .len()
        .checked_mul(size_of::<u16>())
        .and_then(|length| u32::try_from(length).ok())
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "destination too long")
        })?;
    let buffer_size = size_of::<FILE_RENAME_INFO>()
        .checked_add(file_name_bytes as usize)
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "rename request too large")
        })?;
    let buffer_size_u32 = u32::try_from(buffer_size).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "rename request too large")
    })?;
    let word_size = size_of::<usize>();
    let word_count = buffer_size
        .checked_add(word_size - 1)
        .map(|length| length / word_size)
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "rename request too large")
        })?;
    let mut buffer = vec![0usize; word_count];
    let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    let renamed = unsafe {
        // SAFETY: `buffer` is aligned for FILE_RENAME_INFO and large enough
        // for its fixed header plus the complete UTF-16 path. The source
        // directory handle remains live and was opened with DELETE access.
        (*information).Anonymous.ReplaceIfExists = false;
        (*information).RootDirectory = ptr::null_mut();
        (*information).FileNameLength = file_name_bytes;
        let file_name = ptr::addr_of_mut!((*information).FileName).cast::<u16>();
        ptr::copy_nonoverlapping(destination_wide.as_ptr(), file_name, destination_wide.len());
        SetFileInformationByHandle(
            file.as_raw_handle() as RawHandle,
            FileRenameInfo,
            information.cast(),
            buffer_size_u32,
        )
    };
    if renamed == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn validate_child_name(name: &str) -> FsResult<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name
            .bytes()
            .any(|byte| matches!(byte, b'/' | b'\\' | b':' | 0))
        || OsStr::new(name).encode_wide().any(|unit| unit == 0)
    {
        return Err(ProjectFolderFilesystemError::InvalidRequest);
    }
    Ok(())
}

fn map_open_error(_error: std::io::Error) -> ProjectFolderFilesystemError {
    ProjectFolderFilesystemError::OpenFailed
}
