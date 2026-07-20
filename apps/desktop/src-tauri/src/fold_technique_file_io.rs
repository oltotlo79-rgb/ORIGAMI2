use std::{
    fs::{self, File},
    io::{Read, Take},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use ori_instructions::{
    FoldTechniqueFileDocumentV1, FoldTechniqueFileError, MAX_FOLD_TECHNIQUE_FILE_BYTES,
    read_fold_technique_file_v1, validate_fold_technique_file_v1, write_fold_technique_file_v1,
};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use super::{
    crease_export::persist_export_bytes_to_destination,
    project_persistence::{metadata_is_plain_regular_file, open_regular_file_no_follow},
    save_path::{self, DialogSaveDestination},
};

const FILE_FILTER_LABEL_JA: &str = "ORIGAMI2 折り技法ファイル";
const FILE_FILTER_LABEL_EN: &str = "ORIGAMI2 fold technique file";
const OPEN_TITLE_JA: &str = "折り技法ファイルを開く";
const OPEN_TITLE_EN: &str = "Open fold technique file";
const SAVE_TITLE_JA: &str = "折り技法ファイルを別名で保存";
const SAVE_TITLE_EN: &str = "Save fold technique file as";
const DEFAULT_FILE_NAME: &str = "fold-techniques.json";

const ERROR_BUSY: &str = "fold_technique_busy";
const ERROR_INVALID_LOCALE: &str = "fold_technique_invalid_locale";
const ERROR_INVALID_REQUEST: &str = "fold_technique_invalid_request";
const ERROR_OPEN_FAILED: &str = "fold_technique_open_failed";
const ERROR_NOT_REGULAR_FILE: &str = "fold_technique_not_regular_file";
const ERROR_TOO_LARGE: &str = "fold_technique_too_large";
const ERROR_READ_FAILED: &str = "fold_technique_read_failed";
const ERROR_INVALID_DOCUMENT: &str = "fold_technique_invalid_document";
const ERROR_SAVE_FAILED: &str = "fold_technique_save_failed";

/// Process-wide single-flight boundary for the two native file dialogs.
///
/// The WebView never receives a path or unvalidated byte buffer. The gate is
/// also enforced natively so a second IPC caller cannot race the visible UI.
#[derive(Clone, Default)]
pub(super) struct FoldTechniqueFileIoState {
    busy: Arc<AtomicBool>,
}

impl FoldTechniqueFileIoState {
    fn try_acquire(&self) -> Result<FoldTechniqueFileIoPermit, String> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| ERROR_BUSY.to_owned())?;
        Ok(FoldTechniqueFileIoPermit {
            busy: Arc::clone(&self.busy),
        })
    }
}

struct FoldTechniqueFileIoPermit {
    busy: Arc<AtomicBool>,
}

impl Drop for FoldTechniqueFileIoPermit {
    fn drop(&mut self) {
        self.busy.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy)]
enum DialogLocale {
    Ja,
    En,
}

impl DialogLocale {
    fn parse(locale: &str) -> Result<Self, String> {
        match locale {
            "ja" => Ok(Self::Ja),
            "en" => Ok(Self::En),
            _ => Err(ERROR_INVALID_LOCALE.to_owned()),
        }
    }

    const fn filter_label(self) -> &'static str {
        match self {
            Self::Ja => FILE_FILTER_LABEL_JA,
            Self::En => FILE_FILTER_LABEL_EN,
        }
    }

    const fn open_title(self) -> &'static str {
        match self {
            Self::Ja => OPEN_TITLE_JA,
            Self::En => OPEN_TITLE_EN,
        }
    }

    const fn save_title(self) -> &'static str {
        match self {
            Self::Ja => SAVE_TITLE_JA,
            Self::En => SAVE_TITLE_EN,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OpenFoldTechniqueFileResponse {
    request_id: u32,
    canceled: bool,
    document: Option<FoldTechniqueFileDocumentV1>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SaveFoldTechniqueFileResponse {
    request_id: u32,
    canceled: bool,
    document: Option<FoldTechniqueFileDocumentV1>,
}

#[tauri::command]
pub(super) async fn open_fold_technique_file(
    app: AppHandle,
    state: tauri::State<'_, FoldTechniqueFileIoState>,
    request_id: u32,
    locale: String,
) -> Result<OpenFoldTechniqueFileResponse, String> {
    validate_request_id(request_id)?;
    let locale = DialogLocale::parse(&locale)?;
    let _permit = state.try_acquire()?;
    let selected = app
        .dialog()
        .file()
        .add_filter(locale.filter_label(), &["json"])
        .set_title(locale.open_title())
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(OpenFoldTechniqueFileResponse {
            request_id,
            canceled: true,
            document: None,
        });
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(|_| ERROR_OPEN_FAILED.to_owned())?;
    let document = tauri::async_runtime::spawn_blocking(move || {
        let _permit = _permit;
        load_fold_technique_document(&path)
    })
    .await
    .map_err(|_| ERROR_READ_FAILED.to_owned())??;
    Ok(OpenFoldTechniqueFileResponse {
        request_id,
        canceled: false,
        document: Some(document),
    })
}

#[tauri::command]
pub(super) async fn save_fold_technique_file_as(
    app: AppHandle,
    state: tauri::State<'_, FoldTechniqueFileIoState>,
    request_id: u32,
    locale: String,
    document: FoldTechniqueFileDocumentV1,
) -> Result<SaveFoldTechniqueFileResponse, String> {
    validate_request_id(request_id)?;
    let locale = DialogLocale::parse(&locale)?;
    let _permit = state.try_acquire()?;

    // Native ori-instructions validation is the trust boundary for every
    // caller-created document. Serialization includes an independent strict
    // read-back before the file picker is shown.
    let file =
        validate_fold_technique_file_v1(document).map_err(map_fold_technique_validation_error)?;
    let bytes = write_fold_technique_file_v1(&file).map_err(map_fold_technique_validation_error)?;
    let canonical_document = file.document().clone();
    let suggested_name = suggested_file_name(&canonical_document);
    let selected = app
        .dialog()
        .file()
        .add_filter(locale.filter_label(), &["json"])
        .set_file_name(suggested_name)
        .set_title(locale.save_title())
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(SaveFoldTechniqueFileResponse {
            request_id,
            canceled: true,
            document: None,
        });
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(|_| ERROR_SAVE_FAILED.to_owned())?;
    let destination = save_path::normalize_dialog_save_path(path, "json")
        .map_err(|_| ERROR_SAVE_FAILED.to_owned())?;
    let saved_document = canonical_document.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = _permit;
        persist_fold_technique_file(&destination, &bytes)
    })
    .await
    .map_err(|_| ERROR_SAVE_FAILED.to_owned())??;
    Ok(SaveFoldTechniqueFileResponse {
        request_id,
        canceled: false,
        document: Some(saved_document),
    })
}

fn validate_request_id(request_id: u32) -> Result<(), String> {
    if request_id == 0 {
        Err(ERROR_INVALID_REQUEST.to_owned())
    } else {
        Ok(())
    }
}

fn load_fold_technique_document(path: &Path) -> Result<FoldTechniqueFileDocumentV1, String> {
    let entry_metadata = fs::symlink_metadata(path).map_err(|_| ERROR_OPEN_FAILED.to_owned())?;
    if !entry_metadata.file_type().is_file() {
        return Err(ERROR_NOT_REGULAR_FILE.to_owned());
    }
    if entry_metadata.len() > MAX_FOLD_TECHNIQUE_FILE_BYTES as u64 {
        return Err(ERROR_TOO_LARGE.to_owned());
    }

    // The no-follow open and the plain-file check both apply to the same
    // handle. A final-component symlink/reparse/FIFO swap after the path
    // precheck therefore fails closed instead of being followed or blocking.
    let file = open_regular_file_no_follow(path).map_err(|_| ERROR_OPEN_FAILED.to_owned())?;
    let opened_metadata = file.metadata().map_err(|_| ERROR_READ_FAILED.to_owned())?;
    if !metadata_is_plain_regular_file(&opened_metadata) {
        return Err(ERROR_NOT_REGULAR_FILE.to_owned());
    }
    if opened_metadata.len() > MAX_FOLD_TECHNIQUE_FILE_BYTES as u64 {
        return Err(ERROR_TOO_LARGE.to_owned());
    }

    let mut limited = file.take((MAX_FOLD_TECHNIQUE_FILE_BYTES as u64) + 1);
    let bytes = read_bounded(&mut limited)?;
    let validated =
        read_fold_technique_file_v1(&bytes).map_err(map_fold_technique_validation_error)?;
    Ok(validated.document().clone())
}

fn read_bounded(reader: &mut Take<File>) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| ERROR_READ_FAILED.to_owned())?;
    if bytes.len() > MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err(ERROR_TOO_LARGE.to_owned());
    }
    Ok(bytes)
}

fn persist_fold_technique_file(
    destination: &DialogSaveDestination,
    bytes: &[u8],
) -> Result<(), String> {
    if bytes.len() > MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err(ERROR_TOO_LARGE.to_owned());
    }
    // The existing export publisher writes a verified sibling staging file
    // and then applies the dialog-confirmed replace/no-replace policy in one
    // atomic name operation. It never opens the destination, so a symlink
    // swapped into the final component is replaced as a directory entry and
    // its target is never followed.
    persist_export_bytes_to_destination(destination, bytes)
        .map_err(|_| ERROR_SAVE_FAILED.to_owned())
}

fn map_fold_technique_validation_error(error: FoldTechniqueFileError) -> String {
    match error {
        FoldTechniqueFileError::InputTooLarge => ERROR_TOO_LARGE.to_owned(),
        _ => ERROR_INVALID_DOCUMENT.to_owned(),
    }
}

fn suggested_file_name(document: &FoldTechniqueFileDocumentV1) -> String {
    let package = document.package_id.trim();
    if package.is_empty() {
        return DEFAULT_FILE_NAME.to_owned();
    }
    let mut base = String::with_capacity(package.len().min(80));
    for character in package.chars().take(72) {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
            base.push(character);
        } else {
            base.push('-');
        }
    }
    let base = base.trim_matches('-');
    if base.is_empty() {
        DEFAULT_FILE_NAME.to_owned()
    } else {
        format!("{base}.json")
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use ori_instructions::{FoldTechniqueFileDocumentV1, validate_fold_technique_file_v1};
    use serde_json::json;

    use super::*;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "origami2-fold-technique-file-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn strict_regular_file_round_trip_uses_only_canonical_document() {
        let directory = TestDirectory::new();
        let path = directory.0.join("technique.json");
        let document = fixture_document();
        let validated = validate_fold_technique_file_v1(document.clone()).expect("valid fixture");
        let bytes = write_fold_technique_file_v1(&validated).expect("canonical bytes");
        fs::write(&path, bytes).expect("write fixture");

        let loaded = load_fold_technique_document(&path).expect("load strict document");

        assert_eq!(loaded, *validated.document());
        assert_eq!(loaded, document);
    }

    #[test]
    fn invalid_and_oversized_inputs_fail_with_fixed_categories() {
        let directory = TestDirectory::new();
        let invalid = directory.0.join("invalid.json");
        fs::write(&invalid, br#"{"schema":"hostile","path":"C:\\secret"}"#)
            .expect("write invalid fixture");
        assert_eq!(
            load_fold_technique_document(&invalid),
            Err(ERROR_INVALID_DOCUMENT.to_owned())
        );

        let oversized = directory.0.join("oversized.json");
        let file = File::create(&oversized).expect("create oversized fixture");
        file.set_len((MAX_FOLD_TECHNIQUE_FILE_BYTES as u64) + 1)
            .expect("extend fixture");
        assert_eq!(
            load_fold_technique_document(&oversized),
            Err(ERROR_TOO_LARGE.to_owned())
        );
    }

    #[test]
    fn directories_and_links_are_not_followed_as_input() {
        let directory = TestDirectory::new();
        assert_eq!(
            load_fold_technique_document(&directory.0),
            Err(ERROR_NOT_REGULAR_FILE.to_owned())
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let target = directory.0.join("target.json");
            fs::write(&target, b"{}").expect("write target");
            let link = directory.0.join("link.json");
            symlink(&target, &link).expect("create symlink");
            assert_eq!(
                load_fold_technique_document(&link),
                Err(ERROR_NOT_REGULAR_FILE.to_owned())
            );
            assert_eq!(fs::read(target).expect("read target"), b"{}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_save_replaces_a_symlink_entry_without_following_its_target() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new();
        let target = directory.0.join("target.json");
        fs::write(&target, b"target stays unchanged").expect("write target");
        let link = directory.0.join("shared.json");
        symlink(&target, &link).expect("create symlink");
        let document = fixture_document();
        let validated = validate_fold_technique_file_v1(document).expect("valid fixture");
        let bytes = write_fold_technique_file_v1(&validated).expect("canonical bytes");

        persist_fold_technique_file(&DialogSaveDestination::confirmed(link.clone()), &bytes)
            .expect("replace link entry");

        assert_eq!(
            fs::read(&target).expect("read target"),
            b"target stays unchanged"
        );
        assert!(
            fs::symlink_metadata(&link)
                .expect("inspect replacement")
                .file_type()
                .is_file()
        );
        assert_eq!(
            load_fold_technique_document(&link).expect("read replacement"),
            *validated.document()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_reparse_input_is_rejected_and_confirmed_save_never_follows_it() {
        use std::os::windows::fs::symlink_file;

        let directory = TestDirectory::new();
        let target = directory.0.join("target.json");
        fs::write(&target, b"target stays unchanged").expect("write target");
        let link = directory.0.join("shared.json");
        if symlink_file(&target, &link).is_err() {
            // Developer Mode or link privilege is not guaranteed on CI.
            return;
        }
        assert_eq!(
            load_fold_technique_document(&link),
            Err(ERROR_NOT_REGULAR_FILE.to_owned())
        );
        let document = fixture_document();
        let validated = validate_fold_technique_file_v1(document).expect("valid fixture");
        let bytes = write_fold_technique_file_v1(&validated).expect("canonical bytes");

        persist_fold_technique_file(&DialogSaveDestination::confirmed(link.clone()), &bytes)
            .expect("replace reparse entry");

        assert_eq!(
            fs::read(&target).expect("read target"),
            b"target stays unchanged"
        );
        assert!(
            fs::symlink_metadata(&link)
                .expect("inspect replacement")
                .file_type()
                .is_file()
        );
        assert_eq!(
            load_fold_technique_document(&link).expect("read replacement"),
            *validated.document()
        );
    }

    #[test]
    fn validated_save_is_atomic_and_reads_back_identically() {
        let directory = TestDirectory::new();
        let path = directory.0.join("shared.json");
        fs::write(&path, b"old").expect("write old destination");
        let document = fixture_document();
        let validated = validate_fold_technique_file_v1(document.clone()).expect("valid fixture");
        let bytes = write_fold_technique_file_v1(&validated).expect("canonical bytes");

        persist_fold_technique_file(&DialogSaveDestination::confirmed(path.clone()), &bytes)
            .expect("atomic save");

        assert_eq!(
            load_fold_technique_document(&path).expect("read saved document"),
            document
        );
    }

    #[test]
    fn request_and_locale_domains_are_closed() {
        assert_eq!(
            validate_request_id(0),
            Err(ERROR_INVALID_REQUEST.to_owned())
        );
        assert!(validate_request_id(1).is_ok());
        assert!(DialogLocale::parse("ja").is_ok());
        assert!(DialogLocale::parse("en").is_ok());
        assert_eq!(
            DialogLocale::parse("en-US").err(),
            Some(ERROR_INVALID_LOCALE.to_owned())
        );
    }

    #[test]
    fn owned_permit_holds_single_flight_until_detached_worker_finishes() {
        let state = FoldTechniqueFileIoState::default();
        let permit = state.try_acquire().expect("first permit");
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _permit = permit;
            release_receiver.recv().expect("release worker");
        });

        assert_eq!(state.try_acquire().err(), Some(ERROR_BUSY.to_owned()));
        release_sender.send(()).expect("release detached worker");
        worker.join().expect("join detached worker");
        drop(state.try_acquire().expect("permit reusable after worker"));
        assert!(state.try_acquire().is_ok());
    }

    fn fixture_document() -> FoldTechniqueFileDocumentV1 {
        serde_json::from_value(json!({
            "schema": "origami2_fold_technique_file",
            "version": 1,
            "package_id": "user.test.techniques",
            "metadata": {
                "authors": ["Test author"],
                "source": { "kind": "user_authored" },
                "license_spdx_id": "LicenseRef-Proprietary"
            },
            "techniques": [{
                "id": "user.test-fold",
                "version": 1,
                "names": [
                    { "locale": "en", "text": "Test fold" },
                    { "locale": "ja", "text": "テスト折り" }
                ],
                "descriptions": [
                    { "locale": "en", "text": "A declarative test." },
                    { "locale": "ja", "text": "宣言データのテストです。" }
                ],
                "parameters": [],
                "preconditions": [],
                "operations": [
                    {
                        "id": "step-1",
                        "names": [{ "locale": "en", "text": "Step 1" }],
                        "action": {
                            "kind": "instruction_cue",
                            "instructions": [{
                                "locale": "en",
                                "text": "Describe the first step."
                            }]
                        },
                        "parameter_bindings": [],
                        "precondition_ids": [],
                        "required_capabilities": ["human_interpretation_v1"],
                        "execution_support": { "status": "declarative_only" }
                    },
                    {
                        "id": "step-2",
                        "names": [{ "locale": "en", "text": "Step 2" }],
                        "action": {
                            "kind": "instruction_cue",
                            "instructions": [{
                                "locale": "en",
                                "text": "Describe the second step."
                            }]
                        },
                        "parameter_bindings": [],
                        "precondition_ids": [],
                        "required_capabilities": ["human_interpretation_v1"],
                        "execution_support": { "status": "declarative_only" }
                    }
                ]
            }]
        }))
        .expect("deserialize fixture")
    }
}
