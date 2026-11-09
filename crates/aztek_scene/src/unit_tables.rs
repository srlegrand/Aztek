//! Which attributes and command flags measure a distance or an angle. Stored values are centimetres and radians; these
//! tags are what scripts, the interface and scene files convert by. PARITY.md lists the audit this came from, and
//! the capture kit's attribute fixtures (`doubleLinear`, `doubleAngle`) check it when they land.

use aztek_ops::units::Unit::{self, Angle, Linear};

const ANGLE_ATTRS: &[&str] = &[
    "rotate", "rotateAxis", "jointOrient", "preferredAngle", "minRotLimit", "maxRotLimit", "restRotate", "constraintRotate",
    "constraintJointOrient", "constraintRotateAxis", "targetOffsetRotate", "targetRotate", "targetJointOrient", "localRotate",
    "twist", "roll", "startAngle", "endAngle", "coneAngle", "penumbraAngle", "shutterAngle", "startSweep", "endSweep", "sweep",
    "minorSweep", "smoothingAngle", "smoothingangle", "miteringAngle", "angleTolerance",
    "outRotate", "outputRotate", "inputRotate", "inRotate1", "inRotate2", "rotateFrame", "rotateUV", "pivotOrient",
    "cutPlaneRotate", "bankLimit", "wedgeAngle", "frontTwist", "upTwist", "sideTwist", "twistRotation", "rollRotation",
    "projectionHorizontalSweep", "rotation", "dTwistStart", "dTwistEnd", "dTwistRampMult", "euler",
];

const LINEAR_ATTRS: &[&str] = &[
    "translate", "rotatePivot", "scalePivot", "rotatePivotTranslate", "scalePivotTranslate", "transMinusRotatePivot", "center",
    "boundingBoxMin", "boundingBoxMax", "boundingBoxSize", "constraintTranslate", "constraintRotatePivot", "constraintRotateTranslate",
    "targetTranslate", "targetRotatePivot", "targetRotateTranslate", "targetScalePivot", "targetOffsetTranslate", "restTranslate",
    "localTranslate", "localPosition", "cutPlaneCenter", "centerOfIllumination", "centerOfInterest", "focusDistance", "nearClipPlane",
    "farClipPlane", "orthographicWidth", "distance", "point1", "point2", "falloffRadius", "falloffCenter", "maxDistance",
    "minDistance",
];

/// Attrs that mean a distance on primitives, and only there: `width` on a texture isn't one.
const SIZE_ATTRS: &[&str] = &["width", "height", "depth", "radius", "sideLength", "tipRadius", "sectionRadius", "thickness", "length"];

fn is_primitive(ty: &str) -> bool {
    (ty.starts_with("poly") && matches!(ty, "polyCube" | "polySphere" | "polyCylinder" | "polyCone" | "polyPlane" | "polyTorus" | "polyPrism" | "polyPyramid" | "polyPipe" | "polyHelix" | "polyDisc" | "polyCapsule" | "polyPlatonicSolid" | "polyPrimitive" | "polySoccerBall" | "polyIcoSphere" | "polyQuadSphere" | "polySuperShape"))
        || matches!(ty, "makeNurbSphere" | "makeNurbCone" | "makeNurbCylinder" | "makeNurbTorus" | "makeNurbPlane" | "makeNurbCube" | "makeNurbCircle" | "makeNurbsSquare")
}

/// HumanIK's joint-state attrs keep the values their solver works in, and only pass through files.
fn is_hik(ty: &str) -> bool {
    ty.starts_with("HIK") || ty.starts_with("hik") || ty == "CustomRigRetargeterNode"
}

/// What an attribute of a node type measures.
pub fn attr_unit(ty: &str, name: &str) -> Unit {
    if is_hik(ty) {
        return Unit::None;
    }
    let leaf = name.rsplit('.').next().unwrap_or(name);
    // `offset` depends on the constraint; `rotation` on a material is a 0 to 1 amount.
    match (ty, leaf) {
        ("pointConstraint" | "poleVectorConstraint" | "geometryConstraint" | "polyExtrudeFacet" | "polyExtrudeEdge" | "polyBevel" | "polyBevel3", "offset") => return Linear,
        ("orientConstraint" | "aimConstraint", "offset") => return Angle,
        ("angleBetween" | "angleBetweenDL", "angle") | ("polySoftEdge", "angle") | ("polyQuad", "angle") => return Angle,
        (t, "inputA" | "inputB" | "output") if t == "animBlendNodeAdditiveRotation" || t == "animBlendNodeAdditiveDA" => return Angle,
        (t, "inputA" | "inputB" | "output") if t == "animBlendNodeAdditiveDL" => return Linear,
        (t, "euler") if t != "angleBetween" && t != "angleBetweenDL" => return Unit::None,
        (t, "rotation") if !matches!(t, "polyPlanarProj" | "polySphProj" | "polyCylProj" | "extrude") => return Unit::None,
        (t, "twist" | "roll") if t != "ikHandle" && !t.starts_with("deform") && t != "polyExtrudeFacet" && t != "polyExtrudeEdge" && t != "polyTorus" => return Unit::None,
        (t, "distance") if t != "distanceBetween" && t != "distanceBetweenDL" && t != "distanceDimShape" => return Unit::None,
        (t, "center") if t == "polySpherize" => return Linear,
        (t, "point1" | "point2") if t != "distanceBetween" && t != "distanceBetweenDL" => return Unit::None,
        (t, "maxDistance" | "minDistance") if t != "closestPointOnSurface" => return Unit::None,
        _ => {}
    }
    if ANGLE_ATTRS.contains(&leaf) {
        return Angle;
    }
    if LINEAR_ATTRS.contains(&leaf) || (SIZE_ATTRS.contains(&leaf) && is_primitive(ty)) {
        return Linear;
    }
    Unit::None
}

type Row = (&'static str, &'static [(&'static str, Unit)]);

/// Flags by command, where the command is not one of the families below.
pub const FLAGS: &[Row] = &[
    ("ambientLight", &[("position", Linear), ("rotation", Angle)]),
    ("directionalLight", &[("position", Linear), ("rotation", Angle)]),
    ("pointLight", &[("position", Linear), ("rotation", Angle)]),
    ("unfrozenTransforms", &[("rotateStep", Angle)]),
    ("xform", &[("translation", Linear), ("rotatePivot", Linear), ("scalePivot", Linear), ("pivots", Linear), ("rotateTranslation", Linear), ("scaleTranslation", Linear), ("rotation", Angle), ("rotateAxis", Angle), ("boundingBox", Linear)]),
    ("rotate", &[("pivot", Linear), ("by", Angle), ("to", Angle), ("x", Angle), ("y", Angle), ("z", Angle)]),
    ("move", &[("x", Linear), ("y", Linear), ("z", Linear), ("by", Linear), ("to", Linear)]),
    ("scale", &[("pivot", Linear)]),
    ("duplicate", &[("translate", Linear), ("rotate", Angle), ("pivotPoint", Linear)]),
    ("joint", &[("position", Linear), ("orientation", Angle), ("scaleOrientation", Angle), ("angleX", Angle), ("angleY", Angle), ("angleZ", Angle), ("limitX", Angle), ("limitY", Angle), ("limitZ", Angle)]),
    ("ikHandle", &[("twist", Angle), ("roll", Angle), ("offset", Linear)]),
    ("spaceLocator", &[("position", Linear)]),
    ("camera", &[("position", Linear), ("rotation", Angle), ("nearClipPlane", Linear), ("farClipPlane", Linear), ("orthographicWidth", Linear), ("centerOfInterest", Linear), ("worldCenterOfInterest", Linear)]),
    ("viewPlace", &[("orthoWidth", Linear)]),
    ("viewClipPlane", &[("nearClipPlane", Linear), ("farClipPlane", Linear)]),
    ("lookThru", &[("nearClip", Linear), ("farClip", Linear)]),
    ("dolly", &[("distance", Linear)]),
    ("tumble", &[("azimuthAngle", Angle), ("elevationAngle", Angle), ("rotationAngles", Angle)]),
    ("orbit", &[("horizontalAngle", Angle), ("verticalAngle", Angle), ("rotationAngles", Angle)]),
    ("roll", &[("degree", Angle)]),
    ("spotLight", &[("position", Linear), ("rotation", Angle), ("coneAngle", Angle), ("penumbra", Angle), ("leftBarnDoorAngle", Angle), ("rightBarnDoorAngle", Angle), ("topBarnDoorAngle", Angle), ("bottomBarnDoorAngle", Angle)]),
    ("curve", &[("point", Linear), ("editPoint", Linear)]),
    ("circle", &[("center", Linear), ("radius", Linear), ("sweep", Angle), ("firstPoint", Linear)]),
    ("nurbsSquare", &[("center", Linear), ("sideLength1", Linear)]),
    ("sphere", &[("pivot", Linear), ("radius", Linear), ("startSweep", Angle), ("endSweep", Angle)]),
    ("cone", &[("pivot", Linear), ("radius", Linear), ("startSweep", Angle), ("endSweep", Angle)]),
    ("cylinder", &[("pivot", Linear), ("radius", Linear), ("startSweep", Angle), ("endSweep", Angle)]),
    ("torus", &[("pivot", Linear), ("radius", Linear), ("startSweep", Angle), ("endSweep", Angle), ("minorSweep", Angle)]),
    ("nurbsPlane", &[("pivot", Linear), ("width", Linear)]),
    ("nurbsCube", &[("pivot", Linear), ("width", Linear)]),
    ("revolve", &[("pivot", Linear), ("startSweep", Angle), ("endSweep", Angle)]),
    ("extrude", &[("length", Linear), ("rotation", Angle)]),
    ("offsetCurve", &[("distance", Linear)]),
    ("offsetSurface", &[("distance", Linear)]),
    ("extendCurve", &[("distance", Linear)]),
    ("extendSurface", &[("distance", Linear)]),
    ("threePointArc", &[("point1", Linear), ("point2", Linear), ("point3", Linear), ("radius", Linear)]),
    ("twoPointArc", &[("point1", Linear), ("point2", Linear), ("radius", Linear)]),
    ("nonLinear", &[("startAngle", Angle), ("endAngle", Angle), ("wavelength", Linear)]),
    ("softMod", &[("falloffRadius", Linear), ("falloffCenter", Linear)]),
    ("softSelect", &[("softSelectDistance", Linear)]),
    ("wire", &[("dropoffDistance", Linear), ("rotation", Angle)]),
    ("wrap", &[("maxDistance", Linear)]),
    ("lattice", &[("position", Linear), ("rotation", Angle)]),
    ("pathAnimation", &[("bankLimit", Angle), ("bankThreshold", Angle)]),
    ("polyMergeVertex", &[("distance", Linear)]),
    ("polyMerge", &[("distance", Linear)]),
    ("polyBevel", &[("offset", Linear), ("smoothingAngle", Angle)]),
    ("polyBevel3", &[("offset", Linear), ("angleTolerance", Angle), ("miteringAngle", Angle), ("smoothingAngle", Angle)]),
    ("polySoftEdge", &[("angle", Angle)]),
    ("polyCut", &[("cutPlaneCenter", Linear), ("cutPlaneRotate", Angle), ("cutPlaneWidth", Linear), ("cutPlaneHeight", Linear), ("extractOffset", Linear)]),
    ("polyMirrorFace", &[("mirrorPosition", Linear), ("pivot", Linear), ("pivotX", Linear), ("pivotY", Linear), ("pivotZ", Linear), ("smoothingAngle", Angle)]),
    ("polyMirror", &[("mirrorPosition", Linear), ("pivot", Linear), ("pivotX", Linear), ("pivotY", Linear), ("pivotZ", Linear), ("tolerance", Linear)]),
    ("polyPoke", &[("translate", Linear), ("translateX", Linear), ("translateY", Linear), ("translateZ", Linear), ("localTranslate", Linear), ("localTranslateX", Linear), ("localTranslateY", Linear), ("localTranslateZ", Linear)]),
    ("polyInset", &[("thickness", Linear), ("depth", Linear)]),
    ("polyCreateFacet", &[("point", Linear)]),
    ("polyAppend", &[("point", Linear)]),
    ("polyChamferVtx", &[("width", Linear)]),
    ("polyWedgeFace", &[("wedgeAngle", Angle), ("center", Linear)]),
    ("polyQuad", &[("angle", Angle)]),
    ("polyQuadrangulate", &[("maxAngle", Angle)]),
    ("polyBridgeEdge", &[("twist", Angle), ("smoothingAngle", Angle)]),
    ("polyBridge", &[("twist", Angle), ("smoothingAngle", Angle)]),
    ("polySpherize", &[("radius", Linear), ("center", Linear)]),
    ("rotateComponents", &[("by", Angle), ("pivot", Linear)]),
    ("scaleComponents", &[("pivot", Linear)]),
    ("moveComponents", &[("by", Linear), ("to", Linear)]),
    ("movePivot", &[("by", Linear), ("to", Linear)]),
    ("distanceDimension", &[("startPoint", Linear), ("endPoint", Linear)]),
    ("closestPointOnMesh", &[("inPosition", Linear)]),
    ("nearestPointOnMesh", &[("inPosition", Linear)]),
    ("setPivot", &[("position", Linear)]),
    ("transformLimits", &[("translation", Linear), ("translationX", Linear), ("translationY", Linear), ("translationZ", Linear), ("rotation", Angle), ("rotationX", Angle), ("rotationY", Angle), ("rotationZ", Angle)]),
];

const PRIMITIVES: &[&str] = &[
    "polyCube", "polySphere", "polyCylinder", "polyCone", "polyPlane", "polyTorus", "polyPrism", "polyPyramid", "polyPipe", "polyHelix",
    "polyDisc", "polyCapsule", "polyPlatonicSolid", "polyPrimitive", "polySoccerBall", "polyIcoSphere", "polyQuadSphere", "polySuperShape",
];
const PRIMITIVE_FLAGS: &[(&str, Unit)] = &[
    ("position", Linear), ("width", Linear), ("height", Linear), ("depth", Linear), ("radius", Linear), ("sideLength", Linear), ("length", Linear),
    ("tipRadius", Linear), ("sectionRadius", Linear), ("thickness", Linear), ("rotation", Angle), ("twist", Angle),
];

const MODIFIERS: &[&str] = &["polyExtrudeFacet", "polyExtrudeEdge", "polyExtrude", "polyMoveVertex", "polyMoveEdge", "polyMoveFacet", "polyChipOff", "polyDuplicateEdge"];
const MODIFIER_FLAGS: &[(&str, Unit)] = &[
    ("translate", Linear), ("translateX", Linear), ("translateY", Linear), ("translateZ", Linear), ("localTranslate", Linear), ("localTranslateX", Linear), ("localTranslateY", Linear), ("localTranslateZ", Linear),
    ("pivot", Linear), ("pivotX", Linear), ("pivotY", Linear), ("pivotZ", Linear), ("offset", Linear), ("thickness", Linear),
    ("rotate", Angle), ("rotateX", Angle), ("rotateY", Angle), ("rotateZ", Angle), ("localRotate", Angle), ("localRotateX", Angle), ("localRotateY", Angle), ("localRotateZ", Angle), ("smoothingAngle", Angle), ("twist", Angle),
];

const CONSTRAINTS: &[&str] = &["pointConstraint", "orientConstraint", "aimConstraint", "geometryConstraint", "normalConstraint", "tangentConstraint", "poleVectorConstraint", "pointOnPolyConstraint"];

/// Numbers among the arguments that measure something.
pub const POSITIONAL: &[(&str, Unit)] = &[("move", Linear), ("rotate", Angle), ("moveVertexAlongDirection", Linear)];

/// What results measure, for commands that answer one thing.
pub const RESULTS: &[(&str, Unit)] = &[
    ("pointPosition", Linear), ("objectCenter", Linear), ("exactWorldBoundingBox", Linear), ("arclen", Linear), ("pointOnCurve", Linear), ("pointOnSurface", Linear),
    ];

/// Every flag's tag, for a command name.
pub fn flag_unit(cmd: &str, flag: &str) -> Unit {
    let lookup = |rows: &[(&str, Unit)]| rows.iter().find(|r| r.0 == flag).map_or(Unit::None, |r| r.1);
    if matches!(flag, "smoothingAngle" | "smoothingangle" | "miteringAngle" | "angleTolerance") {
        return Angle;
    }
    if let Some(r) = FLAGS.iter().find(|r| r.0 == cmd) {
        return lookup(r.1);
    }
    if PRIMITIVES.contains(&cmd) {
        return lookup(PRIMITIVE_FLAGS);
    }
    if MODIFIERS.contains(&cmd) {
        return lookup(MODIFIER_FLAGS);
    }
    if CONSTRAINTS.contains(&cmd) && flag == "offset" {
        return if matches!(cmd, "orientConstraint" | "aimConstraint") { Angle } else { Linear };
    }
    Unit::None
}

/// Tags every registered command. Runs once the session's commands are in.
pub fn tag_commands(c: &mut aztek_ops::Commands<crate::session::Session>) {
    // Runs again as the host adds its commands: what's tagged stays as it is.
    let names: Vec<String> = c.all().map(|d| d.name.clone()).collect();
    for name in names {
        let Some(def) = c.find_mut(&name) else { continue };
        for f in &mut def.flags {
            if f.unit != Unit::None {
                continue;
            }
            f.unit = flag_unit(&name, &f.name);
            // Flags declare angle defaults in degrees, as attributes do.
            if f.unit == Angle {
                aztek_ops::units::scale_json(&mut f.default_value, std::f64::consts::PI / 180.0);
            }
        }
        if let Some(p) = POSITIONAL.iter().find(|p| p.0 == name) {
            def.positional_unit = p.1;
        }
        if let Some(r) = RESULTS.iter().find(|r| r.0 == name) {
            def.result_unit = r.1;
        }
    }
}
