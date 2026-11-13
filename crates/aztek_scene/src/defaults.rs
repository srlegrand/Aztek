//! The nodes every scene has, as the reference's empty scene has them: what `ls` lists after File > New.
//!
//! Two kinds. Default nodes (`ls -defaultNodes`) are made with the scene and never saved: a file names them as
//! `select -ne :time1` and sets what differs. The others are saved with every scene (`createNode lightLinker -s`,
//! `createNode displayLayer -n "defaultLayer"`) but are there before any file is read. None of them can be deleted or
//! renamed, nor can the four startup cameras.
//!
//! The names, types and order are the reference's (2026, which adds `openPBR_shader1`), checked against its saved files;
//! the attributes are those its node reference lists. What a file sets on one of them that isn't modelled here is kept
//! as the file wrote it and written back (`Graph::default_lines`).

use std::sync::Arc;

use glam::DVec3;

use crate::graph::Graph;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugType, PlugValue, NO_NODE};

/// Default nodes, in the order the reference makes and writes them: (name, type).
pub const DEFAULT_NODES: &[(&str, &str)] = &[
    ("time1", "time"),
    ("sequenceManager1", "sequenceManager"),
    ("hardwareRenderingGlobals", "hardwareRenderingGlobals"),
    ("renderPartition", "partition"),
    ("renderGlobalsList1", "renderGlobalsList"),
    ("defaultLightList1", "defaultLightList"),
    ("defaultShaderList1", "defaultShaderList"),
    ("postProcessList1", "postProcessList"),
    ("defaultRenderUtilityList1", "defaultRenderUtilityList"),
    ("defaultRenderingList1", "defaultRenderingList"),
    ("lightList1", "lightList"),
    ("defaultTextureList1", "defaultTextureList"),
    ("lambert1", "lambert"),
    ("standardSurface1", "standardSurface"),
    ("openPBR_shader1", "openPBRSurface"),
    ("particleCloud1", "particleCloud"),
    ("initialShadingGroup", "shadingEngine"),
    ("initialParticleSE", "shadingEngine"),
    ("initialMaterialInfo", "materialInfo"),
    ("shaderGlow1", "shaderGlow"),
    ("dof1", "dof"),
    ("defaultRenderGlobals", "renderGlobals"),
    ("defaultRenderQuality", "renderQuality"),
    ("defaultResolution", "resolution"),
    ("defaultLightSet", "objectSet"),
    ("defaultObjectSet", "objectSet"),
    ("defaultViewColorManager", "viewColorManager"),
    ("defaultColorMgtGlobals", "colorManagementGlobals"),
    ("hardwareRenderGlobals", "hwRenderGlobals"),
    ("characterPartition", "partition"),
    ("defaultHardwareRenderGlobals", "hwRenderGlobals"),
    ("ikSystem", "ikSystem"),
    ("hyperGraphInfo", "hyperGraphInfo"),
    ("hyperGraphLayout", "hyperLayout"),
    ("globalCacheControl", "globalCacheControl"),
    ("strokeGlobals", "strokeGlobals"),
    ("dynController1", "dynController"),
];

/// Saved with every scene, there before any is read: (name, type).
pub const SCENE_NODES: &[(&str, &str)] = &[
    ("lightLinker1", "lightLinker"),
    ("layerManager", "displayLayerManager"),
    ("defaultLayer", "displayLayer"),
    ("renderLayerManager", "renderLayerManager"),
    ("defaultRenderLayer", "renderLayer"),
];

/// The default nodes an empty scene of the reference's writes, though nothing changed them.
pub const ALWAYS_WRITTEN: &[&str] = &[
    "time1",
    "hardwareRenderingGlobals",
    "renderPartition",
    "renderGlobalsList1",
    "defaultShaderList1",
    "postProcessList1",
    "defaultRenderingList1",
    "standardSurface1",
    "openPBR_shader1",
    "initialShadingGroup",
    "initialParticleSE",
    "defaultRenderGlobals",
    "defaultResolution",
    "defaultColorMgtGlobals",
    "hardwareRenderGlobals",
];

/// Nodes that list others by connection; a file's connections into them are made again on writing, so reading them
/// connects nothing. (node, the listing attribute's short name)
pub const LISTS: &[(&str, &str)] = &[
    ("renderPartition", "st"),
    ("characterPartition", "st"),
    ("renderGlobalsList1", "rg"),
    ("renderGlobalsList1", "rq"),
    ("renderGlobalsList1", "rr"),
    ("defaultLightList1", "lda"),
    ("defaultShaderList1", "s"),
    ("postProcessList1", "p"),
    ("defaultRenderUtilityList1", "u"),
    ("defaultRenderingList1", "r"),
    ("lightList1", "l"),
    ("defaultTextureList1", "tx"),
    ("hyperGraphInfo", "b"),
    ("layerManager", "dli"),
    ("renderLayerManager", "rlmi"),
];

/// A default node: made with the scene, not saved, named in files as `:name`.
pub fn is_default_node(name: &str) -> bool {
    DEFAULT_NODES.iter().any(|d| d.0 == name)
}

/// A node every scene has, default or saved.
pub fn is_scene_node(name: &str) -> bool {
    is_default_node(name) || SCENE_NODES.iter().any(|d| d.0 == name)
}

/// The type a default or scene node has, by name.
pub fn type_of(name: &str) -> Option<&'static str> {
    DEFAULT_NODES.iter().chain(SCENE_NODES).find(|d| d.0 == name).map(|d| d.1)
}

/// Whether the node can't be deleted or renamed: a node every scene has, a startup camera, or a locked node.
pub fn undeletable(g: &Graph, id: crate::plug::NodeId) -> bool {
    let Some(n) = g.node(id) else { return false };
    n.locked || (n.namespace.is_empty() && is_scene_node(n.name())) || crate::cameras::is_startup(g, crate::dag::transform_of(g, id))
}

/// Why `delete` or `rename` refuses the node, or None.
pub fn refusal(g: &Graph, id: crate::plug::NodeId, verb: &str) -> Option<String> {
    let n = g.node(id)?;
    let name = n.name().to_string();
    if n.locked {
        return Some(format!("{verb}: {name} is locked and can't be {verb}d"));
    }
    if n.namespace.is_empty() && is_scene_node(&name) {
        return Some(format!("{verb}: {name} is a default node and can't be {verb}d"));
    }
    None
}

// ---------------------------------------------------------------------------------------------------------------------
// Types

const IN: u32 = flags::INPUT | flags::STORABLE;

fn base(name: &str, short: &str, ty: PlugType, v: PlugValue, fl: u32) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: v, flags: fl, ..Default::default() }
}

/// One attribute from a line of a type's table: `kind long short default [hidden] [out] [nostore]`. Kinds: bool, int,
/// float, time, str, msg (a message), msgs (an array of them), ints (an array of numbers), c3 (a colour: children
/// long+R/G/B and short+r/g/b, or as given after the default: `c3 long short r,g,b cr,cg,cb`), v3 (as c3, X/Y/Z).
/// A string's default has ~ for a space and - for nothing.
fn attr(line: &str) -> AttrDef {
    let w: Vec<&str> = line.split_whitespace().collect();
    let (kind, name, short) = (w[0], w[1], w[2]);
    let d = w.get(3).copied().unwrap_or("0");
    let rest = &w[4.min(w.len())..];
    let mut fl = IN;
    if rest.contains(&"out") {
        fl = flags::OUTPUT;
    }
    if rest.contains(&"nostore") {
        fl &= !flags::STORABLE;
    }
    if rest.contains(&"hidden") {
        fl |= flags::HIDDEN;
    }
    let num = |s: &str| s.parse::<f64>().unwrap_or(0.0);
    let mut a = match kind {
        "bool" => base(name, short, PlugType::Bool, PlugValue::Bool(d == "1"), fl),
        "int" => base(name, short, PlugType::Int, PlugValue::Int(num(d) as i64), fl),
        "float" | "time" => base(name, short, PlugType::Float, PlugValue::Float(num(d)), fl),
        "str" => base(name, short, PlugType::String, PlugValue::String(if d == "-" { String::new() } else { d.replace('~', " ") }), fl),
        "msg" => base(name, short, PlugType::Int, PlugValue::Int(0), flags::INPUT | flags::OUTPUT | flags::MESSAGE),
        "msgs" => base(name, short, PlugType::Int, PlugValue::Int(0), flags::INPUT | flags::ARRAY | flags::MESSAGE),
        "ints" => base(name, short, PlugType::Int, PlugValue::Int(0), fl | flags::ARRAY),
        "c3" | "v3" => {
            let v: Vec<f64> = d.split(',').map(num).collect();
            let v = DVec3::new(v[0], *v.get(1).unwrap_or(&v[0]), *v.get(2).unwrap_or(&v[0]));
            let mut a = base(name, short, PlugType::Vec3, PlugValue::Vec3(v), fl | if kind == "c3" { flags::COLOR } else { 0 });
            let ends: [&str; 3] = if kind == "c3" { ["R", "G", "B"] } else { ["X", "Y", "Z"] };
            for k in 0..3 {
                a.child_names[k] = format!("{name}{}", ends[k]);
                a.child_short[k] = format!("{short}{}", ends[k].to_lowercase());
            }
            if let Some(given) = rest.iter().find(|x| x.contains(',')) {
                for (k, s) in given.split(',').enumerate().take(3) {
                    a.child_short[k] = s.to_string();
                }
            }
            a
        }
        other => panic!("defaults: attribute kind {other}"),
    };
    if kind == "time" {
        a.doc = "A time, in frames.".into();
    }
    a
}

/// Each type: (name, classification for the Hypershade or "", what it is, its attributes one per line).
const TYPES: &[(&str, &str, &str, &str)] = &[
    ("time", "", "The scene's current time: outTime follows the frame, for nodes connected to it.", "
        time outTime o 0 out
        time unwarpedTime unw 0 out
        bool enableTimewarp etw 0
        time timecodeMayaStart tms 0
        time timecodeProductionStart tps 0"),
    ("sequenceManager", "", "The scene's camera sequence: its shots and range.", "
        bool enabled en 0 nostore
        time outTime o 1 hidden
        bool rangeEnabled ren 0
        time rangeMax rmax 0
        time rangeMin rmin 0
        msgs sequences seqts
        bool skipGaps sg 0
        msg trackInfoManager tim"),
    ("hardwareRenderingGlobals", "", "Viewport 2.0's settings for the scene.", "
        bool alphaCutPrepass acpp 0
        int renderMode rm 4
        int lightingMode lm 1
        float bloomAmount blat 1
        bool bloomEnable blen 0
        str bloomFilter blfl -
        float bloomFilterAux blfa 0
        float bloomFilterRadius blfr 0
        float bloomThreshold blth 0
        int bumpBakeResolution bbr 64
        int colorBakeResolution cbr 64
        bool compressSharedVertexData csvd 1
        bool consolidateWorld cons 1
        str currentRendererName cur - hidden
        float defaultLightIntensity dli 3.14
        bool enableTextureIblMaxRes eimr 0
        bool enableTextureMaxRes etmr 1
        bool floatingPointRTEnable fprt 0
        int floatingPointRTFormat rtfm 0
        bool gammaCorrectionEnable gamm 0
        float gammaValue gmmv 2.2
        int holdOutDetailMode hodm 0
        bool holdOutMode hom 1
        float hwFogAlpha hfa 1
        c3 hwFogColor hfc 0.5
        float hwFogDensity hfd 0.1
        bool hwFogEnable hfon 0 hidden
        float hwFogEnd hfe 100
        int hwFogFalloff hff 0
        float hwFogStart hfs 0
        bool hwInstancing hwi 0
        bool isCustomUVBorderColor isuvbc 0 hidden nostore
        bool lineAAEnable laa 0
        int maxHardwareLights mhl 8
        int motionBlurAtlasSizeX mbasx 8
        int motionBlurAtlasSizeY mbasy 4
        bool motionBlurCurved mbc 0
        bool motionBlurEnable mbe 0
        float motionBlurFadeAmount mbfa 0.5
        float motionBlurFadeEmphasis mbfe 0
        str motionBlurFadeFilter mbff -
        int motionBlurMultiframeChartSizeX mbcsx 256
        int motionBlurMultiframeChartSizeY mbcsy 256
        bool motionBlurMultiframeEnable mbme 0
        int motionBlurSampleCount mbsc 1
        float motionBlurShutterOpenFraction mbsof 0.2
        int motionBlurType mbt 0
        int multiSampleCount aasc 3
        bool multiSampleEnable msaa 0
        int multiSampleQuality aasq 0
        bool quadDrawAlwaysOnTop qdaot 0 hidden nostore
        float quadDrawOverrideTransparency qdot -1 hidden nostore
        bool renderDepthOfField adof 1 hidden
        str renderOverrideName on -
        bool singleSidedLighting sslt 0
        float ssaoAmount aoam 1
        bool ssaoEnable aoon 0
        str ssaoFilter aoft GaussVariableFilter1
        int ssaoFilterRadius aofr 16
        int ssaoRadius aora 16
        int ssaoSamples aosm 1
        int textureAutoMaxResolution tamr 2048 nostore
        int textureIblMaxResolution tir 2048
        int textureMaxResMode tmrm 0
        int textureMaxResolution tmr 2048
        int transparencyAlgorithm ta 1
        float transparencyQuality tq 0.33
        bool transparentShadow ts 0
        bool useMaximumHardwareLights uml 1 hidden
        int vertexAnimationCache vac 0 hidden
        bool xrayJointDisplay jxr 0
        bool xrayMode xry 0"),
    ("partition", "", "Sets no two of which share a member: renderPartition holds the shading groups.", "
        str annotation an -
        msg environment envr
        int partitionType pt 0 hidden
        msgs sets st"),
    ("renderGlobalsList", "", "Lists the render settings nodes.", "
        msgs renderGlobals rg
        msgs renderQualities rq
        msgs renderResolutions rr"),
    ("defaultLightList", "", "Lists the lights that light everything.", ""),
    ("defaultShaderList", "", "Lists the scene's materials.", "msgs shaders s"),
    ("postProcessList", "", "Lists the effects applied after rendering.", "msgs postProcesses p"),
    ("defaultRenderUtilityList", "", "Lists the scene's rendering utilities.", "msgs utilities u"),
    ("defaultRenderingList", "", "Lists the render layers.", "msgs rendering r"),
    ("lightList", "", "Lists the scene's lights.", "msgs lights l"),
    ("defaultTextureList", "", "Lists the scene's textures.", "msgs textures tx"),
    ("openPBRSurface", "shader/surface", "The OpenPBR surface material.", "
        c3 baseColor bc 0.8
        float baseDiffuseRoughness dr 0
        float baseMetalness m 0
        float baseWeight b 1
        c3 coatColor ctc 1
        float coatDarkening ctdk 1
        float coatIOR ctior 1.6
        float coatRoughness ctr 0
        float coatRoughnessAnisotropy cta 0
        float coatWeight ct 0
        c3 emissionColor ec 1
        float emissionLuminance elu 0
        c3 fuzzColor fzc 1
        float fuzzRoughness fzr 0.5
        float fuzzWeight fw 0
        float geometryOpacity geop 1
        bool geometryThinWalled getw 0
        c3 specularColor sc 1 scr,scg,spb
        float specularIOR sior 1.5
        float specularRoughness sr 0.3
        float specularRoughnessAnisotropy sa 0
        float specularWeight s 1
        c3 subsurfaceColor subc 0.8
        float subsurfaceRadius subs 1
        c3 subsurfaceRadiusScale subr 1,0.5,0.25
        float subsurfaceScatterAnisotropy suba 0
        float subsurfaceWeight sub 0
        float thinFilmIOR tfior 1.4
        float thinFilmThickness tft 0.5
        float thinFilmWeight tfw 0
        c3 transmissionColor trc 1
        float transmissionDepth td 0
        float transmissionDispersionAbbeNumber tdan 20
        float transmissionDispersionScale tdi 0
        c3 transmissionScatter ts 0
        float transmissionScatterAnisotropy tsa 0
        float transmissionWeight t 0
        c3 outColor oc 0 out
        c3 outTransparency ot 0 out"),
    ("particleCloud", "shader/volume/particle", "The volume material particles render with.", "
        c3 blobMap m 1
        c3 color c 0,0.58824,0.644
        float density d 1
        float diffuseCoeff dc 0
        float filterRadius flrs 0
        float glowIntensity gi 0
        c3 incandescence i 0
        float noise n 0.75
        float noiseAnimRate nanr 0
        float noiseAspect na 0
        float noiseFreq nf 0.15
        int renderState rdst 0
        float roundness rdns 1
        float solidCoreSize scs 0
        c3 surfaceColor sc 0.4
        float translucence trsl 0.5
        float translucenceCoeff tc 0
        c3 transparency t 0.5
        c3 outColor oi 0 out
        c3 outGlowColor ogi 0 out ogr,ogg,ogb
        c3 outParticleEmission oe 0 out
        c3 outTransparency ot 0 out"),
    ("shaderGlow", "postprocess/glow", "The glow and halo applied to glowing materials after rendering.", "
        bool autoExposure ae 1
        c3 glowColor gc 1 gr,gg,gb
        float glowEccentricity gecc 0.1
        int glowFilterWidth gfw 1
        float glowIntensity gi 1
        float glowOpacity go 0
        float glowRadialNoise gn 0
        float glowRingFrequency grf 0.5
        float glowRingIntensity gri 0
        float glowSpread gs 0.05
        float glowStarLevel gv 0
        c3 haloColor hc 1 hr,hg,hb
        float haloEccentricity hecc 0.1
        int haloFilterWidth hfw 1
        float haloIntensity hi 1
        float haloOpacity ho 0
        float haloRadialNoise hn 0
        float haloRingFrequency hrf 0.5
        float haloRingIntensity hri 0
        float haloSpread hs 0.3
        float haloStarLevel hv 0
        float quality qual 0.5
        float radialFrequency rf 0.5
        float rotation ra 0
        float starPoints sp 4
        float threshold th 0"),
    ("dof", "", "Depth of field applied after rendering.", ""),
    ("renderQuality", "", "The software renderer's quality settings.", "
        float blueThreshold bct 0.6
        float coverageThreshold cct 0.125
        int edgeAntiAliasing eaa 3
        float greenThreshold gct 0.3
        int maxShadingSamples mss 8
        int maxVisibilitySamples mvm 4
        int shadingSamples ss 1
        int particleSamples pss 1
        int pixelFilterType pft 1
        float pixelFilterWidthX pfwx 2.2
        float pixelFilterWidthY pfwy 2.2
        float plugInFilterWeight pifw 1
        bool enableRaytracing ert 0
        float rayTraceBias rtb 0
        float redThreshold rct 0.4
        int reflections rfl 1
        int refractions rfr 6
        bool renderSample rsdn 0 hidden
        int shadows sl 2
        bool useMultiPixelFilter ufil 0
        int visibilitySamples mvs 1
        int volumeSamples vs 1"),
    ("viewColorManager", "", "The view's colour transform: exposure, contrast and profiles.", "
        float contrast c 0
        float contrastPivot cp 0.18
        int displayColorProfile dp 3
        float exposure exp 0
        int imageColorProfile ip 2
        str lutFile lut -"),
    ("colorManagementGlobals", "", "The scene's colour management: config, working space, view and output transforms.", "
        bool cmEnabled cme 1
        bool configFileEnabled cfe 0
        str configFilePath cfp -
        str defaultInputSpaceName din - nostore
        str displayName dn -
        bool outputTransformEnabled ote 0
        str outputTransformName otn -
        bool outputTransformUseColorConversion otc 0
        bool outputUseViewTransform ovt 1
        bool playblastOutputTransformEnabled pote 1
        str playblastOutputTransformName potn -
        bool playblastOutputTransformUseColorConversion potc 0
        bool playblastOutputUseViewTransform povt 1
        str viewName vn -
        str viewTransformName vtn -
        str workingSpaceName wsn -"),
    ("hwRenderGlobals", "", "The hardware renderer's settings.", "
        bool antiAliasPolygons aap 0
        int alphaSource as 0
        c3 backgroundColor bc 0
        int byFrame bf 1
        bool cameraIcons cai 0
        bool collisionIcons coi 0
        bool displayShadows sd 0
        int drawStyle ds 3
        float edgeSmoothing es 1
        bool emitterIcons ei 0
        int endFrame ef 10
        int extension ex 3
        bool fieldIcons fii 0
        str filename fn im
        bool fullImageResolution fir 0
        bool geometryMask gh 0
        bool grid gr 0
        int imageFormat if 0
        str imfPluginKey imfkey -
        str imfPluginKeyExt imfext -
        bool lightIcons li 0
        int lightingMode lm 0
        bool lineSmoothing ls 0
        float motionBlur mb 0
        bool multiPassRendering mpr 0
        int renderPasses rp 0
        str resolution res ntsc_4d~646~485~1.333
        int startFrame sf 1
        bool texturing txt 1
        bool transformIcons ti 0
        bool writeZDepth wzd 0"),
    ("hyperGraphInfo", "", "The Hypergraph's bookmarks.", "msgs bookmarks b"),
    ("hyperLayout", "", "Where the Hypergraph places nodes.", "
        bool allNodesFreeform anf 0 hidden
        str imageName img -
        float imageScale ims 1 hidden"),
    ("globalCacheControl", "", "Whether caches are read and written.", "
        int enableStatus ebls 0
        bool writeEnable webl 0"),
    ("strokeGlobals", "", "Paint Effects' scene settings.", "
        float canvasScale pcs 1
        bool forceDepth fdp 1
        bool forceRealLights frl 1
        bool forceTubeDirAlongPath ftd 1
        v3 lightDirection ldr 0.2,-0.9,-0.5 ldx,ldy,ldz
        float sceneScale pss 5
        bool sceneWrapH swh 0
        bool sceneWrapV swv 0
        bool useCanvasLight ucl 1
        bool wrapH wrh 0
        bool wrapV wrv 0"),
    ("dynController", "", "The dynamics solver's scene settings.", ""),
    ("displayLayerManager", "", "Numbers the display layers and knows the current one.", "
        int currentDisplayLayer cdl 0
        ints displayLayerId dli 0"),
    ("renderLayerManager", "", "Numbers the render layers and knows the current one.", "
        int currentRenderLayer crl 0
        ints renderLayerId rlmi 0"),
    ("renderLayer", "", "A render layer: defaultRenderLayer is the scene as it is.", "
        str attributeOverrideScript aos -
        int displayOrder do 0
        bool global g 0
        bool isDefaultPrecompTemplateOverride idpo 1
        ints layerChildren rlc 0
        int layerParent rlp 0
        int psdAlphaChannel ac 0
        int psdBlendMode bm 0
        str precompTemplate tpc -
        int identification rlid 0
        bool renderable rndr 1
        int drawColor c 0
        msg shadingGroupOverride sgo"),
];

/// The classification of a type here, for shadingNode and the Hypershade.
pub fn classification(ty: &str) -> Option<&'static str> {
    TYPES.iter().find(|t| t.0 == ty && !t.1.is_empty()).map(|t| t.1)
}


/// time1: outTime and unwarpedTime are the current frame.
struct Time;
impl NodeBehavior for Time {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn compute(&mut self, ctx: &EvalContext, d: &mut NodeData) {
        let frame = ctx.time * crate::anim::fps();
        d.set_output(TIME_OUT, PlugValue::Float(frame));
        d.set_output(TIME_UNWARPED, PlugValue::Float(frame));
    }
}
const TIME_OUT: usize = 0;
const TIME_UNWARPED: usize = 1;

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        for &(name, _, doc, table) in TYPES {
            if types::find(name).is_some() {
                continue;
            }
            let own: Vec<AttrDef> = table.lines().map(str::trim).filter(|l| !l.is_empty()).map(attr).collect();
            let time = name == "time";
            types::register_type(NodeType {
                name: name.into(),
                doc: doc.into(),
                icon: if time { "clock".into() } else { String::new() },
                time_dependent: time,
                own_attrs: own,
                create: Some(if time { Arc::new(|| Box::new(Time) as Box<dyn NodeBehavior>) } else { Arc::new(|| Box::new(crate::node::Plain) as Box<dyn NodeBehavior>) }),
                ..Default::default()
            })
            .expect(name);
        }
        crate::light_cmds::register_types();
    });
}

/// The values the reference's empty scene gives these nodes where they differ from their type's defaults, as its
/// saved files show them: (node, short attribute, the setAttr line's value text, as a file's line is kept).
const SCENE_VALUES: &[(&str, &str, &str)] = &[
    ("hardwareRenderingGlobals", "otfna", "-type \"stringArray\" 22 \"NURBS Curves\" \"NURBS Surfaces\" \"Polygons\" \"Subdiv Surface\" \"Particles\" \"Particle Instance\" \"Fluids\" \"Strokes\" \"Image Planes\" \"UI\" \"Lights\" \"Cameras\" \"Locators\" \"Joints\" \"IK Handles\" \"Deformers\" \"Motion Trails\" \"Components\" \"Hair Systems\" \"Follicles\" \"Misc. UI\" \"Ornaments\""),
    ("hardwareRenderingGlobals", "otfva", "-type \"Int32Array\" 22 0 1 1 1 1 1 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0"),
    ("hardwareRenderGlobals", "ctrs", "256"),
    ("hardwareRenderGlobals", "btrs", "512"),
];

/// Modelled values the empty scene sets: (node, attribute, value).
fn scene_settings() -> Vec<(&'static str, &'static str, PlugValue)> {
    let grey = PlugValue::Vec3(DVec3::splat(0.4));
    let s = |t: &str| PlugValue::String(t.into());
    vec![
        ("hardwareRenderingGlobals", "floatingPointRTEnable", PlugValue::Bool(true)),
        ("hardwareRenderingGlobals", "floatingPointRTFormat", PlugValue::Int(1)),
        ("standardSurface1", "baseColor", grey.clone()),
        ("standardSurface1", "specularRoughness", PlugValue::Float(0.5)),
        ("openPBR_shader1", "baseColor", grey),
        ("openPBR_shader1", "specularRoughness", PlugValue::Float(0.5)),
        ("defaultColorMgtGlobals", "configFileEnabled", PlugValue::Bool(true)),
        ("defaultColorMgtGlobals", "configFilePath", s("<MAYA_RESOURCES>/OCIO-configs/Maya2022-default/config.ocio")),
        ("defaultColorMgtGlobals", "viewTransformName", s("ACES 1.0 SDR-video (sRGB)")),
        ("defaultColorMgtGlobals", "viewName", s("ACES 1.0 SDR-video")),
        ("defaultColorMgtGlobals", "displayName", s("sRGB")),
        ("defaultColorMgtGlobals", "workingSpaceName", s("ACEScg")),
        ("defaultColorMgtGlobals", "outputTransformName", s("ACES 1.0 SDR-video (sRGB)")),
        ("defaultColorMgtGlobals", "playblastOutputTransformName", s("ACES 1.0 SDR-video (sRGB)")),
        ("defaultRenderLayer", "global", PlugValue::Bool(true)),
    ]
}

/// Makes every node the scene lacks of those every scene has, off the undo stack, with the values an empty scene gives
/// them. The cameras and sets come from `cameras::make_defaults`, lambert1 and the shading groups from `shading`.
pub fn make(g: &mut Graph) {
    types::register_builtins();
    register_types();
    if types::find("ikSystem").is_none() {
        crate::ik::register_types();
    }
    g.without_undo(|g| {
        let mut made = Vec::new();
        for &(name, ty) in DEFAULT_NODES.iter().chain(SCENE_NODES) {
            if g.find(name).ok().flatten().is_some() {
                continue;
            }
            let Some(t) = types::find(ty) else { continue };
            if let Ok(id) = g.create_node(&t, name, NO_NODE, false) {
                made.push((name, id));
            }
        }
        if made.is_empty() {
            return;
        }
        for (node, attr, v) in scene_settings() {
            if !made.iter().any(|m| m.0 == node) {
                continue;
            }
            if let Ok(p) = g.plug(&format!("{node}.{attr}")) {
                let _ = g.set_value(p, v);
            }
        }
        for &(node, attr, text) in SCENE_VALUES {
            if made.iter().any(|m| m.0 == node) {
                g.keep_default_line(node, attr, text);
            }
        }
    });
}

/// The order the default nodes are written in.
pub fn write_order(name: &str) -> usize {
    DEFAULT_NODES.iter().position(|d| d.0 == name).unwrap_or(usize::MAX)
}
