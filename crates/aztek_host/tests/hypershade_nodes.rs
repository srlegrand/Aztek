//! The Hypershade's nodes: the procedural textures with the reference's attributes, their placement nodes, and the
//! pictures Harold makes of a network for the swatches and for the views.

use aztek_host::Core;
use serde_json::{json, Value};

fn ppm(path: &str) -> (usize, usize, Vec<u8>) {
    let bytes = std::fs::read(path).unwrap();
    let mut parts = bytes.splitn(4, |&b| b == b'\n');
    assert_eq!(parts.next().unwrap(), b"P6");
    let size = String::from_utf8_lossy(parts.next().unwrap()).to_string();
    let mut wh = size.split(' ').map(|x| x.parse::<usize>().unwrap());
    let (w, h) = (wh.next().unwrap(), wh.next().unwrap());
    assert_eq!(parts.next().unwrap(), b"255");
    (w, h, parts.next().unwrap().to_vec())
}

#[test]
fn textures_have_the_reference_s_attributes_and_classes() {
    let mut c = Core::new();
    let have = |c: &mut Core, class: &str| -> Vec<String> { c.mel(&format!("listNodeTypes \"{class}\";")).as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect() };
    let three = have(&mut c, "texture/3d");
    for t in ["brownian", "cloud", "crater", "granite", "leather", "marble", "rock", "snow", "solidFractal", "stucco", "volumeNoise", "wood"] {
        assert!(three.contains(&t.to_string()), "{t} in {three:?}");
    }
    let two = have(&mut c, "texture/2d");
    for t in ["bulge", "checker", "file", "fractal", "grid", "mountain", "noise", "ramp", "water"] {
        assert!(two.contains(&t.to_string()), "{t} in {two:?}");
    }
    let utils = have(&mut c, "utility");
    for t in ["samplerInfo", "gammaCorrect", "contrast", "bump3d", "place2dTexture", "place3dTexture"] {
        assert!(utils.contains(&t.to_string()), "{t} in {utils:?}");
    }
    // Defaults and short names as the reference documents them.
    c.mel("shadingNode -asTexture marble -name \"m\"; shadingNode -asTexture granite -name \"g\"; shadingNode -asTexture wood -name \"w\"; shadingNode -asTexture water -name \"wa\"; shadingNode -asTexture volumeNoise -name \"vn\";");
    assert_eq!(c.mel("getAttr m.vw;"), json!(0.1));
    assert_eq!(c.mel("getAttr m.amplitude;"), json!(1.5));
    assert_eq!(c.mel("getAttr m.depthMax;"), json!(20.0));
    assert_eq!(c.mel("getAttr g.cs;"), json!(0.15));
    assert_eq!(c.mel("getAttr w.ls;"), json!(0.05));
    assert_eq!(c.mel("getAttr w.centerV;"), json!(-0.5));
    assert_eq!(c.mel("getAttr wa.nw;"), json!(8));
    assert_eq!(c.mel("getAttr wa.rippleFrequency;"), json!(25.0));
    assert_eq!(c.mel("getAttr -asString vn.noiseType;"), json!("Billow"));
    // A 3D texture is placed by a transform's world inverse matrix.
    c.mel("shadingNode -asUtility place3dTexture -name \"p3\"; connectAttr p3.worldInverseMatrix[0] m.placementMatrix; setAttr p3.scale 2 2 2;");
    assert_eq!(c.mel("getAttr m.placementMatrix;")[0], json!(0.5));
    assert_eq!(c.mel("nodeType p3;"), json!("place3dTexture"));
}

#[test]
fn harold_pictures_a_network_for_swatches_and_views() {
    let dir = std::env::temp_dir().join(format!("aztek_swatches_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = |n: &str| dir.join(n).to_string_lossy().replace('\\', "/");
    let mut c = Core::new();
    c.mel("shadingNode -asShader standardSurface -name \"mat\"; sets -renderable true -noSurfaceShader true -empty -name \"matSG\"; connectAttr mat.outColor matSG.surfaceShader;");
    c.mel("shadingNode -asTexture checker -name \"chk\"; connectAttr chk.outColor mat.baseColor;");

    // The texture, flat: its two colours in squares, four a side.
    let r = c.mel(&format!("haroldSwatch -s 64 -f \"{}\" chk;", file("chk.ppm")));
    assert_eq!(r["width"], json!(64), "{r}");
    let (w, _, px) = ppm(&file("chk.ppm"));
    let at = |x: usize, y: usize| px[(y * w + x) * 3];
    assert_eq!((at(4, 4), at(20, 4), at(4, 20)), (0, 255, 255), "v runs up: the top left square is (0, 3), odd");

    // The material on a ball: lit, with both colours of the texture on it, the ground dark round it.
    c.mel(&format!("haroldSwatch -s 96 -f \"{}\" mat;", file("mat.ppm")));
    let (w, h, px) = ppm(&file("mat.ppm"));
    let lum: Vec<u8> = px.chunks(3).map(|p| p[1]).collect();
    assert!(lum[0] < 40, "the corner is ground");
    let inside: Vec<u8> = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| ((x as f32 - 48.0).powi(2) + (y as f32 - 48.0).powi(2)).sqrt() < 30.0).map(|(x, y)| lum[y * w + x]).collect();
    assert!(inside.iter().any(|&v| v > 150) && inside.iter().any(|&v| v < 30), "white and black squares both show");

    // The signature holds while nothing changes, and moves when the network does.
    let a = c.mel("haroldSwatch -sig mat;");
    assert_eq!(c.mel("haroldSwatch -sig mat;"), a);
    c.mel("setAttr chk.color1 1 0 0;");
    assert_ne!(c.mel("haroldSwatch -sig mat;"), a, "a change upstream changes the material's picture");

    // The views draw the network's picture as the material's texture, and its mean where textures are off.
    let sg = c.s.graph.find("matSG").unwrap().unwrap();
    let d = aztek_scene::shading::draw_material(&mut c.s.graph, sg, None, None).unwrap();
    let map = d.color_map.expect("a map");
    let pic = map.baked.expect("worked out into a picture");
    assert_eq!((pic.width, pic.height), (256, 256));
    assert!((d.color[0] - 0.5).abs() < 0.02 && d.color[1] < 0.02, "half red, half black: {:?}", d.color);
    // An image file straight into the colour is still read as the file it is.
    c.mel("shadingNode -asTexture file -name \"img\"; connectAttr -f img.outColor mat.baseColor;");
    let d = aztek_scene::shading::draw_material(&mut c.s.graph, sg, None, None).unwrap();
    assert!(d.color_map.is_some_and(|m| m.baked.is_none()));
    let _: Value = json!(null);
    let _ = std::fs::remove_dir_all(&dir);
}
