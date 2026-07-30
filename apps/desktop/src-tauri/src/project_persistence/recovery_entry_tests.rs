use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use ori_domain::CreasePattern;

use super::*;

static NEXT_TEST_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn windows_full_file_identity_rejects_invalid_sentinels() {
    assert_eq!(windows_file_identity_from_full_file_id_v1(1, [0; 16]), None);
    assert_eq!(
        windows_file_identity_from_full_file_id_v1(1, [u8::MAX; 16]),
        None
    );
    let mut valid_identifier = [0; 16];
    valid_identifier[0] = 1;
    assert_eq!(
        windows_file_identity_from_full_file_id_v1(0, valid_identifier),
        None
    );
    assert_eq!(
        windows_file_identity_from_full_file_id_v1(7, valid_identifier),
        Some(FileSystemObjectIdentityV1(7, 1, 0))
    );
}

#[test]
fn windows_full_file_identity_preserves_upper_64_bits() {
    let mut first_identifier = [0; 16];
    first_identifier[..8].copy_from_slice(&11_u64.to_le_bytes());
    first_identifier[8..].copy_from_slice(&17_u64.to_le_bytes());
    let mut second_identifier = first_identifier;
    second_identifier[8..].copy_from_slice(&19_u64.to_le_bytes());

    let first = windows_file_identity_from_full_file_id_v1(23, first_identifier).expect("first ID");
    let second =
        windows_file_identity_from_full_file_id_v1(23, second_identifier).expect("second ID");

    assert_eq!(first, FileSystemObjectIdentityV1(23, 11, 17));
    assert_eq!(second, FileSystemObjectIdentityV1(23, 11, 19));
    assert_ne!(first, second);
}

#[cfg(unix)]
#[test]
fn stable_bounded_read_rejects_same_inode_same_length_mutation() {
    let directory = TestDirectory::new("stable-read-mutation");
    let path = directory.slot();
    fs::write(&path, b"old-hash").expect("original bytes");
    let entry_metadata = fs::symlink_metadata(&path).expect("entry metadata");
    let (mut file, opened_metadata) =
        open_inspected_regular_file_no_follow_for_stable_read_with_pre_open_hook(
            &path,
            &entry_metadata,
            false,
            || Ok(()),
        )
        .expect("stable read handle");
    let opened_identity =
        regular_file_identity_v1(&file, &opened_metadata).expect("opened identity");

    let result = read_opened_regular_file_bounded_stably_with_post_read_hook_v1(
        &path,
        &mut file,
        &opened_metadata,
        false,
        64,
        || fs::write(&path, b"new-hash"),
    );

    assert!(
        result.is_err(),
        "same-inode same-length mutation must fail closed"
    );
    let final_metadata = file.metadata().expect("final handle metadata");
    assert_eq!(
        regular_file_identity_v1(&file, &final_metadata),
        Some(opened_identity),
        "the mutation retained the authenticated inode"
    );
    assert_eq!(opened_metadata.len(), final_metadata.len());
    assert_eq!(fs::read(&path).expect("mutated bytes"), b"new-hash");
}

#[cfg(any(unix, target_os = "windows"))]
#[test]
fn stable_bounded_read_rejects_path_identity_swap_after_first_read() {
    let directory = TestDirectory::new("stable-read-path-swap");
    let path = directory.slot();
    let displaced = directory.0.join("displaced.ori2");
    let replacement = directory.0.join("replacement.ori2");
    fs::write(&path, b"same-hash").expect("original bytes");
    fs::write(&replacement, b"same-hash").expect("replacement bytes");
    let entry_metadata = fs::symlink_metadata(&path).expect("entry metadata");
    let (mut file, opened_metadata) =
        open_inspected_regular_file_no_follow_for_stable_read_with_pre_open_hook(
            &path,
            &entry_metadata,
            false,
            || Ok(()),
        )
        .expect("stable read handle");

    let result = read_opened_regular_file_bounded_stably_with_post_read_hook_v1(
        &path,
        &mut file,
        &opened_metadata,
        false,
        64,
        || {
            fs::rename(&path, &displaced)?;
            fs::rename(&replacement, &path)
        },
    );

    assert!(
        result.is_err(),
        "a byte-identical pathname replacement must fail closed"
    );
    assert_eq!(fs::read(&path).expect("replacement remains"), b"same-hash");
    assert_eq!(
        fs::read(&displaced).expect("authenticated file remains"),
        b"same-hash"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_stable_read_handle_denies_writers_without_changing_general_open_sharing() {
    let directory = TestDirectory::new("stable-read-sharing");
    let path = directory.slot();
    fs::write(&path, b"stable").expect("original bytes");
    let entry_metadata = fs::symlink_metadata(&path).expect("entry metadata");
    let (stable_file, _) =
        open_inspected_regular_file_no_follow_for_stable_read_with_pre_open_hook(
            &path,
            &entry_metadata,
            false,
            || Ok(()),
        )
        .expect("stable read handle");
    assert!(
        std::fs::OpenOptions::new().write(true).open(&path).is_err(),
        "stable reads must withhold writer sharing"
    );
    drop(stable_file);

    let ordinary_file = open_regular_file_no_follow(&path).expect("ordinary read handle");
    assert!(
        std::fs::OpenOptions::new().write(true).open(&path).is_ok(),
        "save/rename-compatible open sharing must remain unchanged"
    );
    drop(ordinary_file);
}

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
