//! Color set behaviour (geo/ColorSets.h).

use aztek_geo::colors::*;
use aztek_geo::primitives::*;
use aztek_geo::{AttrType, Mesh};

fn cube() -> Mesh {
    make_cube(&CubeOptions::default())
}

#[test]
fn add_remove_rename_current() {
    let mut m = cube();
    assert!(sets(&m).is_empty());
    assert_eq!(current(&m), "");
    add(&mut m, "a", [0.5; 4]);
    assert_eq!(current(&m), "a");
    assert!(values(&m, "a").iter().all(|v| *v == [0.5; 4]));
    add_white(&mut m, "b");
    assert_eq!(sets(&m), vec!["a", "b"]);
    assert_eq!(current(&m), "a");
    assert!(set_current(&mut m, "b"));
    assert!(!set_current(&mut m, "zz"));
    assert!(rename(&mut m, "b", "c"));
    assert_eq!(current(&m), "c");
    assert!(remove(&mut m, "c"));
    assert_eq!(current(&m), "a");
    assert_eq!(unique_name(&m, "a"), "a1");
    assert_eq!(unique_name(&m, ""), "colorSet");
    assert_eq!(set_of("color:x"), "x");
    assert_eq!(set_of("color:"), "");
    assert_eq!(set_of("uv0"), "");
}

#[test]
fn copy_move_half() {
    let mut m = cube();
    add(&mut m, "a", [0.1, 0.2, 0.3, 1.0]);
    set_half(&mut m, "a", true);
    assert!(is_half(&m, "a"));
    assert_eq!(values(&m, "a")[0][0], round_to_half(0.1));
    assert!(copy(&mut m, "a", "b"));
    assert!(is_half(&m, "b"));
    assert_eq!(values(&m, "b"), values(&m, "a"));
    assert!(move_to(&mut m, "b", 0));
    assert_eq!(sets(&m), vec!["b", "a"]);
    set_half(&mut m, "a", false);
    assert!(!is_half(&m, "a"));
}

#[test]
fn point_colors_and_adopt() {
    let mut m = cube();
    let per_point: Vec<[f32; 4]> = (0..m.point_capacity()).map(|p| [p as f32, 0.0, 0.0, 1.0]).collect();
    add_white(&mut m, "s");
    set_from_points(&mut m, "s", &per_point);
    assert_eq!(point_colors(&m, "s"), per_point);

    let mut legacy = cube();
    legacy.point_attrs_mut().add("color", AttrType::Float4);
    legacy.corner_attrs_mut().add("color1", AttrType::Float4);
    adopt(&mut legacy);
    let mut s = sets(&legacy);
    s.sort();
    assert_eq!(s, vec!["colorSet", "colorSet1"]);
    assert!(!legacy.point_attrs().has("color"));
    assert_eq!(current(&legacy), "colorSet1");
}
