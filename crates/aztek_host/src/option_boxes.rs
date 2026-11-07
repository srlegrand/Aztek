//! What each menu item's option box holds: rows in the reference's order, with its labels and defaults, on the command's own flags.
//! The panel builds the dialog from this, and a test checks every row's flag exists on its command, so no row is dead.
//! Labels and defaults are from memory of the reference's dialogs: the docs' option box pages have not been read against them.

use serde_json::{json, Value};

fn b(flag: &str, label: &str, on: bool) -> Value {
    json!({"flag": flag, "label": label, "kind": "bool", "default": on})
}
fn f(flag: &str, label: &str, v: f64) -> Value {
    json!({"flag": flag, "label": label, "kind": "float", "default": v})
}
fn i(flag: &str, label: &str, v: i64) -> Value {
    json!({"flag": flag, "label": label, "kind": "int", "default": v})
}
fn v3(flag: &str, label: &str, v: [f64; 3]) -> Value {
    json!({"flag": flag, "label": label, "kind": "vec3", "default": v})
}
fn s(flag: &str, label: &str) -> Value {
    json!({"flag": flag, "label": label, "kind": "string", "default": ""})
}
/// A pick of one: (label, the value sent). The default is the first.
fn e(flag: &str, label: &str, choices: &[(&str, Value)]) -> Value {
    let c: Vec<Value> = choices.iter().map(|(l, v)| json!([l, v])).collect();
    json!({"flag": flag, "label": label, "kind": "enum", "default": choices[0].1, "choices": c})
}
/// All, X, Y, Z ticks; the axes left out go in the flag.
fn skip(flag: &str, label: &str) -> Value {
    json!({"flag": flag, "label": label, "kind": "axes", "default": []})
}

fn nums(names: &[&str]) -> Vec<(String, Value)> {
    names.iter().enumerate().map(|(k, n)| (n.to_string(), json!(k))).collect()
}

fn en(flag: &str, label: &str, names: &[&str]) -> Value {
    let n = nums(names);
    let r: Vec<(&str, Value)> = n.iter().map(|(a, v)| (a.as_str(), v.clone())).collect();
    e(flag, label, &r)
}

fn degree() -> Value {
    e("degree", "Degree", &[("3 Cubic", json!(3)), ("1 Linear", json!(1)), ("2", json!(2)), ("5", json!(5)), ("7", json!(7))])
}

fn world_up() -> Value {
    e("worldUpType", "World up type", &[("Vector", json!("vector")), ("Scene Up", json!("scene")), ("Object Up", json!("object")), ("Object Rotation Up", json!("objectrotation")), ("None", json!("none"))])
}

/// The rows for a command's option box; empty when it has no layout of its own.
pub fn rows(command: &str) -> Vec<Value> {
    match command {
        // Constrain
        "parentConstraint" => vec![b("maintainOffset", "Maintain offset", true), skip("skipTranslate", "Translate"), skip("skipRotate", "Rotate"), f("weight", "Weight", 1.0)],
        "pointConstraint" => vec![b("maintainOffset", "Maintain offset", false), v3("offset", "Offset", [0.0; 3]), skip("skip", "Constraint axes"), f("weight", "Weight", 1.0)],
        "orientConstraint" => vec![
            b("maintainOffset", "Maintain offset", false),
            v3("offset", "Offset", [0.0; 3]),
            skip("skip", "Constraint axes"),
            f("weight", "Weight", 1.0),
            en("interpType", "Interpolation type", &["No Flip", "Average", "Shortest", "Longest", "Cache"]),
        ],
        "scaleConstraint" => vec![b("maintainOffset", "Maintain offset", false), v3("offset", "Offset", [1.0; 3]), skip("skip", "Constraint axes"), f("weight", "Weight", 1.0)],
        "aimConstraint" => vec![
            b("maintainOffset", "Maintain offset", false),
            v3("offset", "Offset", [0.0; 3]),
            v3("aimVector", "Aim vector", [1.0, 0.0, 0.0]),
            v3("upVector", "Up vector", [0.0, 1.0, 0.0]),
            world_up(),
            v3("worldUpVector", "World up vector", [0.0, 1.0, 0.0]),
            s("worldUpObject", "World up object"),
            skip("skip", "Constraint axes"),
            f("weight", "Weight", 1.0),
        ],
        "poleVectorConstraint" | "geometryConstraint" => vec![f("weight", "Weight", 1.0)],
        "normalConstraint" | "tangentConstraint" => vec![
            f("weight", "Weight", 1.0),
            v3("aimVector", "Aim vector", [1.0, 0.0, 0.0]),
            v3("upVector", "Up vector", [0.0, 1.0, 0.0]),
            world_up(),
            v3("worldUpVector", "World up vector", [0.0, 1.0, 0.0]),
            s("worldUpObject", "World up object"),
        ],
        // Skeleton and skin
        "skinCluster" => vec![
            e("toSelectedBones", "Bind to", &[("Joint hierarchy", json!(false)), ("Selected joints", json!(true))]),
            en("bindMethod", "Bind method", &["Closest distance", "Closest in hierarchy", "Heat map", "Geodesic voxel"]),
            en("skinMethod", "Skinning method", &["Classic linear", "Dual quaternion", "Weight blended"]),
            e("normalizeWeights", "Normalize weights", &[("Interactive", json!(1)), ("None", json!(0)), ("Post", json!(2))]),
            en("weightDistribution", "Weight distribution", &["Distance", "Neighbors"]),
            i("maximumInfluences", "Max influences", 5),
            b("obeyMaxInfluences", "Maintain max influences", false),
            f("dropoffRate", "Dropoff rate", 4.0),
            b("includeHiddenSelections", "Include hidden selections on creation", false),
        ],
        "ikHandle" => vec![
            e("solver", "Current solver", &[("Rotate-Plane Solver", json!("ikRPsolver")), ("Single-Chain Solver", json!("ikSCsolver"))]),
            b("autoPriority", "Autopriority", false),
            i("priority", "Priority", 1),
            f("weight", "Weight", 1.0),
            f("positionWeight", "POWeight", 1.0),
        ],
        "mirrorJoint" => vec![b("mirrorXY", "Mirror across XY", false), b("mirrorYZ", "Mirror across YZ", true), b("mirrorXZ", "Mirror across XZ", false), b("mirrorBehavior", "Mirror function: Behavior", true)],
        // Deform
        "lattice" => vec![
            v3("divisions", "Divisions", [2.0, 5.0, 2.0]),
            v3("ldivisions", "Local divisions", [2.0, 2.0, 2.0]),
            b("objectCentered", "Positioning: Center around selection", true),
            b("freezeMapping", "Freeze mode", false),
            en("outsideLattice", "Outside lattice", &["Inside Lattice", "All", "Falloff"]),
            f("outsideFalloffDist", "Falloff distance", 1.0),
        ],
        "softMod" => vec![f("falloffRadius", "Falloff radius", 5.0), en("falloffMode", "Falloff mode", &["Volume", "Surface"]), b("falloffAroundSelection", "Falloff around selection", true), b("relative", "Mode: Relative", false)],
        "wrap" => vec![b("exclusiveBind", "Exclusive bind", false), b("autoWeightThreshold", "Auto weight threshold", true), f("weightThreshold", "Weight threshold", 0.0), f("maxDistance", "Max distance", 1.0), en("falloffMode", "Falloff mode", &["Volume", "Surface"])],
        "wire" => vec![b("groupWithBase", "Holders", false), f("crossingEffect", "Crossing effect", 0.0), f("tension", "Tension", 1.0), f("localInfluence", "Local influence", 0.0)],
        "blendShape" => vec![f("envelope", "Envelope", 1.0), e("origin", "Origin", &[("Local", json!("local")), ("World", json!("world"))]), b("topologyCheck", "Check topology", true)],
        // Edit Mesh
        "polyExtrude" => vec![
            b("keepFacesTogether", "Keep faces together", true),
            i("divisions", "Divisions", 1),
            f("smoothingAngle", "Smoothing angle", 30.0),
            f("thickness", "Thickness", 0.0),
            f("offset", "Offset", 0.0),
            f("taper", "Taper", 1.0),
            f("twist", "Twist", 0.0),
        ],
        "polyBevel" => vec![
            e("offsetAsFraction", "Width type", &[("Fractional", json!(true)), ("Absolute", json!(false))]),
            f("fraction", "Fraction", 0.5),
            f("offset", "Width", 0.5),
            i("segments", "Segments", 1),
            f("depth", "Depth", 1.0),
            b("chamfer", "Chamfer", true),
            f("smoothingAngle", "Smoothing angle", 30.0),
        ],
        "polyChipOff" => vec![b("keepFacesTogether", "Keep faces together", true), f("offset", "Offset", 0.0)],
        "polyPoke" => vec![v3("localTranslate", "Vertex offset", [0.0; 3]), b("worldSpace", "Offset space: World", false)],
        "polyWedgeFace" => vec![f("wedgeAngle", "Arc angle", 90.0), i("divisions", "Divisions", 4)],
        "polyConnectComponents" => vec![b("insertWithEdgeFlow", "Insert with edge flow", false), f("adjustEdgeFlow", "Adjust edge flow", 0.0)],
        // Mesh
        "polyMirrorFace" => vec![
            e("mirrorAxis", "Mirror axis position", &[("World", json!(2)), ("Bounding Box", json!(0)), ("Object", json!(1))]),
            f("mirrorPosition", "Mirror axis position value", 0.0),
            en("axis", "Mirror axis", &["X", "Y", "Z"]),
            e("axisDirection", "Mirror direction", &[("+", json!(1)), ("-", json!(0))]),
            e("mergeMode", "Border", &[("Merge border vertices", json!(1)), ("Bridge border edges", json!(2)), ("Do not merge", json!(0))]),
            f("mergeThreshold", "Merge threshold", 0.001),
            b("cutMesh", "Cut geometry", true),
            b("flipUVs", "Flip UVs", false),
            f("smoothingAngle", "Smoothing angle", 30.0),
        ],
        // UV
        "polyAutoProjection" => vec![
            e("planes", "Planes", &[("6", json!(6)), ("3", json!(3)), ("4", json!(4)), ("5", json!(5)), ("8", json!(8)), ("12", json!(12))]),
            b("insertBeforeDeformers", "Insert projection before deformers", true),
            f("percentageSpace", "Spacing, percentage", 0.2),
            b("createNewMap", "Create new UV set", false),
        ],
        // Curves and surfaces
        "rebuildCurve" => vec![
            en("rebuildType", "Rebuild type", &["Uniform", "Reduce", "Match Knots", "No Multiple Knots", "Curvature", "End Conditions", "Clean"]),
            en("keepRange", "Parameter range", &["0 to 1", "Keep", "0 to #spans"]),
            b("keepControlPoints", "Keep CVs", false),
            b("keepEndPoints", "Keep ends", true),
            b("keepTangents", "Keep tangents", false),
            i("spans", "Number of spans", 4),
            degree(),
            f("tolerance", "Tolerance", 0.01),
            b("replaceOriginal", "Replace original", true),
        ],
        "attachCurve" => vec![en("method", "Attach method", &["Connect", "Blend"]), f("blendBias", "Blend bias", 0.5), b("replaceOriginal", "Keep originals off", true)],
        "closeCurve" => vec![en("preserveShape", "Shape", &["Ignore", "Preserve", "Blend"]), f("blendBias", "Blend bias", 0.5), b("replaceOriginal", "Replace original", true)],
        "extendCurve" => vec![
            f("distance", "Distance", 1.0),
            en("start", "Extend curve at", &["End", "Start", "Both"]),
            en("extensionType", "Extension type", &["Linear", "Circular", "Extrapolate"]),
            b("replaceOriginal", "Replace original", true),
        ],
        "offsetCurve" => vec![f("distance", "Offset distance", 1.0), v3("normal", "Normal direction", [0.0, 1.0, 0.0]), f("tolerance", "Tolerance", 0.01), b("replaceOriginal", "Replace original", false)],
        "loft" => vec![
            b("uniform", "Parameterization: Uniform", true),
            b("autoReverse", "Auto reverse", true),
            b("close", "Close", false),
            degree(),
            i("sectionSpans", "Section spans", 1),
            en("polygon", "Output geometry", &["NURBS", "Polygons"]),
        ],
        "revolve" => vec![
            v3("axis", "Axis", [0.0, 1.0, 0.0]),
            v3("pivot", "Pivot", [0.0; 3]),
            degree(),
            f("startSweep", "Start sweep angle", 0.0),
            f("endSweep", "End sweep angle", 360.0),
            i("sections", "Segments", 8),
            en("polygon", "Output geometry", &["NURBS", "Polygons"]),
        ],
        "extrude" => vec![
            e("extrudeType", "Style", &[("Tube", json!(2)), ("Flat", json!(1)), ("Distance", json!(0))]),
            f("length", "Extrude length", 1.0),
            v3("direction", "Direction", [0.0, 1.0, 0.0]),
            f("rotation", "Rotation", 0.0),
            f("scale", "Scale", 1.0),
            en("polygon", "Output geometry", &["NURBS", "Polygons"]),
        ],
        "detachCurve" => vec![b("keep", "Keep", true), b("replaceOriginal", "Replace original", true)],
        "insertKnotCurve" => vec![i("numberOfKnots", "Multiplicity", 1), b("addKnots", "Add knots", true), b("insertBetween", "Insert between", false), b("replaceOriginal", "Replace original", true)],
        "attachSurface" => vec![en("method", "Attach method", &["Connect", "Blend"]), f("blendBias", "Blend bias", 0.5), b("keepMultipleKnots", "Keep multiple knots", true), b("replaceOriginal", "Replace original", true)],
        // NURBS primitives
        "sphere" | "cylinder" | "cone" | "torus" => {
            let mut r = vec![v3("pivot", "Pivot", [0.0; 3]), v3("axis", "Axis", [0.0, 1.0, 0.0]), f("radius", "Radius", 1.0), f("startSweep", "Start sweep angle", 0.0), f("endSweep", "End sweep angle", 360.0), degree(), i("sections", "Sections", 8), i("spans", "Spans", 1), f("heightRatio", "Height ratio", if command == "sphere" { 2.0 } else { 2.0 })];
            if command == "torus" {
                r.push(f("minorSweep", "Minor sweep", 360.0));
            }
            r
        }
        "nurbsPlane" => vec![v3("pivot", "Pivot", [0.0; 3]), v3("axis", "Axis", [0.0, 1.0, 0.0]), f("width", "Width", 1.0), f("lengthRatio", "Length ratio", 1.0), degree(), i("patchesU", "U patches", 1), i("patchesV", "V patches", 1)],
        "circle" => vec![v3("center", "Center", [0.0; 3]), v3("normal", "Normal", [0.0, 1.0, 0.0]), f("radius", "Radius", 1.0), f("sweep", "Sweep angle", 360.0), degree(), i("sections", "Sections", 8)],
        "nurbsSquare" => vec![v3("center", "Center", [0.0; 3]), v3("normal", "Normal", [0.0, 1.0, 0.0]), f("sideLength1", "Side length", 1.0)],
        // UV
        "u3dUnfold" => vec![i("iterations", "Iterations", 1), b("pack", "Pack", true), b("borderintersection", "Prevent border intersections", true), b("triangleflip", "Prevent triangle flips", true)],
        "u3dLayout" => vec![f("shellSpacing", "Shell spacing", 0.0), f("rotateStep", "Rotate step", 0.0), i("resolution", "Resolution", 256)],
        // Deform
        "deltaMush" => vec![i("smoothingIterations", "Smoothing iterations", 10), f("smoothingStep", "Smoothing step", 0.5), b("pinBorderVertices", "Pin border vertices", true), f("displacement", "Displacement", 1.0), f("envelope", "Envelope", 1.0)],
        "tension" => vec![i("smoothingIterations", "Smoothing iterations", 10), f("smoothingStep", "Smoothing step", 0.5), b("pinBorderVertices", "Pin border vertices", true), f("squashConstraint", "Squash constraint", 0.0), f("stretchConstraint", "Stretch constraint", 0.0), f("envelope", "Envelope", 1.0)],
        "jiggle" => vec![b("enable", "Enable", true), f("stiffness", "Stiffness", 0.5), f("damping", "Damping", 0.5), f("jiggleWeight", "Jiggle weight", 1.0), f("motionMultiplier", "Motion multiplier", 1.0), f("envelope", "Envelope", 1.0)],
        // Mesh and Modify
        "polyUnite" => vec![e("mergeUVSets", "Merge UV sets", &[("Merge by name", json!(1)), ("Do not merge", json!(0)), ("Merge by UV links", json!(2))]), b("centerPivot", "Center pivot", false), b("objectPivot", "Object pivot", false)],
        "freezeTransforms" => vec![b("translate", "Translate", true), b("rotate", "Rotate", true), b("scale", "Scale", true), b("jointOrient", "Joint orient", false), b("preserveNormals", "Preserve normals", false)],
        // Skin
        "copySkinWeights" => vec![
            e("mirrorMode", "Mirror across", &[("YZ", json!("YZ")), ("XY", json!("XY")), ("XZ", json!("XZ"))]),
            b("mirrorInverse", "Direction: Negative to Positive", false),
            e("surfaceAssociation", "Surface association", &[("Closest point on surface", json!("closestPoint")), ("Ray cast", json!("rayCast")), ("Closest component", json!("closestComponent"))]),
            e("influenceAssociation", "Influence association", &[("Closest joint", json!("closestJoint")), ("Closest bone", json!("closestBone")), ("One to one", json!("oneToOne")), ("Label", json!("label")), ("Name", json!("name"))]),
            b("normalize", "Normalize", false),
        ],
        // Keys
        "setKeyframe" => vec![
            e("hierarchy", "Hierarchy", &[("Selected", json!("none")), ("Below", json!("below"))]),
            b("shape", "Shapes", true),
            b("controlPoints", "Control points", false),
        ],
        "cutKey" => vec![e("hierarchy", "Hierarchy", &[("Selected", json!("none")), ("Below", json!("below"))]), b("shape", "Shapes", true), b("controlPoints", "Control points", false), b("clear", "Delete without copying", true)],
        "bakeResults" => vec![
            e("hierarchy", "Hierarchy", &[("Selected", json!("none")), ("Below", json!("below"))]),
            f("sampleBy", "Sample by", 1.0),
            b("preserveOutsideKeys", "Keep keys outside bake range", false),
            b("sparseAnimCurveBake", "Sparse curve bake", false),
            b("minimizeRotation", "Minimize rotation", true),
            b("simulation", "Keep unbaked: Simulation", false),
            b("shape", "Shapes", true),
        ],
        _ => Vec::new(),
    }
}

/// Every command with a layout, for the test.
pub const COMMANDS: &[&str] = &[
    "parentConstraint", "pointConstraint", "orientConstraint", "scaleConstraint", "aimConstraint", "poleVectorConstraint", "geometryConstraint", "normalConstraint", "tangentConstraint",
    "skinCluster", "ikHandle", "mirrorJoint", "lattice", "softMod", "wrap", "wire", "blendShape", "polyExtrude", "polyBevel", "polyChipOff", "polyPoke", "polyWedgeFace",
    "polyConnectComponents", "polyMirrorFace", "polyAutoProjection", "rebuildCurve", "attachCurve", "closeCurve", "extendCurve", "offsetCurve", "loft", "revolve", "extrude",
    "setKeyframe", "cutKey", "bakeResults", "detachCurve", "insertKnotCurve", "attachSurface", "sphere", "cylinder", "cone", "torus", "nurbsPlane", "circle", "nurbsSquare", "u3dUnfold",
    "u3dLayout", "deltaMush", "tension", "jiggle", "polyUnite", "freezeTransforms", "copySkinWeights",
];
