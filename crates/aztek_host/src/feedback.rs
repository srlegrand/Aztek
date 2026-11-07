//! The "This doesn't feel right!" report: the new-issue link and the zipped copy of the scene. Nothing here touches the
//! network. The shell opens the link in a browser, and the person presses Submit there.

use std::io::Write;
use std::path::Path;

use aztek_scene::Session;
use serde_json::{json, Value};

pub const NEW_ISSUE_URL: &str = "https://github.com/srlegrand/Aztek/issues/new";
/// Past about this length browsers or GitHub can refuse the link.
pub const URL_LIMIT: usize = 7000;
/// What the title holds before the prefix, in characters.
const TITLE_LIMIT: usize = 120;
/// The title's prefix. A labels= query is ignored for people without triage rights.
const TITLE_PREFIX: &str = "[Feel] ";
const SHORTENED_NOTE: &str = "[The rest of this description did not fit in the link. Paste the full text here with Copy Full Text.]";
/// The entry inside the zip. Generic: never the scene's own file name.
const SCENE_ENTRY: &str = "aztek-scene.ma";

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub url: String,
    pub title: String,
    /// The body the link carries.
    pub body: String,
    /// The body before any shortening.
    pub full_body: String,
    pub shortened: bool,
}

/// Percent-encodes everything but the unreserved characters, so a line break or an ampersand can't change the query.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The title given, or else the first line of the description, on one line and capped, with the prefix.
pub fn issue_title(title: &str, description: &str) -> String {
    let from_text = description.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let chosen = if title.trim().is_empty() { from_text } else { title.trim() };
    let flat: String = chosen.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let bare = flat.strip_prefix(TITLE_PREFIX.trim_end()).unwrap_or(&flat).trim();
    let mut short: String = bare.chars().take(TITLE_LIMIT).collect();
    if bare.chars().count() > TITLE_LIMIT {
        short = format!("{}...", short.trim_end());
    }
    format!("{TITLE_PREFIX}{short}")
}

fn body_text(description: &str, environment: &str) -> String {
    format!("{}\n\n### Environment\n\n{}\n", description.trim_end(), environment.trim_end())
}

fn link(title: &str, body: &str) -> String {
    format!("{NEW_ISSUE_URL}?title={}&body={}", encode(title), encode(body))
}

/// The link for this report, at most `limit` characters long when that can be done. A body too long is cut in the
/// description only, with a note there, so the environment always arrives.
pub fn issue(title: &str, description: &str, environment: &str, limit: usize) -> Issue {
    let title = issue_title(title, description);
    let description = description.trim_end();
    let full_body = body_text(description, environment);
    let url = link(&title, &full_body);
    if url.len() <= limit {
        return Issue { url, title, body: full_body.clone(), full_body, shortened: false };
    }
    // Encoded length depends on the characters kept: find the most that fit.
    let chars: Vec<char> = description.chars().collect();
    let cut_to = |n: usize| -> String {
        let kept: String = chars[..n].iter().collect();
        body_text(&format!("{}\n\n{SHORTENED_NOTE}", kept.trim_end()), environment)
    };
    let (mut low, mut high) = (0, chars.len());
    while low < high {
        let mid = (low + high).div_ceil(2);
        if link(&title, &cut_to(mid)).len() <= limit {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    let body = cut_to(low);
    Issue { url: link(&title, &body), title, body, full_body, shortened: true }
}

/// The environment section: one line per value, and which attachments the person meant to add. Nothing that names the
/// scene, a path or the user.
pub fn environment(values: &[(&str, String)], attachments: &[String]) -> String {
    let mut lines: Vec<String> = values.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("- {k}: {v}")).collect();
    let attached = if attachments.is_empty() { "none".to_string() } else { attachments.join(", ") };
    lines.push(format!("- Attachments meant to be added: {attached}"));
    lines.join("\n")
}

/// The report from the shell's JSON: title, description, version, os, qt, attachments.
pub fn issue_from_json(call: &Value, gpu: Option<String>) -> Value {
    let text = |k: &str| call.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let attachments: Vec<String> =
        call.get("attachments").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let env = environment(
        &[("Aztek", text("version")), ("OS", text("os")), ("Qt", text("qt")), ("GPU", gpu.unwrap_or_default())],
        &attachments,
    );
    let i = issue(&text("title"), &text("description"), &env, URL_LIMIT);
    json!({"url": i.url, "title": i.title, "body": i.body, "fullBody": i.full_body, "shortened": i.shortened})
}

/// Writes one text file into a new deflated zip, and returns the zip's size in bytes.
pub fn zip_text(entry: &str, text: &str, path: &Path) -> Result<u64, String> {
    let shown = path.display();
    let file = std::fs::File::create(path).map_err(|e| format!("{shown}: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(text.len() as u64 >= u32::MAX as u64);
    zip.start_file(entry, options).map_err(|e| format!("{shown}: {e}"))?;
    zip.write_all(text.as_bytes()).map_err(|e| format!("{shown}: {e}"))?;
    zip.finish().map_err(|e| format!("{shown}: {e}"))?;
    std::fs::metadata(path).map(|m| m.len()).map_err(|e| format!("{shown}: {e}"))
}

/// A zipped `.ma` copy of the scene. The scene keeps its name, its edits and its undo.
pub fn scene_zip(s: &mut Session, path: &Path) -> Result<u64, String> {
    let (text, _warnings) = aztek_scene::ma_file::write_text(s, SCENE_ENTRY, None);
    zip_text(SCENE_ENTRY, &text, path)
}
