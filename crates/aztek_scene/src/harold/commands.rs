//! Render commands: `render` as Maya's, `hrender` as Harold's own with `arnoldRender` accepted for scripts written for
//! it, `renderSettings` for output names, and `renderWindowEditor` for the render view's images. The render view's
//! state lives here: the running render, interactive rendering, and kept snapshots.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_render::render::{JobOptions, Render};
use glam::Vec4;
use serde_json::{json, Value};

use super::{settings, Request, Translator};
use crate::session::Session;

type Cmd = CommandDef<Session>;

fn swatch_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.pos_str(0) else { fail!("haroldSwatch: name a node") };
    let Some(id) = s.graph.find(name).ok().flatten() else { fail!("haroldSwatch: no node {name}") };
    let sig = format!("{:x}", super::swatch::signature(&mut s.graph, id));
    if a.bool("signature") {
        return Ok(json!(sig));
    }
    let size = a.int("size", 128).clamp(8, 1024) as u32;
    let project = crate::shading::project_dir(s);
    let scene = s.scene_path.clone();
    let Some(pic) = super::swatch::of_node(&mut s.graph, id, size, scene.as_deref(), project.as_deref()) else { fail!("haroldSwatch: {name} has nothing to show") };
    let file = match a.get("file").and_then(Value::as_str) {
        Some(f) => PathBuf::from(f),
        None => std::env::temp_dir().join(format!("aztek_swatch_{}_{}_{size}.ppm", std::process::id(), name.replace(['|', ':', '/'], "_"))),
    };
    let mut bytes = format!("P6\n{} {}\n255\n", pic.width, pic.height).into_bytes();
    bytes.extend(pic.rgba.chunks(4).flat_map(|c| [c[0], c[1], c[2]]));
    if let Err(e) = std::fs::write(&file, bytes) {
        fail!("haroldSwatch: {}: {e}", file.display());
    }
    Ok(json!({"file": file.to_string_lossy(), "width": pic.width, "height": pic.height, "signature": sig, "mean": pic.mean}))
}

/// A kept image in the render view.
#[derive(Clone)]
pub struct Snapshot {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Vec4>,
    pub info: String,
}

#[derive(Default)]
pub struct State {
    pub translator: Translator,
    pub job: Option<Render>,
    /// Interactive: restart when the scene changes.
    pub ipr: bool,
    /// The graph version and time the running render was made from.
    pub made_from: (u64, u64),
    pub request: Request,
    pub snapshots: Vec<Snapshot>,
    pub last_paths: Vec<String>,
    pub log: Vec<String>,
    /// Display settings of the render view.
    pub exposure: f32,
    pub gamma: f32,
    /// Seconds the last translation took, and the restart latency of the last interactive change.
    pub translate_seconds: f64,
}

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

fn scene_name(s: &Session) -> String {
    s.scene_path.as_deref().and_then(|p| Path::new(p).file_stem()).map_or("untitled".into(), |x| x.to_string_lossy().to_string())
}

/// Where images go: the project's images folder, the scene's folder's images, or a temporary folder.
pub fn images_dir(s: &mut Session) -> PathBuf {
    if let Some(p) = crate::shading::project_dir(s).filter(|p| !p.is_empty()) {
        return Path::new(&p).join("images");
    }
    if let Some(p) = s.scene_path.as_deref().and_then(|p| Path::new(p).parent()) {
        return p.join("images");
    }
    std::env::temp_dir().join("aztek_images")
}

fn time_key(s: &Session) -> u64 {
    s.graph.time().to_bits()
}

/// Starts a render of the current frame. Interactive renders leave a core free and show a preview first.
pub fn start(s: &mut Session, req: Request, interactive: bool) -> Result<(), String> {
    if let Some(mut j) = s.extras.harold.job.take() {
        j.cancel();
    }
    let t = std::time::Instant::now();
    let mut tr = std::mem::take(&mut s.extras.harold.translator);
    let r = super::translate(s, &mut tr, &Request { interactive, ..req.clone() });
    s.extras.harold.translator = tr;
    let (scene, _) = r?;
    let made_from = (s.graph.version(), time_key(s));
    let st = &mut s.extras.harold;
    st.translate_seconds = t.elapsed().as_secs_f64();
    st.log.extend(scene.warnings.iter().cloned());
    st.request = req;
    let threads = scene.options.threads;
    st.job = Some(Render::start(Arc::new(scene), JobOptions { preview: interactive, threads }));
    st.made_from = made_from;
    Ok(())
}

/// For the render view's poll: restarts an interactive render when the scene or the frame changed. True if it did.
pub fn tick(s: &mut Session) -> bool {
    if !s.extras.harold.ipr {
        return false;
    }
    let now = (s.graph.version(), time_key(s));
    if now == s.extras.harold.made_from {
        return false;
    }
    let req = s.extras.harold.request.clone();
    if let Err(e) = start(s, req, true) {
        s.extras.harold.log.push(e);
    }
    true
}

pub fn status(s: &Session) -> Value {
    let st = &s.extras.harold;
    let status = st.job.as_ref().map(Render::status).unwrap_or_default();
    let (w, h) = st.job.as_ref().map_or((0, 0), |j| (j.scene().options.width, j.scene().options.height));
    json!({
        "running": st.job.as_ref().is_some_and(|_| !status.done),
        "ipr": st.ipr,
        "pass": status.pass,
        "passes": status.passes,
        "done": status.done,
        "preview": status.preview,
        "rays": status.rays,
        "raysPerSecond": status.rays_per_second,
        "seconds": status.elapsed.as_secs_f64(),
        "memoryMB": status.memory_bytes as f64 / 1048576.0,
        "translateSeconds": st.translate_seconds,
        "width": w,
        "height": h,
        "aovs": st.job.as_ref().map(Render::aov_names).unwrap_or_default(),
        "snapshots": st.snapshots.iter().map(|x| json!({"name": x.name, "info": x.info, "width": x.width, "height": x.height})).collect::<Vec<_>>(),
        "camera": st.job.as_ref().map(|j| j.scene().camera.name.clone()).unwrap_or_default(),
        "warnings": st.log.iter().rev().take(50).cloned().collect::<Vec<_>>(),
        "exposure": st.exposure,
        "gamma": if st.gamma > 0.0 { st.gamma } else { 1.0 },
    })
}

/// The current image of an AOV, or of a snapshot by index.
pub fn image(s: &Session, aov: &str, snapshot: Option<usize>) -> Option<(u32, u32, Vec<Vec4>)> {
    let st = &s.extras.harold;
    if let Some(k) = snapshot {
        let sn = st.snapshots.get(k)?;
        return Some((sn.width, sn.height, sn.pixels.clone()));
    }
    let job = st.job.as_ref()?;
    if aov == "denoised" {
        let film = job.film();
        let c = film.pixels(film.index_of("RGBA")?);
        let al = film.index_of("albedo").map(|i| film.pixels(i));
        let n = film.index_of("N").map(|i| film.pixels(i));
        let out = aztek_render::denoise::denoise(film.width, film.height, &c, al.as_deref(), n.as_deref(), &Default::default());
        return Some((film.width, film.height, out));
    }
    job.image(aov)
}

/// Keeps the current image in the render view's list.
pub fn snapshot(s: &mut Session) -> Option<usize> {
    let (w, h, px) = image(s, "RGBA", None)?;
    let st = &mut s.extras.harold;
    let status = st.job.as_ref().map(Render::status).unwrap_or_default();
    let cam = st.job.as_ref().map(|j| j.scene().camera.name.clone()).unwrap_or_default();
    let n = st.snapshots.len() + 1;
    st.snapshots.push(Snapshot { name: format!("Snapshot {n}"), width: w, height: h, pixels: px, info: format!("{cam}, {:.1}s", status.elapsed.as_secs_f64()) });
    Some(st.snapshots.len() - 1)
}

/// Writes the current render: every AOV into an EXR, or the beauty and each AOV beside it for other formats.
pub fn write_current(s: &mut Session, path: &str) -> Result<Vec<String>, String> {
    let st = &s.extras.harold;
    let Some(job) = st.job.as_ref() else { return Err("nothing rendered yet".into()) };
    let film = job.film();
    let p = Path::new(path);
    let fmt = aztek_render::output::Format::of_path(p).ok_or_else(|| format!("{path}: no image format for that extension"))?;
    let mut extra = Vec::new();
    if job.scene().options.denoise {
        if let Some((_, _, px)) = image(s, "denoised", None) {
            extra.push(("denoised".to_string(), px));
        }
    }
    aztek_render::output::write_film(&film, p, fmt, false, &extra)
}

/// Renders frames to completion and writes them, as batch rendering does. Returns the files written.
pub fn render_frames(s: &mut Session, req: &Request, frames: &[f64]) -> Result<Vec<String>, String> {
    let fps = crate::anim::fps();
    let dir = images_dir(s);
    let scene = scene_name(s);
    let mut written = Vec::new();
    let original = s.graph.time();
    for &fr in frames {
        s.graph.set_time(fr / fps);
        s.graph.evaluate();
        let mut tr = std::mem::take(&mut s.extras.harold.translator);
        let r = super::translate(s, &mut tr, req);
        s.extras.harold.translator = tr;
        let (sc, common) = r?;
        let warnings = sc.warnings.clone();
        let cam = sc.camera.name.clone();
        let denoise = sc.options.denoise;
        let threads = sc.options.threads;
        let (film, status) = aztek_render::render::render(Arc::new(sc), threads);
        for w in &warnings {
            log::warn!("Harold: {w}");
        }
        let frame = if frames.len() > 1 || common.animation { Some(fr) } else { None };
        let path = settings::output_path(&common, &scene, &cam, "masterLayer", None, frame, &dir.display().to_string());
        let fmt = aztek_render::output::Format::from_name(&common.format).unwrap_or(aztek_render::output::Format::Exr);
        let mut extra = Vec::new();
        if denoise {
            let c = film.pixels(0);
            let al = film.index_of("albedo").map(|i| film.pixels(i));
            let n = film.index_of("N").map(|i| film.pixels(i));
            extra.push(("denoised".to_string(), aztek_render::denoise::denoise(film.width, film.height, &c, al.as_deref(), n.as_deref(), &Default::default())));
        }
        let files = aztek_render::output::write_film(&film, Path::new(&path), fmt, common.half, &extra)?;
        s.extras.harold.log.push(format!("frame {fr}: {:.2}s, {:.1} M rays/s, {}", status.elapsed.as_secs_f64(), status.rays_per_second / 1e6, files.join(", ")));
        written.extend(files);
    }
    s.graph.set_time(original);
    s.graph.evaluate();
    s.extras.harold.last_paths = written.clone();
    Ok(written)
}

fn request_from(a: &Args) -> Request {
    let num = |k: &str| a.get(k).and_then(Value::as_f64).filter(|x| *x > 0.0).map(|x| x as u32);
    let region = a.get("region").and_then(Value::as_array).filter(|v| v.len() == 4).map(|v| {
        let n: Vec<u32> = v.iter().map(|x| x.as_f64().unwrap_or(0.0).max(0.0) as u32).collect();
        [n[0], n[1], n[2], n[3]]
    });
    Request {
        camera: a.get("camera").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string).or_else(|| a.pos_str(0).map(str::to_string)),
        width: num("width"),
        height: num("height"),
        region,
        interactive: false,
        aovs: Vec::new(),
    }
}

/// "1 10", "1-10", "1..10 2", "5": the frames of a sequence flag.
fn parse_sequence(text: &str) -> Vec<f64> {
    let t = text.replace("..", " ").replace(['-', ':'], " ");
    let n: Vec<f64> = t.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    match n.as_slice() {
        [a] => vec![*a],
        [a, b] => frames(*a, *b, 1.0),
        [a, b, step, ..] => frames(*a, *b, *step),
        _ => Vec::new(),
    }
}

fn frames(a: f64, b: f64, step: f64) -> Vec<f64> {
    let step = step.abs().max(1e-3);
    let mut out = Vec::new();
    let mut x = a;
    while x <= b + 1e-9 && out.len() < 100_000 {
        out.push(x);
        x += step;
    }
    out
}

fn hrender(s: &mut Session, a: &Args) -> CmdResult {
    if let Some(mode) = a.get("ipr").and_then(Value::as_str) {
        match mode {
            "start" | "refresh" => {
                let mut req = request_from(a);
                req.interactive = true;
                s.extras.harold.ipr = true;
                start(s, req, true).map_err(aztek_ops::CommandError::new)?;
            }
            "stop" => {
                s.extras.harold.ipr = false;
                if let Some(mut j) = s.extras.harold.job.take() {
                    j.cancel();
                    s.extras.harold.job = Some(j);
                }
            }
            "pause" => s.extras.harold.ipr = false,
            other => fail!("hrender: -ipr takes start, stop, refresh or pause, not {other}"),
        }
        return Ok(json!(true));
    }
    let req = request_from(a);
    let seq = a.get("seq").and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_f64().map(|x| x.to_string())));
    let batch = a.bool("batch");
    let frames = match seq {
        Some(t) => parse_sequence(&t),
        None if batch => {
            let (_, common, _) = settings::read(&mut s.graph, &mut Vec::new());
            if common.animation {
                frames(common.start, common.end, common.by)
            } else {
                vec![s.graph.time() * crate::anim::fps()]
            }
        }
        None => Vec::new(),
    };
    if frames.is_empty() && a.bool("async") {
        // The render view's Render Current Frame: start and come back at once; the view polls.
        s.extras.harold.ipr = false;
        start(s, req, false).map_err(aztek_ops::CommandError::new)?;
        return Ok(json!(true));
    }
    if frames.is_empty() {
        // The current frame into the render view: wait for it, as `render` does.
        start(s, req, false).map_err(aztek_ops::CommandError::new)?;
        if let Some(j) = s.extras.harold.job.as_mut() {
            j.wait();
        }
        let dir = images_dir(s);
        let (_, common, _) = settings::read(&mut s.graph, &mut Vec::new());
        let cam = s.extras.harold.job.as_ref().map(|j| j.scene().camera.name.clone()).unwrap_or_default();
        let name = scene_name(s);
        let path = settings::output_path(&common, &name, &cam, "masterLayer", None, None, &dir.join("tmp").display().to_string());
        let files = write_current(s, &path).map_err(aztek_ops::CommandError::new)?;
        s.extras.harold.last_paths = files.clone();
        return Ok(json!(files.first().cloned().unwrap_or_default()));
    }
    let files = render_frames(s, &req, &frames).map_err(aztek_ops::CommandError::new)?;
    Ok(json!(files))
}

/// Maya's `render`: the current frame through the given camera, waiting, returning the image's path.
fn render_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let r = hrender(s, a)?;
    Ok(match r {
        Value::Array(v) => v.into_iter().next().unwrap_or(Value::String(String::new())),
        other => other,
    })
}

fn render_settings(s: &mut Session, a: &Args) -> CmdResult {
    let (_, common, _) = settings::read(&mut s.graph, &mut Vec::new());
    let name = scene_name(s);
    let cam = a.get("camera").and_then(Value::as_str).map(str::to_string).or_else(|| super::find_camera(&s.graph, None).and_then(|c| s.graph.node(c).map(|n| n.name().to_string()))).unwrap_or_default();
    let dir = if a.bool("fullPath") { images_dir(s).display().to_string() } else { String::new() };
    let frame = |f: f64| if common.animation { Some(f) } else { None };
    let one = |f: Option<f64>| settings::output_path(&common, &name, &cam, "masterLayer", None, f, &dir);
    if a.bool("firstImageName") {
        return Ok(json!([one(frame(common.start))]));
    }
    if a.bool("lastImageName") {
        return Ok(json!([one(frame(common.end))]));
    }
    if a.bool("imageGenericName") {
        let mut c = common.clone();
        c.padding = 1;
        let p = settings::output_path(&c, &name, &cam, "masterLayer", None, frame(0.0), &dir);
        return Ok(json!([p.replace(".0.", ".#.")]));
    }
    if a.bool("fullPathTemp") {
        return Ok(json!([images_dir(s).join("tmp").display().to_string()]));
    }
    Ok(json!([one(frame(common.start))]))
}

fn render_window_editor(s: &mut Session, a: &Args) -> CmdResult {
    let query = a.bool("query");
    if let Some(p) = a.get("writeImage").and_then(Value::as_str) {
        let files = write_current(s, p).map_err(aztek_ops::CommandError::new)?;
        return Ok(json!(files));
    }
    if a.bool("snapShot") && !query {
        return Ok(json!(snapshot(s).map_or(-1, |k| k as i64)));
    }
    if a.bool("removeAllImages") {
        s.extras.harold.snapshots.clear();
        return Ok(json!(true));
    }
    if let Some(k) = a.get("removeImage").and_then(Value::as_i64) {
        let v = &mut s.extras.harold.snapshots;
        if (k as usize) < v.len() {
            v.remove(k as usize);
        }
        return Ok(json!(true));
    }
    if let Some(e) = a.get("exposure").and_then(Value::as_f64).filter(|_| !query) {
        s.extras.harold.exposure = e as f32;
    }
    if let Some(gm) = a.get("gamma").and_then(Value::as_f64).filter(|_| !query) {
        s.extras.harold.gamma = gm as f32;
    }
    if query {
        let q = crate::system_cmds::q_one(a).unwrap_or_default();
        let st = &s.extras.harold;
        return Ok(match q.as_str() {
            "nbImages" | "nim" => json!(st.snapshots.len()),
            "exposure" | "exp" => json!(st.exposure),
            "gamma" | "ga" => json!(if st.gamma > 0.0 { st.gamma } else { 1.0 }),
            "currentCamera" | "crc" => json!(st.job.as_ref().map(|j| j.scene().camera.name.clone()).unwrap_or_default()),
            _ => json!(status(s)),
        });
    }
    Ok(json!("renderView"))
}

pub fn register(c: &mut Commands<Session>) {
    let common = |cmd: Cmd| {
        cmd.flag(f("camera", "cam", ArgType::String, "Camera to render through."))
            .flag(f("width", "w", ArgType::Int, "Image width."))
            .flag(f("height", "h", ArgType::Int, "Image height."))
            .flag(f("region", "reg", ArgType::List, "x0 y0 x1 y1, pixels from the top left."))
    };
    c.register(
        Cmd::new("haroldSwatch", "A shading node as a small picture rendered by Harold: a material on a ball, a texture or utility flat over the UV square. Writes a .ppm file and returns its path, size and the node's signature; -signature alone gives the signature, which changes when the node or anything connected into it does.", swatch_cmd)
            .positional("node")
            .flag(f("size", "s", ArgType::Int, "Pixels a side; 128 without it."))
            .flag(f("file", "f", ArgType::String, "Where to write the picture; a temporary file without it."))
            .flag(f("signature", "sig", ArgType::Bool, "Only the signature."))
            .undoable(false),
    );
    for (name, doc) in [("hrender", "Renders with Harold: the current frame, a sequence (-seq), every frame the settings name (-batch), or interactively (-ipr start)."), ("arnoldRender", "hrender under the name other scripts use.")] {
        c.register(
            common(Cmd::new(name, doc, hrender).positional("camera"))
                .flag(f("seq", "seq", ArgType::Any, "Frames: \"1 10\", \"1 10 2\" or one frame."))
                .flag(f("batch", "b", ArgType::Bool, "Render the frames the render settings name and write them."))
                .flag(f("ipr", "ipr", ArgType::String, "start, stop, refresh or pause interactive rendering."))
                .flag(f("async", "as", ArgType::Bool, "Start the current frame and return at once; the render view shows it as it refines."))
                .undoable(false),
        );
    }
    c.register(common(Cmd::new("render", "Renders the current frame and returns the image's path.", render_cmd).positional("camera")).flag(f("batch", "b", ArgType::Bool, "Render the frames the render settings name.")).undoable(false));
    c.register(
        crate::system_cmds::q_flags(Cmd::new("renderSettings", "Output file names the render settings give.", render_settings))
            .flag(f("firstImageName", "fin", ArgType::Bool, "The first frame's file name."))
            .flag(f("lastImageName", "lin", ArgType::Bool, "The last frame's file name."))
            .flag(f("imageGenericName", "ign", ArgType::Bool, "The file name with # for the frame."))
            .flag(f("fullPath", "fp", ArgType::Bool, "With the images folder."))
            .flag(f("fullPathTemp", "fpt", ArgType::Bool, "The folder of images the render view keeps."))
            .flag(f("camera", "cam", ArgType::String, "For this camera."))
            .flag(f("layer", "lyr", ArgType::String, "For this render layer."))
            .undoable(false),
    );
    c.register(
        crate::system_cmds::q_flags(Cmd::new("renderWindowEditor", "The render view's images: write, keep, remove, exposure and gamma.", render_window_editor).positional("editor"))
            .flag(f("writeImage", "wi", ArgType::String, "Write the current image to a file."))
            .flag(f("snapShot", "snp", ArgType::Bool, "Keep the current image."))
            .flag(f("removeAllImages", "ra", ArgType::Bool, "Forget kept images."))
            .flag(f("removeImage", "ri", ArgType::Int, "Forget one kept image."))
            .flag(f("nbImages", "nim", ArgType::Bool, "How many images are kept."))
            .flag(f("exposure", "exp", ArgType::Float, "Display exposure, stops."))
            .flag(f("gamma", "ga", ArgType::Float, "Display gamma."))
            .flag(f("currentCamera", "crc", ArgType::Bool, "The camera rendered through."))
            .undoable(false),
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn sequences() {
        assert_eq!(super::parse_sequence("1 3"), vec![1.0, 2.0, 3.0]);
        assert_eq!(super::parse_sequence("1..5 2"), vec![1.0, 3.0, 5.0]);
        assert_eq!(super::parse_sequence("7"), vec![7.0]);
    }
}
