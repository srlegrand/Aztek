//! Dynamics in the scene: cloth, particles, rigid bodies, colliders, fields, emitters and constraints, all solved by one
//! solver node (`aztek_dyn`). Attribute names follow the old systems' where they mean the same, so files map across.
//!
//! - `solver` (shape under `solver1`): time, gravity, wind, ground plane, substeps. Bodies connect `solver1.message` to
//!   their `.solver`.
//! - `cloth` (a deformer, `cloth1`): the mesh coming in is the animation it starts from and is attracted to; the
//!   simulated mesh goes out to the shape, so a skinned mesh goes into the cloth and the cloth into the shape.
//! - `particles` (shape under `particles1`), fed by `emitter` shapes through `.newParticles[]`.
//! - `rigidBody` (`rigidBody1`): drives its object's translate and rotate; passive ones only push.
//! - `collider` (`collider1`): an animated mesh the others hit.
//! - `field` (shape under `gravityField1` and the like): connected to bodies through their `.inputForce[]`.
//! - `dynamicConstraint`: welds, point to surface, component to component, tear, and the rigid pin, hinge, spring, nail.
//!
//! `dyn_solve` steps the solver from an evaluate hook; `dyn_cmds` holds the commands.

use std::any::Any;
use std::sync::{Arc, Mutex};

use aztek_geo::Mesh;
use glam::{DMat4, DVec3};

use crate::attr_defs::{bool_attr, enum_attr, float_attr_short, mesh_attr, vec3_attr, STORED_FLAGS};
use crate::deform::{data_attr, IN, OUT};
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugType, PlugValue};
use crate::transform::Shape;

pub const SOLVER: &str = "solver";
pub const CLOTH: &str = "cloth";
pub const PARTICLES: &str = "particles";
pub const RIGID: &str = "rigidBody";
pub const COLLIDER: &str = "collider";
pub const FIELD: &str = "field";
pub const EMITTER: &str = "emitter";
pub const CONSTRAINT: &str = "dynamicConstraint";

/// Every type this module makes.
pub const TYPES: &[&str] = &[SOLVER, CLOTH, PARTICLES, RIGID, COLLIDER, FIELD, EMITTER, CONSTRAINT];

pub const FIELD_KINDS: &[&str] = &["gravity", "air", "drag", "newton", "radial", "turbulence", "uniform", "vortex", "volumeAxis"];
pub const EMITTER_KINDS: &[&str] = &["directional", "omni", "surface", "curve", "volume"];
pub const CONSTRAINT_KINDS: &[&str] = &["componentToComponent", "pointToSurface", "weld", "tear", "pin", "hinge", "spring", "nail", "pointToPoint"];

pub fn is_dynamics_type(name: &str) -> bool {
    TYPES.contains(&name)
}

fn f(name: &str, short: &str, def: f64, doc: &str) -> AttrDef {
    float_attr_short(name, short, def, doc, None, None)
}

fn fr(name: &str, short: &str, def: f64, doc: &str, lo: f64, hi: Option<f64>) -> AttrDef {
    float_attr_short(name, short, def, doc, Some(lo), hi)
}

fn b(name: &str, short: &str, def: bool, doc: &str) -> AttrDef {
    AttrDef { short_name: short.into(), ..bool_attr(name, def, doc) }
}

fn e(name: &str, short: &str, names: &[&str], def: i64, doc: &str) -> AttrDef {
    AttrDef { short_name: short.into(), ..enum_attr(name, names, def, doc) }
}

fn i(name: &str, short: &str, def: i64, doc: &str, lo: i64) -> AttrDef {
    crate::deform::int_attr(name, short, def, doc, Some(lo as f64))
}

fn v(name: &str, short: &str, def: DVec3, doc: &str) -> AttrDef {
    vec3_attr(name, short, def, STORED_FLAGS | flags::KEYABLE, doc)
}

fn s(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::String, default_value: PlugValue::String(String::new()), flags: STORED_FLAGS, doc: doc.into(), ..Default::default() }
}

/// An input that only says what is connected: the solver, fields, emitters.
fn link(name: &str, short: &str, array: bool, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Int,
        default_value: PlugValue::Int(0),
        flags: flags::INPUT | flags::HIDDEN | flags::MESSAGE | if array { flags::ARRAY } else { 0 },
        doc: doc.into(),
        ..Default::default()
    }
}

fn out_int(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: flags::OUTPUT, doc: doc.into(), ..Default::default() }
}

fn out_float(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Float, default_value: PlugValue::Float(0.0), flags: flags::OUTPUT, doc: doc.into(), ..Default::default() }
}

fn out_vec(name: &str, short: &str, doc: &str) -> AttrDef {
    vec3_attr(name, short, DVec3::ZERO, flags::OUTPUT, doc)
}

/// What every body has: its solver, the fields acting on it, and where a node read from a file came from.
fn body_common() -> Vec<AttrDef> {
    vec![
        link("solver", "slv", false, "The solver it belongs to."),
        link("inputForce", "ifc", true, "The fields acting on it."),
        b("isDynamic", "isd", true, "Off: it takes no part in the solve."),
    ]
}

/// Last on every body: its cache, and what a file from another system gave it.
fn body_file() -> Vec<AttrDef> {
    vec![
        s("diskCache", "dkc", "A cache file the body plays from instead of being solved, where it has the frame."),
        s("fileType", "fty", "The type the node had in the file it was read from, when that was another system's."),
        s("fileAttributes", "fat", "What the file set on it that is not used here, kept to be written back."),
    ]
}

/// The impossible state handling's settings, on cloth and particles.
fn resolve_attrs() -> Vec<AttrDef> {
    vec![
        b("resolveImpossible", "rim", true, "A vertex the solve cannot place (pushed through, squeezed, overstretched) leaves the simulation for a while and comes back once it can be placed."),
        e("fallback", "fbk", &["auto", "animation", "neighbours"], 0, "Where a vertex goes while out: its animated input, the average of its neighbours, or the input where that is near and possible."),
        fr("blendFrames", "bfr", 4.0, "Frames to blend out and back in.", 1.0, None),
        i("returnFrames", "rfr", 4, "Frames a vertex must be placeable before it comes back.", 1),
        i("mostFramesOut", "mfo", 48, "Frames after which a vertex comes back whatever happens.", 1),
        fr("stretchLimit", "slim", 3.0, "Stretch past this ratio of rest length counts as impossible.", 1.0, None),
        out_int("outOfSimulation", "oos", "How many vertices are out of the simulation now."),
    ]
}

fn register(t: NodeType) {
    let name = t.name.clone();
    types::register_type(t).unwrap_or_else(|e| panic!("{name}: {e}"));
}

/// The cloth deformer: registered with the other deformers.
pub(crate) fn register_cloth(gf: Arc<NodeType>) {
    let mut own = body_common();
    own.extend(vec![
        fr("thickness", "thss", 0.1, "Collision thickness: the soft zone kept between the cloth and what it touches.", 0.0, None),
        fr("selfCollideWidthScale", "scws", 1.0, "Self collision thickness as a scale of the thickness.", 0.0, None),
        fr("stretchResistance", "stch", 20.0, "Resistance to stretching: 1 is 100 newtons per metre of width (cotton about 15, silk 3).", 0.0, None),
        fr("compressionResistance", "comr", 10.0, "Resistance to compression, as stretch: lower lets the cloth buckle into wrinkles.", 0.0, None),
        fr("bendResistance", "bnd", 0.1, "Resistance to bending: 1 is 1e-4 newton metres (silk about 0.02, denim 3, leather 10).", 0.0, None),
        fr("deformResistance", "dety", 0.0, "Resistance to changing area, as stretch: 0 off.", 0.0, None),
        fr("density", "dens", 0.15, "Kilograms per square metre (silk 0.04, cotton 0.15, denim 0.45). 0: pointMass is each point's mass instead.", 0.0, None),
        fr("pointMass", "pmss", 1.0, "Each point's mass in kilograms, when density is 0.", 0.0, None),
        fr("friction", "fron", 0.1, "Friction against what it touches.", 0.0, None),
        fr("damp", "adng", 0.0, "Fraction of motion lost per second.", 0.0, None),
        fr("drag", "drg", 0.05, "Air drag across the surface: 0.05 is a flat plate's.", 0.0, None),
        fr("tangentialDrag", "tdrg", 0.0, "Air drag along the surface, as drag.", 0.0, None),
        fr("inputMeshAttract", "imat", 0.0, "Pull towards the incoming mesh, 0 to 1.", 0.0, Some(1.0)),
        fr("bounce", "boce", 0.0, "Kept normal speed off colliders.", 0.0, Some(1.0)),
        b("collide", "cold", true, "Collides with colliders and other bodies."),
        b("selfCollide", "scld", true, "Collides with itself."),
        b("restFromInput", "rfi", false, "Rest lengths follow the incoming mesh each frame, as for cloth on a body that changes shape."),
        fr("tearRatio", "trr", 0.0, "Edges tear past this ratio of rest length; 0 never.", 0.0, None),
        fr("gravityScale", "grsc", 1.0, "Scale of the solver's gravity on this body.", 0.0, None),
        data_attr("inputAttractPerVertex", "iapv", "Per point attraction to the incoming mesh, painted."),
        data_attr("massPerVertex", "mpv", "Per point mass scale, painted."),
        data_attr("pinPerVertex", "pnpv", "Per point pins: 1 follows the incoming mesh exactly."),
    ]);
    own.extend(resolve_attrs());
    own.extend(body_file());
    register(NodeType {
        name: CLOTH.into(),
        base: Some(gf),
        icon: "shirt".into(),
        doc: "Cloth: the incoming mesh simulated by its solver, colliding with everything else the solver holds.".into(),
        time_dependent: true,
        own_attrs: own,
        create: Some(Arc::new(|| Box::<ClothNode>::default())),
        ..Default::default()
    });
}

pub(crate) fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        crate::deform::register_types();
        let shape = types::find("shape").expect("shape");
        register(NodeType {
            name: SOLVER.into(),
            base: Some(shape.clone()),
            dag: true,
            icon: "atom".into(),
            doc: "The solver: steps every body connected to it together, so cloth, particles and rigid bodies all meet.".into(),
            time_dependent: true,
            own_attrs: vec![
                b("enable", "ena", true, "Off: nothing connected moves."),
                f("startFrame", "stf", 1.0, "The frame the simulation starts from. Going back to it starts again."),
                i("subSteps", "sstp", 12, "Steps each frame is split into: more for stiffer, faster things.", 1),
                i("iterations", "itr", 2, "Passes over stretch in each step.", 1),
                i("contactEvery", "cev", 4, "Steps between searches for contacts.", 1),
                fr("gravity", "grty", 9.81, "Gravity's strength, metres per second squared, whatever the scene's unit.", 0.0, None),
                v("gravityDirection", "grdi", DVec3::NEG_Y, "Which way gravity pulls."),
                fr("airDensity", "ady", 1.2, "Kilograms per cubic metre, for drag.", 0.0, None),
                fr("windSpeed", "wisp", 0.0, "Wind speed, metres per second.", 0.0, None),
                v("windDirection", "widi", DVec3::X, "Which way the wind blows."),
                b("usePlane", "nupl", false, "A ground plane everything rests on."),
                v("planeOrigin", "npor", DVec3::ZERO, "A point on the ground plane; only its height is used: the plane faces up."),
                fr("planeFriction", "npfr", 0.1, "Ground plane friction.", 0.0, None),
                fr("spaceScale", "spsc", 0.0, "Metres in one scene unit. 0 (the default): from the scene's unit, so a centimetre of geometry is a centimetre of cloth. Files from other systems set it to reproduce how they played.", 0.0, None),
                fr("timeScale", "tisc", 1.0, "Speeds the simulation up or slows it down.", 0.0, None),
                fr("maxVelocity", "mxv", 50.0, "No point moves faster, metres per second.", 1e-6, None),
                s("fileType", "fty", "The type the node had in the file it was read from, when that was another system's."),
                s("fileAttributes", "fat", "What the file set on it that is not used here, kept to be written back."),
                out_int("outOfSimulation", "oos", "Vertices out of the simulation now, over every body."),
                out_float("solveTime", "slt", "Milliseconds the last frame took."),
            ],
            create: Some(Arc::new(|| {
                crate::dyn_solve::SOLVERS_MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Box::<SolverNode>::default()
            })),
            ..Default::default()
        });
        let mut own = body_common();
        own.extend(vec![
            link("newParticles", "npt", true, "The emitters feeding it."),
            e("lifespanMode", "lfm", &["liveForever", "constant", "random", "lifespanPP"], 0, "How long particles live."),
            fr("lifespan", "lfsp", 1.0, "Seconds each lives.", 0.0, None),
            fr("lifespanRandom", "lfr", 0.0, "How much lifespans vary.", 0.0, None),
            fr("radius", "rad", 0.1, "Collision radius of each particle.", 0.0, None),
            fr("pointMass", "pmss", 1.0, "Mass of each particle.", 0.0, None),
            i("maxCount", "mxc", -1, "Most particles at once; -1 no limit.", -1),
            fr("friction", "fron", 0.1, "Friction against what they touch.", 0.0, None),
            fr("bounce", "boce", 0.0, "Kept normal speed off colliders.", 0.0, Some(1.0)),
            fr("damp", "adng", 0.0, "Fraction of motion lost per second.", 0.0, None),
            fr("drag", "drg", 0.0, "Drag through air.", 0.0, None),
            b("collide", "cold", true, "Collides with colliders and other bodies."),
            b("selfCollide", "scld", false, "Particles push each other apart."),
            fr("gravityScale", "grsc", 1.0, "Scale of the solver's gravity on them.", 0.0, None),
            e("particleRenderType", "prt", &["multiPoint", "multiStreak", "numeric", "points", "spheres", "sprites", "streak", "blobbySurface", "cloud", "tube"], 3, "How they are drawn: points, spheres or blobs."),
            fr("pointSize", "psz", 2.0, "Drawn size of points, pixels.", 1.0, None),
            s("creationExpression", "cexp", "Run once for each new particle."),
            s("runtimeExpression", "rexp", "Run for each particle every frame."),
            data_attr("startPositions", "spns", "Particles made with the particle command, there from the start: x y z each."),
            out_int("count", "cnt", "How many particles there are."),
        ]);
        own.extend(resolve_attrs());
        own.extend(body_file());
        register(NodeType {
            name: PARTICLES.into(),
            base: Some(shape.clone()),
            dag: true,
            icon: "grain".into(),
            doc: "Particles: points with velocity, age, colour and lifespan, born from emitters, pushed by fields, hitting everything in their solver.".into(),
            time_dependent: true,
            own_attrs: own,
            create: Some(Arc::new(|| Box::<ParticlesNode>::default())),
            ..Default::default()
        });
        let mut own = body_common();
        own.extend(vec![
            AttrDef { flags: STORED_FLAGS | flags::HIDDEN, ..mesh_attr("inputMesh", "imsh", STORED_FLAGS | flags::HIDDEN, "The object's mesh as it was made a rigid body, object space.") },
            s("object", "obj", "The object it moves: its transform's path."),
            b("active", "act", true, "On, it moves; off, it only pushes others."),
            fr("mass", "mas", 1.0, "Mass of the whole body.", 0.0, None),
            fr("bounciness", "b", 0.6, "Kept normal speed in contacts.", 0.0, Some(1.0)),
            fr("staticFriction", "sf", 0.2, "Friction at rest.", 0.0, None),
            fr("dynamicFriction", "df", 0.2, "Friction sliding.", 0.0, None),
            e("standInObject", "si", &["none", "cube", "sphere", "capsule"], 0, "The shape it collides as: its own mesh, or a box, sphere or capsule around it."),
            i("particleResolution", "prs", 6, "Particles across its longest side.", 2),
            v("initialVelocity", "iv", DVec3::ZERO, "Velocity at the start frame."),
            v("initialSpin", "is", DVec3::ZERO, "Spin at the start frame, degrees per second."),
            AttrDef { flags: STORED_FLAGS | flags::HIDDEN, ..crate::attr_defs::matrix_attr("initialMatrix", STORED_FLAGS | flags::HIDDEN, "The object's world matrix at the start frame.") },
            out_vec("outTranslate", "otr", "Where the object is, for its translate."),
            out_vec("outRotate", "oro", "How it is turned, for its rotate (degrees, xyz)."),
        ]);
        own.extend(body_file());
        register(NodeType {
            name: RIGID.into(),
            icon: "cube".into(),
            doc: "A rigid body: an object moved by the solver as one piece, or a passive one that only pushes.".into(),
            time_dependent: true,
            own_attrs: own,
            create: Some(Arc::new(|| Box::<RigidNode>::default())),
            ..Default::default()
        });
        let mut own = body_common();
        own.extend(vec![
            mesh_attr("inputMesh", "imsh", flags::INPUT | flags::HIDDEN, "The colliding mesh, object space."),
            crate::deform::matrix_in("inputMatrix", "imx", "The colliding mesh's world matrix."),
            fr("thickness", "thss", 0.0, "Collision thickness added to the other body's.", 0.0, None),
            fr("friction", "fron", 0.1, "Friction.", 0.0, None),
            b("collide", "cold", true, "Off: nothing hits it."),
        ]);
        own.extend(body_file());
        register(NodeType {
            name: COLLIDER.into(),
            icon: "shield".into(),
            doc: "A collider: an animated mesh that cloth, particles and rigid bodies hit.".into(),
            time_dependent: true,
            own_attrs: own,
            create: Some(Arc::new(|| Box::<Plainly>::default())),
            ..Default::default()
        });
        register(NodeType {
            name: FIELD.into(),
            base: Some(shape.clone()),
            dag: true,
            icon: "wind".into(),
            doc: "A field: a force in space acting on the bodies connected to it.".into(),
            own_attrs: vec![
                e("fieldType", "fdt", FIELD_KINDS, 0, "What it does."),
                f("magnitude", "mag", 9.8, "Strength."),
                fr("attenuation", "att", 0.0, "How fast it weakens with distance.", 0.0, None),
                fr("maxDistance", "max", 20.0, "How far it reaches when useMaxDistance is on.", 0.0, None),
                b("useMaxDistance", "umd", false, "Limits its reach."),
                v("direction", "d", DVec3::NEG_Y, "Which way it pushes: gravity, air, uniform, and the axis of vortex and volume axis."),
                f("speed", "spd", 5.0, "Air: the wind's speed, metres per second."),
                fr("inheritVelocity", "iv", 0.0, "Air: how much of the existing motion is kept.", 0.0, Some(1.0)),
                b("useDirection", "ud", false, "Drag: only motion along the direction."),
                fr("minDistance", "mnd", 0.2, "Newton: distances under this count as this.", 0.0, None),
                fr("frequency", "frq", 1.0, "Turbulence: how fine the noise is.", 0.0, None),
                v("phase", "ph", DVec3::ZERO, "Turbulence: shifts the noise."),
                f("awayFromAxis", "afa", 1.0, "Volume axis: push away from the axis."),
                f("alongAxis", "ala", 0.0, "Volume axis: push along the axis."),
                f("aroundAxis", "ara", 0.0, "Volume axis: push around the axis."),
                b("applyToAll", "ata", false, "Acts on every body in every solver, connected or not."),
                s("fileType", "fty", "The type the node had in the file it was read from, when that was another system's."),
                s("fileAttributes", "fat", "What the file set on it that is not used here, kept to be written back."),
            ],
            create: Some(Arc::new(|| Box::<Glyph>::default())),
            ..Default::default()
        });
        register(NodeType {
            name: EMITTER.into(),
            base: Some(shape.clone()),
            dag: true,
            icon: "sparkles".into(),
            doc: "An emitter: makes particles for the particles connected to it.".into(),
            own_attrs: vec![
                e("emitterType", "emt", EMITTER_KINDS, 1, "Where particles come from: a point in one direction or all, a mesh's surface, a curve, or a volume."),
                fr("rate", "rat", 100.0, "Particles per second.", 0.0, None),
                f("speed", "spd", 1.0, "Speed of new particles."),
                fr("speedRandom", "srnd", 0.0, "How much speeds vary.", 0.0, None),
                fr("spread", "spr", 0.0, "Directional: how far from the direction they may go, 0 to 1 (all ways).", 0.0, Some(1.0)),
                v("direction", "d", DVec3::X, "Directional: which way."),
                e("volumeShape", "vol", &["cube", "sphere"], 0, "Volume: the shape they fill."),
                fr("inheritVelocity", "iv", 0.0, "How much of the emitter's own motion they take.", 0.0, None),
                mesh_attr("inputMesh", "imsh", flags::INPUT | flags::HIDDEN, "Surface: the mesh they come off."),
                crate::deform::matrix_in("inputMatrix", "imx", "Surface: the mesh's world matrix."),
                i("seed", "rsd", 0, "Random seed.", 0),
                s("fileType", "fty", "The type the node had in the file it was read from, when that was another system's."),
                s("fileAttributes", "fat", "What the file set on it that is not used here, kept to be written back."),
            ],
            create: Some(Arc::new(|| Box::<Glyph>::default())),
            ..Default::default()
        });
        register(NodeType {
            name: CONSTRAINT.into(),
            icon: "link".into(),
            doc: "A dynamic constraint between points of bodies, or to a surface or a place.".into(),
            own_attrs: vec![
                link("solver", "slv", false, "The solver it belongs to."),
                e("constraintType", "cty", CONSTRAINT_KINDS, 0, "What it holds."),
                b("enable", "ena", true, "Off: it holds nothing."),
                fr("strength", "stg", 20.0, "How stiff: higher holds harder.", 0.0, None),
                f("restLength", "rln", 0.0, "Length it keeps, for springs and component to component with a fixed length."),
                e("restLengthMethod", "rlm", &["fromStart", "constant"], 0, "Rest length as found at the start, or the rest length set."),
                fr("glueStrength", "gls", 0.0, "Tear: breaks once stretched further than this; 0 never.", 0.0, None),
                s("members", "mbr", "Components held, as a selection: pCube1.vtx[0:3] cloth mesh points, or rigid objects."),
                s("targets", "tgt", "What they are held to: components, a surface mesh, or empty for where they are."),
                s("fileType", "fty", "The type the node had in the file it was read from, when that was another system's."),
                s("fileAttributes", "fat", "What the file set on it that is not used here, kept to be written back."),
            ],
            create: Some(Arc::new(|| Box::<Plainly>::default())),
            ..Default::default()
        });
        register(NodeType {
            name: "heldNode".into(),
            icon: "archive".into(),
            doc: "A node of another dynamics system that is not simulated here, held as the file had it and written back.".into(),
            own_attrs: vec![
                s("fileType", "fty", "Its type in the file."),
                s("fileAttributes", "fat", "Its setAttr lines."),
                s("fileParent", "fpr", "Its parent in the file."),
                s("fileConnections", "fcn", "Its connectAttr lines."),
            ],
            create: Some(Arc::new(|| Box::<Plainly>::default())),
            ..Default::default()
        });
        // Old systems' node types read from files as the one each maps to; see ma_file::dynamics for what changes.
        for (old, new) in [("nucleus", SOLVER), ("rigidSolver", SOLVER), ("bulletSolverShape", SOLVER), ("nCloth", CLOTH), ("nParticle", PARTICLES), ("particle", PARTICLES), ("nRigid", COLLIDER), ("bulletRigidBodyShape", RIGID), ("pointEmitter", EMITTER)] {
            types::add_file_alias(old, new);
        }
    });
}

/// A node that computes nothing of its own.
#[derive(Default)]
pub struct Plainly;

impl NodeBehavior for Plainly {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A shape drawn as a glyph at its transform.
#[derive(Default)]
pub struct Glyph;

impl NodeBehavior for Glyph {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// What the solver last gave a cloth: points in the mesh's object space, and each one's simulation weight.
#[derive(Clone)]
pub struct ClothOut {
    pub points: Arc<Vec<DVec3>>,
    pub weights: Arc<Vec<f64>>,
}

#[derive(Default)]
pub struct ClothNode {
    pub out: Mutex<Option<ClothOut>>,
}

impl NodeBehavior for ClothNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let out = self.out.lock().unwrap().clone();
        let oos = d.ty().attr_index("outOfSimulation");
        match out {
            Some(o) if o.points.len() == input.point_capacity() as usize => {
                d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(input.with_positions(o.points.to_vec())))));
                if let Some(a) = oos {
                    d.set_output(a, PlugValue::Int(o.weights.iter().filter(|&&w| w < 1.0).count() as i64));
                }
            }
            _ => {
                d.set_output(OUT, PlugValue::Mesh(Some(input)));
                if let Some(a) = oos {
                    d.set_output(a, PlugValue::Int(0));
                }
            }
        }
    }
    fn traits(&self) -> u32 {
        crate::node::traits::DEFORMER
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Particles as the solver last left them, world space.
#[derive(Clone, Default, Debug)]
pub struct ParticleState {
    pub position: Vec<DVec3>,
    pub velocity: Vec<DVec3>,
    pub id: Vec<u64>,
    pub age: Vec<f64>,
    pub lifespan: Vec<f64>,
    pub rgb: Vec<DVec3>,
    pub opacity: Vec<f64>,
    pub radius: Vec<f64>,
    /// Simulation weights, for showing the ones out of it.
    pub weight: Vec<f64>,
}

#[derive(Default)]
pub struct ParticlesNode {
    pub state: Mutex<Arc<ParticleState>>,
}

impl NodeBehavior for ParticlesNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
        let st = self.state.lock().unwrap().clone();
        if let Some(a) = d.ty().attr_index("count") {
            d.set_output(a, PlugValue::Int(st.position.len() as i64));
        }
        if let Some(a) = d.ty().attr_index("outOfSimulation") {
            d.set_output(a, PlugValue::Int(st.weight.iter().filter(|&&w| w < 1.0).count() as i64));
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Default)]
pub struct RigidNode {
    /// The object's matrix in its parent's space, when the solver has moved it.
    pub local: Mutex<Option<DMat4>>,
}

impl NodeBehavior for RigidNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let (Some(t), Some(r)) = (d.ty().attr_index("outTranslate"), d.ty().attr_index("outRotate")) else { return };
        let m = self.local.lock().unwrap().unwrap_or_else(|| d.matrix(d.ty().attr_index("initialMatrix").unwrap()));
        let (_, rot, tr) = m.to_scale_rotation_translation();
        let (x, y, z) = rot.to_euler(glam::EulerRot::ZYX);
        // Rotate order xyz applies x first: R = Rz Ry Rx.
        d.set_output(t, PlugValue::Vec3(tr));
        d.set_output(r, PlugValue::Vec3(DVec3::new(z.to_degrees(), y.to_degrees(), x.to_degrees())));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Default)]
pub struct SolverNode {
    pub state: Mutex<crate::dyn_solve::State>,
}

impl NodeBehavior for SolverNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
        let st = self.state.lock().unwrap();
        if let Some(a) = d.ty().attr_index("outOfSimulation") {
            d.set_output(a, PlugValue::Int(st.out_count as i64));
        }
        if let Some(a) = d.ty().attr_index("solveTime") {
            d.set_output(a, PlugValue::Float(st.solve_ms));
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A mesh's points (all of them, by index) and its faces.
pub(crate) fn mesh_parts(m: &Mesh) -> (Vec<DVec3>, Vec<Vec<u32>>) {
    let points = m.positions().to_vec();
    let faces = (0..m.face_capacity()).filter(|&f| m.face_valid(f)).map(|f| m.face_points(f).into_iter().map(|p| p as u32).collect()).collect();
    (points, faces)
}

/// A cloth's simulation weights by point, for drawing the ones out of the simulation.
pub fn cloth_weights(g: &crate::graph::Graph, cloth: crate::plug::NodeId) -> Option<Arc<Vec<f64>>> {
    let n = g.node(cloth)?.behavior().as_any().downcast_ref::<ClothNode>()?;
    n.out.lock().unwrap().as_ref().map(|o| o.weights.clone())
}

/// Particles as they are now.
pub fn particle_state(g: &crate::graph::Graph, shape: crate::plug::NodeId) -> Option<Arc<ParticleState>> {
    let n = g.node(shape)?.behavior().as_any().downcast_ref::<ParticlesNode>()?;
    Some(n.state.lock().unwrap().clone())
}

/// The mesh shape a cloth drives.
pub fn cloth_mesh(g: &crate::graph::Graph, cloth: crate::plug::NodeId) -> Option<crate::plug::NodeId> {
    crate::deform::mesh_fed_by(g, cloth)
}

/// The cloth nodes in the scene.
pub fn cloths(g: &crate::graph::Graph) -> Vec<crate::plug::NodeId> {
    g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| n.ty().name == CLOTH)).collect()
}
