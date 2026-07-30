use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(target_os = "windows")]
use super::PROJECT_REPLACE_SAVE_FAILED_MESSAGE;
#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
use super::commit_unix_staged_project_file_with_journal_and_hooks;
#[cfg(target_os = "windows")]
use super::commit_windows_staged_project_file_with_journal_and_hook;
use super::{
    DialogSaveDestination, DiskSingleFileRecoveryFs, ExistingDestinationPolicy, Ori2ProjectArchive,
    ProjectDocument, RecoveryProjectLoad, SINGLE_FILE_JOURNAL_SCHEMA_V1,
    SingleFileJournalPayloadV1, SingleFileJournalPhaseV1, SingleFileRecoveryFs,
    SingleFileRecoveryObject, acquire_project_file_operation,
    acquire_project_file_operation_with_pre_identity_open_hook, decode_single_file_journal_v1,
    encode_single_file_journal_v1, hash_regular_file_no_follow_with_pre_open_hook,
    inspect_recovery_project_with_pre_open_hook, journal_path_for_target,
    load_project_archive_from_path, load_project_archive_from_path_with_pre_open_hook,
    persist_document_atomically_with_pre_publish_hook, persist_project_archive_to_destination,
    prepare_staged_file, project_directory_identity, recover_authenticated_single_file_v1,
    recover_single_file_journal_for_target,
    recover_single_file_journal_for_target_inner_with_pre_open_hook, sha256_hex_bytes,
    target_path_fingerprint, write_complete_staged_payload, write_project_archive_ori2,
};
use ori_core::{Command, EditorState};
use ori_domain::{CreasePattern, ProjectId};
use ori_domain::{Point2, VertexId};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::process::Command as ProcessCommand;

static NEXT_JOURNAL_TEST_ID: AtomicU64 = AtomicU64::new(0);
static PROJECT_FILE_OPERATION_POISON_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn journal_test_directory(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "origami2-single-journal-{label}-{}-{}",
        std::process::id(),
        NEXT_JOURNAL_TEST_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).expect("create journal test directory");
    path
}

fn test_archive_and_bytes(name: &str) -> (Ori2ProjectArchive, Vec<u8>) {
    let archive =
        Ori2ProjectArchive::document_only(ProjectDocument::new(name, CreasePattern::empty()));
    let bytes = write_project_archive_ori2(&archive).expect("serialize test archive");
    (archive, bytes)
}

#[test]
fn replace_confirmed_missing_target_rejects_concurrent_file_creation() {
    let directory = journal_test_directory("missing-replace-file-conflict");
    let target = directory.join("project.ori2");
    let (archive, bytes) = test_archive_and_bytes("new archive");
    let sentinel = b"concurrent destination";

    let result = persist_document_atomically_with_pre_publish_hook(
        &target,
        &archive,
        &bytes,
        ExistingDestinationPolicy::ReplaceConfirmed,
        || fs::write(&target, sentinel),
    );

    assert!(
        result.is_err(),
        "a concurrent file must win create-only publication"
    );
    assert_eq!(
        fs::read(&target).expect("read concurrent destination"),
        sentinel
    );
    assert_eq!(
        fs::read_dir(&directory)
            .expect("conflict directory")
            .map(|entry| entry.expect("conflict entry").file_name())
            .collect::<Vec<_>>(),
        vec![std::ffi::OsString::from("project.ori2")],
        "failed publication must clean only its private staging entry"
    );
    fs::remove_dir_all(directory).expect("cleanup conflict directory");
}

#[test]
fn replace_confirmed_missing_target_rejects_concurrent_non_regular_entry() {
    let directory = journal_test_directory("missing-replace-directory-conflict");
    let target = directory.join("project.ori2");
    let sentinel = target.join("sentinel");
    let (archive, bytes) = test_archive_and_bytes("new archive");

    let result = persist_document_atomically_with_pre_publish_hook(
        &target,
        &archive,
        &bytes,
        ExistingDestinationPolicy::ReplaceConfirmed,
        || {
            fs::create_dir(&target)?;
            fs::write(&sentinel, b"preserve directory contents")
        },
    );

    assert!(
        result.is_err(),
        "a concurrent non-regular entry must block publication"
    );
    assert!(
        fs::symlink_metadata(&target)
            .expect("concurrent directory metadata")
            .file_type()
            .is_dir()
    );
    assert_eq!(
        fs::read(&sentinel).expect("read concurrent directory sentinel"),
        b"preserve directory contents"
    );
    assert_eq!(
        fs::read_dir(&directory)
            .expect("conflict directory")
            .map(|entry| entry.expect("conflict entry").file_name())
            .collect::<Vec<_>>(),
        vec![std::ffi::OsString::from("project.ori2")],
        "failed publication must preserve the competing directory and clean staging"
    );
    fs::remove_dir_all(directory).expect("cleanup conflict directory");
}

#[test]
fn replace_confirmed_missing_target_publishes_normally_with_no_replace() {
    let directory = journal_test_directory("missing-replace-success");
    let target = directory.join("project.ori2");
    let (archive, bytes) = test_archive_and_bytes("new archive");

    persist_document_atomically_with_pre_publish_hook(
        &target,
        &archive,
        &bytes,
        ExistingDestinationPolicy::ReplaceConfirmed,
        || Ok(()),
    )
    .expect("publish initially missing destination");

    assert_eq!(
        load_project_archive_from_path(&target).expect("load published archive"),
        archive
    );
    assert_eq!(
        fs::read_dir(&directory)
            .expect("published directory")
            .map(|entry| entry.expect("published entry").file_name())
            .collect::<Vec<_>>(),
        vec![std::ffi::OsString::from("project.ori2")],
        "create-only success must leave no staging or recovery journal"
    );
    fs::remove_dir_all(directory).expect("cleanup published directory");
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

    let mut tampered: serde_json::Value = serde_json::from_slice(&encoded).expect("journal JSON");
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
            std::env::var_os("ORIGAMI2_TEST_SINGLE_FILE_SAVE_READY").expect("ready marker path"),
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
        let document = ProjectDocument::new("old archive before crash", editor.pattern().clone());
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
        authenticated_objects: Default::default(),
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
// This Windows-only fixture restores the FILE_ATTRIBUTE_READONLY bit; Unix modes are absent.
#[allow(clippy::permissions_set_readonly_false)]
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

    fs::set_permissions(&directory, fs::Permissions::from_mode(0o555)).expect("read-only parent");
    let read_only_error = persist_project_archive_to_destination(
        &DialogSaveDestination::confirmed(target.clone()),
        &new_archive,
    )
    .expect_err("read-only parent must reject save");
    assert_eq!(fs::read(&target).expect("old target"), old_bytes);
    assert_eq!(fs::read(&sentinel).expect("sentinel"), b"unowned");

    fs::set_permissions(&directory, fs::Permissions::from_mode(0o500)).expect("owner-only parent");
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

        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("restore parent");
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
fn poisoned_single_flight_registry_recovers_for_real_save_and_load_paths() {
    let _poison_test = PROJECT_FILE_OPERATION_POISON_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let directory = journal_test_directory("single-flight-poison-retry");
    let target = directory.join("project.ori2");
    let foreign = directory.join("foreign.ori2");
    let initial =
        Ori2ProjectArchive::document_only(ProjectDocument::new("initial", CreasePattern::empty()));
    let replacement = Ori2ProjectArchive::document_only(ProjectDocument::new(
        "replacement",
        CreasePattern::empty(),
    ));
    fs::write(
        &target,
        write_project_archive_ori2(&initial).expect("initial archive"),
    )
    .expect("initial target");
    fs::write(
        &foreign,
        write_project_archive_ori2(&initial).expect("foreign archive"),
    )
    .expect("foreign target");

    let poisoned = std::panic::catch_unwind(|| {
        let _active = super::ACTIVE_PROJECT_FILE_OPERATIONS
            .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
            .lock()
            .expect("unpoisoned operation registry");
        panic!("poison operation registry");
    });
    assert!(poisoned.is_err());

    assert_eq!(
        load_project_archive_from_path(&foreign).expect("foreign load recovers poison"),
        initial
    );
    persist_project_archive_to_destination(
        &DialogSaveDestination::confirmed(target.clone()),
        &replacement,
    )
    .expect("save retries after poison recovery");
    assert_eq!(
        load_project_archive_from_path(&target).expect("replacement load"),
        replacement
    );
    assert!(
        !super::ACTIVE_PROJECT_FILE_OPERATIONS
            .get()
            .expect("initialized operation registry")
            .is_poisoned()
    );
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn poisoned_stale_guard_cannot_release_replacement_owner_or_foreign_target() {
    let _poison_test = PROJECT_FILE_OPERATION_POISON_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let directory = journal_test_directory("single-flight-poison-aba");
    let target = directory.join("project.ori2");
    let foreign = directory.join("foreign.ori2");
    let archive =
        Ori2ProjectArchive::document_only(ProjectDocument::new("archive", CreasePattern::empty()));
    let bytes = write_project_archive_ori2(&archive).expect("archive");
    fs::write(&target, &bytes).expect("target");
    fs::write(&foreign, &bytes).expect("foreign");

    let stale = acquire_project_file_operation(&target).expect("stale owner");
    let replacement_owner =
        super::next_project_file_operation_owner().expect("replacement owner token");
    let replacement_keys = stale.keys.clone();
    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut active = super::ACTIVE_PROJECT_FILE_OPERATIONS
            .get()
            .expect("initialized operation registry")
            .lock()
            .expect("unpoisoned operation registry");
        for key in &replacement_keys {
            assert_eq!(
                active.insert(key.clone(), replacement_owner),
                Some(stale.owner)
            );
        }
        panic!("poison after replacement publication");
    }));
    assert!(poisoned.is_err());

    let replacement = super::ProjectFileOperationGuard {
        keys: replacement_keys,
        owner: replacement_owner,
    };
    drop(stale);
    assert!(
        load_project_archive_from_path(&target).is_err(),
        "the stale guard must not erase the replacement lease"
    );
    assert_eq!(
        load_project_archive_from_path(&foreign).expect("foreign target remains independent"),
        archive
    );
    drop(replacement);
    assert_eq!(
        load_project_archive_from_path(&target).expect("replacement owner released"),
        archive
    );
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
    let _poison_test = PROJECT_FILE_OPERATION_POISON_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let directory = journal_test_directory("single-flight-bounded");
    let baseline = super::ACTIVE_PROJECT_FILE_OPERATIONS
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        .lock()
        .expect("operation set")
        .len();
    for index in 0..512 {
        drop(
            acquire_project_file_operation(&directory.join(format!("distinct-{index:04}.ori2")))
                .expect("distinct target"),
        );
    }
    assert_eq!(
        super::ACTIVE_PROJECT_FILE_OPERATIONS
            .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
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
    let owner =
        acquire_project_file_operation(&directory.join("Project.ori2")).expect("mixed-case owner");
    assert!(acquire_project_file_operation(&directory.join("project.ORI2")).is_err());
    drop(owner);
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[cfg(unix)]
#[test]
fn single_flight_keeps_unix_case_sensitive_targets_distinct() {
    let directory = journal_test_directory("single-flight-case");
    let owner =
        acquire_project_file_operation(&directory.join("Project.ori2")).expect("mixed-case owner");
    let lower_owner = acquire_project_file_operation(&directory.join("project.ori2"))
        .expect("Unix case-sensitive target");
    drop(lower_owner);
    drop(owner);
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
#[test]
fn unix_journal_commit_rejects_old_destination_swap_after_journal_prepare() {
    let directory = journal_test_directory("unix-commit-old-swap");
    let target = directory.join("project.ori2");
    let replacement = directory.join("replacement.ori2");
    let displaced = directory.join("displaced.ori2");
    fs::write(&target, b"old-hash").expect("old destination");
    fs::write(&replacement, b"new-hash").expect("same-length replacement");
    let (archive, bytes) = test_archive_and_bytes("unix commit swap");
    let mut staged = prepare_staged_file(&target, &archive, &bytes).expect("staged project");
    let staged_path = staged.path.clone();

    let result = commit_unix_staged_project_file_with_journal_and_hooks(
        &mut staged,
        &target,
        archive.document.project_id,
        || Ok(()),
        || {
            fs::rename(&target, &displaced)?;
            fs::rename(&replacement, &target)
        },
        || Ok(()),
    );

    assert!(
        result.is_err(),
        "the journaled old destination seal must survive until backup"
    );
    assert_eq!(
        fs::read(&target).expect("replacement preserved"),
        b"new-hash"
    );
    assert_eq!(
        fs::read(&displaced).expect("old destination preserved"),
        b"old-hash"
    );
    assert!(staged_path.exists(), "verified stage remains recoverable");
    drop(staged);
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
#[test]
fn unix_journal_commit_rejects_staged_path_swap_after_old_move() {
    let directory = journal_test_directory("unix-commit-stage-swap");
    let target = directory.join("project.ori2");
    fs::write(&target, b"old-hash").expect("old destination");
    let (archive, bytes) = test_archive_and_bytes("unix staged swap");
    let mut staged = prepare_staged_file(&target, &archive, &bytes).expect("staged project");
    let staged_path = staged.path.clone();
    let displaced = directory.join("displaced-stage.ori2");
    let replacement = directory.join("replacement-stage.ori2");
    let replacement_bytes = vec![0xa5; bytes.len()];
    fs::write(&replacement, &replacement_bytes).expect("same-length replacement stage");

    let result = commit_unix_staged_project_file_with_journal_and_hooks(
        &mut staged,
        &target,
        archive.document.project_id,
        || Ok(()),
        || Ok(()),
        || {
            fs::rename(&staged_path, &displaced)?;
            fs::rename(&replacement, &staged_path)
        },
    );

    assert!(
        result.is_err(),
        "publication must retain the staged handle identity from journal preparation"
    );
    assert!(!target.exists(), "a replacement stage was not published");
    assert_eq!(
        fs::read(&staged_path).expect("replacement stage preserved"),
        replacement_bytes
    );
    assert_eq!(
        fs::read(&displaced).expect("authenticated stage preserved"),
        bytes
    );
    let backups = fs::read_dir(&directory)
        .expect("commit directory")
        .map(|entry| entry.expect("commit entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".origami2-backup-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        fs::read(&backups[0]).expect("old backup preserved"),
        b"old-hash"
    );
    drop(staged);
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[cfg(target_os = "windows")]
#[test]
fn windows_journal_commit_rejects_old_destination_swap_before_handle_publication() {
    let directory = journal_test_directory("windows-commit-old-swap");
    let target = directory.join("project.ori2");
    let replacement = directory.join("replacement.ori2");
    let displaced = directory.join("displaced.ori2");
    fs::write(&target, b"old-hash").expect("old destination");
    fs::write(&replacement, b"new-hash").expect("same-length replacement");
    let (archive, bytes) = test_archive_and_bytes("windows commit swap");
    let mut staged = prepare_staged_file(&target, &archive, &bytes).expect("staged project");
    let staged_path = staged.path.clone();

    let result = commit_windows_staged_project_file_with_journal_and_hook(
        &mut staged,
        &target,
        archive.document.project_id,
        || {
            fs::rename(&target, &displaced)?;
            fs::rename(&replacement, &target)
        },
    );

    assert!(
        result.is_err(),
        "the old destination identity must be revalidated before replacement"
    );
    assert_eq!(
        fs::read(&target).expect("replacement preserved"),
        b"new-hash"
    );
    assert_eq!(
        fs::read(&displaced).expect("old destination preserved"),
        b"old-hash"
    );
    assert!(staged_path.exists(), "verified stage was not published");
    drop(staged);
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn no_follow_hash_rejects_same_length_file_swap_between_inspection_and_open() {
    let directory = journal_test_directory("hash-file-swap");
    let path = directory.join("project.ori2");
    let displaced = directory.join("displaced.ori2");
    let replacement = directory.join("replacement.ori2");
    fs::write(&path, b"old-hash").expect("original");
    fs::write(&replacement, b"new-hash").expect("same-length replacement");

    let result = hash_regular_file_no_follow_with_pre_open_hook(&path, || {
        fs::rename(&path, &displaced)?;
        fs::rename(&replacement, &path)
    });

    assert!(
        result.is_err(),
        "the opened file identity must match the inspected entry"
    );
    assert_eq!(fs::read(&path).expect("replacement"), b"new-hash");
    assert_eq!(
        fs::read(&displaced).expect("displaced original"),
        b"old-hash"
    );
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn recovery_object_hash_rejects_same_length_file_swap_between_inspection_and_open() {
    let directory = journal_test_directory("recovery-hash-file-swap");
    let target = directory.join("project.ori2");
    let displaced = directory.join("displaced.ori2");
    let replacement = directory.join("replacement.ori2");
    fs::write(&target, b"old-hash").expect("original");
    fs::write(&replacement, b"new-hash").expect("same-length replacement");
    let adapter = DiskSingleFileRecoveryFs {
        directory_identity: project_directory_identity(&directory)
            .expect("recovery directory identity"),
        authenticated_objects: Default::default(),
        temp: directory.join("temp.ori2"),
        backup: directory.join("backup.ori2"),
        journal: directory.join("journal.json"),
        directory: directory.clone(),
        target: target.clone(),
    };

    let result = adapter.object_sha256_with_pre_open_hook(SingleFileRecoveryObject::Target, || {
        fs::rename(&target, &displaced)?;
        fs::rename(&replacement, &target)
    });

    assert!(
        result.is_err(),
        "recovery hashing must reject an opened object with a new identity"
    );
    assert_eq!(fs::read(&target).expect("replacement"), b"new-hash");
    assert_eq!(
        fs::read(&displaced).expect("displaced original"),
        b"old-hash"
    );
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn journal_read_rejects_same_length_file_swap_between_inspection_and_open() {
    let directory = journal_test_directory("journal-read-file-swap");
    let target = directory.join("project.ori2");
    let temp_name = "temp-journal-read-swap";
    let backup_name = "backup-journal-read-swap";
    let temp = directory.join(temp_name);
    let old_bytes = b"old complete ori2";
    let new_bytes = b"new complete ori2";
    fs::write(&target, old_bytes).expect("old target");
    fs::write(&temp, new_bytes).expect("new staged object");
    let project_id = ProjectId::new();
    let fingerprint = target_path_fingerprint(&target).expect("target fingerprint");
    let payload = SingleFileJournalPayloadV1 {
        schema_version: SINGLE_FILE_JOURNAL_SCHEMA_V1,
        project_id,
        target_path_sha256: fingerprint.clone(),
        transaction_id: "journal-read-swap".to_owned(),
        temp_object_id: temp_name.to_owned(),
        temp_sha256: sha256_hex_bytes(new_bytes),
        backup_object_id: backup_name.to_owned(),
        old_sha256: Some(sha256_hex_bytes(old_bytes)),
        phase: SingleFileJournalPhaseV1::Prepared,
    };
    let journal = journal_path_for_target(&target, &fingerprint).expect("journal path");
    let replacement = directory.join("replacement-journal.json");
    let displaced = directory.join("displaced-journal.json");
    let encoded = encode_single_file_journal_v1(payload).expect("journal bytes");
    fs::write(&journal, &encoded).expect("original journal");
    fs::write(&replacement, &encoded).expect("same-length replacement journal");

    let result = recover_single_file_journal_for_target_inner_with_pre_open_hook(
        &target,
        Some(project_id),
        || {
            fs::rename(&journal, &displaced)?;
            fs::rename(&replacement, &journal)
        },
    );

    assert!(
        result.is_err(),
        "journal recovery must bind the read handle to the inspected entry"
    );
    assert_eq!(fs::read(&target).expect("old target preserved"), old_bytes);
    assert_eq!(fs::read(&temp).expect("staged object preserved"), new_bytes);
    assert_eq!(
        fs::read(&journal).expect("replacement journal preserved"),
        encoded
    );
    assert_eq!(
        fs::read(&displaced).expect("inspected journal preserved"),
        encoded
    );
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn recovery_inspection_rejects_valid_file_swap_between_inspection_and_open() {
    let directory = journal_test_directory("recovery-inspect-file-swap");
    let path = directory.join("current-project.ori2");
    let replacement = directory.join("replacement.ori2");
    let displaced = directory.join("displaced.ori2");
    let (_archive, bytes) = test_archive_and_bytes("recovery swap");
    fs::write(&path, &bytes).expect("original recovery archive");
    fs::write(&replacement, &bytes).expect("valid same-length replacement");

    let result = inspect_recovery_project_with_pre_open_hook(&path, || {
        fs::rename(&path, &displaced)?;
        fs::rename(&replacement, &path)
    });

    assert_eq!(
        result,
        RecoveryProjectLoad::Invalid,
        "a valid replacement archive still has the wrong identity"
    );
    assert_eq!(fs::read(&path).expect("replacement preserved"), bytes);
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn project_load_rejects_valid_file_swap_between_inspection_and_open() {
    let directory = journal_test_directory("project-load-file-swap");
    let path = directory.join("project.ori2");
    let replacement = directory.join("replacement.ori2");
    let displaced = directory.join("displaced.ori2");
    let (archive, bytes) = test_archive_and_bytes("project swap");
    fs::write(&path, &bytes).expect("original project archive");
    fs::write(&replacement, &bytes).expect("valid same-length replacement");

    let result = load_project_archive_from_path_with_pre_open_hook(&path, || {
        fs::rename(&path, &displaced)?;
        fs::rename(&replacement, &path)
    });

    assert!(
        result.is_err(),
        "project loading must not accept a valid replacement identity"
    );
    assert_eq!(
        load_project_archive_from_path(&path).expect("replacement remains a valid archive"),
        archive
    );
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn operation_identity_key_rejects_file_swap_between_inspection_and_open() {
    let directory = journal_test_directory("operation-identity-file-swap");
    let path = directory.join("project.ori2");
    let replacement = directory.join("replacement.ori2");
    let displaced = directory.join("displaced.ori2");
    fs::write(&path, b"old-hash").expect("original");
    fs::write(&replacement, b"new-hash").expect("same-length replacement");

    let result = acquire_project_file_operation_with_pre_identity_open_hook(&path, || {
        fs::rename(&path, &displaced)?;
        fs::rename(&replacement, &path)
    });

    assert!(
        result.is_err(),
        "the ownership identity key must match the inspected entry"
    );
    drop(acquire_project_file_operation(&path).expect("failed attempt leaked no ownership"));
    assert_eq!(fs::read(&path).expect("replacement"), b"new-hash");
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn recovery_rename_rejects_authenticated_source_swap_before_publication() {
    let directory = journal_test_directory("recovery-rename-source-swap");
    let target = directory.join("project.ori2");
    let temp = directory.join("temp.ori2");
    let replacement = directory.join("replacement.ori2");
    let displaced = directory.join("displaced.ori2");
    fs::write(&temp, b"old-hash").expect("authenticated source");
    fs::write(&replacement, b"new-hash").expect("same-length replacement");
    let mut adapter = DiskSingleFileRecoveryFs {
        directory_identity: project_directory_identity(&directory)
            .expect("recovery directory identity"),
        authenticated_objects: Default::default(),
        target: target.clone(),
        temp: temp.clone(),
        backup: directory.join("backup.ori2"),
        journal: directory.join("journal.json"),
        directory: directory.clone(),
    };
    assert_eq!(
        adapter
            .object_sha256(SingleFileRecoveryObject::Temp)
            .expect("authenticate temp"),
        Some(sha256_hex_bytes(b"old-hash"))
    );

    let result = adapter.rename_object_with_pre_rename_hook(
        SingleFileRecoveryObject::Temp,
        SingleFileRecoveryObject::Target,
        || {
            fs::rename(&temp, &displaced)?;
            fs::rename(&replacement, &temp)
        },
    );

    assert!(
        result.is_err(),
        "only the authenticated source identity may be published"
    );
    assert!(!target.exists());
    assert_eq!(fs::read(&temp).expect("replacement preserved"), b"new-hash");
    assert_eq!(
        fs::read(&displaced).expect("authenticated source preserved"),
        b"old-hash"
    );
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn recovery_rename_atomically_rejects_destination_conflict() {
    let directory = journal_test_directory("recovery-rename-destination-conflict");
    let target = directory.join("project.ori2");
    let temp = directory.join("temp.ori2");
    fs::write(&temp, b"authenticated source").expect("authenticated source");
    let mut adapter = DiskSingleFileRecoveryFs {
        directory_identity: project_directory_identity(&directory)
            .expect("recovery directory identity"),
        authenticated_objects: Default::default(),
        target: target.clone(),
        temp: temp.clone(),
        backup: directory.join("backup.ori2"),
        journal: directory.join("journal.json"),
        directory: directory.clone(),
    };
    assert!(
        adapter
            .object_sha256(SingleFileRecoveryObject::Temp)
            .expect("authenticate temp")
            .is_some()
    );

    let result = adapter.rename_object_with_pre_rename_hook(
        SingleFileRecoveryObject::Temp,
        SingleFileRecoveryObject::Target,
        || fs::write(&target, b"destination sentinel"),
    );

    assert!(
        result.is_err(),
        "destination reservation and rename must never overwrite a conflict"
    );
    assert_eq!(
        fs::read(&temp).expect("source preserved"),
        b"authenticated source"
    );
    assert_eq!(
        fs::read(&target).expect("destination preserved"),
        b"destination sentinel"
    );
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[test]
fn recovery_remove_rejects_file_swap_before_final_identity_check() {
    let directory = journal_test_directory("recovery-remove-file-swap");
    let journal = directory.join("journal.json");
    let replacement = directory.join("replacement.json");
    let displaced = directory.join("displaced.json");
    fs::write(&journal, b"old-hash").expect("original private object");
    fs::write(&replacement, b"new-hash").expect("same-length replacement");
    let mut adapter = DiskSingleFileRecoveryFs {
        directory_identity: project_directory_identity(&directory)
            .expect("recovery directory identity"),
        authenticated_objects: Default::default(),
        target: directory.join("project.ori2"),
        temp: directory.join("temp.ori2"),
        backup: directory.join("backup.ori2"),
        journal: journal.clone(),
        directory: directory.clone(),
    };

    let result =
        adapter.remove_object_with_pre_delete_hook(SingleFileRecoveryObject::Journal, || {
            fs::rename(&journal, &displaced)?;
            fs::rename(&replacement, &journal)
        });

    assert!(
        result.is_err(),
        "a replacement private object must not be unlinked"
    );
    assert_eq!(
        fs::read(&journal).expect("replacement preserved"),
        b"new-hash"
    );
    assert_eq!(
        fs::read(&displaced).expect("inspected object preserved"),
        b"old-hash"
    );
    fs::remove_dir_all(directory).expect("cleanup test directory");
}

#[cfg(unix)]
#[test]
fn no_follow_hash_rejects_symlink_swap_between_inspection_and_open() {
    use std::os::unix::fs::symlink;

    let directory = journal_test_directory("hash-symlink-swap");
    let path = directory.join("project.ori2");
    let displaced = directory.join("displaced.ori2");
    let sentinel = directory.join("outside-sentinel");
    fs::write(&path, b"original").expect("original");
    fs::write(&sentinel, b"preserve").expect("sentinel");

    let result = hash_regular_file_no_follow_with_pre_open_hook(&path, || {
        fs::rename(&path, &displaced)?;
        symlink(&sentinel, &path)
    });

    assert!(
        result.is_err(),
        "a symlink installed after inspection must never be followed"
    );
    assert_eq!(
        fs::read(&sentinel).expect("sentinel preserved"),
        b"preserve"
    );
    assert_eq!(
        fs::read(&displaced).expect("displaced original"),
        b"original"
    );
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
