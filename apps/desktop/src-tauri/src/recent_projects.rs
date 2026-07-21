//! Private recent-project registry. Filesystem paths and identities never cross IPC.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SCHEMA: u8 = 1;
const MAX_RECENT: usize = 10;
const MAX_DISPLAY_NAME_BYTES: usize = 160;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
        PathBuf::from(format!(r"C:\work\p{index}.ori2"))
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
}
