//! Lights as typed in MEL, through a real Core with its default scene, and what the view's shader is given for them.

use aztek_host::lighting::{gpu_light, LightMode};
use aztek_host::Core;
use aztek_scene::lights;
use glam::{DMat4, DVec3};
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
fn the_commands_as_typed() {
    let mut c = Core::new();
    assert_eq!(mel(&mut c, "directionalLight -rgb 1 0 0 -i 0.5;"), "directionalLightShape1");
    assert_eq!(mel(&mut c, "directionalLight -q -i directionalLightShape1;"), 0.5);
    assert_eq!(mel(&mut c, "spotLight -ca 33 -p 2 -do 4;"), "spotLightShape1");
    assert_eq!(mel(&mut c, "getAttr spotLightShape1.dropoff;"), 4.0);
    assert_eq!(mel(&mut c, "shadingNode -asLight areaLight;"), "areaLight1");
    assert_eq!(mel(&mut c, "shadingNode -asLight volumeLight -n \"vol\";"), "vol");
    assert_eq!(mel(&mut c, "ls -lights;").as_array().unwrap().len(), 4);
    assert_eq!(mel(&mut c, "listLights;"), json!(["directionalLight1", "spotLight1", "areaLight1", "vol"]));
    assert_eq!(mel(&mut c, "sets -q defaultLightSet;"), json!(["directionalLight1", "spotLight1", "areaLight1", "vol"]));
    assert_eq!(mel(&mut c, "CreatePointLight;"), "pointLight1");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["pointLight1"]));
}

#[test]
fn the_create_menus_procedures_take_their_arguments() {
    let mut c = Core::new();
    assert_eq!(mel(&mut c, "defaultPointLight(2, 1,0.5,0.25, 0, 0, 0,0,0, 1);"), "pointLight1");
    assert_eq!(mel(&mut c, "getAttr pointLightShape1.intensity;"), 2.0);
    assert_eq!(mel(&mut c, "getAttr pointLightShape1.colorB;"), 0.25);
    assert_eq!(mel(&mut c, "defaultAmbientLight(1, 0.3, 1,1,1, \"0\", 0,0,0, \"1\");"), "ambientLight1");
    assert_eq!(mel(&mut c, "getAttr ambientLightShape1.ambientShade;"), 0.3);
    assert_eq!(mel(&mut c, "defaultSpotLight(1, 1,1,1, 0, 60, 3, 2, 0, 0,0,0, 1, 0);"), "spotLight1");
    assert_eq!(mel(&mut c, "getAttr spotLightShape1.coneAngle;"), 60.0);
    assert_eq!(mel(&mut c, "getAttr spotLightShape1.dropoff;"), 2.0);
    mel(&mut c, "defaultDirectionalLight(1, 1,1,1, \"0\", 0,0,0, 0);");
    mel(&mut c, "defaultAreaLight(1, 1,1,1, 0, 0, 0,0,0, 0);");
    mel(&mut c, "defaultVolumeLight(1, 1,1,1);");
    assert_eq!(mel(&mut c, "ls -lights;").as_array().unwrap().len(), 6);
}

#[test]
fn the_type_menu_swaps_the_shape_and_keeps_what_they_share() {
    let mut c = Core::new();
    mel(&mut c, "pointLight -i 3 -rgb 1 0 0;");
    assert_eq!(mel(&mut c, "aztekChangeLightType pointLight1 \"spotLight\";"), "pointLightShape1");
    assert_eq!(mel(&mut c, "nodeType pointLightShape1;"), "spotLight");
    assert_eq!(mel(&mut c, "getAttr pointLightShape1.intensity;"), 3.0);
    assert_eq!(mel(&mut c, "getAttr pointLightShape1.colorG;"), 0.0);
    assert_eq!(mel(&mut c, "getAttr pointLightShape1.coneAngle;"), 40.0);
    assert_eq!(mel(&mut c, "sets -q defaultLightSet;"), json!(["pointLight1"]));
    mel(&mut c, "undo;");
    assert_eq!(mel(&mut c, "nodeType pointLightShape1;"), "pointLight");
    assert_eq!(mel(&mut c, "aztekIlluminatesByDefault pointLight1;"), true);
    assert_eq!(mel(&mut c, "aztekIlluminatesByDefault -set false pointLight1;"), false);
    assert_eq!(mel(&mut c, "sets -q defaultLightSet;"), json!([]));
}

#[test]
fn light_modes_parse_as_the_flag_documents_them() {
    for (word, mode) in [("default", LightMode::Default), ("all", LightMode::All), ("selected", LightMode::Selected), ("active", LightMode::Selected), ("flat", LightMode::Flat), ("none", LightMode::None)] {
        assert_eq!(LightMode::parse(word), Some(mode));
    }
    assert_eq!(LightMode::parse("sideways"), None);
}

/// A light as the shader gets it, through a camera at the origin looking down -Z.
fn light(c: &mut Core, name: &str) -> aztek_scene::lights::SceneLight {
    let g = &mut c.s.graph;
    g.evaluate();
    let id = g.find(name).unwrap().unwrap();
    let sh = lights::shape_of(g, id).unwrap();
    lights::scene_light(g, sh).unwrap()
}

#[test]
fn what_the_shader_is_given() {
    let mut c = Core::new();
    let view = DMat4::IDENTITY;
    mel(&mut c, "spotLight -ca 60 -p 10 -do 3 -i 2 -rgb 1 0.5 0 -d 2 -pos 1 2 3 -n s;");
    let g = gpu_light(&light(&mut c, "s"), &view);
    assert_eq!(g.pos, [1.0, 2.0, 3.0, 3.0]);
    assert_eq!(g.dir, [0.0, 0.0, -1.0, 2.0], "shines down -Z with its decay");
    assert_eq!(&g.color, &[2.0, 1.0, 0.0, 3.0], "colour times intensity; diffuse and specular");
    // A positive penumbra widens outside the cone's half angle of 30.
    assert!((g.params[0] - 40f32.to_radians().cos()).abs() < 1e-5);
    assert!((g.params[1] - 30f32.to_radians().cos()).abs() < 1e-5);
    assert_eq!(g.params[2], 3.0);

    mel(&mut c, "setAttr sShape.penumbraAngle -10;");
    let g = gpu_light(&light(&mut c, "s"), &view);
    assert!((g.params[0] - 30f32.to_radians().cos()).abs() < 1e-5, "a negative one narrows inside it");
    assert!((g.params[1] - 20f32.to_radians().cos()).abs() < 1e-5);

    mel(&mut c, "setAttr sShape.emitDiffuse 0;");
    assert_eq!(gpu_light(&light(&mut c, "s"), &view).color[3], 2.0, "specular alone");
    mel(&mut c, "setAttr sShape.emitSpecular 0;");
    assert_eq!(gpu_light(&light(&mut c, "s"), &view).color[3], 0.0);

    mel(&mut c, "ambientLight -ambientShade 0.25 -n a;");
    let g = gpu_light(&light(&mut c, "a"), &view);
    assert_eq!(g.pos[3], 0.0);
    assert_eq!(g.params[0], 0.25);
    assert_eq!(g.color[3], 1.0, "an ambient light has no highlight");

    // An area light's half sizes follow its scale, and only with normalize off does its size change its strength.
    mel(&mut c, "shadingNode -asLight areaLight -n r; setAttr r.scaleX 3; setAttr r.scaleY 2; setAttr rShape.intensity 2;");
    let g = gpu_light(&light(&mut c, "r"), &view);
    assert_eq!(&g.a[..3], &[3.0, 0.0, 0.0]);
    assert_eq!(&g.b[..3], &[0.0, 2.0, 0.0]);
    assert_eq!(g.color[0], 2.0);
    mel(&mut c, "setAttr rShape.normalize 0;");
    assert_eq!(gpu_light(&light(&mut c, "r"), &view).color[0], 12.0);

    // A volume light's rows take view space to its unit shape, through the camera too.
    mel(&mut c, "shadingNode -asLight volumeLight -n v; move 10 0 0 v; setAttr v.scaleX 2;");
    let moved = DMat4::from_translation(DVec3::new(0.0, 0.0, -5.0));
    let g = gpu_light(&light(&mut c, "v"), &moved);
    let p = [12.0f32, 0.0, -5.0, 1.0];
    let dot = |r: [f32; 4]| r.iter().zip(&p).map(|(a, b)| a * b).sum::<f32>();
    assert!((dot(g.a) - 1.0).abs() < 1e-5 && dot(g.b).abs() < 1e-5 && dot(g.c).abs() < 1e-5, "{:?}", (dot(g.a), dot(g.b), dot(g.c)));
}

#[test]
fn links_become_a_shapes_mask() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n box; polySphere -n ball; pointLight -n one; pointLight -n two; pointLight -exc true -n three;");
    mel(&mut c, "lightlink -b -l one -o box; lightlink -m -l three -o ball;");
    let g = &c.s.graph;
    let links = aztek_scene::light_cmds::LinkTable::read(g);
    let id = |n: &str| lights::shape_of(g, g.find(n).unwrap().unwrap()).unwrap();
    let used = [id("one"), id("two"), id("three")];
    let shape = |n: &str| aztek_scene::dag::shapes(g, g.find(n).unwrap().unwrap(), false)[0];
    assert_eq!(aztek_host::lighting::mask_for(g, &links, &used, shape("box")), 0b010);
    assert_eq!(aztek_host::lighting::mask_for(g, &links, &used, shape("ball")), 0b111);
}

#[test]
fn the_channel_box_labels_a_colour_r_g_b() {
    let mut c = Core::new();
    mel(&mut c, "pointLight -i 2;");
    let text = aztek_host::query::channel_box(&c).to_string();
    for label in ["Color R", "Color G", "Color B", "Intensity"] {
        assert!(text.contains(label), "{label} in {text}");
    }
    assert!(!text.contains("Color X"), "{text}");
}
