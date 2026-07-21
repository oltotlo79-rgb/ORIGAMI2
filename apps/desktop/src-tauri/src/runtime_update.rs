use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Mutex;

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
    fn write_pending(&mut self, sha256: &str) -> Result<(), ()>;
    fn flush(&mut self) -> Result<(), ()>;
    fn handoff(&mut self, name: &str) -> Result<(), ()>;
    fn confirm(&mut self) -> Result<bool, ()>;
    fn mark_applied(&mut self, sha256: &str) -> Result<(), ()>;
    fn clear_pending(&mut self) -> Result<(), ()>;
    fn rollback(&mut self) -> Result<(), ()>;
    fn pending(&mut self) -> Result<bool, ()>;
}

#[derive(Default)]
pub struct OfflineAdapter;
impl Adapter for OfflineAdapter {
    fn get(&mut self, _: &str, _: u64, _: usize) -> Result<Response, ()> {
        Err(())
    }
    fn verify_signature(&mut self, _: &str, _: &[u8]) -> Result<bool, ()> {
        Err(())
    }
    fn stage_atomic(&mut self, _: &str, _: &[u8]) -> Result<(), ()> {
        Err(())
    }
    fn write_pending(&mut self, _: &str) -> Result<(), ()> {
        Err(())
    }
    fn flush(&mut self) -> Result<(), ()> {
        Err(())
    }
    fn handoff(&mut self, _: &str) -> Result<(), ()> {
        Err(())
    }
    fn confirm(&mut self) -> Result<bool, ()> {
        Err(())
    }
    fn mark_applied(&mut self, _: &str) -> Result<(), ()> {
        Err(())
    }
    fn clear_pending(&mut self) -> Result<(), ()> {
        Err(())
    }
    fn rollback(&mut self) -> Result<(), ()> {
        Err(())
    }
    fn pending(&mut self) -> Result<bool, ()> {
        Ok(false)
    }
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
    cancelled: bool,
}
impl<A: Adapter> Updater<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            feed: None,
            staged: false,
            cancelled: false,
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
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
    pub fn check(&mut self) -> Result<Candidate, &'static str> {
        self.cancelled = false;
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
        let feed: Feed = serde_json::from_slice(&response.body).map_err(|_| "malformed")?;
        validate_feed(&feed)?;
        if self.cancelled {
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
    pub fn download(
        &mut self,
        version: &str,
        platform: &str,
    ) -> Result<&'static str, &'static str> {
        self.cancelled = false;
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
        if self.cancelled {
            return Err("offline");
        }
        let staging = (|| {
            self.adapter
                .stage_atomic(&feed.asset_name, &response.body)
                .map_err(|_| "disk")?;
            self.adapter
                .write_pending(&feed.sha256)
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
        || !feed.payload_url.starts_with(ASSET_PREFIX)
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
fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub struct State(pub Mutex<Updater<OfflineAdapter>>);
impl Default for State {
    fn default() -> Self {
        Self(Mutex::new(Updater::new(OfflineAdapter)))
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
    }
    impl Adapter for Mock {
        fn get(&mut self, _: &str, timeout: u64, limit: usize) -> Result<Response, ()> {
            assert_eq!(timeout, TIMEOUT_SECS);
            assert!(matches!(limit, METADATA_LIMIT | PAYLOAD_LIMIT));
            self.responses.pop_front().unwrap_or(Err(()))
        }
        fn verify_signature(&mut self, _: &str, _: &[u8]) -> Result<bool, ()> {
            self.events.push("signature");
            Ok(self.signature)
        }
        fn stage_atomic(&mut self, _: &str, _: &[u8]) -> Result<(), ()> {
            self.events.push("stage");
            Ok(())
        }
        fn write_pending(&mut self, _: &str) -> Result<(), ()> {
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
        })
    }
    #[test]
    fn mock_complete_path_flushes_journal_before_handoff() {
        let mut updater = fixture(false, true, true);
        assert!(updater.check().is_ok());
        assert_eq!(updater.download("2.0.0", "windows-x64"), Ok("verified"));
        assert_eq!(updater.apply("2.0.0", "windows-x64"), Ok("applied"));
        assert_eq!(
            updater.adapter.events,
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
}
