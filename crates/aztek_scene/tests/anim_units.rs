//! The clock: playbackOptions, play, currentUnit, timeControl. These settings are process-wide, so one test runs them in
//! order and puts them back.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn q(s: &mut Session, cmd: &str, flag: &str) -> Value {
    run(s, cmd, json!({"query": true, "queried": [flag]}))
}

#[test]
fn the_clock() {
    let mut s = Session::new();

    // Time units by name.
    assert_eq!(q(&mut s, "currentUnit", "time"), json!("film"));
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 24, "value": 1.0}));
    // Keys stay at the same moment: 24 at film is 1 second, 30 at ntsc.
    run(&mut s, "currentUnit", json!({"time": "ntsc"}));
    assert_eq!(q(&mut s, "currentUnit", "time"), json!("ntsc"));
    assert_eq!(q(&mut s, "playbackOptions", "framesPerSecond"), json!(30.0));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["timeChange"]})), json!([30.0]));
    assert_eq!(q(&mut s, "playbackOptions", "maxTime"), json!(150));
    // With -ua off the frame numbers stay.
    run(&mut s, "currentUnit", json!({"time": "pal", "updateAnimation": false}));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["timeChange"]})), json!([30.0]));
    assert_eq!(q(&mut s, "currentUnit", "time"), json!("pal"));
    for (name, fps) in [("game", 15.0), ("show", 48.0), ("palf", 50.0), ("ntscf", 60.0), ("120fps", 120.0)] {
        run(&mut s, "currentUnit", json!({"time": name, "updateAnimation": false}));
        assert_eq!(q(&mut s, "playbackOptions", "framesPerSecond"), json!(fps), "{name}");
        assert_eq!(q(&mut s, "currentUnit", "time"), json!(name));
    }
    assert!(s.run("currentUnit", json!({"time": "nonsense"})).is_err());
    run(&mut s, "currentUnit", json!({"time": "film", "updateAnimation": false}));
    run(&mut s, "playbackOptions", json!({"minTime": 1.0, "maxTime": 120.0, "animationStartTime": 1.0, "animationEndTime": 200.0}));

    // Linear and angle.
    assert_eq!(q(&mut s, "currentUnit", "linear"), json!("cm"));
    assert_eq!(q(&mut s, "currentUnit", "angle"), json!("deg"));
    run(&mut s, "currentUnit", json!({"linear": "meter"}));
    assert_eq!(q(&mut s, "currentUnit", "linear"), json!("m"));
    assert_eq!(run(&mut s, "currentUnit", json!({"query": true, "queried": ["linear"], "fullName": true})), json!("meter"));
    run(&mut s, "currentUnit", json!({"linear": "cm"}));

    // Times with a unit, and -e -u.
    assert_eq!(run(&mut s, "currentTime", json!({"args": ["2sec"]})), json!(48));
    assert_eq!(run(&mut s, "currentTime", json!({"args": ["30ntsc"]})), json!(24));
    assert_eq!(run(&mut s, "currentTime", json!({"args": [5], "edit": true, "update": false})), json!(5));
    assert_eq!(run(&mut s, "currentTime", json!({"query": true})), json!(5));

    // Playback options.
    run(&mut s, "playbackOptions", json!({"edit": true, "by": 2.0, "maxPlaybackSpeed": 24.0, "view": "active", "loop": "once", "playbackSpeed": 0.5, "preroll": 3.0}));
    assert_eq!(q(&mut s, "playbackOptions", "by"), json!(2.0));
    assert_eq!(q(&mut s, "playbackOptions", "maxPlaybackSpeed"), json!(24.0));
    assert_eq!(q(&mut s, "playbackOptions", "view"), json!("active"));
    assert_eq!(q(&mut s, "playbackOptions", "loop"), json!("once"));
    assert_eq!(q(&mut s, "playbackOptions", "playbackSpeed"), json!(0.5));
    assert_eq!(q(&mut s, "playbackOptions", "preroll"), json!(3.0));
    assert_eq!(q(&mut s, "playbackOptions", "animationEndTime"), json!(200));

    // Play: state, direction, and -w running through by -by.
    assert_eq!(run(&mut s, "play", json!({"query": true, "queried": ["state"]})), json!(false));
    run(&mut s, "play", json!({"forward": false, "state": true}));
    assert_eq!(run(&mut s, "play", json!({"query": true, "queried": ["state"]})), json!(true));
    assert_eq!(run(&mut s, "play", json!({"query": true, "queried": ["forward"]})), json!(false));
    assert_eq!(aztek_scene::anim_time::playing(), Some(-1));
    run(&mut s, "play", json!({"state": false, "forward": true}));
    assert_eq!(aztek_scene::anim_time::playing(), None);
    run(&mut s, "currentTime", json!({"args": [100]}));
    run(&mut s, "play", json!({"forward": true, "wait": true}));
    assert_eq!(run(&mut s, "currentTime", json!({"query": true})), json!(120));
    run(&mut s, "playbackOptions", json!({"edit": true, "by": 1.0, "maxPlaybackSpeed": 0.0, "view": "all", "loop": "continuous", "playbackSpeed": 1.0, "preroll": 0.0}));

    // The time slider's state.
    assert_eq!(run(&mut s, "timeControl", json!({"args": ["slider"], "query": true, "queried": ["rangeVisible"]})), json!(false));
    run(&mut s, "timeControl", json!({"args": ["slider"], "edit": true, "rangeArray": [10.0, 20.0]}));
    assert_eq!(run(&mut s, "timeControl", json!({"args": ["slider"], "query": true, "queried": ["rangeArray"]})), json!([10.0, 20.0]));
    assert_eq!(run(&mut s, "timeControl", json!({"args": ["slider"], "query": true, "queried": ["rangeVisible"]})), json!(true));
    run(&mut s, "timeControl", json!({"edit": true, "rangeArray": []}));
    run(&mut s, "timeControl", json!({"edit": true, "showKeys": "none"}));
    assert_eq!(run(&mut s, "timeControl", json!({"query": true, "queried": ["showKeys"]})), json!("none"));

    // New curves weighted, as the default.
    run(&mut s, "keyTangent", json!({"global": true, "weightedTangents": true}));
    assert_eq!(run(&mut s, "keyTangent", json!({"global": true, "query": true, "queried": ["weightedTangents"]})), json!(true));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tz"], "time": 1}));
    assert_eq!(run(&mut s, "keyTangent", json!({"args": ["pCube1.tz"], "query": true, "queried": ["weightedTangents"]})), json!([true]));
    run(&mut s, "keyTangent", json!({"global": true, "weightedTangents": false}));
}
