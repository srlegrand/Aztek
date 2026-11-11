//! UV sets: made, copied, renamed and deleted by polyUVSet, each with its own UVs and seams, kept in a scene file.

use aztek_host::Core;
use serde_json::json;

#[test]
fn each_set_has_its_own_uvs_and_seams() {
    let mut c = Core::new();
    c.mel("polyCube -n box;");
    assert_eq!(c.mel("polyUVSet -q -auv box;"), json!(["map1"]));
    let (uvs, shells) = (c.mel("polyEvaluate -uv box;"), c.mel("polyEvaluate -us box;"));
    assert_eq!(c.mel("polyUVSet -cr -uvs \"lightmap\" box;"), json!(["lightmap"]));
    // An unnamed set gets the next free name, and so does a second.
    assert_eq!(c.mel("polyUVSet -cr box;"), json!(["uvSet1"]));
    assert_eq!(c.mel("polyUVSet -cr box;"), json!(["uvSet2"]));
    assert_eq!(c.mel("polyUVSet -cp -uvs \"map1\" -nuv \"copy\" box;"), json!(["copy"]));
    assert_eq!(c.mel("polyUVSet -q -auv box;"), json!(["map1", "lightmap", "uvSet1", "uvSet2", "copy"]));

    c.mel("polyUVSet -cuv -uvs \"lightmap\" box;");
    assert_eq!(c.mel("polyUVSet -q -cuv box;"), json!(["lightmap"]));
    c.mel("polyAutoProjection box.f[0:5];");
    assert_eq!(c.mel("polyEvaluate -us box;"), json!(6));
    // The first set is as it was: its UVs and where they are cut.
    c.mel("polyUVSet -cuv -uvs \"map1\" box;");
    assert_eq!(c.mel("polyEvaluate -uv box;"), uvs);
    assert_eq!(c.mel("polyEvaluate -us box;"), shells);
    c.mel("polyUVSet -cuv -uvs \"copy\" box;");
    assert_eq!(c.mel("polyEvaluate -us box;"), shells, "a copy is cut where its source is");
    c.mel("undo;");
    assert_eq!(c.mel("polyUVSet -q -cuv box;"), json!(["map1"]));

    c.mel("polyUVSet -rn -uvs \"copy\" -nuv \"second\" box;");
    assert!(c.mel_checked("polyUVSet -rn -uvs \"nope\" -nuv \"x\" box;").is_err());
    assert!(c.mel_checked("polyUVSet -cuv -uvs \"nope\" box;").is_err());
    c.mel("polyUVSet -d -uvs \"uvSet1\" box; polyUVSet -d -uvs \"uvSet2\" box;");
    assert_eq!(c.mel("polyUVSet -q -auv box;"), json!(["map1", "lightmap", "second"]));
    // The order does not follow the current set.
    c.mel("polyUVSet -cuv -uvs \"second\" box;");
    assert_eq!(c.mel("polyUVSet -q -auv box;"), json!(["map1", "lightmap", "second"]));

    // A scene file keeps every set, its name, the order and the current one.
    let file = std::env::temp_dir().join(format!("aztek_uv_sets_{}.ma", std::process::id()));
    let path = file.to_string_lossy().replace('\\', "/");
    c.mel("polyUVSet -cuv -uvs \"lightmap\" box;");
    c.mel(&format!("file -rename \"{path}\"; file -save -type \"mayaAscii\";"));
    c.mel(&format!("file -f -open \"{path}\";"));
    assert_eq!(c.mel("polyUVSet -q -auv box;"), json!(["map1", "lightmap", "second"]));
    assert_eq!(c.mel("polyUVSet -q -cuv box;"), json!(["lightmap"]));
    assert_eq!(c.mel("polyEvaluate -us box;"), json!(6));
    c.mel("polyUVSet -cuv -uvs \"map1\" box;");
    assert_eq!(c.mel("polyEvaluate -uv box;"), uvs);
    assert_eq!(c.mel("polyEvaluate -us box;"), shells);
    // An evaluation between an edit and its undo left the edit on a mesh read from a file.
    c.mel("undo;");
    assert_eq!(c.mel("polyUVSet -q -cuv box;"), json!(["lightmap"]));
    c.mel("polyTriangulate box;");
    assert_eq!(c.mel("polyEvaluate -f box;"), json!(12));
    c.mel("undo;");
    assert_eq!(c.mel("polyEvaluate -f box;"), json!(6));
    let _ = std::fs::remove_file(file);
}
