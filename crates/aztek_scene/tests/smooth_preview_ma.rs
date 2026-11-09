//! Smooth mesh preview survives a save: written as displaySmoothMesh and smoothLevel, read back into smoothPreview.

use aztek_scene::ma_file::{read_text, write_text};
use aztek_scene::Session;
use serde_json::json;

#[test]
fn smooth_preview_saves_and_reads_back() {
    let mut s = Session::new();
    s.run("polyCube", json!({"name": "cage"})).unwrap();
    s.run("setAttr", json!({"args": ["cageShape.smoothPreview", 3]})).unwrap();
    let (text, _) = write_text(&mut s, "cage.ma", None);
    assert!(text.contains("setAttr \".displaySmoothMesh\" 2;") && text.contains("setAttr \".smoothLevel\" 3;"), "{text}");
    let mut back = Session::new();
    assert!(read_text(&mut back, &text, false).is_empty());
    assert_eq!(back.run("getAttr", json!({"args": ["cageShape.smoothPreview"]})).unwrap(), json!(3));
    // A file from the reference that only turns it on gets the reference's default of two levels.
    let other = text.replace("\tsetAttr \".smoothLevel\" 3;\n", "");
    let mut third = Session::new();
    assert!(read_text(&mut third, &other, false).is_empty());
    assert_eq!(third.run("getAttr", json!({"args": ["cageShape.smoothPreview"]})).unwrap(), json!(2));
}
