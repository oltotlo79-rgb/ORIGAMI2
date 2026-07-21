use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const API_URL: &str = "https://api.github.com/repos/oltotlo79-rgb/ORIGAMI2/releases/latest";
const ASSET_PREFIX: &str = "https://objects.githubusercontent.com/";
const TIMEOUT_SECS: u64 = 10;
const METADATA_LIMIT: usize = 128 * 1024;
const PAYLOAD_LIMIT: usize = 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct Response {
    pub final_url: String,
    pub redirected: bool,
    pub body: Vec<u8>,
}

pub trait Adapter: Send {
    fn get(&mut self, url: &str, timeout_secs: u64, limit: usize) -> Result<Response, ()>;
    fn verify_signature(&mut self, name: &str, bytes: &[u8]) -> Result<bool, ()>;
    fn stage_atomic(&mut self, name: &str, bytes: &[u8]) -> Result<(), ()>;
    fn write_pending(&mut self, sha256: &str, asset_name: &str) -> Result<(), ()>;
    fn flush(&mut self) -> Result<(), ()>;
    fn handoff(&mut self, name: &str) -> Result<(), ()>;
    fn confirm(&mut self) -> Result<bool, ()>;
    fn mark_applied(&mut self, sha256: &str) -> Result<(), ()>;
    fn clear_pending(&mut self) -> Result<(), ()>;
    fn rollback(&mut self) -> Result<(), ()>;
    fn pending(&mut self) -> Result<bool, ()>;
    fn was_applied(&mut self, sha256: &str) -> Result<bool, ()>;
}

pub struct ProductionAdapter {
    root: PathBuf,
    staged: Option<PathBuf>,
    installer_succeeded: bool,
}

impl Default for ProductionAdapter {
    fn default() -> Self {
        Self {
            root: production_staging_root(),
            staged: None,
            installer_succeeded: false,
        }
    }
}

impl Adapter for ProductionAdapter {
    fn get(&mut self, url: &str, timeout_secs: u64, limit: usize) -> Result<Response, ()> {
        if timeout_secs != TIMEOUT_SECS
            || !matches!(limit, METADATA_LIMIT | PAYLOAD_LIMIT)
            || !allowed_url(url, limit)
        {
            return Err(());
        }
        ensure_secure_root(&self.root)?;
        let output_path = reserve_http_output(&self.root)?;
        let limit_text = limit.to_string();
        let output = match Command::new(curl_executable())
            .args([
                "--silent",
                "--show-error",
                "--fail",
                "--proto",
                "=https",
                "--tlsv1.2",
                "--max-redirs",
                "0",
                "--connect-timeout",
                "10",
                "--max-time",
                "10",
                "--max-filesize",
                &limit_text,
                "--user-agent",
                "ORIGAMI2-runtime-updater",
                "--header",
                "Accept: application/octet-stream",
                "--output",
            ])
            .arg(&output_path)
            .args(["--write-out", "%{url_effective}", url])
            .output()
        {
            Ok(output) => output,
            Err(_) => {
                let _ = fs::remove_file(&output_path);
                return Err(());
            }
        };
        if !output.status.success() {
            let _ = fs::remove_file(&output_path);
            return Err(());
        }
        let final_url = String::from_utf8(output.stdout).map_err(|_| ())?;
        if final_url != url {
            let _ = fs::remove_file(&output_path);
            return Err(());
        }
        let file = File::open(&output_path).map_err(|_| ())?;
        if file.metadata().map_err(|_| ())?.len() > limit as u64 {
            let _ = fs::remove_file(&output_path);
            return Err(());
        }
        let mut body = Vec::new();
        file.take(limit as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| ())?;
        let _ = fs::remove_file(&output_path);
        if body.len() > limit {
            return Err(());
        }
        Ok(Response {
            final_url,
            redirected: false,
            body,
        })
    }

    fn verify_signature(&mut self, name: &str, bytes: &[u8]) -> Result<bool, ()> {
        ensure_safe_name(name)?;
        ensure_secure_root(&self.root)?;
        let path = self.root.join(format!("verify-{name}"));
        write_new_synced(&path, bytes)?;
        let valid = verify_platform_signature(&path);
        let _ = fs::remove_file(path);
        valid
    }

    fn stage_atomic(&mut self, name: &str, bytes: &[u8]) -> Result<(), ()> {
        ensure_safe_name(name)?;
        ensure_secure_root(&self.root)?;
        let temporary = self.root.join(format!(".{name}.partial"));
        let destination = self.root.join(name);
        write_new_synced(&temporary, bytes)?;
        if destination.exists() {
            let _ = fs::remove_file(&temporary);
            return Err(());
        }
        if temporary.parent() != destination.parent() {
            let _ = fs::remove_file(&temporary);
            return Err(());
        }
        fs::rename(&temporary, &destination).map_err(|_| ())?;
        self.staged = Some(destination);
        Ok(())
    }

    fn write_pending(&mut self, sha256: &str, asset_name: &str) -> Result<(), ()> {
        ensure_safe_name(asset_name)?;
        if sha256.len() != 64 {
            return Err(());
        }
        let bytes = serde_json::to_vec(&PendingRecord {
            sha256: sha256.to_owned(),
            asset_name: asset_name.to_owned(),
        })
        .map_err(|_| ())?;
        write_new_synced(&self.root.join("pending.json"), &bytes)
    }
    fn flush(&mut self) -> Result<(), ()> {
        File::open(&self.root)
            .and_then(|file| file.sync_all())
            .map_err(|_| ())
    }
    fn handoff(&mut self, name: &str) -> Result<(), ()> {
        let path = self
            .staged
            .as_ref()
            .filter(|path| path.file_name().and_then(|v| v.to_str()) == Some(name))
            .ok_or(())?;
        self.installer_succeeded = launch_platform_installer(path)?;
        Ok(())
    }
    fn confirm(&mut self) -> Result<bool, ()> {
        Ok(self.installer_succeeded)
    }
    fn mark_applied(&mut self, sha256: &str) -> Result<(), ()> {
        let applied = self.root.join(format!("applied-{sha256}"));
        write_new_synced(&applied, b"applied")
    }
    fn clear_pending(&mut self) -> Result<(), ()> {
        match fs::remove_file(self.root.join("pending.json")) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(()),
        }
    }
    fn rollback(&mut self) -> Result<(), ()> {
        if let Some(path) = self.staged.take() {
            match fs::remove_file(path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return Err(()),
            }
        }
        self.installer_succeeded = false;
        Ok(())
    }
    fn pending(&mut self) -> Result<bool, ()> {
        let path = self.root.join("pending.json");
        if !path.is_file() {
            return Ok(false);
        }
        let bytes = fs::read(path).map_err(|_| ())?;
        if bytes.len() > 512 {
            return Err(());
        }
        let record: PendingRecord = serde_json::from_slice(&bytes).map_err(|_| ())?;
        ensure_safe_name(&record.asset_name)?;
        if record.sha256.len() != 64 {
            return Err(());
        }
        self.staged = Some(self.root.join(record.asset_name));
        Ok(true)
    }
    fn was_applied(&mut self, sha256: &str) -> Result<bool, ()> {
        Ok(self.root.join(format!("applied-{sha256}")).is_file())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingRecord {
    sha256: String,
    asset_name: String,
}

fn allowed_url(url: &str, limit: usize) -> bool {
    (limit == METADATA_LIMIT && (url == API_URL || allowed_api_asset_url(url)))
        || (limit == PAYLOAD_LIMIT
            && (allowed_api_asset_url(url)
                || (url.starts_with(ASSET_PREFIX)
                    && !url[ASSET_PREFIX.len()..].is_empty()
                    && !url.contains(['?', '#', '\\']))))
}
fn allowed_api_asset_url(url: &str) -> bool {
    url.strip_prefix("https://api.github.com/repos/oltotlo79-rgb/ORIGAMI2/releases/assets/")
        .is_some_and(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
}
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn current_platform() -> Option<&'static str> {
    Some("windows-x64")
}
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn current_platform() -> Option<&'static str> {
    Some("macos-arm64")
}
#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
)))]
fn current_platform() -> Option<&'static str> {
    None
}
#[cfg(target_os = "windows")]
fn production_staging_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("ORIGAMI2")
        .join("runtime-update-v1")
}
#[cfg(target_os = "macos")]
fn production_staging_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Library/Caches/ORIGAMI2/runtime-update-v1")
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn production_staging_root() -> PathBuf {
    PathBuf::new()
}
#[cfg(target_os = "windows")]
fn curl_executable() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("System32/curl.exe")
}
#[cfg(not(target_os = "windows"))]
fn curl_executable() -> PathBuf {
    PathBuf::from("/usr/bin/curl")
}
fn ensure_safe_name(name: &str) -> Result<(), ()> {
    if name.is_empty()
        || name.len() > 180
        || name.contains(['/', '\\'])
        || name == "."
        || name == ".."
    {
        Err(())
    } else {
        Ok(())
    }
}
fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), ()> {
    let mut file = open_new_nofollow(path).map_err(|_| ())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| ())
}
fn ensure_secure_root(root: &Path) -> Result<(), ()> {
    if root.as_os_str().is_empty() {
        return Err(());
    }
    fs::create_dir_all(root).map_err(|_| ())?;
    for path in [root, root.parent().ok_or(())?] {
        let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(());
        }
    }
    Ok(())
}
#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink()
        || metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
}
#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
fn open_new_nofollow(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}
fn reserve_http_output(root: &Path) -> Result<PathBuf, ()> {
    for suffix in 0_u8..16 {
        let path = root.join(format!(".http-response-{}-{suffix}", std::process::id()));
        match open_new_nofollow(&path) {
            Ok(file) => {
                file.sync_all().map_err(|_| ())?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(()),
        }
    }
    Err(())
}
#[cfg(target_os = "windows")]
fn verify_platform_signature(path: &Path) -> Result<bool, ()> {
    let escaped = powershell_single_quoted_path(path);
    Command::new(powershell_executable()).args(["-NoProfile", "-NonInteractive", "-Command", &format!("$s=Get-AuthenticodeSignature -LiteralPath {escaped}; $s.Status -eq 'Valid' -and $null -ne $s.SignerCertificate -and $null -ne $s.TimeStamperCertificate")]).output().map(|output| output.status.success() && String::from_utf8_lossy(&output.stdout).trim().eq_ignore_ascii_case("true")).map_err(|_| ())
}
#[cfg(any(target_os = "windows", test))]
fn powershell_single_quoted_path(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}
#[cfg(target_os = "windows")]
fn powershell_executable() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("System32/WindowsPowerShell/v1.0/powershell.exe")
}
#[cfg(target_os = "macos")]
fn verify_platform_signature(path: &Path) -> Result<bool, ()> {
    {
        use std::io::{Seek, SeekFrom};
        let mut archive = File::open(path).map_err(|_| ())?;
        if archive.metadata().map_err(|_| ())?.len() < 4 {
            return Ok(false);
        }
        archive.seek(SeekFrom::End(-4)).map_err(|_| ())?;
        let mut isize = [0_u8; 4];
        archive.read_exact(&mut isize).map_err(|_| ())?;
        if u32::from_le_bytes(isize) > 2 * 1024 * 1024 * 1024 {
            return Ok(false);
        }
    }
    let listing = Command::new("/usr/bin/tar")
        .args(["-tzf"])
        .arg(path)
        .output()
        .map_err(|_| ())?;
    if !listing.status.success() || listing.stdout.len() > 1024 * 1024 {
        return Ok(false);
    }
    let entries = String::from_utf8(listing.stdout).map_err(|_| ())?;
    if !archive_listing_is_safe(&entries) {
        return Ok(false);
    }
    let directory = path
        .parent()
        .ok_or(())?
        .join(format!("verify-app-{}", std::process::id()));
    fs::create_dir(&directory).map_err(|_| ())?;
    let extracted = Command::new("/usr/bin/tar")
        .args(["--no-same-owner", "-xzf"])
        .arg(path)
        .arg("-C")
        .arg(&directory)
        .status()
        .map_err(|_| ())?;
    if !extracted.success() {
        let _ = fs::remove_dir_all(&directory);
        return Ok(false);
    }
    let app = directory.join("ORIGAMI2.app");
    let valid = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict", "--verbose=2"])
        .arg(app)
        .status()
        .map(|status| status.success())
        .map_err(|_| ());
    let _ = fs::remove_dir_all(directory);
    valid
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn verify_platform_signature(_: &Path) -> Result<bool, ()> {
    Ok(false)
}
#[cfg(target_os = "windows")]
fn launch_platform_installer(path: &Path) -> Result<bool, ()> {
    Command::new(path)
        .arg("/S")
        .status()
        .map(|status| status.success())
        .map_err(|_| ())
}
#[cfg(target_os = "macos")]
fn launch_platform_installer(path: &Path) -> Result<bool, ()> {
    Command::new("/usr/bin/open")
        .arg(path)
        .status()
        .map(|status| status.success())
        .map_err(|_| ())
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn launch_platform_installer(_: &Path) -> Result<bool, ()> {
    Ok(false)
}
#[cfg(any(target_os = "macos", test))]
fn archive_listing_is_safe(entries: &str) -> bool {
    let mut count = 0_usize;
    for entry in entries.lines() {
        count += 1;
        if count > 4096
            || entry.is_empty()
            || entry.starts_with('/')
            || entry.contains("..")
            || entry.contains('\\')
            || !(entry == "ORIGAMI2.app" || entry.starts_with("ORIGAMI2.app/"))
        {
            return false;
        }
    }
    count > 0
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Feed {
    version: String,
    platform: String,
    release_notes: String,
    byte_length: usize,
    signature_policy: String,
    asset_name: String,
    sha256: String,
    payload_url: String,
}
#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    body: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}
#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    url: String,
    size: usize,
}
#[derive(Deserialize)]
struct UpdateManifest {
    schema: String,
    version: String,
    platform: String,
    #[serde(rename = "signaturePolicy")]
    signature_policy: String,
    assets: Vec<ManifestAsset>,
}
#[derive(Deserialize)]
struct ManifestAsset {
    name: String,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    version: String,
    platform: String,
    release_notes: String,
    byte_length: usize,
}

pub struct Updater<A: Adapter> {
    adapter: A,
    feed: Option<Feed>,
    staged: bool,
    cancelled: Arc<AtomicBool>,
}
impl<A: Adapter> Updater<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            feed: None,
            staged: false,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn recover(&mut self) -> Result<&'static str, &'static str> {
        if self.adapter.pending().map_err(|_| "disk")? {
            self.adapter.rollback().map_err(|_| "disk")?;
            self.adapter.clear_pending().map_err(|_| "disk")?;
            self.adapter.flush().map_err(|_| "disk")?;
        }
        Ok("ready")
    }
    pub fn check(&mut self) -> Result<Candidate, &'static str> {
        self.cancelled.store(false, Ordering::Release);
        let response = self
            .adapter
            .get(API_URL, TIMEOUT_SECS, METADATA_LIMIT)
            .map_err(|_| "offline")?;
        if response.redirected
            || response.final_url != API_URL
            || response.body.len() > METADATA_LIMIT
        {
            return Err("malformed");
        }
        let feed: Feed = match serde_json::from_slice(&response.body) {
            Ok(feed) => feed,
            Err(_) => self.feed_from_github(&response.body)?,
        };
        validate_feed(&feed)?;
        if compare_stable_versions(&feed.version, env!("CARGO_PKG_VERSION"))? <= 0 {
            return Err("rollback");
        }
        if self.cancelled.load(Ordering::Acquire) {
            return Err("offline");
        }
        let candidate = Candidate {
            version: feed.version.clone(),
            platform: feed.platform.clone(),
            release_notes: feed.release_notes.clone(),
            byte_length: feed.byte_length,
        };
        self.feed = Some(feed);
        Ok(candidate)
    }
    fn feed_from_github(&mut self, body: &[u8]) -> Result<Feed, &'static str> {
        let release: GithubRelease = serde_json::from_slice(body).map_err(|_| "malformed")?;
        if release.draft
            || release.prerelease
            || release.body.len() > 100_000
            || release.assets.len() > 32
        {
            return Err("malformed");
        }
        let version = release
            .tag_name
            .strip_prefix('v')
            .ok_or("malformed")?
            .to_owned();
        if !stable_version(&version) {
            return Err("malformed");
        }
        let platform = current_platform().ok_or("malformed")?;
        let prefix = format!("ORIGAMI2-v{version}-{platform}");
        let payload_name = if platform == "windows-x64" {
            format!("{prefix}-setup.exe")
        } else {
            format!("{prefix}-app.tar.gz")
        };
        let manifest_name = format!("{prefix}.update.json");
        let payload = release
            .assets
            .iter()
            .find(|asset| asset.name == payload_name)
            .ok_or("malformed")?;
        let manifest_asset = release
            .assets
            .iter()
            .find(|asset| asset.name == manifest_name)
            .ok_or("malformed")?;
        if !allowed_api_asset_url(&payload.url) || !allowed_api_asset_url(&manifest_asset.url) {
            return Err("malformed");
        }
        let response = self
            .adapter
            .get(&manifest_asset.url, TIMEOUT_SECS, METADATA_LIMIT)
            .map_err(|_| "offline")?;
        if response.redirected
            || response.final_url != manifest_asset.url
            || response.body.len() > METADATA_LIMIT
        {
            return Err("malformed");
        }
        let manifest: UpdateManifest =
            serde_json::from_slice(&response.body).map_err(|_| "malformed")?;
        if manifest.schema != "origami2.update-manifest.v1"
            || manifest.version != version
            || manifest.platform != platform
            || manifest.signature_policy != "platform-signed"
        {
            return Err("malformed");
        }
        let sha256 = manifest
            .assets
            .iter()
            .find(|asset| asset.name == payload_name)
            .ok_or("malformed")?
            .sha256
            .clone();
        Ok(Feed {
            version,
            platform: platform.to_owned(),
            release_notes: release.body,
            byte_length: payload.size,
            signature_policy: manifest.signature_policy,
            asset_name: payload_name,
            sha256,
            payload_url: payload.url.clone(),
        })
    }
    pub fn download(
        &mut self,
        version: &str,
        platform: &str,
    ) -> Result<&'static str, &'static str> {
        self.cancelled.store(false, Ordering::Release);
        let feed = self.feed.clone().ok_or("malformed")?;
        if feed.version != version || feed.platform != platform {
            return Err("rollback");
        }
        let response = self
            .adapter
            .get(&feed.payload_url, TIMEOUT_SECS, PAYLOAD_LIMIT)
            .map_err(|_| "offline")?;
        if response.redirected
            || response.final_url != feed.payload_url
            || response.body.len() != feed.byte_length
            || response.body.len() > PAYLOAD_LIMIT
        {
            return Err("malformed");
        }
        if hex_sha256(&response.body) != feed.sha256 {
            return Err("signature");
        }
        if !self
            .adapter
            .verify_signature(&feed.asset_name, &response.body)
            .map_err(|_| "signature")?
        {
            return Err("signature");
        }
        if self.cancelled.load(Ordering::Acquire) {
            return Err("offline");
        }
        let staging = (|| {
            self.adapter
                .stage_atomic(&feed.asset_name, &response.body)
                .map_err(|_| "disk")?;
            self.adapter
                .write_pending(&feed.sha256, &feed.asset_name)
                .map_err(|_| "disk")?;
            self.adapter.flush().map_err(|_| "disk")
        })();
        if staging.is_err() {
            let _ = self.adapter.rollback();
            let _ = self.adapter.clear_pending();
            let _ = self.adapter.flush();
            return Err("disk");
        }
        self.staged = true;
        Ok("verified")
    }
    pub fn apply(&mut self, version: &str, platform: &str) -> Result<&'static str, &'static str> {
        let feed = self.feed.clone().ok_or("malformed")?;
        if !self.staged || feed.version != version || feed.platform != platform {
            return Err("rollback");
        }
        if self.adapter.was_applied(&feed.sha256).map_err(|_| "disk")? {
            return Err("rollback");
        }
        let result = (|| {
            self.adapter.handoff(&feed.asset_name).map_err(|_| "disk")?;
            if !self.adapter.confirm().map_err(|_| "disk")? {
                return Err("rollback");
            }
            self.adapter
                .mark_applied(&feed.sha256)
                .map_err(|_| "disk")?;
            self.adapter.clear_pending().map_err(|_| "disk")?;
            self.adapter.flush().map_err(|_| "disk")?;
            Ok("applied")
        })();
        if result.is_err() {
            let _ = self.adapter.rollback();
            let _ = self.adapter.clear_pending();
            let _ = self.adapter.flush();
        }
        result
    }
}

fn validate_feed(feed: &Feed) -> Result<(), &'static str> {
    if feed.signature_policy != "platform-signed"
        || !stable_version(&feed.version)
        || !matches!(feed.platform.as_str(), "windows-x64" | "macos-arm64")
        || feed.release_notes.len() > 100_000
        || feed.byte_length == 0
        || feed.byte_length > PAYLOAD_LIMIT
        || feed.sha256.len() != 64
        || !feed
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || feed.asset_name.contains(['/', '\\'])
        || !allowed_url(&feed.payload_url, PAYLOAD_LIMIT)
    {
        return Err("malformed");
    }
    Ok(())
}
fn stable_version(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && (p == &"0" || !p.starts_with('0'))
        })
}
fn compare_stable_versions(left: &str, right: &str) -> Result<i8, &'static str> {
    if !stable_version(left) || !stable_version(right) {
        return Err("malformed");
    }
    for (a, b) in left.split('.').zip(right.split('.')) {
        let ordering = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
        if !ordering.is_eq() {
            return Ok(if ordering.is_gt() { 1 } else { -1 });
        }
    }
    Ok(0)
}
fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub struct State(
    pub Mutex<Updater<ProductionAdapter>>,
    pub Arc<AtomicBool>,
    pub Mutex<Option<String>>,
);
impl Default for State {
    fn default() -> Self {
        let updater = Updater::new(ProductionAdapter::default());
        let cancel = Arc::clone(&updater.cancelled);
        Self(Mutex::new(updater), cancel, Mutex::new(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct Mock {
        responses: VecDeque<Result<Response, ()>>,
        events: Vec<&'static str>,
        signature: bool,
        confirm: bool,
        pending: bool,
        applied: bool,
        cancel_on_payload: Option<Arc<AtomicBool>>,
        stage_fail: bool,
    }
    impl Adapter for Mock {
        fn get(&mut self, _: &str, timeout: u64, limit: usize) -> Result<Response, ()> {
            assert_eq!(timeout, TIMEOUT_SECS);
            assert!(matches!(limit, METADATA_LIMIT | PAYLOAD_LIMIT));
            let response = self.responses.pop_front().unwrap_or(Err(()));
            if limit == PAYLOAD_LIMIT {
                if let Some(cancel) = &self.cancel_on_payload {
                    cancel.store(true, Ordering::Release);
                }
            }
            response
        }
        fn verify_signature(&mut self, _: &str, _: &[u8]) -> Result<bool, ()> {
            self.events.push("signature");
            Ok(self.signature)
        }
        fn stage_atomic(&mut self, _: &str, _: &[u8]) -> Result<(), ()> {
            self.events.push("stage");
            if self.stage_fail { Err(()) } else { Ok(()) }
        }
        fn write_pending(&mut self, _: &str, _: &str) -> Result<(), ()> {
            self.events.push("journal");
            self.pending = true;
            Ok(())
        }
        fn flush(&mut self) -> Result<(), ()> {
            self.events.push("flush");
            Ok(())
        }
        fn handoff(&mut self, _: &str) -> Result<(), ()> {
            self.events.push("handoff");
            Ok(())
        }
        fn confirm(&mut self) -> Result<bool, ()> {
            self.events.push("confirm");
            Ok(self.confirm)
        }
        fn mark_applied(&mut self, _: &str) -> Result<(), ()> {
            self.events.push("applied");
            self.applied = true;
            Ok(())
        }
        fn clear_pending(&mut self) -> Result<(), ()> {
            self.events.push("clear");
            self.pending = false;
            Ok(())
        }
        fn rollback(&mut self) -> Result<(), ()> {
            self.events.push("rollback");
            Ok(())
        }
        fn pending(&mut self) -> Result<bool, ()> {
            Ok(self.pending)
        }
        fn was_applied(&mut self, _: &str) -> Result<bool, ()> {
            Ok(self.applied)
        }
    }
    fn fixture(redirected: bool, signature: bool, confirm: bool) -> Updater<Mock> {
        let payload = b"payload".to_vec();
        let feed = serde_json::json!({
            "version":"2.0.0", "platform":"windows-x64", "release_notes":"notes",
            "byte_length":payload.len(), "signature_policy":"platform-signed",
            "asset_name":"ORIGAMI2-v2.0.0-windows-x64-setup.exe",
            "sha256":hex_sha256(&payload), "payload_url":"https://objects.githubusercontent.com/release/payload"
        });
        Updater::new(Mock {
            responses: VecDeque::from([
                Ok(Response {
                    final_url: API_URL.into(),
                    redirected,
                    body: serde_json::to_vec(&feed).unwrap(),
                }),
                Ok(Response {
                    final_url: "https://objects.githubusercontent.com/release/payload".into(),
                    redirected: false,
                    body: payload,
                }),
            ]),
            events: vec![],
            signature,
            confirm,
            pending: false,
            applied: false,
            cancel_on_payload: None,
            stage_fail: false,
        })
    }
    #[test]
    fn mock_complete_path_flushes_journal_before_handoff() {
        let mut updater = fixture(false, true, true);
        assert!(updater.check().is_ok());
        assert_eq!(updater.download("2.0.0", "windows-x64"), Ok("verified"));
        assert_eq!(updater.apply("2.0.0", "windows-x64"), Ok("applied"));
        assert_eq!(updater.apply("2.0.0", "windows-x64"), Err("rollback"));
        assert_eq!(
            &updater.adapter.events[..9],
            [
                "signature",
                "stage",
                "journal",
                "flush",
                "handoff",
                "confirm",
                "applied",
                "clear",
                "flush"
            ]
        );
        assert_eq!(updater.adapter.events.len(), 9);
    }
    #[test]
    fn redirects_signature_failure_and_failed_confirmation_are_fail_closed() {
        assert_eq!(fixture(true, true, true).check().unwrap_err(), "malformed");
        let mut unsigned = fixture(false, false, true);
        unsigned.check().unwrap();
        assert_eq!(unsigned.download("2.0.0", "windows-x64"), Err("signature"));
        let mut failed = fixture(false, true, false);
        failed.check().unwrap();
        failed.download("2.0.0", "windows-x64").unwrap();
        assert_eq!(failed.apply("2.0.0", "windows-x64"), Err("rollback"));
        assert!(
            failed
                .adapter
                .events
                .ends_with(&["rollback", "clear", "flush"])
        );
    }

    #[test]
    fn cancellation_racing_with_payload_response_never_reaches_staging() {
        let mut updater = fixture(false, true, true);
        updater.check().unwrap();
        updater.adapter.cancel_on_payload = Some(Arc::clone(&updater.cancelled));
        assert_eq!(updater.download("2.0.0", "windows-x64"), Err("offline"));
        assert!(!updater.adapter.events.contains(&"stage"));
    }

    #[test]
    fn staging_disk_failure_rolls_back_without_handoff() {
        let mut updater = fixture(false, true, true);
        updater.check().unwrap();
        updater.adapter.stage_fail = true;
        assert_eq!(updater.download("2.0.0", "windows-x64"), Err("disk"));
        assert!(
            updater
                .adapter
                .events
                .ends_with(&["stage", "rollback", "clear", "flush"])
        );
        assert!(!updater.adapter.events.contains(&"handoff"));
    }

    #[test]
    fn production_network_policy_accepts_only_exact_https_authorities() {
        assert!(allowed_url(API_URL, METADATA_LIMIT));
        assert!(allowed_url(
            "https://objects.githubusercontent.com/release/payload",
            PAYLOAD_LIMIT
        ));
        for url in [
            "http://api.github.com/repos/oltotlo79-rgb/ORIGAMI2/releases/latest",
            "https://api.github.com.evil.example/release",
            "https://objects.githubusercontent.com.evil.example/payload",
            "https://objects.githubusercontent.com/release/payload?token=secret",
        ] {
            assert!(!allowed_url(url, PAYLOAD_LIMIT));
        }
        assert!(ensure_safe_name("ORIGAMI2-v2.0.0-windows-x64-setup.exe").is_ok());
        assert!(ensure_safe_name("../setup.exe").is_err());
        assert_eq!(compare_stable_versions("2.0.0", "1.99.99"), Ok(1));
        assert_eq!(compare_stable_versions("1.0.0", "1.0.0"), Ok(0));
        assert_eq!(compare_stable_versions("0.9.9", "1.0.0"), Ok(-1));
        assert_eq!(
            powershell_single_quoted_path(Path::new("C:\\Updates\\O'Brien.exe")),
            "'C:\\Updates\\O''Brien.exe'"
        );
        assert!(archive_listing_is_safe(
            "ORIGAMI2.app/\nORIGAMI2.app/Contents/MacOS/ORIGAMI2\n"
        ));
        assert!(!archive_listing_is_safe("ORIGAMI2.app/../../escape\n"));
        let bomb = "ORIGAMI2.app/x\n".repeat(4097);
        assert!(!archive_listing_is_safe(&bomb));
        let source = include_str!("runtime_update.rs");
        for contract in [
            "--proto",
            "=https",
            "--max-redirs",
            "--max-filesize",
            "temporary.parent() != destination.parent()",
        ] {
            assert!(source.contains(contract));
        }
    }

    #[cfg(unix)]
    #[test]
    fn staging_root_rejects_symbolic_links() {
        use std::os::unix::fs::symlink;
        let base =
            std::env::temp_dir().join(format!("origami2-updater-link-test-{}", std::process::id()));
        let target = base.join("target");
        let link = base.join("link");
        fs::create_dir_all(&target).unwrap();
        symlink(&target, &link).unwrap();
        assert_eq!(ensure_secure_root(&link), Err(()));
        fs::remove_file(link).unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn github_release_metadata_is_bound_to_the_signed_update_manifest() {
        let Some(platform) = current_platform() else {
            return;
        };
        let payload = b"payload".to_vec();
        let prefix = format!("ORIGAMI2-v2.0.0-{platform}");
        let payload_name = if platform == "windows-x64" {
            format!("{prefix}-setup.exe")
        } else {
            format!("{prefix}-app.tar.gz")
        };
        let manifest_name = format!("{prefix}.update.json");
        let payload_url = "https://api.github.com/repos/oltotlo79-rgb/ORIGAMI2/releases/assets/41";
        let manifest_url = "https://api.github.com/repos/oltotlo79-rgb/ORIGAMI2/releases/assets/42";
        let release = serde_json::json!({ "tag_name":"v2.0.0", "body":"notes", "draft":false, "prerelease":false, "assets":[
            {"name":payload_name,"url":payload_url,"size":payload.len()}, {"name":manifest_name,"url":manifest_url,"size":512}
        ]});
        let manifest = serde_json::json!({ "schema":"origami2.update-manifest.v1", "version":"2.0.0", "platform":platform, "signaturePolicy":"platform-signed", "assets":[{"name":payload_name,"sha256":hex_sha256(&payload)}] });
        let adapter = Mock {
            responses: VecDeque::from([
                Ok(Response {
                    final_url: API_URL.into(),
                    redirected: false,
                    body: serde_json::to_vec(&release).unwrap(),
                }),
                Ok(Response {
                    final_url: manifest_url.into(),
                    redirected: false,
                    body: serde_json::to_vec(&manifest).unwrap(),
                }),
                Ok(Response {
                    final_url: payload_url.into(),
                    redirected: false,
                    body: payload,
                }),
            ]),
            events: vec![],
            signature: true,
            confirm: true,
            pending: false,
            applied: false,
            cancel_on_payload: None,
            stage_fail: false,
        };
        let mut updater = Updater::new(adapter);
        assert!(updater.check().is_ok());
        assert_eq!(updater.download("2.0.0", platform), Ok("verified"));
    }
}
