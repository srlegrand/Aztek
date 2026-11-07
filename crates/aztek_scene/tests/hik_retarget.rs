//! HumanIK's workflow with numbers: slots, definitions, sources, the retarget between bodies of different sizes and joint counts,
//! bakes, and a character in the words of a file saved by the reference.

use aztek_scene::Session;
use glam::{DMat3, DMat4, DQuat, DVec3};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pos(s: &mut Session, j: &str) -> DVec3 {
    let v = run(s, "xform", json!({"args": [j], "query": true, "worldSpace": true, "translation": true}));
    DVec3::new(v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap())
}

fn world_q(s: &mut Session, j: &str) -> DQuat {
    let v = run(s, "xform", json!({"args": [j], "query": true, "worldSpace": true, "matrix": true}));
    let a: Vec<f64> = v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    let m = DMat4::from_cols_array(&a.try_into().unwrap());
    DQuat::from_mat3(&DMat3::from_mat4(m)).normalize()
}

fn near_v(a: DVec3, b: DVec3, tol: f64, what: &str) {
    assert!((a - b).length() <= tol, "{what}: {a:?} is not {b:?}");
}

fn turn_angle(a: DQuat, b: DQuat) -> f64 {
    (a.inverse() * b).to_axis_angle().1.abs().min((b.inverse() * a).to_axis_angle().1.abs()).to_degrees()
}

/// Source at 170, target twice the size, both named by hikCreateSkeleton.
fn two_bodies(s: &mut Session) {
    run(s, "hikCreateSkeleton", json!({"name": "Src", "character": true}));
    run(s, "hikCreateSkeleton", json!({"name": "Dst", "height": 340.0, "spineCount": 2, "character": true}));
}

fn key(s: &mut Session, plug: &str, t: f64, v: f64) {
    run(s, "setKeyframe", json!({"args": [plug], "time": t, "value": v}));
}

fn frame(s: &mut Session, t: f64) {
    run(s, "currentTime", json!({"args": [t]}));
}

#[test]
fn slot_numbers_are_the_published_ones() {
    use aztek_scene::hik_slots::{slot_index, slot_names, SLOT_COUNT};
    assert_eq!(SLOT_COUNT, 212);
    for (name, id) in [("Reference", 0), ("Hips", 1), ("RightFoot", 7), ("Spine", 8), ("LeftArm", 9), ("Head", 15), ("LeftShoulder", 18), ("Neck", 20), ("LeftFingerBase", 21), ("Spine1", 23), ("Spine5", 27), ("Neck1", 32), ("LeftUpLegRoll", 41), ("HipsTranslation", 49), ("LeftHandThumb1", 50), ("LeftHandIndex1", 54), ("RightHandThumb1", 74), ("LeftFootThumb1", 98), ("LeftInHandThumb", 146), ("LeftShoulderExtra", 170), ("RightShoulderExtra", 171), ("LeafLeftUpLegRoll1", 172), ("LeafRightUpLegRoll5", 206), ("LeafRightForeArmRoll5", 211)] {
        assert_eq!(slot_index(name), Some(id), "{name}");
    }
    assert_eq!(slot_names().len(), 212);
    let mut s = Session::new();
    assert_eq!(run(&mut s, "GetHIKNodeName", json!({"args": [15]})), json!("Head"));
    assert_eq!(run(&mut s, "hikGetNodeCount", json!({})), json!(212));
}

#[test]
fn a_rotation_follows_at_any_size() {
    let mut s = Session::new();
    two_bodies(&mut s);
    assert!(run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]})).as_i64().unwrap() > 10);
    let rest = [world_q(&mut s, "Dst_LeftArm"), world_q(&mut s, "Dst_Head"), world_q(&mut s, "Dst_RightForeArm")];
    let src_rest = [world_q(&mut s, "Src_LeftArm"), world_q(&mut s, "Src_Head"), world_q(&mut s, "Src_RightForeArm")];
    key(&mut s, "Src_LeftArm.rz", 1.0, 0.0);
    key(&mut s, "Src_LeftArm.rz", 10.0, 50.0);
    key(&mut s, "Src_Spine.rx", 10.0, 20.0);
    key(&mut s, "Src_Head.ry", 10.0, 35.0);
    key(&mut s, "Src_RightForeArm.ry", 10.0, -40.0);
    frame(&mut s, 10.0);
    // Each mapped joint turns in the world as its source does, measured from their T-poses.
    for (i, (a, b)) in [("Src_LeftArm", "Dst_LeftArm"), ("Src_Head", "Dst_Head"), ("Src_RightForeArm", "Dst_RightForeArm")].iter().enumerate() {
        let want = world_q(&mut s, a) * src_rest[i].inverse() * rest[i];
        let got = world_q(&mut s, b);
        assert!(turn_angle(want, got) < 1e-3, "{b}: {} degrees off", turn_angle(want, got));
    }
    assert_eq!(run(&mut s, "hikSetCurrentCharacter", json!({"args": ["Dst"]})), json!("Dst"));
    assert_eq!(run(&mut s, "hikGetCurrentSource", json!({})), json!("Src"));
    assert_eq!(run(&mut s, "mayaHIKgetInputCharacter", json!({"args": ["Dst"]})), json!("Src"));
}

#[test]
fn stride_and_feet_scale_with_the_body() {
    let mut s = Session::new();
    two_bodies(&mut s);
    let (hip0, foot0, knee0) = (pos(&mut s, "Dst_Hips"), pos(&mut s, "Dst_LeftFoot"), pos(&mut s, "Dst_LeftLeg"));
    let side_foot0 = pos(&mut s, "Dst_RightFoot");
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    near_v(pos(&mut s, "Dst_LeftFoot"), foot0, 1e-6, "the T-pose stands");
    near_v(pos(&mut s, "Dst_LeftLeg"), knee0, 1e-6, "the knee too");
    // The source walks: hips forward and down, left foot forward on the floor, right knee bent by the pose.
    key(&mut s, "Src_Hips.tz", 1.0, 0.0);
    key(&mut s, "Src_Hips.tz", 10.0, 20.0);
    key(&mut s, "Src_Hips.ty", 10.0, 91.0);
    key(&mut s, "Src_LeftUpLeg.rx", 10.0, -30.0);
    key(&mut s, "Src_LeftLeg.rx", 10.0, 30.0);
    frame(&mut s, 10.0);
    let (src_hip, src_foot, src_foot0) = (pos(&mut s, "Src_Hips"), pos(&mut s, "Src_LeftFoot"), DVec3::new(9.0, 8.0, 0.0));
    let (hip, foot) = (pos(&mut s, "Dst_Hips"), pos(&mut s, "Dst_LeftFoot"));
    // Twice the size: twice the stride, and the ankle goes where the source's scaled offset says, off its own T-pose.
    near_v(hip, hip0 + (src_hip - DVec3::new(0.0, 95.0, 0.0)) * 2.0, 1e-6, "hips scale");
    near_v(foot, foot0 + (src_foot - src_foot0) * 2.0, 1e-5, "foot scales and reaches");
    // Bone lengths hold.
    let len = |a: DVec3, b: DVec3| (a - b).length();
    let (up, knee) = (pos(&mut s, "Dst_LeftUpLeg"), pos(&mut s, "Dst_LeftLeg"));
    assert!((len(up, knee) - 2.0 * (40.0f64.powi(2) + 1.0).sqrt()).abs() < 1e-6, "thigh {}", len(up, knee));
    // The knee stays forward of the line from hip to ankle: no flip.
    let line = (foot - up).normalize();
    let bend = (knee - up) - line * (knee - up).dot(line);
    assert!(bend.z > 0.0, "{bend:?}");
    let _ = side_foot0;
}

#[test]
fn a_planted_foot_stays_planted() {
    let mut s = Session::new();
    two_bodies(&mut s);
    // The source's right foot held on the floor by IK while its hips travel and drop.
    run(&mut s, "ikHandle", json!({"startJoint": "Src_RightUpLeg", "endEffector": "Src_RightFoot", "solver": "ikRPsolver", "name": "plant"}));
    let foot0 = pos(&mut s, "Dst_RightFoot");
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    for (t, z, y) in [(1.0, 0.0, 95.0), (5.0, 6.0, 90.0), (9.0, -6.0, 88.0)] {
        key(&mut s, "Src_Hips.tz", t, z);
        key(&mut s, "Src_Hips.ty", t, y);
    }
    let mut last_knee_z = None;
    for t in [1.0, 3.0, 5.0, 7.0, 9.0] {
        frame(&mut s, t);
        near_v(pos(&mut s, "Dst_RightFoot"), foot0, 1e-2, "planted at any leg length");
        // The knee goes on bending the same way all along: no flip.
        let (up, knee, foot) = (pos(&mut s, "Dst_RightUpLeg"), pos(&mut s, "Dst_RightLeg"), pos(&mut s, "Dst_RightFoot"));
        let line = (foot - up).normalize();
        let bend = (knee - up) - line * (knee - up).dot(line);
        assert!(bend.z > 0.0, "frame {t}: {bend:?}");
        if let Some(k) = last_knee_z {
            let k: f64 = k;
            assert!((bend.z - k).abs() < 80.0, "frame {t} jumps");
        }
        last_knee_z = Some(bend.z);
    }
}

#[test]
fn different_joint_counts_and_no_source() {
    let mut s = Session::new();
    run(&mut s, "hikCreateSkeleton", json!({"name": "Src", "spineCount": 4, "character": true}));
    run(&mut s, "hikCreateSkeleton", json!({"name": "Dst", "spineCount": 2, "character": true}));
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    key(&mut s, "Src_Spine.rx", 1.0, 10.0);
    key(&mut s, "Src_Spine1.rx", 1.0, 10.0);
    key(&mut s, "Src_Spine2.rx", 1.0, 10.0);
    key(&mut s, "Src_Spine3.rx", 1.0, 10.0);
    frame(&mut s, 1.0);
    // Dst's upper spine is at the top of the source's chain: 40 degrees of bend, the lower at half.
    let upper = world_q(&mut s, "Dst_Spine1");
    let want = world_q(&mut s, "Src_Spine3");
    assert!(turn_angle(upper, want) < 1e-3, "{}", turn_angle(upper, want));
    let lower = world_q(&mut s, "Dst_Spine");
    let want = world_q(&mut s, "Src_Spine1");
    assert!(turn_angle(lower, want) < 1e-3, "{}", turn_angle(lower, want));
    // A joint the source lacks follows its parent. Take the source's left hand out of its definition.
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "None"]}));
    run(&mut s, "hikCharacterLock", json!({"args": ["Src", 0]}));
    run(&mut s, "setCharacterObject", json!({"args": ["Src_LeftHand", "Src", "LeftHand", 1]}));
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    key(&mut s, "Src_LeftForeArm.rz", 2.0, 30.0);
    frame(&mut s, 2.0);
    let (a, b) = (world_q(&mut s, "Dst_LeftHand"), world_q(&mut s, "Dst_LeftForeArm"));
    let rest_rel = 0.0; // Both chains are built with the same T-pose relation at identity-oriented joints.
    assert!((turn_angle(a, b) - rest_rel).abs() < 3.0, "{}", turn_angle(a, b));
}

#[test]
fn retarget_specific_attributes() {
    // Mirror: the left arm's turn goes on the right, mirrored.
    let mut s = Session::new();
    two_bodies(&mut s);
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    let props = run(&mut s, "listConnections", json!({"args": ["Dst.propertyState"], "source": true, "destination": false}))[0].as_str().unwrap().to_string();
    let rest = pos(&mut s, "Dst_RightHand");
    key(&mut s, "Src_LeftArm.rz", 1.0, 40.0);
    run(&mut s, "setAttr", json!({"args": [format!("{props}.Mirror"), 1.0]}));
    frame(&mut s, 1.0);
    let h = pos(&mut s, "Dst_RightHand");
    assert!(h.y > rest.y + 50.0, "{h:?}");
    assert!((pos(&mut s, "Dst_LeftHand").y - pos(&mut s, "Dst_LeftForeArm").y).abs() < 10.0);
    // Action Space User at 50: half the movement. Off: the source's size. Match Source: the same spots.
    let mut s = Session::new();
    two_bodies(&mut s);
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    let props = run(&mut s, "listConnections", json!({"args": ["Dst.propertyState"], "source": true, "destination": false}))[0].as_str().unwrap().to_string();
    key(&mut s, "Src_Hips.tz", 1.0, 10.0);
    frame(&mut s, 1.0);
    let z = |s: &mut Session| pos(s, "Dst_Hips").z;
    assert!((z(&mut s) - 20.0).abs() < 1e-6, "auto");
    run(&mut s, "setAttr", json!({"args": [format!("{props}.ScaleCompensationMode"), 2.0]}));
    run(&mut s, "setAttr", json!({"args": [format!("{props}.ScaleCompensation"), 50.0]}));
    assert!((z(&mut s) - 5.0).abs() < 1e-6, "user");
    run(&mut s, "setAttr", json!({"args": [format!("{props}.ScaleCompensationMode"), 0.0]}));
    assert!((z(&mut s) - 10.0).abs() < 1e-6, "off");
    run(&mut s, "setAttr", json!({"args": [format!("{props}.MatchSource"), 1.0]}));
    let src = pos(&mut s, "Src_Hips");
    near_v(pos(&mut s, "Dst_Hips"), src, 1e-6, "match source");
}

#[test]
fn sources_and_bakes() {
    let mut s = Session::new();
    two_bodies(&mut s);
    run(&mut s, "hikSetCurrentCharacter", json!({"args": ["Dst"]}));
    // Stance holds the T-pose whatever the joints held.
    run(&mut s, "setAttr", json!({"args": ["Dst_LeftArm.rz", 33.0]}));
    let posed = pos(&mut s, "Dst_LeftHand");
    run(&mut s, "hikCharacterLock", json!({"args": ["Dst", 0]}));
    run(&mut s, "setAttr", json!({"args": ["Dst_LeftArm.rz", 0.0]}));
    run(&mut s, "hikCharacterLock", json!({"args": ["Dst", 1, 1]}));
    run(&mut s, "setAttr", json!({"args": ["Dst_LeftArm.rz", 33.0]}));
    run(&mut s, "hikSetCurrentSource", json!({"args": ["Stance"]}));
    assert_eq!(run(&mut s, "hikGetCurrentSource", json!({})), json!("Stance"));
    assert!((pos(&mut s, "Dst_LeftHand") - posed).length() > 50.0, "stance moved it");
    // None gives back what the joints held.
    run(&mut s, "hikSetCurrentSource", json!({"args": ["None"]}));
    near_v(pos(&mut s, "Dst_LeftHand"), posed, 1e-6, "back");
    run(&mut s, "setAttr", json!({"args": ["Dst_LeftArm.rz", 0.0]}));
    // A retarget baked stays when the source goes.
    key(&mut s, "Src_LeftArm.rz", 1.0, 0.0);
    key(&mut s, "Src_LeftArm.rz", 5.0, 40.0);
    key(&mut s, "Src_Hips.tz", 1.0, 0.0);
    key(&mut s, "Src_Hips.tz", 5.0, 20.0);
    run(&mut s, "hikSetCurrentSource", json!({"args": ["Src"]}));
    frame(&mut s, 5.0);
    let (hand, hip) = (pos(&mut s, "Dst_LeftHand"), pos(&mut s, "Dst_Hips"));
    assert!(run(&mut s, "hikBakeToSkeleton", json!({"args": ["Dst"], "time": "1:5"})).as_i64().unwrap() > 50);
    assert_eq!(run(&mut s, "hikGetCurrentSource", json!({})), json!("None"));
    run(&mut s, "cutKey", json!({"args": ["Src_LeftArm.rz"], "clear": true}));
    run(&mut s, "cutKey", json!({"args": ["Src_Hips.tz"], "clear": true}));
    frame(&mut s, 5.0);
    near_v(pos(&mut s, "Dst_LeftHand"), hand, 1e-3, "baked hand");
    near_v(pos(&mut s, "Dst_Hips"), hip, 1e-3, "baked hips");
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["Dst_LeftArm.rz"], "query": true, "queried": ["keyframeCount"]})), json!(5));
}

#[test]
fn bake_to_control_rig_keys_the_controls() {
    let mut s = Session::new();
    two_bodies(&mut s);
    run(&mut s, "hikSetCurrentCharacter", json!({"args": ["Dst"]}));
    key(&mut s, "Src_Hips.tz", 1.0, 0.0);
    key(&mut s, "Src_Hips.tz", 3.0, 10.0);
    key(&mut s, "Src_Spine.rx", 3.0, 15.0);
    key(&mut s, "Src_LeftArm.rz", 3.0, 30.0);
    run(&mut s, "hikSetCurrentSource", json!({"args": ["Src"]}));
    frame(&mut s, 3.0);
    let (hips, head, hand, foot) = (pos(&mut s, "Dst_Hips"), pos(&mut s, "Dst_Head"), pos(&mut s, "Dst_LeftHand"), pos(&mut s, "Dst_LeftFoot"));
    run(&mut s, "hikBakeToControlRig", json!({"args": ["Dst"], "time": "1:3"}));
    assert_eq!(run(&mut s, "hikGetCurrentSource", json!({})), json!("Control Rig"));
    frame(&mut s, 3.0);
    near_v(pos(&mut s, "Dst_Hips"), hips, 1e-2, "hips");
    near_v(pos(&mut s, "Dst_Head"), head, 1.0, "head");
    near_v(pos(&mut s, "Dst_LeftHand"), hand, 1e-1, "hand");
    near_v(pos(&mut s, "Dst_LeftFoot"), foot, 1e-1, "foot");
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["Dst_Ctrl_Hips.tz"], "query": true, "queried": ["keyframeCount"]})), json!(3));
}

#[test]
fn definition_checks_templates_and_name_matching() {
    use aztek_scene::hik::slot_for_name as f;
    for (name, want) in [("mixamorig:LeftForeArm", "LeftForeArm"), ("mixamorig:RightHandIndex2", "RightHandIndex2"), ("mixamorig:Spine1", "Spine1"), ("pelvis", "Hips"), ("thigh_l", "LeftUpLeg"), ("calf_r", "RightLeg"), ("hand_l", "LeftHand"), ("upperarm_r", "RightArm"), ("index_02_l", "LeftHandIndex2"), ("L_Thumb1", "LeftHandThumb1")] {
        assert_eq!(f(name).as_deref(), Some(want), "{name}");
    }
    let mut s = Session::new();
    run(&mut s, "hikCreateCharacter", json!({"args": ["Empty"]}));
    let st = run(&mut s, "hikDefinitionStatus", json!({"args": ["Empty"]}));
    assert_eq!(st["valid"], json!(false));
    assert_eq!(st["status"]["Hips"], json!("red"));
    assert_eq!(st["summary"], json!("Character definition: Invalid"));
    assert!(s.run("hikCharacterLock", json!({"args": ["Empty", 1, 1]})).is_err(), "won't validate");

    // A Mixamo-like skeleton by hand, with a prefix, in T-pose.
    let bones: &[(&str, Option<&str>, [f64; 3])] = &[
        ("Hips", None, [0.0, 100.0, 0.0]), ("Spine", Some("Hips"), [0.0, 110.0, 0.0]), ("Spine1", Some("Spine"), [0.0, 120.0, 0.0]), ("Spine2", Some("Spine1"), [0.0, 130.0, 0.0]), ("Neck", Some("Spine2"), [0.0, 145.0, 0.0]), ("Head", Some("Neck"), [0.0, 155.0, 0.0]),
        ("LeftShoulder", Some("Spine2"), [5.0, 140.0, 0.0]), ("LeftArm", Some("LeftShoulder"), [15.0, 140.0, 0.0]), ("LeftForeArm", Some("LeftArm"), [40.0, 140.0, 0.0]), ("LeftHand", Some("LeftForeArm"), [65.0, 140.0, 0.0]),
        ("RightShoulder", Some("Spine2"), [-5.0, 140.0, 0.0]), ("RightArm", Some("RightShoulder"), [-15.0, 140.0, 0.0]), ("RightForeArm", Some("RightArm"), [-40.0, 140.0, 0.0]), ("RightHand", Some("RightForeArm"), [-65.0, 140.0, 0.0]),
        ("LeftUpLeg", Some("Hips"), [9.0, 95.0, 0.0]), ("LeftLeg", Some("LeftUpLeg"), [9.0, 52.0, 1.0]), ("LeftFoot", Some("LeftLeg"), [9.0, 8.0, 0.0]), ("LeftToeBase", Some("LeftFoot"), [9.0, 1.0, 12.0]),
        ("RightUpLeg", Some("Hips"), [-9.0, 95.0, 0.0]), ("RightLeg", Some("RightUpLeg"), [-9.0, 52.0, 1.0]), ("RightFoot", Some("RightLeg"), [-9.0, 8.0, 0.0]), ("RightToeBase", Some("RightFoot"), [-9.0, 1.0, 12.0]),
    ];
    let mut s = Session::new();
    for (n, up, p) in bones {
        match up {
            Some(x) => run(&mut s, "select", json!({"args": [format!("mixamorig_{x}")]})),
            None => run(&mut s, "select", json!({"clear": true})),
        };
        run(&mut s, "joint", json!({"position": p, "name": format!("mixamorig_{n}")}));
    }
    run(&mut s, "hikCreateCharacter", json!({"args": ["Mix"]}));
    assert_eq!(run(&mut s, "hikNameMatch", json!({"args": ["mixamorig_Hips"], "character": "Mix", "prefix": "mixamorig_"})), json!(22));
    assert_eq!(run(&mut s, "hikGetSkNode", json!({"args": ["Mix", "Spine2"]})), json!("mixamorig_Spine2"));
    let st = run(&mut s, "hikDefinitionStatus", json!({"args": ["Mix"]}));
    assert_eq!(st["summary"], json!("Character definition: Valid"), "{st}");
    assert_eq!(st["status"]["LeftArm"], json!("green"));
    // Standing away from the origin changes nothing.
    run(&mut s, "setAttr", json!({"args": ["mixamorig_Hips.tx", -70.0]}));
    let st = run(&mut s, "hikDefinitionStatus", json!({"args": ["Mix"]}));
    assert_eq!(st["summary"], json!("Character definition: Valid"), "{st}");
    run(&mut s, "setAttr", json!({"args": ["mixamorig_Hips.tx", 0.0]}));
    // A bent arm is yellow, and says why.
    run(&mut s, "setAttr", json!({"args": ["mixamorig_LeftArm.rz", 40.0]}));
    let st = run(&mut s, "hikDefinitionStatus", json!({"args": ["Mix"]}));
    assert_eq!(st["status"]["LeftForeArm"], json!("yellow"));
    assert!(st["messages"][0].as_str().unwrap().contains("along the X axis"), "{st}");
    run(&mut s, "setAttr", json!({"args": ["mixamorig_LeftArm.rz", 0.0]}));
    run(&mut s, "hikCharacterLock", json!({"args": ["Mix", 1, 1]}));
    assert_eq!(run(&mut s, "hikIsDefinitionLocked", json!({"args": ["Mix"]})), json!(true));

    // A template saved and read onto a new character.
    let path = std::env::temp_dir().join(format!("aztek_hik_{}.xml", std::process::id()));
    let path = path.to_string_lossy().to_string();
    run(&mut s, "hikSaveTemplate", json!({"args": [path, "Mix"]}));
    run(&mut s, "hikCreateCharacter", json!({"args": ["Two"]}));
    assert_eq!(run(&mut s, "hikLoadTemplate", json!({"args": [path, "Two"]})), json!(22));
    assert_eq!(run(&mut s, "hikGetSkNode", json!({"args": ["Two", 15]})), json!("mixamorig_Head"));
    let _ = std::fs::remove_file(&path);

    // Mirror matching fills the right from the left.
    run(&mut s, "hikCreateCharacter", json!({"args": ["Three"]}));
    run(&mut s, "setCharacterObject", json!({"args": ["mixamorig_LeftArm", "Three", 9, 0]}));
    assert_eq!(run(&mut s, "hikMirrorMatching", json!({"args": ["Three"]})), json!(1));
    assert_eq!(run(&mut s, "hikGetSkNode", json!({"args": ["Three", "RightArm"]})), json!("mixamorig_RightArm"));
}

/// In the words of a scene saved by the reference: a joint's .ch to the slot, the T-pose by axis, the nodes in their places.
const REF_TEXT: &str = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode joint -n "pep_Hips";
	setAttr ".t" -type "double3" 0 90 0 ;
createNode joint -n "pep_Head" -p "pep_Hips";
	setAttr ".t" -type "double3" 0 60 0 ;
createNode HIKCharacterNode -n "pep_character";
	setAttr ".InputCharacterizationLock" yes;
	setAttr ".ReferenceMinRLimitx" -45;
	setAttr ".HipsTy" 90;
	setAttr ".HipsMinRLimitx" -45;
	setAttr ".HeadTy" 150;
createNode HIKProperty2State -n "HIKproperties1";
	setAttr ".lkr" 0.60000002384185791;
	setAttr ".FootBottomToAnkle" 6.13;
	setAttr ".LeftUpLegRollEx" 1;
createNode HIKSolverNode -n "HIKSolverNode1";
	setAttr ".ihi" 0;
	setAttr ".InputStance" yes;
createNode HIKState2SK -n "HIKState2SK1";
	setAttr ".ihi" 0;
	setAttr ".HipsTy" 90;
	setAttr ".HipsPGX" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1;
createNode HIKFK2State -n "HIKFK2State1";
	setAttr ".HipsGX" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 90 0 1;
createNode keyingGroup -n "pep_FullBodyKG";
	setAttr ".cat" -type "string" "FullBody";
connectAttr "pep_Hips.ch" "pep_character.Hips";
connectAttr "pep_Head.ch" "pep_character.Head";
connectAttr "HIKproperties1.msg" "pep_character.propertyState";
connectAttr "HIKproperties1.OutputPropertySetState" "HIKSolverNode1.InputPropertySetState";
connectAttr "pep_character.OutputCharacterDefinition" "HIKSolverNode1.InputCharacterDefinition";
connectAttr "HIKFK2State1.OutputCharacterState" "HIKSolverNode1.InputCharacterState";
connectAttr "pep_character.OutputCharacterDefinition" "HIKState2SK1.InputCharacterDefinition";
connectAttr "HIKSolverNode1.OutputCharacterState" "HIKState2SK1.InputCharacterState";
connectAttr "pep_character.OutputCharacterDefinition" "HIKFK2State1.InputCharacterDefinition";
connectAttr "pep_Hips.pm" "HIKState2SK1.HipsPGX";
connectAttr "pep_Hips.jo" "HIKState2SK1.HipsPreR";
"#;

#[test]
fn reads_and_writes_a_character_in_the_references_words() {
    let mut s = Session::new();
    let warnings = aztek_scene::ma_file::read_text(&mut s, REF_TEXT, false);
    assert!(warnings.iter().all(|w| !w.contains("HIK") && !w.contains("pep_character")), "{warnings:?}");
    assert_eq!(run(&mut s, "ls", json!({"type": "HIKCharacterNode"})), json!(["pep_character"]));
    assert_eq!(run(&mut s, "hikGetSkNode", json!({"args": ["pep_character", "Head"]})), json!("pep_Head"));
    assert_eq!(run(&mut s, "hikIsDefinitionLocked", json!({"args": ["pep_character"]})), json!(true));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pep_character.HeadTy"]})), json!(150.0));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pep_character.Hips"], "source": true, "destination": false})), json!(["pep_Hips"]));
    let (text, _) = aztek_scene::ma_file::write_text(&mut s, "t.ma", None);
    for line in [
        "createNode HIKCharacterNode -n \"pep_character\";",
        "setAttr \".InputCharacterizationLock\" yes;",
        "setAttr \".HeadTy\" 150;",
        "connectAttr \"pep_Hips.ch\" \"pep_character.Hips\";",
        "connectAttr \"HIKproperties1.msg\" \"pep_character.propertyState\";",
        "connectAttr \"pep_character.OutputCharacterDefinition\" \"HIKSolverNode1.InputCharacterDefinition\";",
        "connectAttr \"HIKSolverNode1.OutputCharacterState\" \"HIKState2SK1.InputCharacterState\";",
        "setAttr \".lkr\" 0.6",
    ] {
        assert!(text.contains(line), "missing: {line}");
    }
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["HIKproperties1.lkr"]})).as_f64().map(|x| (x * 10.0).round()), Some(6.0));
}

#[test]
fn a_retarget_written_and_read_still_plays() {
    let mut s = Session::new();
    two_bodies(&mut s);
    key(&mut s, "Src_LeftArm.rz", 1.0, 0.0);
    key(&mut s, "Src_LeftArm.rz", 10.0, 45.0);
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    frame(&mut s, 10.0);
    let want = pos(&mut s, "Dst_LeftHand");
    let (text, _) = aztek_scene::ma_file::write_text(&mut s, "t.ma", None);
    assert!(text.contains("connectAttr \"Dst_LeftArm.pm\" \"HIKState2SK1.LeftArmPGX\";"), "the connections the reference has");
    assert!(text.contains("connectAttr \"HIKState2SK1.LeftArmRx\" \"Dst_LeftArm.rx\";"));
    let mut b = Session::new();
    aztek_scene::ma_file::read_text(&mut b, &text, false);
    frame(&mut b, 10.0);
    near_v(pos(&mut b, "Dst_LeftHand"), want, 1e-3, "read back");
    run(&mut b, "hikSetCurrentCharacter", json!({"args": ["Dst"]}));
    assert_eq!(run(&mut b, "hikGetCurrentSource", json!({})), json!("Src"));
    frame(&mut b, 1.0);
    let rest = pos(&mut b, "Dst_LeftHand");
    assert!((rest - want).length() > 50.0, "it animates: {rest:?}");
}

#[test]
fn a_rig_with_limbs_under_their_own_groups() {
    // As rigs with stretchy spines have it: arm and leg chains hang under groups outside the spine's hierarchy.
    let mut s = Session::new();
    run(&mut s, "hikCreateSkeleton", json!({"name": "Src", "character": true}));
    run(&mut s, "group", json!({"empty": true, "name": "limbs"}));
    run(&mut s, "hikCreateSkeleton", json!({"name": "Dst", "character": true}));
    for j in ["Dst_LeftArm", "Dst_RightArm", "Dst_LeftUpLeg", "Dst_RightUpLeg"] {
        run(&mut s, "parent", json!({"args": [j, "limbs"]}));
    }
    run(&mut s, "hikCharacterLock", json!({"args": ["Dst", 1]}));
    let (hand0, foot0) = (pos(&mut s, "Dst_LeftHand"), pos(&mut s, "Dst_LeftFoot"));
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    near_v(pos(&mut s, "Dst_LeftHand"), hand0, 1e-6, "stands");
    key(&mut s, "Src_Hips.tz", 1.0, 25.0);
    frame(&mut s, 1.0);
    // The arm follows the body, though nothing in the hierarchy says so.
    let moved = pos(&mut s, "Dst_LeftHand") - hand0;
    near_v(moved, DVec3::new(0.0, 0.0, 25.0), 1e-5, "the hand comes along");
    let moved = pos(&mut s, "Dst_LeftFoot") - foot0;
    near_v(moved, DVec3::new(0.0, 0.0, 25.0), 1e-5, "the foot comes along");
}

#[test]
fn a_rig_bound_with_its_arms_down() {
    let mut s = Session::new();
    run(&mut s, "hikCreateSkeleton", json!({"name": "Src", "character": true}));
    run(&mut s, "hikCreateSkeleton", json!({"name": "Dst", "character": true}));
    run(&mut s, "setAttr", json!({"args": ["Dst_LeftArm.rz", -45.0]}));
    run(&mut s, "setAttr", json!({"args": ["Dst_RightArm.rz", 45.0]}));
    run(&mut s, "hikCharacterLock", json!({"args": ["Dst", 1]}));
    let bound = pos(&mut s, "Dst_LeftHand");
    assert!(run(&mut s, "hikStraightenArms", json!({"args": ["Dst"]})).as_i64().unwrap() >= 3);
    near_v(pos(&mut s, "Dst_LeftHand"), bound, 1e-9, "the joints stay until driven");
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    // The source stands in T, so the target's arm comes out along X from the shoulder.
    let (arm, hand) = (pos(&mut s, "Dst_LeftArm"), pos(&mut s, "Dst_LeftHand"));
    assert!((hand.y - arm.y).abs() < 1e-6 && (hand.z - arm.z).abs() < 1e-6, "{arm:?} {hand:?}");
    assert!(hand.x > arm.x + 40.0, "{hand:?}");
    let (arm, hand) = (pos(&mut s, "Dst_RightArm"), pos(&mut s, "Dst_RightHand"));
    assert!((hand.y - arm.y).abs() < 1e-6 && hand.x < arm.x - 40.0, "{arm:?} {hand:?}");
}

#[test]
fn a_group_a_rig_drives_holds_still_while_driven() {
    // The hand's group follows the forearm by constraint, as some rigs keep a wrist apart from its chain.
    let mut s = Session::new();
    run(&mut s, "hikCreateSkeleton", json!({"name": "Src", "character": true}));
    run(&mut s, "hikCreateSkeleton", json!({"name": "Dst", "character": true}));
    run(&mut s, "group", json!({"empty": true, "name": "wristGrp"}));
    run(&mut s, "parent", json!({"args": ["Dst_LeftHand", "wristGrp"]}));
    run(&mut s, "parentConstraint", json!({"args": ["Dst_LeftForeArm", "wristGrp"], "maintainOffset": true}));
    run(&mut s, "hikCharacterLock", json!({"args": ["Dst", 1]}));
    let rest = pos(&mut s, "Dst_LeftHand");
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]}));
    near_v(pos(&mut s, "Dst_LeftHand"), rest, 1e-5, "stands");
    key(&mut s, "Src_LeftForeArm.rz", 1.0, 90.0);
    frame(&mut s, 1.0);
    // 25 along from the elbow, now straight up: the solve holds, the group doesn't drag it twice.
    let (elbow, hand) = (pos(&mut s, "Dst_LeftForeArm"), pos(&mut s, "Dst_LeftHand"));
    assert!((hand.y - elbow.y - 25.0).abs() < 1.5, "{elbow:?} {hand:?}");
    // None gives the group its constraint back.
    run(&mut s, "hikSetSource", json!({"args": ["Dst", "None"]}));
    assert!(!run(&mut s, "listConnections", json!({"args": ["wristGrp.tx"], "source": true, "destination": false})).as_array().unwrap().is_empty());
    near_v(pos(&mut s, "Dst_LeftHand"), rest, 1e-5, "back");
}
