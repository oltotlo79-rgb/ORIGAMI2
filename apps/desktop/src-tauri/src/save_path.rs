use std::{
    ffi::OsStr,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExistingDestinationPolicy {
    ReplaceConfirmed,
    RejectExisting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DialogSaveDestination {
    path: PathBuf,
    existing_destination_policy: ExistingDestinationPolicy,
}

impl DialogSaveDestination {
    pub(super) fn confirmed(path: PathBuf) -> Self {
        Self {
            path,
            existing_destination_policy: ExistingDestinationPolicy::ReplaceConfirmed,
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn existing_destination_policy(&self) -> ExistingDestinationPolicy {
        self.existing_destination_policy
    }

    pub(super) fn into_path(self) -> PathBuf {
        self.path
    }
}

impl AsRef<Path> for DialogSaveDestination {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

impl PartialEq<PathBuf> for DialogSaveDestination {
    fn eq(&self, other: &PathBuf) -> bool {
        self.path == *other
    }
}

pub(super) fn normalize_dialog_save_path(
    selected_path: PathBuf,
    expected_extension: &str,
) -> Result<DialogSaveDestination, String> {
    if has_extension_ignore_ascii_case(&selected_path, expected_extension) {
        return Ok(DialogSaveDestination::confirmed(selected_path));
    }

    let mut normalized_path = selected_path;
    normalized_path.set_extension(expected_extension);
    match fs::symlink_metadata(&normalized_path) {
        Ok(_) => Err(
            "拡張子を補正した保存先には既存のファイルがあります。上書き確認が行われていないため保存を中止しました。"
                .to_owned(),
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(DialogSaveDestination {
            path: normalized_path,
            existing_destination_policy: ExistingDestinationPolicy::RejectExisting,
        }),
        Err(_) => Err(
            "拡張子を補正した保存先に既存ファイルがないことを確認できないため、保存を中止しました。"
                .to_owned(),
        ),
    }
}

fn has_extension_ignore_ascii_case(path: &Path, expected_extension: &str) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected_extension))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "origami2-save-path-test-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn matching_extension_carries_confirmed_replacement_policy() {
        let destination = normalize_dialog_save_path(PathBuf::from("bird.ORI2"), "ori2").unwrap();

        assert_eq!(destination.path(), Path::new("bird.ORI2"));
        assert_eq!(
            destination.existing_destination_policy(),
            ExistingDestinationPolicy::ReplaceConfirmed
        );
    }

    #[test]
    fn corrected_extension_carries_atomic_create_new_policy() {
        let directory = TestDirectory::new();
        let selected = directory.0.join("bird.backup");
        let corrected = directory.0.join("bird.ori2");

        let destination = normalize_dialog_save_path(selected, "ori2").unwrap();

        assert_eq!(destination.path(), corrected);
        assert_eq!(
            destination.existing_destination_policy(),
            ExistingDestinationPolicy::RejectExisting
        );
    }

    #[test]
    fn corrected_directory_is_rejected_without_exposing_its_path() {
        let directory = TestDirectory::new();
        let selected = directory.0.join("private-name.txt");
        let corrected = directory.0.join("private-name.pdf");
        fs::create_dir(&corrected).unwrap();

        let error = normalize_dialog_save_path(selected, "pdf").unwrap_err();

        assert!(error.contains("上書き確認"));
        assert!(!error.contains("private-name"));
        assert!(corrected.is_dir());
    }

    #[test]
    fn corrected_hard_link_is_rejected_without_replacing_either_name() {
        let directory = TestDirectory::new();
        let selected = directory.0.join("private-link.txt");
        let original = directory.0.join("original.bin");
        let corrected = directory.0.join("private-link.ori2");
        fs::write(&original, b"owner data").unwrap();
        fs::hard_link(&original, &corrected).unwrap();

        let error = normalize_dialog_save_path(selected, "ori2").unwrap_err();

        assert!(error.contains("上書き確認"));
        assert!(!error.contains("private-link"));
        assert_eq!(fs::read(original).unwrap(), b"owner data");
        assert_eq!(fs::read(corrected).unwrap(), b"owner data");
    }

    #[cfg(unix)]
    #[test]
    fn corrected_dangling_symlink_is_rejected_without_following_it() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new();
        let selected = directory.0.join("private-link.txt");
        let corrected = directory.0.join("private-link.pdf");
        symlink(directory.0.join("missing-target"), &corrected).unwrap();

        let error = normalize_dialog_save_path(selected, "pdf").unwrap_err();

        assert!(error.contains("上書き確認"));
        assert!(!error.contains("private-link"));
        assert!(
            fs::symlink_metadata(corrected)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}
