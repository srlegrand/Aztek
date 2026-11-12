//! The Deform and skin commands as MEL spells them: flags, short names, query and edit modes.

use aztek_host::Core;
use serde_json::{json, Value};

fn mel(c: &mut Core, src: &str) -> Value {
    let v = c.mel(src);
    let bad: Vec<String> = c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect();
    assert!(bad.is_empty(), "{src}: {bad:?}");
    v
}

fn num(v: Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("{v} is not a number"))
}

#[test]
fn blend_shape_in_mel() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n a; polyCube -n b; polyCube -n d; move -r 1 0 0 b.vtx[0]; move -r 0 1 0 d.vtx[0];");
    assert_eq!(mel(&mut c, "blendShape -n bs -w 0 0.5 -w 1 0.25 b d a;"), json!(["bs"]));
    assert_eq!(mel(&mut c, "blendShape -q -w bs;"), json!([0.5, 0.25]));
    assert_eq!(mel(&mut c, "blendShape -q -wc bs;"), json!(2));
    assert_eq!(mel(&mut c, "blendShape -q -t bs;"), json!(["bShape", "dShape"]));
    assert_eq!(mel(&mut c, "blendShape -q -g bs;"), json!(["aShape"]));
    assert_eq!(num(mel(&mut c, "getAttr bs.b;")), 0.5);
    mel(&mut c, "setAttr bs.d 0.75;");
    assert_eq!(num(mel(&mut c, "getAttr bs.weight[1];")), 0.75);
    assert_eq!(mel(&mut c, "aliasAttr -q bs;"), json!(["b", "weight[0]", "d", "weight[1]"]));
    assert_eq!(mel(&mut c, "listAttr -m bs.weight;"), json!(["b", "d"]));
    mel(&mut c, "blendShape -e -w 0 0.1 bs;");
    assert_eq!(num(mel(&mut c, "getAttr bs.b;")), 0.1);
    mel(&mut c, "blendShape -e -en 0.5 bs;");
    assert_eq!(mel(&mut c, "blendShape -q -en bs;"), json!(0.5));
    mel(&mut c, "polyCube -n e; blendShape -e -t a 2 e 1.0 bs;");
    assert_eq!(mel(&mut c, "blendShape -q -wc bs;"), json!(3));
    mel(&mut c, "polyCube -n m; blendShape -e -ib -t a 2 m 0.5 bs;");
    mel(&mut c, "blendShape -e -rm -t a 2 e 1.0 bs;");
    assert_eq!(mel(&mut c, "blendShape -q -wc bs;"), json!(2));
    mel(&mut c, "blendShape -e -rtd 0 0 bs;");
    assert_eq!(mel(&mut c, "blendShape -q -wc bs;"), json!(2));
    assert_eq!(mel(&mut c, "blendShape -q -o bs;"), json!("local"));
    mel(&mut c, "polyCube -n z; polyCube -n y;");
    assert_eq!(mel(&mut c, "blendShape -foc -tc 0 -o world y z;"), json!(["blendShape1"]));
    assert_eq!(mel(&mut c, "blendShape -q -o blendShape1;"), json!("world"));
    assert_eq!(mel(&mut c, "listHistory -pdo z;"), mel(&mut c, "listHistory -pdo -type geometryFilter z;").as_array().map(|v| json!(v.iter().cloned().chain([json!("polyCube6")]).collect::<Vec<_>>())).unwrap());
    mel(&mut c, "sculptTarget -e -t 0 bs;");
    assert_eq!(mel(&mut c, "sculptTarget -q bs;"), json!(0));
    mel(&mut c, "sculptTarget -e -t -1 bs;");
}

#[test]
fn deformers_in_mel() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n a; polyCube -n t; curve -d 1 -p 0 0 0 -p 0 1 0 -n k;");
    assert_eq!(mel(&mut c, "sculpt -mode project -mxd 0.5 -dds 2 a;"), json!(["sculpt1", "sculptor1", "sculpt1StretchOrigin"]));
    assert_eq!(mel(&mut c, "sculpt -q -mode sculpt1;"), json!("project"));
    assert_eq!(mel(&mut c, "jiggle -stf 0.2 -dmp 0.4 -jw 0.5 -mm 2 -sf 5 a;"), json!(["jiggle1"]));
    assert_eq!(mel(&mut c, "jiggle -q -stf jiggle1;").as_f64().map(|x| (x * 10.0).round()), Some(2.0));
    mel(&mut c, "jiggle -e -ena 2 jiggle1;");
    assert_eq!(mel(&mut c, "jiggle -q -ena jiggle1;"), json!(2));
    assert_eq!(mel(&mut c, "shrinkWrap -prj 5 -bd 1 -ti 0.1 -off 0.2 -rev 1 a t;"), json!(["shrinkWrap1"]));
    assert_eq!(mel(&mut c, "morph -mt t a;"), json!(["morph1"]));
    assert_eq!(mel(&mut c, "wrinkle -x 1 -sc 2 -st lines a;"), json!(["wrinkle1", "wrinkle1Handle"]));
    assert_eq!(mel(&mut c, "proximityWrap -drv t -md 2 a;"), json!(["proximityWrap1"]));
    assert_eq!(mel(&mut c, "solidify -se 0.5 a;"), json!(["solidify1"]));
    assert_eq!(mel(&mut c, "deformer -type pointOnCurve a k;"), json!(["pointOnCurve1"]));
    assert_eq!(mel(&mut c, "deformer -type textureDeformer a;"), json!(["textureDeformer1"]));
    assert_eq!(mel(&mut c, "curveWarp -c k -ls 0.5 a;"), json!(["curveWarp1"]));
    assert_eq!(num(mel(&mut c, "getAttr curveWarp1.lengthScale;")), 0.5);
}

#[test]
fn sets_and_paint_in_mel() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n a; cluster -n cl a.vtx[0:3];");
    assert_eq!(mel(&mut c, "sets -q clSet;"), json!(["a.vtx[0:3]"]));
    mel(&mut c, "sets -add clSet a.vtx[5]; sets -rm clSet a.vtx[0];");
    assert_eq!(mel(&mut c, "sets -q clSet;"), json!(["a.vtx[1:3]", "a.vtx[5]"]));
    assert_eq!(mel(&mut c, "sets -im clSet a.vtx[5];"), json!(true));
    assert_eq!(mel(&mut c, "ls -io;"), json!(["aShapeOrig"]));
    assert_eq!(mel(&mut c, "deformableShape -og a;"), json!(["aShapeOrig.worldMesh"]));
    mel(&mut c, "select -r a; artSetToolAndSelectAttr artAttrCtx cluster.cl.weights;");
    assert_eq!(mel(&mut c, "artAttrCtx -q -asl artAttrCtx1;"), json!("cluster.cl.weights"));
    assert_eq!(aztek_host::take_ui_requests_for_tests(), json!(["ArtPaintAttrTool"]));
    mel(&mut c, "artAttrCtx -e -val 0.25 -op 1 -r 10 -sao absolute -stP solid artAttrCtx1; artAttrCtx -e -clear artAttrCtx1;");
    assert_eq!(num(mel(&mut c, "getAttr cl.weightList[0].weights[5];")), 0.25);
    assert_eq!(num(mel(&mut c, "getAttr cl.weightList[0].weights[0];")), 0.0);
    mel(&mut c, "setAttr cl.weightList[0].weights[5] 0.75;");
    assert_eq!(num(mel(&mut c, "getAttr cl.wl[0].w[5];")), 0.75);
}

#[test]
fn skin_in_mel() {
    let dir = std::env::temp_dir().join(format!("aztek_mel_deform_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.to_string_lossy().to_string();
    let mut c = Core::new();
    mel(&mut c, "polyCube -h 2 -sy 2 -n a; select -cl; joint -p 0 -1 0 -n j1; joint -p 0 1 0 -n j2;");
    assert_eq!(mel(&mut c, "skinCluster -n sk -bm 2 -hmf 0.5 -sm 2 j1 a;"), json!(["sk"]));
    assert_eq!(mel(&mut c, "skinCluster -q -bm sk;"), json!(2));
    assert_eq!(mel(&mut c, "skinCluster -q -hmf sk;"), json!(0.5));
    assert_eq!(mel(&mut c, "skinCluster -q -sm sk;"), json!(2));
    mel(&mut c, "skinPercent -dqb 0.5 sk a.vtx[0];");
    mel(&mut c, &format!("deformerWeights -ex -df sk -p \"{path}\" w.xml;"));
    assert!(dir.join("w.xml").exists());
    mel(&mut c, &format!("deformerWeights -im -m index -df sk -p \"{path}\" w.xml;"));
    mel(&mut c, "skinCluster -e -mjm 1 sk; skinCluster -e -mjm 0 sk; skinCluster -e -rui sk;");
    mel(&mut c, "skinCluster -e -nw 2 sk;");
    assert_eq!(mel(&mut c, "skinCluster -q -nw sk;"), json!(2));
    mel(&mut c, "weightHammerVerts a.vtx[0];");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn original_shapes_show_with_shapes_and_deformer_sets_only_when_asked() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n a; cluster a;");
    let names = |v: &Value| v.to_string();
    assert!(!names(&aztek_host::outliner(&c, "")).contains("aShapeOrig"));
    // Shapes on lists the original shape under its transform, as the reference's Outliner does (issue 32).
    let plain = aztek_host::outliner(&c, "shapes");
    assert!(names(&plain).contains("aShapeOrig") && !names(&plain).contains("cluster1Set"), "{plain}");
    let all = aztek_host::outliner(&c, "shapes intermediate");
    assert!(names(&all).contains("aShapeOrig") && names(&all).contains("cluster1Set"), "{all}");
}

#[test]
fn duplicating_a_deformed_mesh_leaves_a_plain_mesh() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n a; cluster -n cl a; move -r 0 1 0 clHandle;");
    let dup = mel(&mut c, "duplicate a;");
    let name = dup[0].as_str().unwrap().to_string();
    assert_eq!(mel(&mut c, &format!("pointPosition {name}.vtx[0];")), mel(&mut c, "pointPosition a.vtx[0];"));
    assert_eq!(mel(&mut c, "ls -io;"), json!(["aShapeOrig"]), "the copy has no original of its own");
}

/// `ls -sl` is what is selected now: objects and components don't linger from an earlier selection.
#[test]
fn selecting_replaces_components_and_clearing_clears_them() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n a; polyCube -n b; select -r a.vtx[0:1];");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["a.vtx[0:1]"]));
    mel(&mut c, "select -r b;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["b"]));
    mel(&mut c, "select -r a.vtx[0];");
    mel(&mut c, "select -cl;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!([]));
    mel(&mut c, "select -r a.vtx[0]; select -add b;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["a.vtx[0]", "b"]));
}
