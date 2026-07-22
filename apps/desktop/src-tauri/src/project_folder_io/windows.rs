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
    BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ADD_FILE, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_FLAG_SEQUENTIAL_SCAN, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
    FILE_ID_INFO, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_RENAME_INFO, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, FileDispositionInfo, FileIdInfo, FileRenameInfo,
    GetFileInformationByHandle, GetFileInformationByHandleEx, GetVolumeInformationByHandleW,
    SetFileInformationByHandle,
};

use super::{DirectoryIdentity, FsResult, ProjectFolderFilesystemError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjectIdentity {
    volume: u64,
    file_id_low: u64,
    file_id_high: u64,
}

#[derive(Debug, Clone, Copy)]
struct HandleInformation {
    identity: ObjectIdentity,
    attributes: u32,
    links: u32,
    size: u64,
}

#[repr(C)]
#[derive(Default)]
struct IoStatusBlock {
    status: usize,
    information: usize,
}

pub(crate) fn flush_directory_handle(directory: &File) -> std::io::Result<()> {
    let mut io_status = IoStatusBlock::default();
    let status = unsafe {
        // SAFETY: `directory` is a live directory handle and `io_status` is
        // valid output storage for the duration of the synchronous call.
        NtFlushBuffersFileEx(
            directory.as_raw_handle() as RawHandle,
            0,
            ptr::null(),
            0,
            ptr::addr_of_mut!(io_status),
        )
    };
    let completion_status = io_status.status as u32 as i32;
    if status != 0 || completion_status != 0 {
        return Err(std::io::Error::other(format!(
            "NtFlushBuffersFileEx failed: status={status:#x}, completion={completion_status:#x}"
        )));
    }
    Ok(())
}

#[repr(C)]
#[derive(Default)]
struct FileIsRemoteDeviceInformation {
    is_remote: u8,
}

const FILE_IS_REMOTE_DEVICE_INFORMATION_CLASS: i32 = 51;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryInformationFile(
        file_handle: RawHandle,
        io_status_block: *mut IoStatusBlock,
        file_information: *mut core::ffi::c_void,
        length: u32,
        file_information_class: i32,
    ) -> i32;

    fn NtFlushBuffersFileEx(
        file_handle: RawHandle,
        flags: u32,
        parameters: *const core::ffi::c_void,
        parameters_size: u32,
        io_status_block: *mut IoStatusBlock,
    ) -> i32;
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

    pub(super) fn open_child_directory_for_rename(&self, name: &str) -> FsResult<Self> {
        validate_child_name(name)?;
        self.revalidate_selected_path()?;
        open_directory(&self.path.join(name), true)
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
        let mut access = FILE_GENERIC_READ | FILE_READ_ATTRIBUTES;
        if deletable {
            access |= DELETE;
        }
        options
            .read(true)
            .access_mode(access)
            // Withhold write/delete sharing while the admitted bytes are
            // consumed. A replacement or mutation must fail or be observed
            // by the identity/size checks below.
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_SEQUENTIAL_SCAN);
        let file = options.open(&path).map_err(map_open_error)?;
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
        validate_child_name(name)?;
        self.revalidate_selected_path()?;
        let path = self.path.join(name);
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create_new(true)
            .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES | DELETE)
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
        Ok(PinnedFile {
            file,
            identity: initial.identity,
            declared_size: bytes.len() as u64,
            deletable: true,
        })
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

    pub(super) fn identity(&self) -> DirectoryIdentity {
        DirectoryIdentity {
            first: self.identity.volume,
            second: self.identity.file_id_low,
            third: self.identity.file_id_high,
        }
    }

    pub(super) fn ensure_stable_replacement_identity(&self) -> FsResult<()> {
        self.revalidate_selected_path()?;
        if !remote_device_query_proves_local(&self.file) {
            return Err(ProjectFolderFilesystemError::ReplacementUnsupported);
        }
        if !handle_has_stable_replacement_file_system(&self.file) {
            return Err(ProjectFolderFilesystemError::ReplacementUnsupported);
        }
        Ok(())
    }

    pub(super) fn sync_directory(&self) -> FsResult<()> {
        self.revalidate_selected_path()?;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .access_mode(FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | FILE_ADD_FILE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
        let flush_handle = options
            .open(&self.path)
            .map_err(|_| ProjectFolderFilesystemError::WriteFailed)?;
        let information = directory_information(&flush_handle)
            .map_err(|_| ProjectFolderFilesystemError::WriteFailed)?;
        if information.identity != self.identity {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        flush_directory_handle(&flush_handle)
            .map_err(|_| ProjectFolderFilesystemError::WriteFailed)?;
        let after = directory_information(&flush_handle)
            .map_err(|_| ProjectFolderFilesystemError::WriteFailed)?;
        if after.identity != self.identity {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        self.revalidate_selected_path()
    }

    pub(super) fn remove_child_file_if_same(&self, name: &str, child: &PinnedFile) -> FsResult<()> {
        validate_child_name(name)?;
        self.revalidate_child_file(name, child)?;
        if !child.deletable {
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        set_delete_disposition(&child.file)
    }

    pub(super) fn remove_child_directory_if_same(&self, name: &str, child: &Self) -> FsResult<()> {
        validate_child_name(name)?;
        self.revalidate_child_directory(name, child)?;
        if !child.deletable {
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        set_delete_disposition(&child.file)
    }

    pub(super) fn publish_child_directory_no_replace(
        &self,
        source_name: &str,
        source: &mut Self,
        target_name: &str,
    ) -> FsResult<()> {
        validate_child_name(source_name)?;
        validate_child_name(target_name)?;
        self.revalidate_selected_path()?;
        self.revalidate_child_directory(source_name, source)?;
        if !source.deletable {
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        rename_handle_no_replace(&source.file, &self.path.join(target_name)).map_err(|_| {
            match std::fs::symlink_metadata(self.path.join(target_name)) {
                Ok(_) => ProjectFolderFilesystemError::TargetExists,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    ProjectFolderFilesystemError::WriteFailed
                }
                Err(_) => ProjectFolderFilesystemError::WriteFailed,
            }
        })?;
        source.path = self.path.join(target_name);
        Ok(())
    }

    pub(super) fn publish_child_file_no_replace(
        &self,
        source_name: &str,
        source: &PinnedFile,
        target_name: &str,
    ) -> FsResult<()> {
        validate_child_name(source_name)?;
        validate_child_name(target_name)?;
        self.revalidate_selected_path()?;
        self.revalidate_child_file(source_name, source)?;
        if !source.deletable {
            return Err(ProjectFolderFilesystemError::WriteFailed);
        }
        rename_handle_no_replace(&source.file, &self.path.join(target_name)).map_err(|_| {
            match std::fs::symlink_metadata(self.path.join(target_name)) {
                Ok(_) => ProjectFolderFilesystemError::TargetExists,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    ProjectFolderFilesystemError::WriteFailed
                }
                Err(_) => ProjectFolderFilesystemError::WriteFailed,
            }
        })
    }

    fn revalidate_child_file(&self, name: &str, child: &PinnedFile) -> FsResult<()> {
        validate_child_name(name)?;
        self.revalidate_selected_path()?;
        let held = validate_plain_file(&child.file, Some(child.declared_size))?;
        if held.identity != child.identity || held.size != child.declared_size {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        if child.deletable {
            // DELETE access plus withheld delete sharing pins the admitted
            // final name. Reopening would itself be incompatible with that
            // intentional share mode.
            return Ok(());
        }
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
        let admitted = validate_plain_file(&file, Some(limit))?;
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
        if !self.deletable {
            let reopened = parent.open_child_file(name, limit)?;
            if reopened.identity != self.identity || reopened.declared_size != self.declared_size {
                return Err(ProjectFolderFilesystemError::ChangedDuringRead);
            }
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
    let identity = full_file_identity(file)?;
    Ok(HandleInformation {
        identity,
        attributes: information.dwFileAttributes,
        links: information.nNumberOfLinks,
        size: (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow),
    })
}

fn full_file_identity(file: &File) -> FsResult<ObjectIdentity> {
    let mut information = FILE_ID_INFO::default();
    let succeeded = unsafe {
        // SAFETY: the file handle remains live and `information` points to
        // writable storage of the exact FILE_ID_INFO size.
        GetFileInformationByHandleEx(
            file.as_raw_handle() as RawHandle,
            FileIdInfo,
            ptr::addr_of_mut!(information).cast(),
            u32::try_from(size_of::<FILE_ID_INFO>())
                .map_err(|_| ProjectFolderFilesystemError::ReadFailed)?,
        )
    };
    if succeeded == 0 {
        return Err(ProjectFolderFilesystemError::ReadFailed);
    }
    object_identity_from_full_file_id(
        information.VolumeSerialNumber,
        information.FileId.Identifier,
    )
}

fn remote_device_query_proves_local(file: &File) -> bool {
    let mut io_status = IoStatusBlock::default();
    let mut information = FileIsRemoteDeviceInformation::default();
    let length = match u32::try_from(size_of::<FileIsRemoteDeviceInformation>()) {
        Ok(length) => length,
        Err(_) => return false,
    };
    let status = unsafe {
        // SAFETY: the pinned file handle remains live and both output
        // pointers refer to writable storage of the declared sizes.
        NtQueryInformationFile(
            file.as_raw_handle() as RawHandle,
            ptr::addr_of_mut!(io_status),
            ptr::addr_of_mut!(information).cast(),
            length,
            FILE_IS_REMOTE_DEVICE_INFORMATION_CLASS,
        )
    };
    remote_device_result_proves_local(
        status,
        io_status.status as u32 as i32,
        io_status.information,
        information.is_remote,
    )
}

fn remote_device_result_proves_local(
    status: i32,
    completion_status: i32,
    bytes_written: usize,
    is_remote: u8,
) -> bool {
    status == 0
        && completion_status == 0
        && bytes_written == size_of::<FileIsRemoteDeviceInformation>()
        && is_remote == 0
}

fn handle_has_stable_replacement_file_system(file: &File) -> bool {
    const FILE_SYSTEM_NAME_CAPACITY: usize = 16;
    let mut file_system_name = [0_u16; FILE_SYSTEM_NAME_CAPACITY];
    let succeeded = unsafe {
        // SAFETY: the pinned handle remains live, unused output pointers are
        // null, and `file_system_name` is writable for the declared length.
        GetVolumeInformationByHandleW(
            file.as_raw_handle() as RawHandle,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            file_system_name.as_mut_ptr(),
            FILE_SYSTEM_NAME_CAPACITY as u32,
        )
    };
    succeeded != 0 && stable_replacement_file_system_name(&file_system_name)
}

fn stable_replacement_file_system_name(buffer: &[u16]) -> bool {
    let Some(terminator) = buffer.iter().position(|unit| *unit == 0) else {
        return false;
    };
    if terminator != 4
        || buffer[..terminator].iter().any(|unit| *unit > 0x7f)
        || buffer[terminator + 1..].iter().any(|unit| *unit != 0)
    {
        return false;
    }
    let mut bytes = [0_u8; 4];
    for (destination, source) in bytes.iter_mut().zip(&buffer[..terminator]) {
        *destination = (*source as u8).to_ascii_lowercase();
    }
    matches!(&bytes, b"ntfs" | b"refs")
}

fn object_identity_from_full_file_id(
    volume: u64,
    identifier: [u8; 16],
) -> FsResult<ObjectIdentity> {
    if volume == 0
        || identifier.iter().all(|byte| *byte == 0)
        || identifier.iter().all(|byte| *byte == 0xff)
    {
        return Err(ProjectFolderFilesystemError::ReadFailed);
    }
    let file_id_low = u64::from_le_bytes(
        identifier[..8]
            .try_into()
            .map_err(|_| ProjectFolderFilesystemError::ReadFailed)?,
    );
    let file_id_high = u64::from_le_bytes(
        identifier[8..]
            .try_into()
            .map_err(|_| ProjectFolderFilesystemError::ReadFailed)?,
    );
    Ok(ObjectIdentity {
        volume,
        file_id_low,
        file_id_high,
    })
}

#[cfg(test)]
fn directory_identity_from_full_file_id(
    volume: u64,
    identifier: [u8; 16],
) -> FsResult<DirectoryIdentity> {
    let identity = object_identity_from_full_file_id(volume, identifier)?;
    Ok(DirectoryIdentity {
        first: identity.volume,
        second: identity.file_id_low,
        third: identity.file_id_high,
    })
}

fn rename_handle_no_replace(file: &File, destination: &Path) -> std::io::Result<()> {
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

fn set_delete_disposition(file: &File) -> FsResult<()> {
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    let removed = unsafe {
        // SAFETY: `file` remains live and was opened with DELETE access.
        SetFileInformationByHandle(
            file.as_raw_handle() as RawHandle,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_identity_preserves_the_complete_windows_file_id() {
        let identity = directory_identity_from_full_file_id(
            0x0102_0304_0506_0708,
            [
                0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25,
                0x26, 0x27,
            ],
        )
        .expect("full file identity");

        assert_eq!(
            identity,
            DirectoryIdentity {
                first: 0x0102_0304_0506_0708,
                second: 0x1716_1514_1312_1110,
                third: 0x2726_2524_2322_2120,
            }
        );
    }

    #[test]
    fn zero_windows_file_identity_is_rejected_without_legacy_fallback() {
        assert_eq!(
            directory_identity_from_full_file_id(0, [1; 16]),
            Err(ProjectFolderFilesystemError::ReadFailed)
        );
        assert_eq!(
            directory_identity_from_full_file_id(1, [0; 16]),
            Err(ProjectFolderFilesystemError::ReadFailed)
        );
        assert_eq!(
            directory_identity_from_full_file_id(1, [0xff; 16]),
            Err(ProjectFolderFilesystemError::ReadFailed)
        );
    }

    #[test]
    fn upper_half_of_windows_file_id_participates_in_identity() {
        let mut first_id = [0x44; 16];
        let mut second_id = first_id;
        first_id[15] = 0x10;
        second_id[15] = 0x20;

        let first =
            directory_identity_from_full_file_id(7, first_id).expect("first full file identity");
        let second =
            directory_identity_from_full_file_id(7, second_id).expect("second full file identity");

        assert_eq!(first.first, second.first);
        assert_eq!(first.second, second.second);
        assert_ne!(first.third, second.third);
        assert_ne!(first, second);
    }

    #[test]
    fn replacement_file_system_name_is_exact_bounded_and_case_insensitive() {
        assert!(stable_replacement_file_system_name(&[
            b'N' as u16,
            b't' as u16,
            b'F' as u16,
            b's' as u16,
            0,
        ]));
        assert!(stable_replacement_file_system_name(&[
            b'r' as u16,
            b'E' as u16,
            b'f' as u16,
            b'S' as u16,
            0,
        ]));
        for rejected in [
            vec![b'F' as u16, b'A' as u16, b'T' as u16, 0],
            vec![
                b'e' as u16,
                b'x' as u16,
                b'F' as u16,
                b'A' as u16,
                b'T' as u16,
                0,
            ],
            vec![b'N' as u16, b'T' as u16, b'F' as u16, b'S' as u16],
            vec![
                b'N' as u16,
                b'T' as u16,
                b'F' as u16,
                b'S' as u16,
                b'X' as u16,
                0,
            ],
            vec![
                b'N' as u16,
                b'T' as u16,
                b'F' as u16,
                b'S' as u16,
                0,
                b'X' as u16,
            ],
            vec![0xd800, b'T' as u16, b'F' as u16, b'S' as u16, 0],
            vec![0],
        ] {
            assert!(!stable_replacement_file_system_name(&rejected));
        }
    }

    #[test]
    fn remote_or_ambiguous_device_results_are_never_admitted() {
        let size = size_of::<FileIsRemoteDeviceInformation>();
        assert!(remote_device_result_proves_local(0, 0, size, 0));
        assert!(!remote_device_result_proves_local(-1, 0, size, 0));
        assert!(!remote_device_result_proves_local(0, -1, size, 0));
        assert!(!remote_device_result_proves_local(0, 0, 0, 0));
        assert!(!remote_device_result_proves_local(0, 0, size, 1));
        assert!(!remote_device_result_proves_local(0, 0, size, 2));
    }

    #[test]
    fn local_test_volume_is_admitted_from_the_pinned_handle() {
        let directory = PinnedDirectory::open_selected(&std::env::temp_dir())
            .expect("open local temporary directory");
        assert!(
            remote_device_query_proves_local(&directory.file),
            "local remote-device query"
        );
        assert!(
            handle_has_stable_replacement_file_system(&directory.file),
            "local stable filesystem"
        );
        directory
            .ensure_stable_replacement_identity()
            .expect("admit local NTFS/ReFS test volume");
    }
}
