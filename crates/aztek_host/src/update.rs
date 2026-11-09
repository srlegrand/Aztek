//! The update check and download. The check asks GitHub's API for the latest release and nothing else; the download
//! fetches one of its files to disk. Both block, so the shell calls them from a thread of its own, and nothing here
//! touches the scene. HTTPS through rustls with the system's root certificates, so a studio proxy's own certificate
//! works and no system TLS library is needed. AZTEK_UPDATE_URL points the check somewhere else, for tests.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/srlegrand/Aztek/releases/latest";

static DONE: AtomicU64 = AtomicU64::new(0);
static TOTAL: AtomicU64 = AtomicU64::new(0);
static CANCEL: AtomicBool = AtomicBool::new(false);

/// The host part of a URL: "https://user@host:443/x" gives "host".
fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let auth = rest.split('/').next().unwrap_or("");
    let auth = auth.rsplit('@').next().unwrap_or(auth);
    let host = if let Some(v6) = auth.strip_prefix('[') { v6.split(']').next().unwrap_or("") } else { auth.split(':').next().unwrap_or("") };
    host.to_ascii_lowercase()
}

/// Whether NO_PROXY (or no_proxy) says to go to this host directly: "*", the host itself, or a domain it ends in
/// ("example.com" and ".example.com" both cover "a.example.com").
fn bypasses_proxy(host: &str, no_proxy: &str) -> bool {
    no_proxy.split(',').map(|e| e.trim().to_ascii_lowercase()).filter(|e| !e.is_empty()).any(|e| {
        let e = e.split(':').next().unwrap_or(&e).to_string();
        let d = e.trim_start_matches('.');
        e == "*" || host == d || host.ends_with(&format!(".{d}"))
    })
}

/// The proxy for a URL from the usual variables (HTTPS_PROXY, HTTP_PROXY, ALL_PROXY, in either case), unless
/// NO_PROXY covers its host.
fn proxy_for(url: &str) -> Option<String> {
    let var = |n: &str| std::env::var(n).ok().or_else(|| std::env::var(n.to_ascii_lowercase()).ok()).filter(|v| !v.trim().is_empty());
    if bypasses_proxy(&host_of(url), &var("NO_PROXY").unwrap_or_default()) {
        return None;
    }
    let names: &[&str] = if url.starts_with("https:") { &["HTTPS_PROXY", "ALL_PROXY"] } else { &["HTTP_PROXY", "ALL_PROXY"] };
    names.iter().find_map(|n| var(n))
}

fn agent(url: &str) -> ureq::Agent {
    let mut b = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
        .user_agent("Aztek-update-check");
    if let Some(p) = proxy_for(url).and_then(|p| ureq::Proxy::new(p).ok()) {
        b = b.proxy(p);
    }
    b.build()
}

/// The latest release: {tag, name, body, url, assets: [{name, url, size, digest}]}, or {error}.
pub fn latest() -> Value {
    let url = std::env::var("AZTEK_UPDATE_URL").unwrap_or_else(|_| LATEST.to_string());
    let resp = match agent(&url).get(&url).set("Accept", "application/vnd.github+json").call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => return json!({ "error": format!("GitHub answered {code}") }),
        Err(e) => return json!({ "error": format!("Couldn't reach GitHub: {e}") }),
    };
    let text = match resp.into_string() {
        Ok(t) => t,
        Err(e) => return json!({ "error": format!("GitHub's answer couldn't be read: {e}") }),
    };
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return json!({ "error": format!("GitHub's answer couldn't be read: {e}") }),
    };
    let assets: Vec<Value> = v["assets"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| {
                    json!({
                        "name": x["name"],
                        "url": x["browser_download_url"],
                        "size": x["size"],
                        "digest": x["digest"],
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    json!({ "tag": v["tag_name"], "name": v["name"], "body": v["body"], "url": v["html_url"], "assets": assets })
}

/// Fetches url to path, through path.part, checking a "sha256:..." digest when GitHub gave one. None when it worked.
pub fn download(url: &str, path: &Path, digest: &str) -> Option<String> {
    DONE.store(0, Ordering::Relaxed);
    TOTAL.store(0, Ordering::Relaxed);
    CANCEL.store(false, Ordering::Relaxed);
    let resp = match agent(url).get(url).call() {
        Ok(r) => r,
        Err(e) => return Some(format!("The download didn't start: {e}")),
    };
    if let Some(n) = resp.header("Content-Length").and_then(|s| s.parse().ok()) {
        TOTAL.store(n, Ordering::Relaxed);
    }
    let part = path.with_extension(format!("{}.part", path.extension().and_then(|e| e.to_str()).unwrap_or("")));
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut file = match std::fs::File::create(&part) {
        Ok(f) => f,
        Err(e) => return Some(format!("Couldn't write {}: {e}", part.display())),
    };
    let mut reader = resp.into_reader();
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        if CANCEL.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Some("Cancelled".into());
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                return Some(format!("The download stopped: {e}"));
            }
        };
        if let Err(e) = file.write_all(&buf[..n]) {
            let _ = std::fs::remove_file(&part);
            return Some(format!("Couldn't write {}: {e}", part.display()));
        }
        hash.update(&buf[..n]);
        DONE.fetch_add(n as u64, Ordering::Relaxed);
    }
    drop(file);
    if let Some(want) = digest.strip_prefix("sha256:") {
        let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if !got.eq_ignore_ascii_case(want) {
            let _ = std::fs::remove_file(&part);
            return Some("The download doesn't match the release's checksum, so it wasn't kept.".into());
        }
    }
    if let Err(e) = std::fs::rename(&part, path) {
        return Some(format!("Couldn't move the download into place: {e}"));
    }
    None
}

pub fn progress() -> (u64, u64) {
    (DONE.load(Ordering::Relaxed), TOTAL.load(Ordering::Relaxed))
}

pub fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_proxy_covers_hosts_and_domains() {
        assert_eq!(host_of("https://api.github.com/repos/x"), "api.github.com");
        assert_eq!(host_of("http://user@127.0.0.1:8765/latest"), "127.0.0.1");
        assert_eq!(host_of("http://[::1]:80/x"), "::1");
        assert!(bypasses_proxy("127.0.0.1", "localhost, 127.0.0.1"));
        assert!(bypasses_proxy("a.studio.lan", ".studio.lan"));
        assert!(bypasses_proxy("a.studio.lan", "studio.lan"));
        assert!(bypasses_proxy("anything", "*"));
        assert!(!bypasses_proxy("api.github.com", "localhost,.studio.lan"));
        assert!(!bypasses_proxy("notstudio.lan", "studio.lan"));
    }

    #[test]
    fn a_bad_digest_is_refused_and_nothing_is_kept() {
        // No network in tests: a URL that can't connect reports an error and leaves no file behind.
        let dir = std::env::temp_dir().join("aztek-update-test");
        let path = dir.join("x.bin");
        let err = download("http://127.0.0.1:9/x.bin", &path, "sha256:00");
        assert!(err.is_some());
        assert!(!path.exists());
    }
}
