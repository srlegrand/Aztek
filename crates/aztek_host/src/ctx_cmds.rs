//! Tool contexts: named tools made by the Ctx commands, which `setToolTo` activates. Each context type maps to a tool the
//! views have; one with no tool yet is still made, and says so once when activated.

use std::cell::RefCell;
use std::collections::BTreeMap;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use aztek_scene::Session;
use glam::DVec3;
use serde_json::{json, Map, Value};

use crate::core::Core;

type Cmd = CommandDef<Session>;

#[derive(Clone, Debug)]
struct Context {
    kind: String,
    props: Map<String, Value>,
}

#[derive(Default)]
struct Contexts {
    all: BTreeMap<String, Context>,
    current: String,
    counters: BTreeMap<String, u32>,
    ready: bool,
    edit_mode: bool,
    /// The context `idle` last saw as current, for its start and finish scripts.
    active: String,
    /// The selection as `idle` last saw it.
    last_selection: Vec<String>,
    /// ctxCompletion was asked for: the current tool finishes at the next idle.
    completing: bool,
    /// What manipPivot set, until the selection changes.
    manip_pivot: Option<[f64; 3]>,
    pivot_pinned: bool,
    /// Flags of manipOptions, setXformManip and viewManip.
    settings: BTreeMap<String, Map<String, Value>>,
}

thread_local! {
    static CTX: RefCell<Contexts> = RefCell::new(Contexts::default());
}

/// Built in contexts, as the reference has them from the start: (name, type).
const BUILT_IN: &[(&str, &str)] = &[
    ("selectSuperContext", "selectSuperContext"),
    ("moveSuperContext", "moveSuperContext"),
    ("RotateSuperContext", "RotateSuperContext"),
    ("scaleSuperContext", "scaleSuperContext"),
    ("artAttrSkinContext", "artAttrSkinContext"),
    ("Move", "manipMoveContext"),
    ("Rotate", "manipRotateContext"),
    ("Scale", "manipScaleContext"),
    ("lassoContext", "lassoContext"),
    ("artSelectContext", "artSelectCtx"),
    ("selectContext", "selectSuperContext"),
    ("ShowManips", "showManipCtx"),
    ("tumbleContext", "tumbleCtx"),
    ("trackContext", "trackCtx"),
    ("dollyContext", "dollyCtx"),
    ("rollContext", "rollCtx"),
    ("jointContext", "jointCtx"),
    ("ikHandleContext", "ikHandleCtx"),
    ("ikSplineHandleContext", "ikSplineHandleCtx"),
    ("curveContext", "curveCVCtx"),
    ("curveCVContext", "curveCVCtx"),
    ("curveEPContext", "curveEPCtx"),
    ("insertJointContext", "insertJointCtx"),
    ("polyCutContext", "polyCutCtx"),
    ("polySplitContext", "polySplitCtx"),
    ("polyCreateFacetContext", "polyCreateFacetCtx"),
    ("polyAppendFacetContext", "polyAppendFacetCtx"),
    ("targetWeldContext", "targetWeldCtx"),
    ("polySelectEditContext", "polySelectEditCtx"),
    ("polyShortestPathContext", "polyShortestPathCtx"),
    ("texMoveContext", "texMoveContext"),
    ("texRotateContext", "texRotateContext"),
    ("texScaleContext", "texScaleContext"),
    ("texSelectContext", "texSelectContext"),
    ("texMoveUVShellContext", "texMoveUVShellContext"),
    ("texCutContext", "texCutContext"),
    ("texSmoothContext", "texSmoothContext"),
    ("texTweakUVContext", "texTweakUVContext"),
    ("texSelectShortestPathCtx", "texSelectShortestPathCtx"),
    ("textureLassoContext", "textureLassoContext"),
    ("texWinToolCtx", "texWinToolCtx"),
];

/// A context type to the view's tool name; None when the views have no such tool yet.
fn tool_of(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "selectSuperContext" | "selectContext" | "polySelectEditCtx" => "select",
        "moveSuperContext" | "manipMoveContext" => "move",
        "RotateSuperContext" | "manipRotateContext" => "rotate",
        "scaleSuperContext" | "manipScaleContext" => "scale",
        "artAttrSkinContext" | "artAttrSkinPaintCtx" => "paintSkin",
        "lassoContext" => "lasso",
        "artSelectCtx" => "paintSelect",
        "tumbleCtx" => "camTumble",
        "trackCtx" => "camTrack",
        "dollyCtx" => "camDolly",
        "rollCtx" => "camRoll",
        "boxZoomCtx" => "camZoom",
        "jointCtx" => "joint",
        "curveCVCtx" => "curveCv",
        "curveEPCtx" => "curveEp",
        "insertJointCtx" => "insertJoint",
        "polySplitCtx" | "polySplitCtx2" => "multiCut",
        "polyCreateFacetCtx" => "createPolygon",
        "polyAppendFacetCtx" => "appendPolygon",
        "targetWeldCtx" => "targetWeld",
        "polyShortestPathCtx" => "shortestPath",
        "sculptMeshCacheCtx" | "artPuttyCtx" => "sculpt",
        "orbitCtx" => "camTumble",
        "boxDollyCtx" => "camBoxDolly",
        "walkCtx" => "camWalk",
        "modelCurrentTimeCtx" => "modelTime",
        "curveAddPtCtx" => "curveAddPt",
        "curveMoveEPCtx" => "move",
        "curveBezierCtx" => "curveBezier",
        "twoPointArcCtx" => "arcTwo",
        "threePointArcCtx" => "arcThree",
        "showManipCtx" => "showManip",
        "polySelectCtx" | "polySuperCtx" | "scriptCtx" | "userCtx" => "select",
        "polyCreaseCtx" => "crease",
        "polyMergeEdgeCtx" => "mergeEdge",
        "polyMergeFacetCtx" => "mergeFacet",
        "createPolyCubeCtx" => "createCube",
        "createPolyPlaneCtx" => "createPlane",
        "snapTogetherCtx" => "snapTogether",
        "setEditCtx" => "editMembership",
        "artSetPaintCtx" | "artUserPaintCtx" | "artAttrPaintVertexCtx" => "paintSelect",
        // Soft select's hold key: the tool stays what it was.
        "softSelectOptionsCtx" => "keep",
        // The Graph Editor's and Dope Sheet's.
        "setKeyCtx" | "keyframeRegionSetKeyCtx" => "key:set",
        "insertKeyCtx" | "keyframeRegionInsertKeyCtx" => "key:insert",
        "moveKeyCtx" | "keyframeRegionMoveKeyCtx" | "directKeyCtx" | "keyframeRegionDirectKeyCtx" | "curveEditorCtx" => "key:move",
        "scaleKeyCtx" | "keyframeRegionScaleKeyCtx" => "key:scale",
        "selectKeyCtx" | "graphSelectContext" | "keyframeRegionSelectKeyCtx" => "key:select",
        "graphDollyCtx" | "keyframeRegionDollyCtx" => "key:dolly",
        "graphTrackCtx" | "keyframeRegionTrackCtx" => "key:track",
        "currentTimeCtx" | "keyframeRegionCurrentTimeCtx" => "key:time",
        // The UV Editor's.
        "texMoveContext" => "uv:move",
        "texRotateContext" => "uv:rotate",
        "texScaleContext" => "uv:scale",
        "texSelectContext" => "uv:select",
        "texMoveUVShellContext" => "uv:shell",
        "texCutContext" => "uv:cut",
        "texSmoothContext" => "uv:smooth",
        "texTweakUVContext" => "uv:tweak",
        "texSelectShortestPathCtx" => "uv:path",
        "textureLassoContext" => "uv:lasso",
        "texWinToolCtx" => "uv:window",
        _ => return None,
    })
}

/// The shell's tool to the reference's context of that tool, for when the user picks a tool by hand.
fn context_of_tool(tool: &str) -> &'static str {
    match tool {
        "move" => "moveSuperContext",
        "rotate" => "RotateSuperContext",
        "scale" => "scaleSuperContext",
        "paintSkin" => "artAttrSkinContext",
        "lasso" => "lassoContext",
        "paintSelect" => "artSelectContext",
        "camTumble" => "tumbleContext",
        "camTrack" => "trackContext",
        "camDolly" => "dollyContext",
        "camRoll" => "rollContext",
        "joint" => "jointContext",
        "curveCv" => "curveContext",
        "curveEp" => "curveEPContext",
        "insertJoint" => "insertJointContext",
        "multiCut" => "polySplitContext",
        "createPolygon" => "polyCreateFacetContext",
        "appendPolygon" => "polyAppendFacetContext",
        "targetWeld" => "targetWeldContext",
        "shortestPath" => "polyShortestPathContext",
        _ => "selectSuperContext",
    }
}

fn with_ctx<R>(f: impl FnOnce(&mut Contexts) -> R) -> R {
    CTX.with(|c| {
        let mut c = c.borrow_mut();
        if !c.ready {
            c.ready = true;
            for (n, k) in BUILT_IN {
                c.all.insert(n.to_string(), Context { kind: k.to_string(), props: Map::new() });
            }
            c.current = "selectSuperContext".into();
        }
        f(&mut c)
    })
}

pub fn delete_context(name: &str) {
    with_ctx(|c| {
        if !BUILT_IN.iter().any(|(n, _)| *n == name) {
            c.all.remove(name);
            if c.current == name {
                c.current = "selectSuperContext".into();
            }
        }
    });
}

pub fn current() -> String {
    with_ctx(|c| c.current.clone())
}

/// The shell changed tool: the current context follows, unless it already means that tool.
pub fn shell_tool_changed(tool: &str) {
    with_ctx(|c| {
        let already = c.all.get(&c.current).and_then(|x| tool_of(&x.kind)) == Some(tool);
        if !already {
            c.current = context_of_tool(tool).to_string();
        }
    });
    let (kind, mode) = match tool {
        "move" => ("manipMoveContext", "Move"),
        "rotate" => ("manipRotateContext", "Rotate"),
        "scale" => ("manipScaleContext", "Scale"),
        _ => return,
    };
    let m = with_ctx(|c| c.all.get(mode).and_then(|x| x.props.get("mode")).and_then(Value::as_i64)).unwrap_or(default_prop(kind, "mode").as_i64().unwrap_or(2));
    view_all("axisOrientation", json!(axis_name(kind, m)));
}

fn add_ctx_flags(c: Cmd, spec: &str) -> Cmd {
    crate::scriptui::add_flags(c, spec)
}

fn f(name: &str, short: &str, ty: ArgType) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, "")
}

const CTX_FLAGS: &str = "image1:i1:a image2:i2:a image3:i3:a title:t:a exists:ex:b query:q:b edit:e:b queried::a name:n:a history:ch:b dragSlider:ds:a";

/// Flags beyond the ones every context has.
const TYPES: &[(&str, &str)] = &[
    ("manipMoveContext", "activeHandle:ah:a alignAlong:aa:a bakePivotOri:bpo:b constrainAlongNormal:xn:b currentActiveHandle:cah:a editPivotMode:epm:b editPivotPosition:epp:b lastMode:lm:a manipVisible:vis:b mode:m:a orientAxes:oa:a orientObject:oo:a orientTowards:ot:a pinPivot:pin:b pivotOriHandle:poh:b position:p:b postCommand:psc:a postDragCommand:pod:a preCommand:prc:a preDragCommand:prd:a preserveChildPosition:pcp:b preserveUV:puv:b reflection:rfl:b reflectionAbout:rab:a reflectionAxis:rfa:a reflectionTolerance:rft:a resetPivotMode:rpm:a snap:s:b snapPivotOri:spo:b snapPivotPos:spp:b snapRelative:sr:b snapValue:sv:a tweakMode:twk:b xformConstraint:xc:a activeHandleNormal:ahn:a interactiveUpdate:iu:b orientJoint:oj:a orientJointEnabled:oje:b secondaryAxisOrient:sao:a snapComponentsRelative:scr:b snapLiveFaceCenter:slf:b snapLivePoint:slp:b translate:tr:a"),
    ("manipRotateContext", "activeHandle:ah:a alignAlong:aa:a bakePivotOri:bpo:b constrainAlongNormal:xn:b currentActiveHandle:cah:a editPivotMode:epm:b editPivotPosition:epp:b lastMode:lm:a manipVisible:vis:b mode:m:a orientAxes:oa:a orientObject:oo:a orientTowards:ot:a pinPivot:pin:b pivotOriHandle:poh:b position:p:b postCommand:psc:a postDragCommand:pod:a preCommand:prc:a preDragCommand:prd:a preserveChildPosition:pcp:b preserveUV:puv:b reflection:rfl:b reflectionAbout:rab:a reflectionAxis:rfa:a reflectionTolerance:rft:a resetPivotMode:rpm:a snap:s:b snapPivotOri:spo:b snapPivotPos:spp:b snapRelative:sr:b snapValue:sv:a tweakMode:twk:b xformConstraint:xc:a centerTrackball:ctb:b modifyTranslation:mt:b rotate:ro:a useCenterPivot:ucp:b useManipPivot:ump:b useObjectPivot:uop:b"),
    ("manipScaleContext", "activeHandle:ah:a alignAlong:aa:a bakePivotOri:bpo:b constrainAlongNormal:xn:b currentActiveHandle:cah:a editPivotMode:epm:b editPivotPosition:epp:b lastMode:lm:a manipVisible:vis:b mode:m:a orientAxes:oa:a orientObject:oo:a orientTowards:ot:a pinPivot:pin:b pivotOriHandle:poh:b position:p:b postCommand:psc:a postDragCommand:pod:a preCommand:prc:a preDragCommand:prd:a preserveChildPosition:pcp:b preserveUV:puv:b reflection:rfl:b reflectionAbout:rab:a reflectionAxis:rfa:a reflectionTolerance:rft:a resetPivotMode:rpm:a snap:s:b snapPivotOri:spo:b snapPivotPos:spp:b snapRelative:sr:b snapValue:sv:a tweakMode:twk:b xformConstraint:xc:a preventNegativeScale:pns:b scale:sc:a useManipPivot:ump:b useObjectPivot:uop:b"),
    ("jointCtx", "autoJointOrient:ajo:a jointAutoLimits:jal:b jointOrientationPlane:jop:a secondaryAxisOrient:sao:a orientJoint:oj:a scaleJointOrient:sjo:b scaleOrientation:so:b scaleCompensate:sc:b degreeOfFreedomJ:dfj:a largeBoneRadius:lbr:a largeJointRadius:ljr:a smallBoneRadius:sbr:a smallJointRadius:sjr:a variableBoneSize:vbs:b createIKHandle:ikh:b ikSolverType:ikt:a poleVectorType:pvt:a snapHandleH:shh:b symmetry:sym:b symmetryAxis:sa:a"),
    ("ikHandleCtx", "autoPriorityH:apH:b createCurve:ccv:b createRootAxis:cra:b forceSolverH:fsH:b ikFKSwitching:ikf:b numSpans:ns:a parentCurve:pcv:b poWeightH:pwH:a priorityH:pH:a rootOnCurve:roc:b rootTwistMode:rtm:b simplifyCurve:scv:b snapCurve:snc:b snapHandleH:snH:b solverTypeH:stH:a stickyH:sH:a twistType:tws:a weightH:wH:a"),
    ("ikSplineHandleCtx", "autoPriorityH:apH:b createCurve:ccv:b createRootAxis:cra:b forceSolverH:fsH:b numSpans:ns:a parentCurve:pcv:b poWeightH:pwH:a priorityH:pH:a rootOnCurve:roc:b rootTwistMode:rtm:b simplifyCurve:scv:b snapCurve:snc:b snapHandleH:snH:b solverTypeH:stH:a stickyH:sH:a twistType:tws:a weightH:wH:a"),
    ("curveCVCtx", "degree:d:a multEndKnots:me:b preserveShape:ps:b rational:r:b refit:rf:b uniform:un:b bezier:bez:b"),
    ("curveEPCtx", "degree:d:a multEndKnots:me:b preserveShape:ps:b rational:r:b refit:rf:b uniform:un:b bezier:bez:b"),
    ("curveSketchCtx", "degree:d:a"),
    ("insertJointCtx", ""),
    ("polyCutCtx", "deleteFaces:df:b extractFaces:ef:a extractOffset:eo:a"),
    ("polySplitCtx", "precsnap:ps:a enablesnap:es:b magnetsnap:ms:a snaptoedge:ste:b smoothingangle:sma:a subdivision:s:a"),
    ("polyCreateFacetCtx", "append:ap:b exists2:ex2:b maximumNumberOfPoints:mnp:a planarConstraint:pc:b subdivision:s:a texture:tx:a autoUV? "),
    ("polyAppendFacetCtx", "append:ap:b maximumNumberOfPoints:mnp:a planarConstraint:pc:b rotate:ro:a subdivision:s:a texture:tx:a twist:tw:b"),
    ("targetWeldCtx", "mergeToCenter:mtc:b preserveUV:puv:b"),
    ("polySelectEditCtx", "mode:m:a insertWithEdgeFlow:ief:b insertWithEdgeFlowEdge:ife:b relativeInsert:r:b splitType:stp:a divisions:div:a absoluteOffset:ao:a autoComplete:ac:b fixQuads:fq:b smoothingAngle:sma:a useEqualMultiplier:uem:b deleteEdge:de:b adjustEdgeFlow:aef:a"),
    ("polyShortestPathCtx", "pathOnEdge:poe:b"),
    ("artAttrCtx", "attrSelected:asl:a selectedattroper:sao:a value:val:a radius:r:a lowerradius:lr:a opacity:op:a stampProfile:stP:a accopacity:aco:b minvalue:mnv:a maxvalue:mxv:a clamp:cl:a clamplower:cll:a clampupper:clu:a colorfeedback:cf:b useColorRamp:ucr:b mappressure:mp:a reflection:rfl:b reflectionaxis:ra:a importfileload:ifl:a exportfilesave:efs:a flood:fl:b floodselect:fs:a outline:o:b tablet:tab:b"),
    ("artSelectCtx", "selectop:sop:a radius:r:a lowerradius:lr:a opacity:op:a stampProfile:stP:a addselection:ads:b unselectall:ual:b selectall:sal:b outline:o:b tablet:tab:b reflection:rfl:b reflectionaxis:ra:a"),
    ("lassoContext", "drawClosed:dc:b"),
    ("skinBindCtx", "about:a:a axis:ax:a colorRamp:cr:a currentInfluence:ci:a displayInactiveMode:dim:a displayNormalized:dn:b falloffCurve:fc:a symmetry:sym:b tolerance:tol:a"),
    ("softModContext", "falloffMode:fm:a falloffRadius:fr:a"),
    ("setKeyCtx", "breakdown:bd:b"),
    ("moveKeyCtx", "moveFunction:mf:a option:o:a"),
    ("selectKeyCtx", ""),
    ("insertKeyCtx", "breakdown:bd:b"),
    ("scaleKeyCtx", "scaleSpecifiedKeys:ssk:b type:typ:a"),
    ("tumbleCtx", "autoOrthoConstrain:aoc:b autoSetPivot:asp:b localTumble:lt:a objectTumble:ot:b orthoLock:ol:b orthoStep:os:a tumbleScale:ts:a alternateContext:ac:b"),
    ("trackCtx", "trackScale:ts:a trackGeometry:tg:b alternateContext:ac:b"),
    ("dollyCtx", "scale:s:a localDolly:ld:b centerOfInterestDolly:cd:b boxDollyType:bdt:a dollyTowardsCenter:dtc:b alternateContext:ac:b toolName:tn:a"),
    ("rollCtx", "rollScale:rs:a alternateContext:ac:b"),
    ("boxZoomCtx", "zoomScale:zs:a"),
    ("orbitCtx", "alternateContext:ac:b localOrbit:lo:b orbitScale:os:a toolName:tn:a"),
    ("boxDollyCtx", "alternateContext:ac:b toolName:tn:a"),
    ("walkCtx", "alternateContext:ac:b crouchCount:crc:a toolName:tn:a walkHeight:wh:a walkSensitivity:wsn:a walkSpeed:ws:a walkToolHud:wth:b"),
    ("curveAddPtCtx", ""),
    ("curveMoveEPCtx", ""),
    ("curveBezierCtx", "degree:d:a manipMode:mm:a preserveShape:ps:b"),
    ("twoPointArcCtx", "degree:d:a spans:s:a"),
    ("threePointArcCtx", "degree:d:a spans:s:a"),
    ("polySelectCtx", "mode:m:a"),
    ("polySuperCtx", "attach:a:a enable:en:b"),
    ("polyCreaseCtx", "extendSelection:es:b relative:r:b"),
    ("polyMergeEdgeCtx", "activeNodes:anq:b firstEdge:fe:a secondEdge:se:a immediate:im:b mergeMode:mm:a mergeTexture:mt:b nodeType:nt:a previous:pvs:b reset:rs:b toolNode:tnq:b caching:cch:b"),
    ("polyMergeFacetCtx", "activeNodes:anq:b firstFacet:ff:a secondFacet:sf:a immediate:im:b mergeMode:mm:a nodeType:nt:a previous:pvs:b reset:rs:b toolNode:tnq:b caching:cch:b"),
    ("createPolyCubeCtx", "width:w:a height:h:a depth:d:a subdivisionsX:sx:a subdivisionsY:sy:a subdivisionsZ:sz:a axis:ax:a createUVs:cuv:a doSubdivisionsCapsUI:dsc:b"),
    ("createPolyPlaneCtx", "width:w:a height:h:a subdivisionsX:sx:a subdivisionsY:sy:a axis:ax:a createUVs:cuv:a"),
    ("snapTogetherCtx", "clearSelection:cs:b setOrientation:so:b snapPolygonFace:spf:b"),
    ("showManipCtx", "currentNodeName:cnn:b incSnap:is:a incSnapRelative:isr:a incSnapUI:isu:b incSnapValue:isv:a lockSelection:ls:b toggleIncSnap:tis:b toolFinish:tf:a toolStart:ts:a"),
    ("setEditCtx", "setName:sn:a"),
    ("softSelectOptionsCtx", "buttonDown:btd:b buttonUp:btu:b"),
    ("scriptCtx", "toolStart:ts:a toolFinish:tf:a finalCommandScript:fcs:a setSelectionCount:ssc:a setAllowExcessCount:sae:b setAutoToggleSelection:sat:b setAutoComplete:sac:b setDoneSelectionPrompt:dsp:a setNoSelectionPrompt:nsp:a setNoSelectionHeadsUp:nsh:a setSelectionPrompt:ssp:a setSelectionHeadsUp:ssh:a totalSelectionSets:tss:a exitUponCompletion:euc:b enableRootSelection:ers:b ignoreInvalidItems:iii:b cumulativeLists:cls:b expandSelectionList:esl:b showManipulators:sm:b baseClassName:bcn:a toolCursorType:tct:a allComponents:alc:b vertex:v:b edge:eg:b facet:fc:b curve:c:b polymesh:pl:b"),
    ("userCtx", "toolStart:ts:a toolFinish:tf:a finalCommandScript:fcs:a setSelectionCount:ssc:a setAllowExcessCount:sae:b setAutoToggleSelection:sat:b setAutoComplete:sac:b setDoneSelectionPrompt:dsp:a setNoSelectionPrompt:nsp:a setNoSelectionHeadsUp:nsh:a setSelectionPrompt:ssp:a setSelectionHeadsUp:ssh:a totalSelectionSets:tss:a exitUponCompletion:euc:b enableRootSelection:ers:b ignoreInvalidItems:iii:b cumulativeLists:cls:b expandSelectionList:esl:b showManipulators:sm:b baseClassName:bcn:a toolCursorType:tct:a allComponents:alc:b vertex:v:b edge:eg:b facet:fc:b curve:c:b polymesh:pl:b"),
    ("graphDollyCtx", ""),
    ("graphTrackCtx", ""),
    ("graphSelectContext", ""),
    ("directKeyCtx", "option:o:a selectedOnly:slo:b"),
    ("currentTimeCtx", ""),
    ("modelCurrentTimeCtx", "percent:per:a snap:sn:b"),
    ("curveEditorCtx", "direction:dir:a relativeTangentSize:rts:a tangentScale:ts:a tangentTension:tt:a"),
    ("keyframeRegionMoveKeyCtx", "moveFunction:mf:a option:o:a"),
    ("keyframeRegionScaleKeyCtx", "scaleSpecifiedKeys:ssk:b type:typ:a"),
    ("keyframeRegionSelectKeyCtx", ""),
    ("keyframeRegionSetKeyCtx", "breakdown:bd:b"),
    ("keyframeRegionInsertKeyCtx", "breakdown:bd:b"),
    ("keyframeRegionDirectKeyCtx", "option:o:a selectedOnly:slo:b"),
    ("keyframeRegionDollyCtx", "scale:s:a"),
    ("keyframeRegionTrackCtx", "scale:s:a"),
    ("keyframeRegionCurrentTimeCtx", ""),
    ("texMoveContext", "snap:s:b snapValue:sv:a snapComponentsRelative:scr:b position:p:b editPivotMode:epm:b editPivotPosition:epp:a tweakMode:twk:b"),
    ("texRotateContext", "snap:s:b snapValue:sv:a snapComponentsRelative:scr:b position:p:b editPivotMode:epm:b editPivotPosition:epp:a tweakMode:twk:b"),
    ("texScaleContext", "snap:s:b snapValue:sv:a snapComponentsRelative:scr:b position:p:b editPivotMode:epm:b editPivotPosition:epp:a tweakMode:twk:b"),
    ("texSelectContext", ""),
    ("texMoveUVShellContext", "iterations:it:a mask:m:a position:p:b shellBorder:sb:a tolerance:to:a"),
    ("texCutContext", "adjustSize:as:b displayShellBorders:dsb:b edgeSelectSensitive:ess:a mode:m:a moveRatio:mr:a showTextureBorders:stb:b size:sz:a steadyStroke:ss:b steadyStrokeDistance:ssd:a touchToSew:tts:b"),
    ("texSmoothContext", ""),
    ("texSmudgeUVContext", "dragSlider:ds:a effectType:et:a functionType:ft:a pressure:prs:a radius:r:a smudgeIsMiddle:sim:b"),
    ("texTweakUVContext", "position:p:b tolerance:to:a"),
    ("texLatticeDeformContext", "envelope:ev:a height:h:a width:w:a latticeColumns:lc:a latticeRows:lr:a snapPixelMode:spm:b useBoundingRect:ubr:b showMoveManipulator:smm:b"),
    ("texSelectShortestPathCtx", ""),
    ("textureLassoContext", "drawClosed:dc:b"),
    ("texWinToolCtx", "alternateContext:ac:b boxzoom:bz:b dolly:do:b toolName:tn:a track:tr:b"),
    ("artSetPaintCtx", "settomodify:stm:a setcolorfeedback:scf:b setdisplaycvs:dcv:b setopertype:sot:a radius:r:a lowerradius:lr:a opacity:op:a value:val:a stampProfile:stP:a accopacity:aco:b minvalue:mnv:a maxvalue:mxv:a clamp:cl:a clamplower:cll:a clampupper:clu:a colorfeedback:cf:b mappressure:mp:a reflection:rfl:b reflectionaxis:ra:a outline:o:b tablet:tab:b flood:fl:b floodselect:fs:a"),
    ("artUserPaintCtx", "toolSetupCmd:tsc:a toolCleanupCmd:tcc:a initializeCmd:ic:a finalizeCmd:fc:a setValueCommand:svc:a getValueCommand:gvc:a setArrayValueCommand:sac:a getArrayAttrCommand:gac:a getSurfaceCommand:gsc:a radius:r:a lowerradius:lr:a opacity:op:a value:val:a stampProfile:stP:a accopacity:aco:b minvalue:mnv:a maxvalue:mxv:a clamp:cl:a clamplower:cll:a clampupper:clu:a colorfeedback:cf:b mappressure:mp:a reflection:rfl:b reflectionaxis:ra:a outline:o:b tablet:tab:b flood:fl:b floodselect:fs:a"),
    ("artPuttyCtx", "mouldtypehead:mth:a mouldtypemouse:mtm:a mouldtypesmooth:mts:a dispdecr:dde:b dispincr:din:b invertrefvector:irv:b maxdisp:md:a refvector:rv:a stampDepth:sd:a radius:r:a lowerradius:lr:a opacity:op:a value:val:a stampProfile:stP:a accopacity:aco:b minvalue:mnv:a maxvalue:mxv:a clamp:cl:a clamplower:cll:a clampupper:clu:a colorfeedback:cf:b mappressure:mp:a reflection:rfl:b reflectionaxis:ra:a outline:o:b tablet:tab:b flood:fl:b floodselect:fs:a"),
    ("artAttrPaintVertexCtx", "colorRGBValue:cl3:a colorAlphaValue:cl4:a paintRGB:pc3:b paintAlpha:pa:b paintoperationtype:pot:a selectedattroper:sao:a radius:r:a lowerradius:lr:a opacity:op:a value:val:a stampProfile:stP:a accopacity:aco:b minvalue:mnv:a maxvalue:mxv:a clamp:cl:a clamplower:cll:a clampupper:clu:a colorfeedback:cf:b mappressure:mp:a reflection:rfl:b reflectionaxis:ra:a outline:o:b tablet:tab:b flood:fl:b floodselect:fs:a"),
];

fn parse_flags(mut c: Cmd, spec: &str) -> Cmd {
    for t in spec.split_whitespace() {
        let mut it = t.split(':');
        let (Some(long), Some(short), Some(ty)) = (it.next(), it.next(), it.next()) else { continue };
        if c.flags.iter().any(|x| x.name == long) {
            continue;
        }
        c = c.flag(f(long, short, if ty == "b" { ArgType::Bool } else { ArgType::Any }));
    }
    c
}

fn asked(a: &Args) -> Option<String> {
    a.get("queried").and_then(Value::as_array).and_then(|q| q.first()).and_then(Value::as_str).map(str::to_string).or_else(|| a.flags.keys().find(|k| !matches!(k.as_str(), "query" | "queried")).cloned())
}

/// `manipMoveContext -m`: the reference's numbers to the axis names the views use.
pub fn axis_name(kind: &str, mode: i64) -> &'static str {
    if kind == "manipRotateContext" {
        return match mode {
            0 => "object",
            1 => "world",
            2 => "gimbal",
            3 => "custom",
            10 => "component",
            _ => "object",
        };
    }
    match mode {
        0 => "object",
        1 => "parent",
        2 => "world",
        3 => "normal",
        4 => "rotationAxis",
        5 => "liveObject",
        6 => "custom",
        10 => "component",
        _ => "object",
    }
}

fn view_all(key: &str, value: Value) {
    crate::core::push_ui_request(format!("viewAll|{}", json!({"key": key, "value": value})));
}

fn apply_to_views(s: &mut Session, kind: &str, flags: &Map<String, Value>) {
    if kind == "softSelectOptionsCtx" {
        if flags.get("buttonDown").is_some_and(truthy) {
            view_all("brushAdjust", json!(true));
        }
        if flags.get("buttonUp").is_some_and(truthy) {
            view_all("brushAdjust", json!(false));
        }
        return;
    }
    if kind == "artPuttyCtx" {
        if let Some(t) = flags.get("mouldtypemouse").and_then(Value::as_str) {
            s.sculpt_ctx.tool = match t {
                "smooth" => "smooth",
                "erase" => "erase",
                _ => "sculpt",
            }
            .to_string();
        }
        if let Some(r) = flags.get("radius").and_then(Value::as_f64) {
            s.sculpt_ctx.size = r;
        }
        if let Some(o) = flags.get("opacity").and_then(Value::as_f64) {
            s.sculpt_ctx.strength = o;
        }
        return;
    }
    if !matches!(kind, "manipMoveContext" | "manipRotateContext" | "manipScaleContext") {
        return;
    }
    // Which handle is active: 0 X, 1 Y, 2 Z, 3 centre, and 4 XY, 5 YZ, 6 XZ for a move.
    if let Some(h) = flags.get("activeHandle").and_then(Value::as_i64) {
        view_all("activeHandle", json!(h));
    }
    if let Some(m) = flags.get("mode").and_then(Value::as_i64) {
        view_all("axisOrientation", json!(axis_name(kind, m)));
    }
    let on_off = |key: &str| flags.get(key).map(|v| v.as_bool().unwrap_or(v.as_i64().is_some_and(|i| i != 0)));
    if let Some(v) = on_off("snapRelative") {
        view_all("snapRelative", json!(v));
    }
    if let Some(v) = on_off("tweakMode") {
        view_all("tweakMode", json!(v));
    }
    if let Some(v) = on_off("preserveChildPosition") {
        view_all("preserveChildren", json!(v));
    }
    if let Some(v) = on_off("preserveUV") {
        s.graph.move_opts.preserve_uv = v;
    }
    if let Some(v) = on_off("snapComponentsRelative") {
        view_all("keepSpacing", json!(v));
    }
    if let Some(v) = on_off("snapPivotPos") {
        view_all("snapPivot", json!(v));
    }
    if let Some(v) = on_off("useManipPivot") {
        view_all("useManipPivot", json!(v));
    }
    if let Some(v) = on_off("preventNegativeScale") {
        view_all("preventNegativeScale", json!(v));
    }
    // The custom axes, however they are given.
    if let Some(e) = crate::axis_orient::from_flags(s, flags) {
        view_all("customAxis", json!(e));
    }
    let step = match kind {
        "manipMoveContext" => "stepMove",
        "manipRotateContext" => "stepRotate",
        _ => "stepScale",
    };
    let on = flags.get("snap").map(|v| v.as_bool().unwrap_or(v.as_i64().is_some_and(|i| i != 0)));
    if let Some(v) = flags.get("snapValue").and_then(Value::as_f64) {
        if on != Some(false) {
            view_all(step, json!(v));
        }
    } else if on == Some(false) {
        view_all(step, json!(0.0));
    }
}

fn default_prop(kind: &str, key: &str) -> Value {
    match (kind, key) {
        ("manipMoveContext" | "manipScaleContext", "mode") => json!(2),
        ("manipRotateContext", "mode") => json!(0),
        ("manipMoveContext", "snapValue") => json!(1.0),
        ("manipRotateContext", "snapValue") => json!(15.0),
        ("manipScaleContext", "snapValue") => json!(1.0),
        (_, "activeHandle") => json!(3),
        (_, "position") => json!([0.0, 0.0, 0.0]),
        (_, "degree") => json!(3),
        (_, "title" | "image1" | "image2" | "image3") => json!(""),
        _ => Value::Null,
    }
}

fn context_command(kind: &'static str, s: &mut Session, a: &Args) -> CmdResult {
    let name = a.pos_str(0).map(str::to_string);
    if a.bool("exists") && !a.bool("query") {
        return Ok(json!(name.is_some_and(|n| with_ctx(|c| c.all.contains_key(&n)))));
    }
    if a.bool("query") {
        let Some(q) = asked(a) else { fail!("{kind}: name what to ask about.") };
        let Some(n) = name else { fail!("{kind}: name a context.") };
        if q == "exists" {
            return Ok(json!(with_ctx(|c| c.all.contains_key(&n))));
        }
        if q == "position" && kind.starts_with("manip") {
            return manip_position(s);
        }
        return with_ctx(|c| {
            let Some(x) = c.all.get(&n) else { fail!("{kind}: Object '{n}' not found.") };
            Ok(x.props.get(&q).cloned().unwrap_or_else(|| default_prop(&x.kind, &q)))
        });
    }
    let mut flags = a.flags.clone();
    flags.retain(|k, v| !matches!(k.as_str(), "query" | "edit" | "queried" | "exists" | "history") && !v.is_null());
    if a.bool("edit") {
        let Some(n) = name else { fail!("{kind}: name a context.") };
        let k = with_ctx(|c| -> Result<String, String> {
            let Some(x) = c.all.get_mut(&n) else { return Err(format!("{kind}: Object '{n}' not found.")) };
            for (fl, v) in &flags {
                x.props.insert(fl.clone(), v.clone());
            }
            Ok(x.kind.clone())
        })
        .map_err(CommandError::new)?;
        apply_to_views(s, &k, &flags);
        return Ok(json!(n));
    }
    let n = with_ctx(|c| -> Result<String, String> {
        let n = match name.or_else(|| flags.get("name").and_then(Value::as_str).map(str::to_string)) {
            Some(n) => {
                if c.all.contains_key(&n) {
                    return Err(format!("{kind}: Object's name '{n}' is not unique."));
                }
                n
            }
            None => {
                let e = c.counters.entry(kind.to_string()).or_insert(0);
                loop {
                    *e += 1;
                    let n = format!("{kind}{e}");
                    if !c.all.contains_key(&n) {
                        break n;
                    }
                }
            }
        };
        c.all.insert(n.clone(), Context { kind: kind.to_string(), props: flags.clone() });
        Ok(n)
    })
    .map_err(CommandError::new)?;
    Ok(json!(n))
}

/// The manipulator's position: the middle of what is selected.
fn manip_position(s: &mut Session) -> CmdResult {
    let names: Vec<String> = s.graph.selection().iter().map(|&i| s.graph.path(i)).collect();
    if names.is_empty() {
        return Ok(json!([0.0, 0.0, 0.0]));
    }
    let v = s.run("exactWorldBoundingBox", json!({"args": names}))?;
    let b: Vec<f64> = v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if b.len() < 6 {
        return Ok(json!([0.0, 0.0, 0.0]));
    }
    Ok(json!([(b[0] + b[3]) * 0.5, (b[1] + b[4]) * 0.5, (b[2] + b[5]) * 0.5]))
}

fn truthy(v: &Value) -> bool {
    v.as_bool().unwrap_or_else(|| v.as_f64().is_some_and(|x| x != 0.0))
}

fn set_tool_to(s: &mut Session, name: &str) -> CmdResult {
    // The paint attribute tool's command lives in the scene and keeps its own names.
    if name == "artAttrCtx1" || s.attr_ctx.tools.iter().any(|t| t == name) {
        with_ctx(|c| c.current = name.to_string());
        crate::core::push_ui_request("ArtPaintAttrTool".to_string());
        return Ok(json!(name));
    }
    let kind = with_ctx(|c| c.all.get(name).map(|x| x.kind.clone()));
    let Some(kind) = kind else { fail!("setToolTo: Object '{name}' not found.") };
    with_ctx(|c| c.current = name.to_string());
    match tool_of(&kind) {
        Some("keep") => {}
        Some(t) if t.starts_with("uv:") => crate::core::push_ui_request(format!("uvTool|{}", &t[3..])),
        Some(t) if t.starts_with("key:") => crate::core::push_ui_request(format!("keyTool|{}", &t[4..])),
        Some(t) => {
            crate::core::push_ui_request(format!("setTool|{t}"));
            // What the tool picks or shows.
            match kind.as_str() {
                "polyMergeEdgeCtx" => drop(s.run("selectMode", json!({"args": ["edge"]}))),
                "polyMergeFacetCtx" => drop(s.run("selectMode", json!({"args": ["face"]}))),
                "showManipCtx" => crate::core::push_ui_request("ShowManipulators".to_string()),
                _ => {}
            }
            let props = with_ctx(|c| c.all.get(name).map(|x| x.props.clone())).unwrap_or_default();
            if kind == "artPuttyCtx" {
                apply_to_views(s, &kind, &props);
            }
            // The tool's own state of a manip context applies as it comes in.
            if kind.starts_with("manip") {
                if let Some(h) = props.get("activeHandle") {
                    view_all("activeHandle", h.clone());
                }
            }
        }
        None => {
            crate::script_jobs::warn_once(&kind, &format!("{kind}: this tool has no interaction in the views yet; the context is current but the views stay on the select tool."));
            crate::core::push_ui_request("setTool|select".to_string());
        }
    }
    Ok(json!(name))
}

/// What manipPivot set, for the views' manipulator.
/// Pin Component Pivot: whether it is on, and no pivot is held yet.
pub fn wants_pin() -> bool {
    with_ctx(|c| c.pivot_pinned && c.manip_pivot.is_none())
}

/// Holds the pivot where it is, for Pin Component Pivot.
pub fn hold_pivot(p: DVec3) {
    with_ctx(|c| c.manip_pivot = Some(p.to_array()));
}

pub fn manip_pivot_override() -> Option<DVec3> {
    with_ctx(|c| c.manip_pivot.map(DVec3::from_array))
}

fn selection_now(core: &mut Core) -> Vec<String> {
    core.s.run("ls", json!({"selection": true, "flatten": true})).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

fn prop_of(name: &str, key: &str) -> Option<Value> {
    with_ctx(|c| c.all.get(name).and_then(|x| x.props.get(key).cloned()))
}

/// The commands a context's selection drives: scriptCtx's final script once enough is picked, the paint contexts' action
/// on what the brush newly took. Start and finish scripts run as a context comes and goes.
pub(crate) fn idle(core: &mut Core) {
    let (cur, prev) = with_ctx(|c| {
        let p = std::mem::replace(&mut c.active, c.current.clone());
        (c.current.clone(), p)
    });
    if prev != cur {
        for flag in ["toolFinish", "toolCleanupCmd", "finalizeCmd"] {
            if let Some(cb) = prop_of(&prev, flag) {
                core.run_callback(&cb, &[]);
            }
        }
        for flag in ["toolStart", "toolSetupCmd", "initializeCmd"] {
            if let Some(cb) = prop_of(&cur, flag) {
                core.run_callback(&cb, &[]);
            }
        }
        let now = selection_now(core);
        with_ctx(|c| c.last_selection = now);
    }
    let kind = with_ctx(|c| c.all.get(&cur).map(|x| x.kind.clone())).unwrap_or_default();
    let watches_pivot = with_ctx(|c| c.manip_pivot.is_some() && !c.pivot_pinned);
    if !matches!(kind.as_str(), "scriptCtx" | "userCtx" | "artSetPaintCtx" | "artUserPaintCtx" | "artAttrPaintVertexCtx") && !watches_pivot {
        with_ctx(|c| c.completing = false);
        return;
    }
    let now = selection_now(core);
    let before = with_ctx(|c| std::mem::replace(&mut c.last_selection, now.clone()));
    if watches_pivot && before != now {
        with_ctx(|c| c.manip_pivot = None);
    }
    let fresh: Vec<String> = now.iter().filter(|x| !before.contains(x)).cloned().collect();
    match kind.as_str() {
        "scriptCtx" | "userCtx" => script_ctx_step(core, &cur, &now),
        "artSetPaintCtx" if !fresh.is_empty() => {
            if let Some(set) = prop_of(&cur, "settomodify").and_then(|v| v.as_str().map(str::to_string)) {
                let flag = match prop_of(&cur, "setopertype").and_then(|v| v.as_str().map(str::to_string)).as_deref() {
                    Some("remove") => "remove",
                    _ => "add",
                };
                let lines = core.log.len();
                let _ = core.mel_checked(&format!("sets -{flag} {set} {};", fresh.join(" ")));
                core.log.truncate(lines);
            }
        }
        "artUserPaintCtx" if !fresh.is_empty() => {
            let value = prop_of(&cur, "value").and_then(|v| v.as_f64()).unwrap_or(1.0);
            if let Some(cb) = prop_of(&cur, "setValueCommand") {
                for item in &fresh {
                    let index = item.rsplit_once('[').and_then(|x| x.1.trim_end_matches(']').parse::<i64>().ok()).unwrap_or(0);
                    let _ = crate::attr_ui::call_for_value(core, &cb, &[json!(0), json!(index), json!(value)]);
                }
            }
        }
        "artAttrPaintVertexCtx" if !fresh.is_empty() => {
            let rgb = prop_of(&cur, "colorRGBValue").unwrap_or(json!([1.0, 1.0, 1.0]));
            let _ = core.exec("polyColorPerVertex", json!({"args": fresh, "rgb": rgb}));
        }
        _ => {}
    }
    with_ctx(|c| c.completing = false);
}

fn script_ctx_step(core: &mut Core, cur: &str, selection: &[String]) {
    let completing = with_ctx(|c| c.completing);
    let count = prop_of(cur, "setSelectionCount").and_then(|v| v.as_i64()).unwrap_or(0).max(0) as usize;
    let auto = prop_of(cur, "setAutoComplete").is_some_and(|v| truthy(&v));
    let enough = !selection.is_empty() && selection.len() >= count;
    if !(enough && (completing || (auto && count > 0))) {
        return;
    }
    if let Some(cb) = prop_of(cur, "finalCommandScript") {
        let run = match &cb {
            // The reference hands the picks to the script as $Selection1.
            Value::String(src) => {
                let items: Vec<String> = selection.iter().map(|x| format!("\"{x}\"")).collect();
                json!(format!("{{ string $Selection1[] = {{{}}}; {src} }}", items.join(",")))
            }
            o => o.clone(),
        };
        core.run_callback(&run, &[]);
    }
    if prop_of(cur, "exitUponCompletion").is_some_and(|v| truthy(&v)) {
        with_ctx(|c| c.current = "selectSuperContext".to_string());
        crate::core::push_ui_request("setTool|select".to_string());
    }
    let _ = core.exec_quiet("select", json!({"clear": true}), aztek_ops::CallSource::Script);
}

/// A command that only holds flags, and does what `act` says with them.
fn settings_command(s: &mut Session, name: &str, a: &Args, default: fn(&str) -> Value, act: fn(&mut Session, &Map<String, Value>)) -> CmdResult {
    if a.bool("query") {
        let Some(q) = asked(a) else { fail!("{name}: name what to ask about.") };
        return Ok(with_ctx(|c| c.settings.get(name).and_then(|x| x.get(&q).cloned())).unwrap_or_else(|| default(&q)));
    }
    let mut flags = a.flags.clone();
    flags.retain(|k, v| !matches!(k.as_str(), "query" | "edit" | "queried") && !v.is_null());
    with_ctx(|c| c.settings.entry(name.to_string()).or_default().extend(flags.clone()));
    act(s, &flags);
    Ok(Value::Null)
}

fn view_manip_act(s: &mut Session, f: &Map<String, Value>) {
    if let Some(v) = f.get("visible") {
        let cmd = json!({"args": ["viewAxis"], "visible": truthy(v)});
        let _ = s.run("headsUpDisplay", cmd);
    }
    if f.get("goHome").is_some_and(truthy) {
        let _ = s.run("viewSet", json!({"home": true}));
    }
    if f.get("fitToView").is_some_and(truthy) {
        let _ = s.run("viewFit", json!({}));
    }
}

fn manip_options_act(_s: &mut Session, f: &Map<String, Value>) {
    if let Some(v) = f.get("scale").or(f.get("handleSize")).and_then(Value::as_f64) {
        view_all("manipSize", json!(v));
    }
}

fn pan_zoom(s: &mut Session, a: &Args) -> CmdResult {
    let cam = a.named().into_iter().next();
    let mut call = Map::new();
    if let Some(c) = &cam {
        call.insert("args".to_string(), json!([c]));
    }
    let mut track = call.clone();
    let mut moved = false;
    for k in ["left", "right", "up", "down"] {
        if a.has(k) {
            track.insert(k.to_string(), json!(a.float(k, 0.0)));
            moved = true;
        }
    }
    if moved {
        s.run("track", Value::Object(track))?;
    }
    if a.has("zoomRatio") {
        let mut zoom = call;
        zoom.insert("orthoScale".to_string(), json!(a.float("zoomRatio", 1.0)));
        s.run("dolly", Value::Object(zoom))?;
    }
    Ok(Value::Null)
}

pub fn register(s: &mut Session) {
    s.register(add_ctx_flags(
        Cmd::new("manipPivot", "Puts the manipulator somewhere else until the selection changes: -p the place, -o the orientation, -reset, -q.", |s, a| {
            if a.bool("query") {
                let q = asked(a).unwrap_or_default();
                return with_ctx(|c| {
                    Ok(match q.as_str() {
                        "position" => json!(c.manip_pivot.unwrap_or([0.0; 3])),
                        "orientation" => c.settings.get("manipPivot").and_then(|x| x.get("orientation").cloned()).unwrap_or(json!([0.0, 0.0, 0.0])),
                        "valid" => json!(c.manip_pivot.is_some()),
                        "pinPivot" => json!(c.pivot_pinned),
                        _ => Value::Null,
                    })
                });
            }
            let selected: Vec<String> = s.run("ls", json!({"selection": true, "flatten": true})).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
            with_ctx(|c| {
                c.last_selection = selected;
                if a.bool("reset") || a.bool("resetPosition") {
                    c.manip_pivot = None;
                }
                if a.has("pinPivot") {
                    c.pivot_pinned = a.bool("pinPivot");
                    // Unpinned: the pivot goes back to following the selection.
                    if !c.pivot_pinned {
                        c.manip_pivot = None;
                    }
                }
                if let Some(p) = a.vec3("position") {
                    c.manip_pivot = Some(p);
                }
                if let Some(o) = a.get("orientation") {
                    c.settings.entry("manipPivot".to_string()).or_default().insert("orientation".to_string(), o.clone());
                }
                if a.bool("reset") || a.bool("resetOrientation") {
                    if let Some(x) = c.settings.get_mut("manipPivot") {
                        x.remove("orientation");
                    }
                }
            });
            view_all("redraw", json!(true));
            Ok(Value::Null)
        })
        .undoable(false),
        "position:p:a orientation:o:a pinPivot:pin:b reset:r:b resetOrientation:ro:b resetPosition:rp:b valid:v:b query:q:b edit:e:b queried::a",
    ));
    s.register(add_ctx_flags(
        Cmd::new("manipOptions", "How manipulators look and work: -scale, -handleSize and so on.", |s, a| {
            fn d(q: &str) -> Value {
                match q {
                    "scale" | "handleSize" => json!(1.0),
                    "lineSize" | "pointSize" => json!(1.0),
                    "refreshMode" => json!(1),
                    "preselectHighlight" | "showExtrudeSliders" | "linePick" | "rememberActiveHandle" => json!(true),
                    _ => json!(false),
                }
            }
            settings_command(s, "manipOptions", a, d, manip_options_act)
        })
        .undoable(false),
        "enableSmartDuplicate:esd:b enableSmartExtrude:ese:b forceRefresh:fr:b handleSize:hs:a hideManipOnCtrl:hmc:b hideManipOnShift:hms:b hideManipOnShiftCtrl:hsc:b linePick:lp:a lineSize:ls:a pivotRotateHandleOffset:pro:a planeHandleOffset:pho:a pointSize:ps:a preselectHighlight:psh:b refreshMode:rm:a relative:r:b rememberActiveHandle:rah:b rememberActiveHandleAfterToolSwitch:raa:b scale:s:a showExtrudeSliders:ses:b showPivotRotateHandle:spr:b showPlaneHandles:spl:b smartDuplicateType:sdt:a query:q:b edit:e:b queried::a",
    ));
    s.register(add_ctx_flags(
        Cmd::new("setXformManip", "The manipulator's settings: -showUnits, -suppress, -useRotatePivot, -useScalePivot, -worldSpace. Held; the views go by the tools' own.", |s, a| {
            fn d(q: &str) -> Value {
                json!(q == "worldSpace")
            }
            settings_command(s, "setXformManip", a, d, |_, _| {})
        })
        .undoable(false),
        "showUnits:su:b suppress:s:b useRotatePivot:urp:b useScalePivot:usp:b worldSpace:ws:b query:q:b edit:e:b queried::a",
    ));
    s.register(add_ctx_flags(
        Cmd::new("viewManip", "The view's orientation aid: -visible shows or hides the axes in the corner, -goHome, -fitToView; the rest are held.", |s, a| {
            fn d(q: &str) -> Value {
                match q {
                    "visible" | "dragSnap" | "drawCompass" => json!(q == "visible"),
                    "size" => json!("auto"),
                    "minOpacity" => json!(0.2),
                    _ => Value::Null,
                }
            }
            settings_command(s, "viewManip", a, d, view_manip_act)
        })
        .undoable(false),
        "bottomLeft:bl:b bottomRight:br:b compassAngle:ca:a dragSnap:ds:b drawCompass:dc:b fitToView:ftv:b frontParameter:fp:a goDefault:gd:b goHome:gh:b homeParameter:hp:a levelCamera:lc:b minOpacity:mo:a postCommand:pc:a preCommand:prc:a restoreCenter:rc:b selectionLockParameter:slp:a setHome:sh:b size:s:a topLeft:tl:b topRight:tr:b visible:v:b zoomToFitScene:zf:b query:q:b edit:e:b queried::a",
    ));
    s.register(
        Cmd::new("pixelMove", "pixelMove x y: moves the selection this many pixels in the view's plane.", |_s, a| {
            let n: Vec<f64> = a.positional.iter().filter_map(Value::as_f64).collect();
            let [x, y, ..] = n.as_slice() else { fail!("pixelMove: give x and y.") };
            crate::core::push_ui_request(format!("pixelMove|{}", json!({"x": x, "y": y})));
            Ok(Value::Null)
        })
        .positional("x y")
        .undoable(false),
    );
    s.register(add_ctx_flags(
        Cmd::new("panZoom", "panZoom [camera]: -left -right -up -down slide the camera, -zoomRatio scales what it sees. It moves the camera; there is no 2D film pan here.", |s, a| pan_zoom(s, a)).positional("[camera]").undoable(false),
        "left:l:a right:r:a up:u:a down:d:a zoomRatio:zr:a absolute:abs:b relative:rel:b",
    ));
    s.register(
        Cmd::new("ctxCompletion", "Tells the current tool to finish, as Enter does.", |_s, _a| {
            with_ctx(|c| c.completing = true);
            crate::core::push_ui_request("ctxCompleted".to_string());
            Ok(Value::Null)
        })
        .undoable(false),
    );
    s.register(
        Cmd::new("ctxAbort", "Drops what the current tool has so far.", |_s, _a| {
            with_ctx(|c| c.completing = false);
            view_all("undoToolPoint", json!(true));
            Ok(Value::Null)
        })
        .undoable(false),
    );
    for (kind, spec) in TYPES {
        let k: &'static str = kind;
        // A command the scene has, such as the paint tool's, stays.
        if s.commands.find(k).is_some() {
            continue;
        }
        let mut c = Cmd::new(k, &format!("A {k} tool context: create by name, -e and -q its settings. setToolTo makes it current."), move |s, a| context_command(k, s, a)).positional("[name]").undoable(false);
        c = parse_flags(c, spec);
        c = parse_flags(c, CTX_FLAGS);
        s.register(c);
    }

    s.register(
        Cmd::new("toolHasOptions", "Whether a tool context has settings to show.", |_s, a| {
            let Some(n) = a.pos_str(0) else { fail!("toolHasOptions: name a context.") };
            let k = with_ctx(|c| c.all.get(n).map(|x| x.kind.clone()));
            let Some(k) = k else { fail!("toolHasOptions: Object '{n}' not found.") };
            Ok(json!(!matches!(k.as_str(), "tumbleCtx" | "trackCtx" | "dollyCtx" | "rollCtx" | "boxZoomCtx" | "scriptCtx" | "userCtx")))
        })
        .positional("context")
        .undoable(false),
    );

    s.register(
        Cmd::new("setToolTo", "Make a tool context current, by name.", |s, a| {
            let Some(n) = a.pos_str(0) else { fail!("setToolTo: name a context.") };
            set_tool_to(s, n)
        })
        .positional("context")
        .undoable(false),
    );

    s.register(
        Cmd::new("currentCtx", "The name of the current tool context.", |_s, _a| Ok(json!(current())))
            .undoable(false)
            .flag(f("query", "q", ArgType::Bool))
            .flag(f("edit", "e", ArgType::Bool)),
    );

    s.register(
        Cmd::new("resetTool", "Put a tool context back to its default settings.", |_s, a| {
            let n = a.pos_str(0).map(str::to_string).unwrap_or_else(current);
            with_ctx(|c| {
                let Some(x) = c.all.get_mut(&n) else { fail!("resetTool: Object '{n}' not found.") };
                x.props.clear();
                Ok(Value::Null)
            })
        })
        .positional("[context]")
        .undoable(false),
    );

    s.register(
        Cmd::new("ctxEditMode", "Toggle the current tool's edit mode: the pivot, for the move, rotate and scale tools. -q reads it.", |_s, a| {
            if a.bool("query") {
                return Ok(json!(with_ctx(|c| c.edit_mode)));
            }
            let on = with_ctx(|c| {
                c.edit_mode = if a.has("buttonDown") || a.has("buttonUp") { a.bool("buttonDown") } else { !c.edit_mode };
                c.edit_mode
            });
            view_all("pivotMode", json!(on));
            Ok(Value::Null)
        })
        .undoable(false)
        .flag(f("query", "q", ArgType::Bool))
        .flag(f("buttonDown", "bd", ArgType::Bool))
        .flag(f("buttonUp", "bu", ArgType::Bool)),
    );

    s.register(
        Cmd::new("contextInfo", "About a tool context: -ex whether it exists, -c its class, -t its title.", |_s, a| {
            let Some(n) = a.pos_str(0) else { fail!("contextInfo: name a context.") };
            with_ctx(|c| {
                let x = c.all.get(n);
                if a.bool("exists") {
                    return Ok(json!(x.is_some()));
                }
                let Some(x) = x else { fail!("contextInfo: Object '{n}' not found.") };
                if a.bool("title") {
                    return Ok(x.props.get("title").cloned().unwrap_or(json!(n)));
                }
                if a.bool("image1") || a.bool("image2") || a.bool("image3") {
                    return Ok(json!(""));
                }
                Ok(json!(x.kind))
            })
        })
        .positional("context")
        .undoable(false)
        .flag(f("exists", "ex", ArgType::Bool))
        .flag(f("class", "c", ArgType::Bool))
        .flag(f("title", "t", ArgType::Bool))
        .flag(f("image1", "i1", ArgType::Bool))
        .flag(f("image2", "i2", ArgType::Bool))
        .flag(f("image3", "i3", ArgType::Bool)),
    );
}
