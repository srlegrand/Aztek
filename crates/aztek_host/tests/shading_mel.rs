//! Materials as typed in MEL, through a real Core with its default scene; images in each format the viewport reads.

use aztek_host::Core;
use serde_json::{json, Value};

fn errors(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

fn mel(c: &mut Core, src: &str) -> Value {
    let v = c.mel(src);
    let e = errors(c);
    assert!(e.is_empty(), "{src}: {e:?}");
    v
}

#[test]
fn a_material_from_start_to_finish() {
    let mut c = Core::new();
    mel(&mut c, "polyCube;");
    assert_eq!(mel(&mut c, "ls -mat;"), json!(["lambert1", "standardSurface1", "openPBR_shader1", "particleCloud1"]));
    assert_eq!(mel(&mut c, "sets -q initialShadingGroup;"), json!(["pCubeShape1"]));
    assert_eq!(mel(&mut c, "shadingNode -asShader lambert;"), "lambert2");
    assert_eq!(mel(&mut c, "sets -renderable true -noSurfaceShader true -empty -name lambert2SG;"), "lambert2SG");
    mel(&mut c, "connectAttr -f lambert2.outColor lambert2SG.surfaceShader;");
    mel(&mut c, "sets -e -forceElement lambert2SG pCube1.f[0:3];");
    assert_eq!(mel(&mut c, "sets -q lambert2SG;"), json!(["pCube1.f[0:3]"]));
    assert_eq!(mel(&mut c, "listSets -type 1 -o pCubeShape1;"), json!(["initialShadingGroup", "lambert2SG"]));
    assert_eq!(mel(&mut c, "shadingNode -asTexture file;"), "file1");
    assert_eq!(mel(&mut c, "shadingNode -asUtility place2dTexture;"), "place2dTexture1");
    mel(&mut c, "defaultNavigation -connectToExisting -source place2dTexture1 -destination file1;");
    mel(&mut c, "defaultNavigation -ce -s file1 -d lambert2.color;");
    assert_eq!(mel(&mut c, "ls -tex;"), json!(["file1"]));
    assert_eq!(mel(&mut c, "listConnections -s 1 -d 0 lambert2.color;"), json!(["file1"]));
    mel(&mut c, "setAttr -type \"string\" file1.fileTextureName \"D:/somewhere/Body.png\";");
    assert_eq!(mel(&mut c, "getAttr file1.ftn;"), "D:/somewhere/Body.png");
    mel(&mut c, "setAttr place2dTexture1.repeatU 3;");
    assert_eq!(mel(&mut c, "getAttr file1.repeatU;"), json!(3.0));
    mel(&mut c, "select -r pCube1; hyperShade -assign lambert2;");
    assert_eq!(mel(&mut c, "sets -q lambert2SG;"), json!(["pCubeShape1"]));
    mel(&mut c, "select -cl; hyperShade -objects lambert2;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["pCubeShape1"]));
    // Back past -objects and select -cl to the assignment.
    mel(&mut c, "undo; undo; undo;");
    assert_eq!(mel(&mut c, "sets -q lambert2SG;"), json!(["pCube1.f[0:3]"]));
}

#[test]
fn hyper_shade_short_flags_and_connections() {
    let mut c = Core::new();
    mel(&mut c, "polyCube; shadingNode -asShader blinn -n shiny; select -r pCube1; hyperShade -a shiny;");
    let shapes = mel(&mut c, "listConnections -type shadingEngine pCubeShape1;");
    assert_eq!(shapes, json!(["shinySG"]));
    assert_eq!(mel(&mut c, "listConnections shinySG.surfaceShader;"), json!(["shiny"]));
}

#[test]
fn images_decode_in_each_format() {
    let dir = std::env::temp_dir().join(format!("aztek_img_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let solid = image::RgbImage::from_fn(4, 2, |x, _| image::Rgb([if x < 2 { 255 } else { 0 }, 128, 0]));
    for ext in ["png", "jpg", "tga", "bmp", "tif"] {
        let p = dir.join(format!("a.{ext}"));
        solid.save(&p).unwrap();
        let d = aztek_host::textures::decode(p.to_str().unwrap()).unwrap();
        assert_eq!((d.width, d.height, d.has_alpha), (4, 2, false), "{ext}");
        assert_eq!(d.rgba.len(), 4 * 2 * 4);
        assert!(d.rgba[0] > 200, "{ext}: {}", d.rgba[0]);
    }
    // Alpha that means something, top row first.
    let clear = image::RgbaImage::from_fn(2, 2, |_, y| image::Rgba([10, 20, 30, if y == 0 { 0 } else { 255 }]));
    let p = dir.join("b.png");
    clear.save(&p).unwrap();
    let d = aztek_host::textures::decode(p.to_str().unwrap()).unwrap();
    assert!(d.has_alpha);
    assert_eq!(d.rgba[3], 0);
    assert_eq!(d.rgba[15], 255);
    // Not an image.
    let bad = dir.join("c.png");
    std::fs::write(&bad, b"not an image").unwrap();
    assert!(aztek_host::textures::decode(bad.to_str().unwrap()).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}
