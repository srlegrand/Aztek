//! The feedback report: the link, its shortening, and the zipped copy of the scene.

use std::io::Read;

use aztek_host::feedback::{encode, environment, issue, issue_from_json, issue_title, scene_zip, zip_text, NEW_ISSUE_URL, URL_LIMIT};
use aztek_host::Core;
use serde_json::json;

const ENV: &str = "- Aztek: 1.0\n- Attachments meant to be added: none";

/// Undoes the percent-encoding of one query value.
fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

fn param(url: &str, name: &str) -> String {
    let query = url.split_once('?').unwrap().1;
    decode(query.split('&').find_map(|p| p.strip_prefix(&format!("{name}="))).unwrap())
}

#[test]
fn encoding_leaves_only_safe_characters() {
    assert_eq!(encode("a b&c=d\n#é"), "a%20b%26c%3Dd%0A%23%C3%A9");
    assert_eq!(encode("AZaz09-_.~"), "AZaz09-_.~");
}

#[test]
fn title_comes_from_the_field_or_the_first_line() {
    assert_eq!(issue_title("Pivot jumps", "ignored"), "[Feel] Pivot jumps");
    assert_eq!(issue_title("  ", "\n\n  First line \nsecond"), "[Feel] First line");
    assert_eq!(issue_title("[Feel] Already", ""), "[Feel] Already");
    let long = issue_title("", &"x".repeat(500));
    assert_eq!(long, format!("[Feel] {}...", "x".repeat(120)));
    assert_eq!(issue_title("a\tb", ""), "[Feel] a b");
}

#[test]
fn a_short_report_goes_whole() {
    let i = issue("", "It moved & jumped\nthen stopped", ENV, URL_LIMIT);
    assert!(!i.shortened);
    assert!(i.url.starts_with(&format!("{NEW_ISSUE_URL}?title=")));
    assert_eq!(param(&i.url, "title"), "[Feel] It moved & jumped");
    assert_eq!(param(&i.url, "body"), i.body);
    assert_eq!(i.body, i.full_body);
    assert_eq!(i.body, format!("It moved & jumped\nthen stopped\n\n### Environment\n\n{ENV}\n"));
}

#[test]
fn a_long_report_is_cut_in_the_description_and_keeps_the_environment() {
    // Many bytes per character when encoded, so characters and encoded length differ a lot.
    for text in ["a".repeat(20_000), "日本語 ".repeat(5_000), "\u{1F600}".repeat(4_000)] {
        let i = issue("Title", &text, ENV, URL_LIMIT);
        assert!(i.shortened);
        assert!(i.url.len() <= URL_LIMIT, "{}", i.url.len());
        assert!(i.url.len() > URL_LIMIT - 40, "wasted room: {}", i.url.len());
        let body = param(&i.url, "body");
        assert_eq!(body, i.body);
        assert!(body.contains("did not fit in the link"));
        assert!(body.ends_with(&format!("### Environment\n\n{ENV}\n")));
        assert!(i.full_body.contains(text.trim_end()));
    }
    // A limit nothing fits: no panic, no description, and it says it was cut.
    let i = issue("t", "hello", ENV, 10);
    assert!(i.shortened);
    assert!(!i.body.contains("hello"));
}

#[test]
fn the_environment_names_no_scene_path_or_user() {
    let env = environment(&[("Aztek", "1.2".into()), ("OS", "Linux".into()), ("GPU", String::new())], &[]);
    assert_eq!(env, "- Aztek: 1.2\n- OS: Linux\n- Attachments meant to be added: none");
    let env = environment(&[], &["screenshot".into(), "scene file (zip)".into()]);
    assert_eq!(env, "- Attachments meant to be added: screenshot, scene file (zip)");
    let v = issue_from_json(&json!({"title": "", "description": "d", "version": "1", "os": "o", "qt": "6", "attachments": ["screenshot"]}), Some("Card".into()));
    assert_eq!(v["body"], "d\n\n### Environment\n\n- Aztek: 1\n- OS: o\n- Qt: 6\n- GPU: Card\n- Attachments meant to be added: screenshot\n");
    assert_eq!(v["shortened"], false);
}

fn unzip(path: &std::path::Path) -> (String, String) {
    let mut a = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    assert_eq!(a.len(), 1);
    let mut f = a.by_index(0).unwrap();
    assert_eq!(f.compression(), zip::CompressionMethod::Deflated);
    let mut text = String::new();
    f.read_to_string(&mut text).unwrap();
    (f.name().unwrap().to_string(), text)
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("aztek_feedback_{}_{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_zip_holds_the_text() {
    let path = temp("text").join("t.zip");
    let text = "line\n".repeat(10_000);
    let bytes = zip_text("t.ma", &text, &path).unwrap();
    assert_eq!(bytes, std::fs::metadata(&path).unwrap().len());
    assert!((bytes as usize) < text.len() / 10);
    assert_eq!(unzip(&path), ("t.ma".to_string(), text));
    assert!(zip_text("t.ma", "x", &temp("text").join("no/such/dir/t.zip")).is_err());
}

#[test]
fn the_scene_copy_leaves_the_scene_as_it_was() {
    let dir = temp("scene");
    let path = dir.join("aztek-scene.zip");
    let named = dir.join("secret_project_shot12.ma").to_str().unwrap().to_string();
    let mut c = Core::new();
    c.mel("polyCube -n \"box\";");
    c.mel("polySphere -n \"ball\";");
    c.mel("select -r box;");
    c.mel(&format!("file -rename \"{named}\";"));
    let state = |c: &mut Core| (c.mel("file -q -sn;"), c.mel("file -q -mf;"), c.mel("ls -sl;"), c.mel("undoInfo -q -un;"));
    let before = state(&mut c);
    assert_eq!(before.1, json!(true));
    c.take_log();
    let version = c.version;

    let bytes = scene_zip(&mut c.s, &path).unwrap();

    assert!(bytes > 0);
    assert_eq!(c.version, version);
    assert_eq!(c.s.scene_path.as_deref(), Some(named.as_str()));
    assert!(c.take_log().as_array().unwrap().is_empty(), "the copy wrote history lines");
    assert_eq!(state(&mut c), before);

    let (entry, text) = unzip(&path);
    assert_eq!(entry, "aztek-scene.ma");
    assert!(text.contains("\"box\""), "{}", &text[..300.min(text.len())]);
    assert!(text.contains("\"ball\""));
    assert!(!text.contains("secret_project"), "the scene's own name got into the copy");

    // The copy opens as a scene.
    let copy = dir.join("copy.ma");
    std::fs::write(&copy, &text).unwrap();
    let mut other = Core::new();
    other.mel(&format!("file -f -o \"{}\";", copy.to_str().unwrap()));
    assert_eq!(other.mel("objExists box;"), json!(true));
}
