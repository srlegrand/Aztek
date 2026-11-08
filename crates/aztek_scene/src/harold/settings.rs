//! Render settings from the scene's nodes: defaultRenderGlobals, defaultResolution, the Harold options node and its
//! filter, driver and AOV nodes.

use aztek_render::scene::{FilterKind, Options};

use super::network::{get_b, get_f, get_s};
use crate::graph::Graph;
use crate::plug::NodeId;

pub const OPTIONS: &str = "defaultArnoldRenderOptions";
pub const FILTER: &str = "defaultArnoldFilter";
pub const DRIVER: &str = "defaultArnoldDriver";
pub const GLOBALS: &str = "defaultRenderGlobals";
pub const RESOLUTION: &str = "defaultResolution";

/// What the Common tab holds: where frames go and which.
#[derive(Clone, Debug)]
pub struct Common {
    pub prefix: String,
    /// File format by extension: exr, png, tif, jpg, tga.
    pub format: String,
    pub animation: bool,
    pub start: f64,
    pub end: f64,
    pub by: f64,
    pub padding: usize,
    pub half: bool,
    pub merge_aovs: bool,
}

/// Motion blur: frame offsets of the shutter and how many keys to take.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub enabled: bool,
    pub deform: bool,
    pub camera: bool,
    pub start: f64,
    pub end: f64,
    pub steps: usize,
}

fn find(g: &Graph, name: &str, ty: &str) -> Option<NodeId> {
    if let Ok(Some(id)) = g.find(name) {
        return Some(id);
    }
    g.nodes().into_iter().find(|&id| g.node(id).is_some_and(|n| n.ty().name == ty))
}

/// Maya's imageFormat numbers for the formats Harold writes.
pub fn format_name(code: i64, key: &str) -> String {
    if !key.is_empty() && super::FORMATS.contains(&key.to_ascii_lowercase().as_str()) {
        return key.to_ascii_lowercase();
    }
    match code {
        32 => "png",
        3 => "tif",
        8 => "jpg",
        19 => "tga",
        _ => "exr",
    }
    .to_string()
}

pub fn format_code(name: &str) -> i64 {
    match name {
        "png" => 32,
        "tif" | "tiff" => 3,
        "jpg" | "jpeg" => 8,
        "tga" => 19,
        _ => 51,
    }
}

pub fn read(g: &mut Graph, warnings: &mut Vec<String>) -> (Options, Common, Motion) {
    let mut o = Options::default();
    let res = find(g, RESOLUTION, "resolution");
    if let Some(r) = res {
        o.width = get_f(g, r, "width", 960.0).max(1.0) as u32;
        o.height = get_f(g, r, "height", 540.0).max(1.0) as u32;
        o.pixel_aspect = get_f(g, r, "pixelAspect", 1.0) as f32;
    }
    let mut motion = Motion { enabled: false, deform: true, camera: false, start: -0.25, end: 0.25, steps: 2 };
    if let Some(a) = find(g, OPTIONS, "aiOptions") {
        let int = |g: &mut Graph, n: &str, d: u32| get_f(g, a, n, d as f64).max(0.0) as u32;
        o.aa_samples = int(g, "AASamples", 3).max(1);
        o.diffuse_samples = int(g, "GIDiffuseSamples", 2);
        o.specular_samples = int(g, "GISpecularSamples", 2);
        o.transmission_samples = int(g, "GITransmissionSamples", 2);
        o.sss_samples = int(g, "GISssSamples", 2);
        o.volume_samples = int(g, "GIVolumeSamples", 2);
        o.total_depth = int(g, "GITotalDepth", 10);
        o.diffuse_depth = int(g, "GIDiffuseDepth", 1);
        o.specular_depth = int(g, "GISpecularDepth", 1);
        o.transmission_depth = int(g, "GITransmissionDepth", 8);
        o.volume_depth = int(g, "GIVolumeDepth", 0);
        o.transparency_depth = int(g, "autoTransparencyDepth", 10);
        o.low_light_threshold = get_f(g, a, "lowLightThreshold", 0.001) as f32;
        if get_b(g, a, "use_sample_clamp", false) {
            o.aa_clamp = Some(get_f(g, a, "AASampleClamp", 10.0) as f32);
            o.clamp_aovs = get_b(g, a, "use_sample_clamp_AOVs", false);
        }
        o.indirect_clamp = get_f(g, a, "indirectSampleClamp", 10.0) as f32;
        if o.indirect_clamp <= 0.0 {
            o.indirect_clamp = f32::INFINITY;
        }
        o.adaptive = get_b(g, a, "enableAdaptiveSampling", false);
        o.aa_samples_max = int(g, "AASamplesMax", 20).max(o.aa_samples);
        o.adaptive_threshold = get_f(g, a, "AAAdaptiveThreshold", 0.015) as f32;
        o.seed = int(g, "AASeed", 1);
        o.texture_max_memory_mb = int(g, "textureMaxMemoryMB", 4096) as usize;
        o.ignore_textures = get_b(g, a, "ignoreTextures", false);
        o.ignore_shadows = get_b(g, a, "ignoreShadows", false);
        o.ignore_sss = get_b(g, a, "ignoreSss", false);
        o.ignore_atmosphere = get_b(g, a, "ignoreAtmosphere", false);
        o.ignore_lights = get_b(g, a, "ignoreLights", false);
        o.ignore_dof = get_b(g, a, "ignoreDof", false);
        o.ignore_motion_blur = get_b(g, a, "ignoreMotionBlur", false);
        o.denoise = get_b(g, a, "denoiseBeauty", false);
        o.background = super::network::get_v(g, a, "background", glam::Vec3::ZERO);
        let threads = get_f(g, a, "threads", 0.0) as i64;
        if !get_b(g, a, "threads_autodetect", true) && threads > 0 {
            o.threads = threads as usize;
        }
        o.bucket_size = int(g, "bucketSize", 64).clamp(8, 256);
        motion.enabled = get_b(g, a, "motion_blur_enable", false) && !o.ignore_motion_blur;
        motion.deform = get_b(g, a, "mb_object_deform_enable", true);
        motion.camera = get_b(g, a, "mb_camera_enable", false);
        motion.steps = (get_f(g, a, "motion_steps", 2.0) as usize).clamp(2, 15);
        let length = get_f(g, a, "motion_frames", 0.5);
        (motion.start, motion.end) = match get_f(g, a, "range_type", 1.0) as i64 {
            0 => (0.0, length),
            2 => (-length, 0.0),
            3 => (get_f(g, a, "motion_start", -0.25), get_f(g, a, "motion_end", 0.25)),
            _ => (-length * 0.5, length * 0.5),
        };
    }
    if let Some(f) = find(g, FILTER, "aiAOVFilter") {
        let t = get_s(g, f, "ai_translator");
        o.filter = FilterKind::parse(if t.is_empty() { "gaussian" } else { &t });
        o.filter_width = get_f(g, f, "width", 2.0) as f32;
    }
    // AOVs: every enabled aiAOV node.
    for id in g.nodes() {
        if !g.node(id).is_some_and(|n| n.ty().name == "aiAOV") {
            continue;
        }
        if !get_b(g, id, "enabled", true) {
            continue;
        }
        let name = get_s(g, id, "name");
        if name.is_empty() {
            continue;
        }
        if aztek_render::film::aov_kind(&name, &["default".to_string()]).is_none() && !name.starts_with("RGBA_") {
            warnings.push(format!("AOV {name}: not one Harold makes, left out"));
            continue;
        }
        if !o.aovs.contains(&name) {
            o.aovs.push(name);
        }
    }
    let mut common = Common { prefix: String::new(), format: "exr".into(), animation: false, start: 1.0, end: 10.0, by: 1.0, padding: 4, half: false, merge_aovs: true };
    if let Some(r) = find(g, GLOBALS, "renderGlobals") {
        common.prefix = get_s(g, r, "imageFilePrefix");
        common.animation = get_b(g, r, "animation", false);
        common.start = get_f(g, r, "startFrame", 1.0);
        common.end = get_f(g, r, "endFrame", 10.0);
        common.by = get_f(g, r, "byFrameStep", 1.0).max(1e-3);
        common.padding = get_f(g, r, "extensionPadding", 4.0).clamp(1.0, 8.0) as usize;
        let key = get_s(g, r, "imfPluginKey");
        common.format = format_name(get_f(g, r, "imageFormat", 51.0) as i64, &key);
        let ren = get_s(g, r, "currentRenderer");
        if !ren.is_empty() && !matches!(ren.as_str(), "arnold" | "harold") {
            warnings.push(format!("renderer {ren} in the file: rendered with Harold"));
        }
    }
    if let Some(d) = find(g, DRIVER, "aiAOVDriver") {
        let t = get_s(g, d, "ai_translator");
        if !t.is_empty() && super::FORMATS.contains(&t.as_str()) {
            common.format = t;
        }
        common.half = get_b(g, d, "halfPrecision", false);
        common.merge_aovs = get_b(g, d, "mergeAOVs", true);
    }
    (o, common, motion)
}

/// The output path for a frame: the prefix with its tokens filled, the frame number padded, the extension.
pub fn output_path(common: &Common, scene: &str, camera: &str, layer: &str, aov: Option<&str>, frame: Option<f64>, dir: &str) -> String {
    let mut p = if common.prefix.is_empty() { "<Scene>".to_string() } else { common.prefix.clone() };
    let fill = |p: &mut String, token: &str, v: &str| {
        for t in [token.to_string(), token.to_lowercase()] {
            *p = p.replace(&t, v);
        }
    };
    fill(&mut p, "<Scene>", scene);
    fill(&mut p, "<Camera>", camera);
    fill(&mut p, "<RenderLayer>", layer);
    fill(&mut p, "<Layer>", layer);
    let aov_name = aov.unwrap_or("beauty");
    let had_aov = p.contains("<AOV>") || p.contains("<RenderPass>");
    fill(&mut p, "<AOV>", aov_name);
    fill(&mut p, "<RenderPass>", aov_name);
    if let (Some(a), false) = (aov, had_aov) {
        p = format!("{p}_{a}");
    }
    if let Some(f) = frame {
        p = format!("{p}.{:0width$}", f.round() as i64, width = common.padding);
    }
    let ext = &common.format;
    if dir.is_empty() || p.starts_with('/') {
        format!("{p}.{ext}")
    } else {
        format!("{dir}/{p}.{ext}")
    }
}
