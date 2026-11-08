//! The renderer from the scene: translation of materials, lights, cameras and settings, render commands, files.

use std::sync::Arc;

use aztek_scene::harold::{self, Request, Translator};
use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn set(s: &mut Session, plug: &str, v: Value) {
    let mut args = vec![json!(plug)];
    match v {
        Value::Array(a) => args.extend(a),
        other => args.push(other),
    }
    run(s, "setAttr", json!({ "args": args }));
}

fn scene() -> Session {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    s
}

fn small(s: &mut Session, aa: i64) {
    set(s, "defaultResolution.width", json!(32));
    set(s, "defaultResolution.height", json!(32));
    set(s, "defaultArnoldRenderOptions.AASamples", json!(aa));
}

fn render(s: &mut Session, camera: &str) -> (aztek_render::film::Film, Vec<String>) {
    let mut tr = Translator::default();
    let (scene, _) = harold::translate(s, &mut tr, &Request { camera: Some(camera.into()), ..Default::default() }).unwrap();
    let w = scene.warnings.clone();
    let (film, _) = aztek_render::render::render(Arc::new(scene), 2);
    (film, w)
}

fn centre(film: &aztek_render::film::Film) -> glam::Vec4 {
    let px = film.pixels(0);
    let (w, h) = (film.width as usize, film.height as usize);
    let mut sum = glam::Vec4::ZERO;
    for y in h / 2 - 2..h / 2 + 2 {
        for x in w / 2 - 2..w / 2 + 2 {
            sum += px[y * w + x];
        }
    }
    sum / 16.0
}

/// A plane under a point light with no decay, seen from above: lambert1's diffuse times colour (0.8 x 0.5) times the
/// light, as the viewport lights it.
#[test]
fn lambert_under_a_point_light() {
    let mut s = scene();
    run(&mut s, "polyPlane", json!({"width": 40, "height": 40}));
    run(&mut s, "pointLight", json!({"position": [0.0, 2.0, 0.0]}));
    small(&mut s, 2);
    set(&mut s, "topShape.orthographicWidth", json!(0.2));
    set(&mut s, "defaultArnoldRenderOptions.GIDiffuseDepth", json!(0));
    let (film, _) = render(&mut s, "top");
    let c = centre(&film);
    let expect = 0.8 * 0.5 * 1.0;
    assert!((c.x - expect).abs() / expect < 0.03, "{} vs {expect}", c.x);
    assert!((c.w - 1.0).abs() < 1e-4, "covered: alpha {}", c.w);
}

/// Exposure doubles light per stop; the light's colour tints.
#[test]
fn light_exposure_and_colour() {
    let mut s = scene();
    run(&mut s, "polyPlane", json!({"width": 40, "height": 40}));
    run(&mut s, "pointLight", json!({"position": [0.0, 2.0, 0.0], "name": "key"}));
    small(&mut s, 1);
    set(&mut s, "topShape.orthographicWidth", json!(0.2));
    set(&mut s, "defaultArnoldRenderOptions.GIDiffuseDepth", json!(0));
    let (a, _) = render(&mut s, "top");
    set(&mut s, "keyShape.aiExposure", json!(1.0));
    set(&mut s, "keyShape.color", json!([1.0, 0.0, 0.0]));
    let (b, _) = render(&mut s, "top");
    let (ca, cb) = (centre(&a), centre(&b));
    assert!((cb.x / ca.x - 2.0).abs() < 0.05, "{} {}", ca.x, cb.x);
    assert!(cb.y.abs() < 1e-5);
}

/// Older materials become the standard surface, with what isn't honoured named.
#[test]
fn legacy_materials_translate_with_warnings() {
    let mut s = scene();
    let plane = run(&mut s, "polyPlane", json!({"width": 40, "height": 40}))[0].as_str().unwrap().to_string();
    run(&mut s, "pointLight", json!({"position": [0.0, 2.0, 0.0]}));
    small(&mut s, 1);
    set(&mut s, "topShape.orthographicWidth", json!(0.2));
    let b = run(&mut s, "shadingNode", json!({"args": ["blinn"], "asShader": true})).as_str().unwrap().to_string();
    run(&mut s, "sets", json!({"renderable": true, "noSurfaceShader": true, "empty": true, "name": "blinnSG"}));
    run(&mut s, "connectAttr", json!({"args": [format!("{b}.outColor"), "blinnSG.surfaceShader"], "force": true}));
    run(&mut s, "sets", json!({"args": [plane], "edit": true, "forceElement": "blinnSG"}));
    set(&mut s, &format!("{b}.translucence"), json!(0.3));
    set(&mut s, &format!("{b}.color"), json!([0.0, 1.0, 0.0]));
    let (film, warnings) = render(&mut s, "top");
    assert!(warnings.iter().any(|w| w.contains("translucence") && w.contains("not honoured")), "{warnings:?}");
    let c = centre(&film);
    assert!(c.y > c.x * 3.0, "green: {c}");
}

/// The standard surface's own attributes, and an aiStandardSurface's, reach the renderer.
#[test]
fn standard_surfaces_and_emission() {
    let mut s = scene();
    let plane = run(&mut s, "polyPlane", json!({"width": 40, "height": 40}))[0].as_str().unwrap().to_string();
    small(&mut s, 1);
    set(&mut s, "topShape.orthographicWidth", json!(0.2));
    let m = run(&mut s, "shadingNode", json!({"args": ["aiStandardSurface"], "asShader": true})).as_str().unwrap().to_string();
    assert_eq!(m, "aiStandardSurface1");
    run(&mut s, "sets", json!({"renderable": true, "noSurfaceShader": true, "empty": true, "name": "glowSG"}));
    run(&mut s, "connectAttr", json!({"args": [format!("{m}.outColor"), "glowSG.surfaceShader"], "force": true}));
    run(&mut s, "sets", json!({"args": [plane], "edit": true, "forceElement": "glowSG"}));
    set(&mut s, &format!("{m}.emission"), json!(2.0));
    set(&mut s, &format!("{m}.emission_color"), json!([0.5, 0.25, 1.0]));
    set(&mut s, &format!("{m}.base"), json!(0.0));
    set(&mut s, &format!("{m}.specular"), json!(0.0));
    set(&mut s, "defaultRenderGlobals.enableDefaultLight", json!(false));
    let (film, _) = render(&mut s, "top");
    let c = centre(&film);
    assert!((c.x - 1.0).abs() < 0.02 && (c.y - 0.5).abs() < 0.02 && (c.z - 2.0).abs() < 0.04, "{c}");
}

/// Render settings save and load with the scene, and the old renderer named in a file is a warning, not an error.
#[test]
fn settings_round_trip_through_a_file() {
    let dir = std::env::temp_dir().join(format!("harold_settings_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("a.ma");
    let mut s = scene();
    run(&mut s, "polyCube", json!({}));
    set(&mut s, "defaultArnoldRenderOptions.AASamples", json!(5));
    set(&mut s, "defaultArnoldRenderOptions.GIDiffuseDepth", json!(3));
    set(&mut s, "defaultResolution.width", json!(1280));
    set(&mut s, "defaultRenderGlobals.imageFilePrefix", json!("shots/<Scene>"));
    aztek_scene::ma_file::save_scene(&mut s, path.to_str().unwrap(), aztek_scene::ma_file::Format::of_path(path.to_str().unwrap())).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("createNode aiOptions -s -n \"defaultArnoldRenderOptions\";"), "{text}");
    let mut b = scene();
    aztek_scene::ma_file::open_scene(&mut b, path.to_str().unwrap()).unwrap();
    assert_eq!(run(&mut b, "getAttr", json!({"args": ["defaultArnoldRenderOptions.AASamples"]})), json!(5));
    assert_eq!(run(&mut b, "getAttr", json!({"args": ["defaultResolution.width"]})), json!(1280));
    let names = run(&mut b, "renderSettings", json!({"firstImageName": true}));
    assert_eq!(names, json!(["shots/a.exr"]));
    // A file from a scene set to another renderer.
    let old = text.replace("createNode aiOptions -s", "select -ne :defaultRenderGlobals;\n\tsetAttr \".ren\" -type \"string\" \"mentalRay\";\ncreateNode aiOptions -s");
    std::fs::write(dir.join("b.ma"), old).unwrap();
    let mut c = scene();
    aztek_scene::ma_file::open_scene(&mut c, dir.join("b.ma").to_str().unwrap()).unwrap();
    let mut w = Vec::new();
    aztek_scene::harold::settings::read(&mut c.graph, &mut w);
    assert!(w.iter().any(|x| x.contains("mentalRay")), "{w:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// hrender with a sequence writes a frame each, numbered, in the images folder.
#[test]
fn hrender_sequence_writes_frames() {
    let dir = std::env::temp_dir().join(format!("harold_seq_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = scene();
    run(&mut s, "polySphere", json!({}));
    run(&mut s, "pointLight", json!({"position": [3.0, 4.0, 5.0]}));
    small(&mut s, 1);
    set(&mut s, "defaultRenderGlobals.imageFilePrefix", json!(format!("{}/shot", dir.display())));
    set(&mut s, "defaultArnoldDriver.ai_translator", json!("png"));
    let files = run(&mut s, "hrender", json!({"seq": "1 2", "camera": "persp"}));
    let files: Vec<String> = files.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert!(files.iter().any(|f| f.ends_with("shot.0001.png")), "{files:?}");
    assert!(files.iter().any(|f| f.ends_with("shot.0002.png")), "{files:?}");
    for f in &files {
        assert!(std::path::Path::new(f).exists(), "{f}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A second translation of an unchanged scene reuses every mesh; moving points refits.
#[test]
fn translation_reuses_meshes() {
    let mut s = scene();
    run(&mut s, "polySphere", json!({}));
    run(&mut s, "polyCube", json!({}));
    let mut tr = Translator::default();
    harold::translate(&mut s, &mut tr, &Request::default()).unwrap();
    assert_eq!(tr.built, 2);
    harold::translate(&mut s, &mut tr, &Request::default()).unwrap();
    assert_eq!((tr.built, tr.reused), (0, 2));
    run(&mut s, "move", json!({"args": [0.0, 1.0, 0.0, "pCube1.vtx[0]"], "relative": true}));
    harold::translate(&mut s, &mut tr, &Request::default()).unwrap();
    assert_eq!(tr.built + tr.refit, 1, "built {} refit {}", tr.built, tr.refit);
}

/// A glowing surface is not a light of the scene's: the default light still comes on (Lou's eye highlights).
#[test]
fn glowing_surface_keeps_the_default_light() {
    let mut s = scene();
    run(&mut s, "polyPlane", json!({"width": 40, "height": 40}));
    let glow = run(&mut s, "polySphere", json!({"radius": 0.1}))[0].as_str().unwrap().to_string();
    run(&mut s, "move", json!({"args": [5.0, 1.0, 5.0, glow.clone()]}));
    let m = run(&mut s, "shadingNode", json!({"args": ["lambert"], "asShader": true})).as_str().unwrap().to_string();
    run(&mut s, "sets", json!({"renderable": true, "noSurfaceShader": true, "empty": true, "name": "glowSG"}));
    run(&mut s, "connectAttr", json!({"args": [format!("{m}.outColor"), "glowSG.surfaceShader"], "force": true}));
    run(&mut s, "sets", json!({"args": [glow], "edit": true, "forceElement": "glowSG"}));
    set(&mut s, &format!("{m}.incandescence"), json!([0.5, 0.5, 0.5]));
    let mut tr = Translator::default();
    let (scene, _) = harold::translate(&mut s, &mut tr, &Request::default()).unwrap();
    assert_eq!(scene.lights.len(), 6, "the default lighting's five and the glow");
    assert!(scene.warnings.iter().any(|w| w.contains("default light")));
}

/// A control's attribute driving a material is read as its value, not as a texture (Lou's GeoUnderHide).
#[test]
fn control_attribute_into_a_material_is_its_value() {
    let mut s = scene();
    let plane = run(&mut s, "polyPlane", json!({"width": 40, "height": 40}))[0].as_str().unwrap().to_string();
    run(&mut s, "pointLight", json!({"position": [0.0, 2.0, 0.0]}));
    small(&mut s, 1);
    set(&mut s, "topShape.orthographicWidth", json!(0.2));
    let ctl = run(&mut s, "createNode", json!({"args": ["transform"], "name": "Main"})).as_str().unwrap().to_string();
    run(&mut s, "addAttr", json!({"args": [ctl.clone()], "longName": "hide", "attributeType": "double", "defaultValue": 1.0}));
    run(&mut s, "connectAttr", json!({"args": ["Main.hide", "lambert1.transparencyR"]}));
    run(&mut s, "connectAttr", json!({"args": ["Main.hide", "lambert1.transparencyG"]}));
    run(&mut s, "connectAttr", json!({"args": ["Main.hide", "lambert1.transparencyB"]}));
    let _ = plane;
    let (film, warnings) = render(&mut s, "top");
    assert!(!warnings.iter().any(|w| w.contains("Main")), "{warnings:?}");
    // Fully transparent: nothing covers the pixel.
    assert!(centre(&film).w < 0.01, "{}", centre(&film));
}

/// A displacement shader on a shading group moves the surface along its normal at render, scaled.
#[test]
fn displacement_moves_points() {
    let mut s = scene();
    let plane = run(&mut s, "polyPlane", json!({"width": 2, "height": 2, "subdivisionsX": 4, "subdivisionsY": 4}))[0].as_str().unwrap().to_string();
    let m = run(&mut s, "shadingNode", json!({"args": ["lambert"], "asShader": true})).as_str().unwrap().to_string();
    run(&mut s, "sets", json!({"renderable": true, "noSurfaceShader": true, "empty": true, "name": "dispSG"}));
    run(&mut s, "connectAttr", json!({"args": [format!("{m}.outColor"), "dispSG.surfaceShader"], "force": true}));
    run(&mut s, "sets", json!({"args": [plane], "edit": true, "forceElement": "dispSG"}));
    let d = run(&mut s, "shadingNode", json!({"args": ["displacementShader"], "asShader": true})).as_str().unwrap().to_string();
    set(&mut s, &format!("{d}.displacement"), json!(0.5));
    set(&mut s, &format!("{d}.scale"), json!(0.4));
    run(&mut s, "connectAttr", json!({"args": [format!("{d}.displacement"), "dispSG.displacementShader"], "force": true}));
    let mut tr = Translator::default();
    let (scene, _) = harold::translate(&mut s, &mut tr, &Request::default()).unwrap();
    let mesh = &scene.geo.meshes[scene.geo.instances[0].mesh as usize];
    assert!(mesh.keys[0].iter().all(|p| (p.y - 0.2).abs() < 1e-4), "{:?}", &mesh.keys[0][..3]);
    set(&mut s, "defaultArnoldRenderOptions.ignoreDisplacement", json!(true));
    let (scene, _) = harold::translate(&mut s, &mut tr, &Request::default()).unwrap();
    let mesh = &scene.geo.meshes[scene.geo.instances[0].mesh as usize];
    assert!(mesh.keys[0].iter().all(|p| p.y.abs() < 1e-4));
}

/// Render attributes on meshes, cameras and lights save and load.
#[test]
fn render_attributes_round_trip() {
    let dir = std::env::temp_dir().join(format!("harold_attrs_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("a.ma");
    let mut s = scene();
    run(&mut s, "polySphere", json!({"name": "ball"}));
    set(&mut s, "ballShape.aiSubdivType", json!(1));
    set(&mut s, "ballShape.aiSubdivIterations", json!(2));
    set(&mut s, "ballShape.primaryVisibility", json!(false));
    run(&mut s, "camera", json!({"name": "shot"}));
    set(&mut s, "shotShape.fStop", json!(2.8));
    set(&mut s, "shotShape.depthOfField", json!(true));
    run(&mut s, "pointLight", json!({"name": "key"}));
    set(&mut s, "keyShape.aiExposure", json!(3.5));
    set(&mut s, "keyShape.aiAov", json!("rim"));
    aztek_scene::ma_file::save_scene(&mut s, path.to_str().unwrap(), aztek_scene::ma_file::Format::of_path(path.to_str().unwrap())).unwrap();
    let mut b = scene();
    aztek_scene::ma_file::open_scene(&mut b, path.to_str().unwrap()).unwrap();
    let get = |s: &mut Session, p: &str| run(s, "getAttr", json!({"args": [p]}));
    assert_eq!(get(&mut b, "ballShape.aiSubdivType"), json!(1));
    assert_eq!(get(&mut b, "ballShape.aiSubdivIterations"), json!(2));
    assert_eq!(get(&mut b, "ballShape.primaryVisibility"), json!(false));
    assert_eq!(get(&mut b, "shotShape.fStop"), json!(2.8));
    assert_eq!(get(&mut b, "shotShape.depthOfField"), json!(true));
    assert_eq!(get(&mut b, "keyShape.aiExposure"), json!(3.5));
    assert_eq!(get(&mut b, "keyShape.aiAov"), json!("rim"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Catmull-Clark at render: Arnold's subdivision settings, or smooth preview's.
#[test]
fn subdivision_at_render() {
    let mut s = scene();
    run(&mut s, "polyCube", json!({"name": "box"}));
    let tris = |s: &mut Session| {
        let mut tr = Translator::default();
        let (scene, _) = harold::translate(s, &mut tr, &Request::default()).unwrap();
        scene.geo.meshes[scene.geo.instances[0].mesh as usize].tris.len()
    };
    assert_eq!(tris(&mut s), 12);
    set(&mut s, "boxShape.aiSubdivType", json!(1));
    set(&mut s, "boxShape.aiSubdivIterations", json!(2));
    assert_eq!(tris(&mut s), 6 * 16 * 2);
}

/// A light whose link to a shape is broken doesn't light it.
#[test]
fn light_linking_is_honoured() {
    let mut s = scene();
    let plane = run(&mut s, "polyPlane", json!({"width": 40, "height": 40}))[0].as_str().unwrap().to_string();
    run(&mut s, "pointLight", json!({"position": [0.0, 2.0, 0.0], "name": "key"}));
    small(&mut s, 1);
    set(&mut s, "topShape.orthographicWidth", json!(0.2));
    set(&mut s, "defaultArnoldRenderOptions.GIDiffuseDepth", json!(0));
    let (lit, _) = render(&mut s, "top");
    assert!(centre(&lit).x > 0.01);
    run(&mut s, "lightlink", json!({"b": true, "light": "key", "object": plane}));
    let (dark, _) = render(&mut s, "top");
    assert!(centre(&dark).x < 1e-5, "{}", centre(&dark));
}

/// Ambient colour adds the surface's colour times it, unlit, as the viewport adds it: with no lights at all (and the
/// default lighting off) a lambert shows exactly that.
#[test]
fn ambient_colour_as_the_viewport() {
    let mut s = scene();
    run(&mut s, "polyPlane", json!({"width": 40, "height": 40}));
    small(&mut s, 1);
    set(&mut s, "topShape.orthographicWidth", json!(0.2));
    set(&mut s, "lambert1.color", json!([0.5, 0.6, 0.8]));
    set(&mut s, "lambert1.ambientColor", json!([0.5, 0.5, 0.25]));
    set(&mut s, "defaultRenderGlobals.enableDefaultLight", json!(false));
    let (film, warnings) = render(&mut s, "top");
    assert!(!warnings.iter().any(|w| w.contains("ambient")), "{warnings:?}");
    let c = centre(&film);
    assert!((c.x - 0.25).abs() < 1e-3 && (c.y - 0.3).abs() < 1e-3 && (c.z - 0.2).abs() < 1e-3, "{c}");
}
