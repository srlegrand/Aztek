//! What a stage draws, without a GPU: sharing, instancing, purposes, visibility, edits touching only what changed, skinning.

use std::time::Duration;

use aztek_usd::draw::{Filter, Purposes, Scene};
use aztek_usd::stages::{self, Entry};
use glam::DVec3;
use openusd::usd::TimeCode;
use std::rc::Rc;

fn write(name: &str, text: &str) -> String {
    let dir = std::env::temp_dir().join(format!("aztek_usd_draw_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join(name);
    std::fs::write(&f, text).unwrap();
    f.to_string_lossy().into_owned()
}

const LONG: Duration = Duration::from_secs(3600);

fn read(e: &Rc<Entry>, scene: &mut Scene, filter: &Filter, t: f64) {
    assert!(scene.update(&e.stage, e.version.get(), &e.changes.borrow(), filter, t, LONG));
}

const STAGE: &str = r#"#usda 1.0
def Xform "World"
{
    def Cube "a"
    {
        double3 xformOp:translate = (1, 0, 0)
        uniform token[] xformOpOrder = ["xformOp:translate"]
        color3f[] primvars:displayColor = [(1, 0, 0)]
    }
    def Cube "same"
    {
        double3 xformOp:translate = (2, 0, 0)
        uniform token[] xformOpOrder = ["xformOp:translate"]
    }
    def Cube "hidden"
    {
        token visibility = "invisible"
    }
    def Cube "off" (
        active = false
    )
    {
    }
    def Cube "guide"
    {
        uniform token purpose = "guide"
    }
    def Cube "render"
    {
        uniform token purpose = "render"
    }
    def Xform "instA" (
        instanceable = true
        references = </Proto>
    )
    {
        double3 xformOp:translate = (0, 5, 0)
        uniform token[] xformOpOrder = ["xformOp:translate"]
    }
    def Xform "instB" (
        instanceable = true
        references = </Proto>
    )
    {
        double3 xformOp:translate = (0, 9, 0)
        uniform token[] xformOpOrder = ["xformOp:translate"]
    }
}
class "Proto"
{
    def Sphere "s"
    {
    }
}
"#;

fn prims_of(s: &Scene) -> Vec<String> {
    let mut v: Vec<String> = s.items.iter().map(|i| i.prim.to_string()).collect();
    v.sort();
    v
}

#[test]
fn visibility_activation_purposes_and_sharing() {
    let e = stages::open_entry(&write("v.usda", STAGE), "", true).unwrap();
    let mut s = Scene::default();
    read(&e, &mut s, &Filter::default(), 1.0);
    assert_eq!(prims_of(&s), vec!["/World/a", "/World/instA", "/World/instB", "/World/same"]);
    // Two cubes of one shape share one geometry; so do the instances' spheres.
    assert_eq!(s.geoms.len(), 2, "one cube and one sphere");
    let a = s.items.iter().find(|i| &*i.prim == "/World/a").unwrap();
    assert_eq!(s.looks[a.look].color, [1.0, 0.0, 0.0]);
    assert_eq!(a.world.w_axis.truncate(), DVec3::new(1.0, 0.0, 0.0));
    let inst = s.items.iter().find(|i| &*i.prim == "/World/instB").unwrap();
    assert_eq!(inst.world.w_axis.y, 9.0);

    let all = Filter { purposes: Purposes { render: true, proxy: true, guide: true }, ..Default::default() };
    read(&e, &mut s, &all, 1.0);
    assert!(prims_of(&s).contains(&"/World/guide".to_string()));
    assert!(prims_of(&s).contains(&"/World/render".to_string()));

    let some = Filter { exclude: vec!["/World/a".into()], root: "/World/same".into(), ..Default::default() };
    read(&e, &mut s, &some, 1.0);
    assert_eq!(prims_of(&s), vec!["/World/same"]);
}

#[test]
fn an_edit_reads_only_what_changed() {
    let e = stages::open_entry(&write("e.usda", STAGE), "", true).unwrap();
    let mut s = Scene::default();
    let f = Filter::default();
    read(&e, &mut s, &f, 1.0);
    let visited = s.prims_visited;
    e.stage.attribute("/World/same.xformOp:translate").unwrap().set(openusd::gf::vec3d(7.0, 0.0, 0.0)).unwrap();
    read(&e, &mut s, &f, 1.0);
    assert_eq!(s.prims_visited, visited + 1, "one prim read again");
    let it = s.items.iter().find(|i| &*i.prim == "/World/same").unwrap();
    assert_eq!(it.world.w_axis.x, 7.0);
    assert_eq!(s.items.len(), 4);
    // A prim going away, and one appearing.
    e.stage.prim("/World/a").unwrap().set_active(false).unwrap();
    read(&e, &mut s, &f, 1.0);
    assert_eq!(s.items.len(), 3);
    e.stage.define_prim("/World/new").unwrap().set_type_name("Cube").unwrap();
    read(&e, &mut s, &f, 1.0);
    assert!(prims_of(&s).contains(&"/World/new".to_string()));
    // Muting says everything changed, without a word about which paths.
    let session = e.session_identifier.clone();
    e.stage.mute_layer(session);
    read(&e, &mut s, &f, 1.0);
    assert!(prims_of(&s).contains(&"/World/new".to_string()));
}

#[test]
fn scrubbing_keeps_what_shows_until_the_new_pass_is_done() {
    let text = r#"#usda 1.0
def Xform "a"
{
    double3 xformOp:translate.timeSamples = {
        1: (0, 0, 0),
        3: (2, 0, 0),
    }
    uniform token[] xformOpOrder = ["xformOp:translate"]
    def Cube "c"
    {
    }
}
"#;
    let e = stages::open_entry(&write("t.usda", text), "", true).unwrap();
    let mut s = Scene::default();
    let f = Filter::default();
    read(&e, &mut s, &f, 1.0);
    assert!(s.has_varying);
    assert_eq!(s.items[0].world.w_axis.x, 0.0);
    read(&e, &mut s, &f, 2.0);
    assert_eq!(s.items.len(), 1);
    assert_eq!(s.items[0].world.w_axis.x, 1.0);
    // The same time twice reads nothing.
    let n = s.prims_visited;
    read(&e, &mut s, &f, 2.0);
    assert_eq!(s.prims_visited, n);
}

#[test]
fn skinned_points_pose_in_the_skeleton_space() {
    let text = r#"#usda 1.0
def SkelRoot "r"
{
    def Skeleton "skel" (
        prepend apiSchemas = ["SkelBindingAPI"]
    )
    {
        uniform token[] joints = ["a"]
        uniform matrix4d[] bindTransforms = [((1,0,0,0),(0,1,0,0),(0,0,1,0),(0,0,0,1))]
        uniform matrix4d[] restTransforms = [((1,0,0,0),(0,1,0,0),(0,0,1,0),(0,0,0,1))]
        rel skel:animationSource = </r/skel/anim>
        def SkelAnimation "anim"
        {
            uniform token[] joints = ["a"]
            float3[] translations.timeSamples = {
                1: [(0, 0, 0)],
                2: [(5, 0, 0)],
            }
            quatf[] rotations = [(1, 0, 0, 0)]
            half3[] scales = [(1, 1, 1)]
        }
    }
    def Mesh "m" (
        prepend apiSchemas = ["SkelBindingAPI"]
    )
    {
        double3 xformOp:translate = (100, 0, 0)
        uniform token[] xformOpOrder = ["xformOp:translate"]
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
        point3f[] points = [(0,0,0),(1,0,0),(0,1,0)]
        rel skel:skeleton = </r/skel>
        int[] primvars:skel:jointIndices = [0, 0, 0] (
            interpolation = "vertex"
            elementSize = 1
        )
        float[] primvars:skel:jointWeights = [1, 1, 1] (
            interpolation = "vertex"
            elementSize = 1
        )
    }
}
"#;
    let e = stages::open_entry(&write("s.usda", text), "", true).unwrap();
    let mut s = Scene::default();
    let f = Filter::default();
    read(&e, &mut s, &f, 2.0);
    let it = &s.items[0];
    // The mesh's own transform doesn't move skinned points: the skeleton's space is what stands.
    assert_eq!(it.world.w_axis.x, 0.0);
    let g = &s.geoms[it.geom];
    assert_eq!(g.vertices[0].pos, [5.0, 0.0, 0.0]);
    read(&e, &mut s, &f, 1.0);
    let it = &s.items[0];
    assert_eq!(s.geoms[it.geom].vertices[0].pos, [0.0, 0.0, 0.0]);
    let _ = TimeCode::new(0.0);
}

#[test]
fn raycast_picks_the_nearest_prim() {
    let e = stages::open_entry(&write("r.usda", STAGE), "", true).unwrap();
    let mut s = Scene::default();
    read(&e, &mut s, &Filter::default(), 1.0);
    let hit = s.raycast(DVec3::new(1.0, 0.0, 50.0), DVec3::NEG_Z).unwrap();
    assert_eq!(&*hit.0, "/World/a");
    assert!(s.raycast(DVec3::new(100.0, 0.0, 50.0), DVec3::NEG_Z).is_none());
    let (lo, hi) = s.bounds(None).unwrap();
    assert!(lo.y <= -1.0 && hi.y >= 10.0);
    let (lo, _) = s.bounds(Some("/World/a")).unwrap();
    assert_eq!(lo.x, 0.0);
}

#[test]
fn value_clips_play() {
    let clip = r#"#usda 1.0
over "c"
{
    double3 xformOp:translate.timeSamples = {
        0: (10, 0, 0),
        10: (20, 0, 0),
    }
}
"#;
    write("clip.usda", clip);
    let text = r#"#usda 1.0
def Xform "c" (
    clips = {
        dictionary default = {
            asset[] assetPaths = [@./clip.usda@]
            string primPath = "/c"
            double2[] active = [(0, 0)]
            double2[] times = [(0, 0), (10, 10)]
        }
    }
)
{
    double3 xformOp:translate
    uniform token[] xformOpOrder = ["xformOp:translate"]
    def Cube "g"
    {
    }
}
"#;
    let e = stages::open_entry(&write("clips.usda", text), "", true).unwrap();
    let mut s = Scene::default();
    let f = Filter::default();
    read(&e, &mut s, &f, 5.0);
    assert_eq!(s.items[0].world.w_axis.x, 15.0, "the clip's samples, interpolated");
    read(&e, &mut s, &f, 10.0);
    assert_eq!(s.items[0].world.w_axis.x, 20.0);
}
