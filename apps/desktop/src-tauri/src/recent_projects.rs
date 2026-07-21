//! Private recent-project registry. Filesystem paths and identities never cross IPC.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{DELETE, FILE_GENERIC_WRITE};

const SCHEMA: u8 = 1;
const MAX_RECENT: usize = 10;
const MAX_DISPLAY_NAME_BYTES: usize = 160;
const LEASE_STALE_MILLIS: u128 = 60_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct RecentProjectView {
    pub opaque_id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedRegistry {
    schema_version: u8,
    entries: Vec<PersistedEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedEntry {
    opaque_id: String,
    display_name: String,
    path: PathBuf,
    identity: CanonicalFileIdentity,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CanonicalFileIdentity {
    pub volume: u64,
    pub file: u128,
}

pub(super) trait RecentProjectFilesystem {
    /// Opens the final component without following links/reparse points and
    /// returns the identity of that pinned regular file.
    fn probe_regular_no_follow(&self, path: &Path) -> Result<CanonicalFileIdentity, ()>;
}

pub(super) trait RecentProjectStorage {
    fn read(&self) -> Result<Option<Vec<u8>>, ()>;
    fn replace_atomically(&mut self, bytes: &[u8]) -> Result<(), ()>;
}

pub(super) struct LocalRecentProjectFilesystem;

impl RecentProjectFilesystem for LocalRecentProjectFilesystem {
    fn probe_regular_no_follow(&self, path: &Path) -> Result<CanonicalFileIdentity, ()> {
        let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) || link_count(&metadata) != 1 {
            return Err(());
        }
        canonical_identity(path, &metadata)
    }
}

pub(super) struct FileRecentProjectStorage {
    path: PathBuf,
    observed: Mutex<Option<Option<[u8; 32]>>>,
}

impl FileRecentProjectStorage {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            observed: Mutex::new(None),
        }
    }
}

impl RecentProjectStorage for FileRecentProjectStorage {
    fn read(&self) -> Result<Option<Vec<u8>>, ()> {
        let bytes = match fs::symlink_metadata(&self.path) {
            Ok(metadata)
                if metadata.is_file()
                    && !is_link_or_reparse(&metadata)
                    && link_count(&metadata) == 1
                    && metadata.len() <= 64 * 1024 =>
            {
                fs::read(&self.path).map(Some).map_err(|_| ())
            }
            Ok(_) => Err(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(()),
        }?;
        *self.observed.lock().map_err(|_| ())? = Some(bytes.as_deref().map(content_digest));
        Ok(bytes)
    }

    fn replace_atomically(&mut self, bytes: &[u8]) -> Result<(), ()> {
        if bytes.len() > 64 * 1024 {
            return Err(());
        }
        let parent = self.path.parent().ok_or(())?;
        fs::create_dir_all(parent).map_err(|_| ())?;
        if let Ok(metadata) = fs::symlink_metadata(&self.path)
            && (!metadata.is_file() || is_link_or_reparse(&metadata) || link_count(&metadata) != 1)
        {
            return Err(());
        }
        let lease = StorageLease::acquire(&self.path)?;
        let expected = self.observed.lock().map_err(|_| ())?.ok_or(())?;
        let actual = read_current_digest(&self.path)?;
        if actual != expected {
            return Err(());
        }
        let staged = self.path.with_extension("recent.next");
        match fs::symlink_metadata(&staged) {
            Ok(metadata)
                if metadata.is_file()
                    && !is_link_or_reparse(&metadata)
                    && link_count(&metadata) == 1 =>
            {
                fs::remove_file(&staged).map_err(|_| ())?
            }
            Ok(_) => return Err(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(()),
        }
        let mut file = open_staged_create_new(&staged)?;
        let result = (|| {
            file.write_all(bytes).map_err(|_| ())?;
            file.sync_all().map_err(|_| ())?;
            publish_staged(&file, &staged, &self.path)
        })();
        drop(file);
        if result.is_err() {
            let _ = fs::remove_file(staged);
        }
        drop(lease);
        if result.is_ok() {
            *self.observed.lock().map_err(|_| ())? = Some(Some(content_digest(bytes)));
        }
        result
    }
}

#[cfg(not(windows))]
fn open_staged_create_new(path: &Path) -> Result<fs::File, ()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| ())
}

#[cfg(windows)]
fn open_staged_create_new(path: &Path) -> Result<fs::File, ()> {
    OpenOptions::new()
        .write(true)
        .access_mode(FILE_GENERIC_WRITE | DELETE)
        .create_new(true)
        .open(path)
        .map_err(|_| ())
}

fn content_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn read_current_digest(path: &Path) -> Result<Option<[u8; 32]>, ()> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.is_file()
                && !is_link_or_reparse(&metadata)
                && link_count(&metadata) == 1
                && metadata.len() <= 64 * 1024 =>
        {
            fs::read(path)
                .map(|bytes| Some(content_digest(&bytes)))
                .map_err(|_| ())
        }
        Ok(_) => Err(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

struct StorageLease(PathBuf);

impl StorageLease {
    fn acquire(destination: &Path) -> Result<Self, ()> {
        let path = destination.with_extension("recent.lock");
        for attempt in 0..2 {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    let now = now_millis()?;
                    if file.write_all(now.to_string().as_bytes()).is_err()
                        || file.sync_all().is_err()
                    {
                        drop(file);
                        let _ = fs::remove_file(&path);
                        return Err(());
                    }
                    return Ok(Self(path));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && attempt == 0 => {
                    retire_stale_lease(&path)?;
                }
                Err(_) => return Err(()),
            }
        }
        Err(())
    }
}

impl Drop for StorageLease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn retire_stale_lease(path: &Path) -> Result<(), ()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.is_file()
        || is_link_or_reparse(&metadata)
        || link_count(&metadata) != 1
        || metadata.len() > 32
    {
        return Err(());
    }
    let bytes = fs::read(path).map_err(|_| ())?;
    let issued = std::str::from_utf8(&bytes)
        .map_err(|_| ())?
        .parse::<u128>()
        .map_err(|_| ())?;
    if now_millis()?.saturating_sub(issued) <= LEASE_STALE_MILLIS {
        return Err(());
    }
    fs::remove_file(path).map_err(|_| ())
}

fn now_millis() -> Result<u128, ()> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .map_err(|_| ())
}

#[derive(Default)]
pub(super) struct RecentProjectRegistry {
    entries: Vec<PersistedEntry>,
}

impl RecentProjectRegistry {
    pub fn load(storage: &impl RecentProjectStorage) -> Self {
        let Some(bytes) = storage.read().ok().flatten() else {
            return Self::default();
        };
        if bytes.len() > 64 * 1024 {
            return Self::default();
        }
        let Ok(value) = serde_json::from_slice::<PersistedRegistry>(&bytes) else {
            return Self::default();
        };
        if value.schema_version != SCHEMA || !valid_entries(&value.entries) {
            return Self::default();
        }
        Self {
            entries: value.entries,
        }
    }

    pub fn views(&self) -> Vec<RecentProjectView> {
        self.entries
            .iter()
            .map(|entry| RecentProjectView {
                opaque_id: entry.opaque_id.clone(),
                display_name: entry.display_name.clone(),
            })
            .collect()
    }

    pub fn remember(
        &mut self,
        path: PathBuf,
        display_name: &str,
        filesystem: &impl RecentProjectFilesystem,
        storage: &mut impl RecentProjectStorage,
    ) -> Result<(), ()> {
        let display_name = safe_display_name(display_name).ok_or(())?;
        let identity = filesystem.probe_regular_no_follow(&path)?;
        let opaque_id = opaque_id(identity);
        let mut next = self.entries.clone();
        next.retain(|entry| entry.identity != identity && entry.path != path);
        next.insert(
            0,
            PersistedEntry {
                opaque_id,
                display_name,
                path,
                identity,
            },
        );
        next.truncate(MAX_RECENT);
        persist(&next, storage)?;
        self.entries = next;
        Ok(())
    }

    /// Resolves an opaque selection only after reopening no-follow and matching
    /// its canonical identity. Missing, renamed/replaced, linked, or special
    /// entries are removed atomically and never return a path.
    pub fn select(
        &mut self,
        opaque: &str,
        filesystem: &impl RecentProjectFilesystem,
        storage: &mut impl RecentProjectStorage,
    ) -> Result<Option<PathBuf>, ()> {
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.opaque_id == opaque)
        else {
            return Ok(None);
        };
        let entry = &self.entries[index];
        if filesystem.probe_regular_no_follow(&entry.path).ok() == Some(entry.identity) {
            return Ok(Some(entry.path.clone()));
        }
        let mut next = self.entries.clone();
        next.remove(index);
        persist(&next, storage)?;
        self.entries = next;
        Ok(None)
    }
}

fn safe_display_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_DISPLAY_NAME_BYTES
        || trimmed.chars().any(char::is_control)
        || trimmed.contains(['/', '\\'])
    {
        return None;
    }
    Some(trimmed.to_owned())
}

fn valid_entries(entries: &[PersistedEntry]) -> bool {
    if entries.len() > MAX_RECENT {
        return false;
    }
    let mut opaque = HashSet::new();
    let mut identities = HashSet::new();
    entries.iter().all(|entry| {
        safe_display_name(&entry.display_name).as_deref() == Some(entry.display_name.as_str())
            && entry.opaque_id == opaque_id(entry.identity)
            && opaque.insert(entry.opaque_id.clone())
            && identities.insert(entry.identity)
            && entry.path.is_absolute()
    })
}

fn opaque_id(identity: CanonicalFileIdentity) -> String {
    let digest = Sha256::digest(
        [
            identity.volume.to_le_bytes().as_slice(),
            identity.file.to_le_bytes().as_slice(),
            b"origami2-recent-v1",
        ]
        .concat(),
    );
    format!(
        "r1-{}",
        digest
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn persist(entries: &[PersistedEntry], storage: &mut impl RecentProjectStorage) -> Result<(), ()> {
    let bytes = serde_json::to_vec(&PersistedRegistry {
        schema_version: SCHEMA,
        entries: entries.to_vec(),
    })
    .map_err(|_| ())?;
    storage.replace_atomically(&bytes)
}

#[cfg(unix)]
fn canonical_identity(_path: &Path, metadata: &fs::Metadata) -> Result<CanonicalFileIdentity, ()> {
    use std::os::unix::fs::MetadataExt;
    Ok(CanonicalFileIdentity {
        volume: metadata.dev(),
        file: u128::from(metadata.ino()),
    })
}

#[cfg(windows)]
fn canonical_identity(path: &Path, _metadata: &fs::Metadata) -> Result<CanonicalFileIdentity, ()> {
    use std::{mem::MaybeUninit, os::windows::io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let file = fs::File::open(path).map_err(|_| ())?;
    let mut info = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) };
    if ok == 0 {
        return Err(());
    }
    let info = unsafe { info.assume_init() };
    if info.nNumberOfLinks != 1 {
        return Err(());
    }
    Ok(CanonicalFileIdentity {
        volume: u64::from(info.dwVolumeSerialNumber),
        file: u128::from((u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)),
    })
}

#[cfg(unix)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(unix)]
fn link_count(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink()
}
#[cfg(windows)]
fn link_count(metadata: &fs::Metadata) -> u64 {
    let _ = metadata;
    1
}

#[cfg(unix)]
fn publish_staged(_file: &fs::File, staged: &Path, destination: &Path) -> Result<(), ()> {
    fs::rename(staged, destination).map_err(|_| ())
}
#[cfg(windows)]
fn publish_staged(file: &fs::File, _staged: &Path, destination: &Path) -> Result<(), ()> {
    super::rename_windows_staged_file(file, destination).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "origami2-recent-lease-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Default)]
    struct MemoryStorage {
        bytes: Option<Vec<u8>>,
        fail: bool,
    }
    impl RecentProjectStorage for MemoryStorage {
        fn read(&self) -> Result<Option<Vec<u8>>, ()> {
            Ok(self.bytes.clone())
        }
        fn replace_atomically(&mut self, bytes: &[u8]) -> Result<(), ()> {
            if self.fail {
                return Err(());
            }
            self.bytes = Some(bytes.to_vec());
            Ok(())
        }
    }
    #[derive(Default)]
    struct Filesystem(HashMap<PathBuf, Result<CanonicalFileIdentity, ()>>);
    impl RecentProjectFilesystem for Filesystem {
        fn probe_regular_no_follow(&self, path: &Path) -> Result<CanonicalFileIdentity, ()> {
            self.0.get(path).cloned().unwrap_or(Err(()))
        }
    }
    fn path(index: u64) -> PathBuf {
        std::env::temp_dir().join(format!("origami2-recent-fixture-p{index}.ori2"))
    }
    fn identity(index: u64) -> CanonicalFileIdentity {
        CanonicalFileIdentity {
            volume: 7,
            file: index as u128,
        }
    }

    #[test]
    fn bounded_mru_exposes_only_opaque_id_and_safe_name() {
        let mut fs = Filesystem::default();
        let mut storage = MemoryStorage::default();
        let mut registry = RecentProjectRegistry::default();
        for index in 0..12 {
            fs.0.insert(path(index), Ok(identity(index)));
            registry
                .remember(path(index), &format!("Bird {index}"), &fs, &mut storage)
                .unwrap();
        }
        let views = registry.views();
        assert_eq!(views.len(), 10);
        assert_eq!(views[0].display_name, "Bird 11");
        let encoded = serde_json::to_string(&views).unwrap();
        assert!(!encoded.contains("C:\\\\work"));
        assert!(!encoded.contains("volume"));
        assert!(!encoded.contains("file"));
    }

    #[test]
    fn missing_renamed_replaced_and_link_like_probe_failure_invalidate_atomically() {
        let p = path(1);
        let mut fs = Filesystem::default();
        fs.0.insert(p.clone(), Ok(identity(1)));
        let mut storage = MemoryStorage::default();
        let mut registry = RecentProjectRegistry::default();
        registry
            .remember(p.clone(), "Crane", &fs, &mut storage)
            .unwrap();
        let id = registry.views()[0].opaque_id.clone();
        fs.0.insert(p, Ok(identity(2)));
        assert_eq!(registry.select(&id, &fs, &mut storage).unwrap(), None);
        assert!(registry.views().is_empty());
        assert!(RecentProjectRegistry::load(&storage).views().is_empty());
    }

    #[test]
    fn failed_atomic_write_never_changes_live_registry() {
        let p = path(1);
        let mut fs = Filesystem::default();
        fs.0.insert(p.clone(), Ok(identity(1)));
        let mut storage = MemoryStorage {
            fail: true,
            ..Default::default()
        };
        let mut registry = RecentProjectRegistry::default();
        assert_eq!(registry.remember(p, "Crane", &fs, &mut storage), Err(()));
        assert!(registry.views().is_empty());
    }

    #[test]
    fn tampered_terminal_data_and_unsafe_names_fail_closed() {
        let storage = MemoryStorage { bytes: Some(br#"{"schema_version":1,"entries":[{"opaque_id":"forged","display_name":"x","path":"C:\\x","identity":{"volume":1,"file":2}}]}"#.to_vec()), fail: false };
        assert!(RecentProjectRegistry::load(&storage).views().is_empty());
        assert_eq!(safe_display_name("../private.ori2"), None);
        assert_eq!(safe_display_name("bad\nname"), None);
    }

    #[test]
    fn fresh_writer_lease_blocks_second_process_and_preserves_live_bytes() {
        let directory = Directory::new();
        let destination = directory.0.join("recent.json");
        fs::write(&destination, b"live").unwrap();
        fs::write(
            destination.with_extension("recent.lock"),
            now_millis().unwrap().to_string(),
        )
        .unwrap();
        let mut storage = FileRecentProjectStorage::new(destination.clone());
        assert_eq!(storage.read().unwrap(), Some(b"live".to_vec()));
        assert_eq!(storage.replace_atomically(b"second"), Err(()));
        assert_eq!(fs::read(destination).unwrap(), b"live");
    }

    #[test]
    fn stale_lease_and_crash_stage_are_recovered_before_atomic_publish() {
        let directory = Directory::new();
        let destination = directory.0.join("recent.json");
        fs::write(&destination, b"old").unwrap();
        fs::write(destination.with_extension("recent.lock"), b"0").unwrap();
        fs::write(destination.with_extension("recent.next"), b"crash-partial").unwrap();
        let mut storage = FileRecentProjectStorage::new(destination.clone());
        assert_eq!(storage.read().unwrap(), Some(b"old".to_vec()));
        storage.replace_atomically(b"new").unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new");
        assert!(!destination.with_extension("recent.lock").exists());
        assert!(!destination.with_extension("recent.next").exists());
    }

    #[test]
    fn hostile_stage_or_storage_failure_keeps_live_registry_unchanged() {
        let directory = Directory::new();
        let destination = directory.0.join("recent.json");
        fs::write(&destination, b"live").unwrap();
        fs::create_dir(destination.with_extension("recent.next")).unwrap();
        let mut storage = FileRecentProjectStorage::new(destination.clone());
        assert_eq!(storage.read().unwrap(), Some(b"live".to_vec()));
        assert_eq!(storage.replace_atomically(b"new"), Err(()));
        assert_eq!(fs::read(destination).unwrap(), b"live");
    }

    #[test]
    fn stale_prelease_snapshot_cannot_overwrite_a_newer_process_commit() {
        let directory = Directory::new();
        let destination = directory.0.join("recent.json");
        fs::write(&destination, b"base").unwrap();
        let mut first = FileRecentProjectStorage::new(destination.clone());
        let mut second = FileRecentProjectStorage::new(destination.clone());
        assert_eq!(first.read().unwrap(), Some(b"base".to_vec()));
        assert_eq!(second.read().unwrap(), Some(b"base".to_vec()));
        first.replace_atomically(b"first commit").unwrap();
        assert_eq!(second.replace_atomically(b"lost update"), Err(()));
        assert_eq!(fs::read(destination).unwrap(), b"first commit");
    }

    #[test]
    fn concurrent_recent_transactions_reload_and_preserve_both_mru_entries() {
        let directory = Directory::new();
        let destination = directory.0.join("recent.json");
        let first_path = path(1);
        let second_path = path(2);
        let mut filesystem = Filesystem::default();
        filesystem.0.insert(first_path.clone(), Ok(identity(1)));
        filesystem.0.insert(second_path.clone(), Ok(identity(2)));

        let mut first_storage = FileRecentProjectStorage::new(destination.clone());
        let mut second_storage = FileRecentProjectStorage::new(destination.clone());
        let mut first = RecentProjectRegistry::load(&first_storage);
        let mut stale_second = RecentProjectRegistry::load(&second_storage);
        first
            .remember(first_path, "First", &filesystem, &mut first_storage)
            .unwrap();
        assert_eq!(
            stale_second.remember(
                second_path.clone(),
                "Second",
                &filesystem,
                &mut second_storage
            ),
            Err(())
        );

        let mut retry_storage = FileRecentProjectStorage::new(destination.clone());
        let mut retry = RecentProjectRegistry::load(&retry_storage);
        retry
            .remember(second_path, "Second", &filesystem, &mut retry_storage)
            .unwrap();
        let final_storage = FileRecentProjectStorage::new(destination);
        let names = RecentProjectRegistry::load(&final_storage)
            .views()
            .into_iter()
            .map(|item| item.display_name)
            .collect::<Vec<_>>();
        assert_eq!(names, ["Second", "First"]);
    }

    #[test]
    fn exhausted_retry_under_live_foreign_lease_keeps_terminal_file_unchanged() {
        let directory = Directory::new();
        let destination = directory.0.join("recent.json");
        let mut filesystem = Filesystem::default();
        filesystem.0.insert(path(1), Ok(identity(1)));
        let mut initial_storage = FileRecentProjectStorage::new(destination.clone());
        let mut initial = RecentProjectRegistry::load(&initial_storage);
        initial
            .remember(path(1), "Original", &filesystem, &mut initial_storage)
            .unwrap();
        let original = fs::read(&destination).unwrap();
        fs::write(
            destination.with_extension("recent.lock"),
            now_millis().unwrap().to_string(),
        )
        .unwrap();

        for _ in 0..2 {
            let mut storage = FileRecentProjectStorage::new(destination.clone());
            let mut registry = RecentProjectRegistry::load(&storage);
            assert_eq!(
                registry.remember(path(1), "Replacement", &filesystem, &mut storage),
                Err(())
            );
        }
        assert_eq!(fs::read(destination).unwrap(), original);
    }
}
