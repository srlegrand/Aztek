//! The render view's side of the C interface: status (which also restarts an interactive render after an edit), the
//! image of an AOV or a kept snapshot for display, and batch rendering from the command line with no window.

use std::ffi::c_char;

use aztek_scene::harold::{commands, Request};
use aztek_scene::Session;
use serde_json::json;

use crate::{cstr, out_json, with};

/// The render view's poll: status as JSON. An interactive render restarts here when the scene changed.
#[no_mangle]
pub extern "C" fn aztek_render_status() -> *mut c_char {
    with(|h| {
        let t = std::time::Instant::now();
        let restarted = commands::tick(&mut h.core.s);
        let mut v = commands::status(&h.core.s);
        if restarted {
            v["restartSeconds"] = json!(t.elapsed().as_secs_f64());
        }
        out_json(v)
    })
}

/// An AOV of the current render, or a kept snapshot (index >= 0), as 8 bit RGBA for display after exposure (stops) and
/// gamma. Null when there is nothing. Given back to aztek_image_free with its size.
#[no_mangle]
pub extern "C" fn aztek_render_image(aov: *const c_char, snapshot: i32, exposure: f64, gamma: f64, width: *mut i32, height: *mut i32) -> *mut u8 {
    let aov = cstr(aov);
    let img = with(|h| commands::image(&h.core.s, if aov.is_empty() { "RGBA" } else { &aov }, (snapshot >= 0).then_some(snapshot as usize)));
    let Some((w, h, px)) = img else { return std::ptr::null_mut() };
    let px = if aov == "Z" {
        // Depth as grey, near white, for looking at.
        let far = px.iter().map(|p| p.x).filter(|z| z.is_finite()).fold(0.0f32, f32::max).max(1e-6);
        px.iter().map(|p| if p.x.is_finite() { glam::Vec4::new(1.0 - p.x / far, 1.0 - p.x / far, 1.0 - p.x / far, 1.0) } else { glam::Vec4::ZERO }).collect()
    } else if aov == "N" {
        px.iter().map(|p| (p.truncate() * 0.5 + glam::Vec3::splat(0.5)).extend(1.0)).collect()
    } else {
        px
    };
    let bytes = aztek_render::output::to_rgba8(&px, exposure as f32, gamma as f32);
    unsafe {
        *width = w as i32;
        *height = h as i32;
    }
    Box::into_raw(bytes.into_boxed_slice()) as *mut u8
}

fn usage() -> i32 {
    eprintln!(
        "aztek -render [options] file.ma\n  -s start  -e end  -b by      frames (default: the render settings', or the current frame)\n  -cam name                    camera\n  -x width  -y height          resolution\n  -rd dir                      output folder\n  -im prefix                   file name prefix, tokens <Scene> <Camera> <RenderLayer> <AOV>\n  -of format                   exr, png, tif, jpg or tga\n  -as n                        camera (AA) samples"
    );
    2
}

/// Batch rendering: `aztek -render -s 1 -e 10 -cam persp file.ma`. Runs with no display; returns the exit code.
#[no_mangle]
pub extern "C" fn aztek_render_batch(argc: i32, argv: *const *const c_char) -> i32 {
    let args: Vec<String> = (0..argc.max(0) as usize).map(|i| cstr(unsafe { *argv.add(i) })).collect();
    let mut file = None;
    let (mut start, mut end, mut by) = (None, None, 1.0);
    let mut req = Request::default();
    let (mut dir, mut prefix, mut format, mut aa) = (None, None, None, None);
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        let next = args.get(i + 1).cloned();
        let take = |i: &mut usize| {
            *i += 1;
            next.clone()
        };
        match a {
            "-render" | "-r" | "-batch" => {}
            "-s" => start = take(&mut i).and_then(|x| x.parse::<f64>().ok()),
            "-e" => end = take(&mut i).and_then(|x| x.parse::<f64>().ok()),
            "-b" => by = take(&mut i).and_then(|x| x.parse::<f64>().ok()).unwrap_or(1.0),
            "-cam" | "-camera" => req.camera = take(&mut i),
            "-x" => req.width = take(&mut i).and_then(|x| x.parse().ok()),
            "-y" => req.height = take(&mut i).and_then(|x| x.parse().ok()),
            "-rd" => dir = take(&mut i),
            "-im" => prefix = take(&mut i),
            "-of" => format = take(&mut i),
            "-as" => aa = take(&mut i).and_then(|x| x.parse::<i64>().ok()),
            "-h" | "-help" | "--help" => return usage(),
            other if other.starts_with('-') => {
                eprintln!("aztek -render: unknown option {other}");
                return usage();
            }
            other => file = Some(other.to_string()),
        }
        i += 1;
    }
    let Some(file) = file else { return usage() };
    crate::core::install_logger();
    let mut s = Session::new();
    let t = std::time::Instant::now();
    match aztek_scene::ma_file::open_scene(&mut s, &file) {
        Ok(w) => eprintln!("Harold: opened {file} in {:.1}s ({} warnings)", t.elapsed().as_secs_f64(), w.len()),
        Err(e) => {
            eprintln!("Harold: {file}: {e}");
            return 1;
        }
    }
    let set = |s: &mut Session, plug: &str, v: serde_json::Value| {
        if let Err(e) = s.run("setAttr", json!({"args": [plug, v]})) {
            eprintln!("Harold: {plug}: {e}");
        }
    };
    if let Some(p) = prefix {
        set(&mut s, "defaultRenderGlobals.imageFilePrefix", json!(p));
    }
    if let Some(f) = format {
        set(&mut s, "defaultArnoldDriver.ai_translator", json!(f.to_lowercase()));
    }
    if let Some(n) = aa {
        set(&mut s, "defaultArnoldRenderOptions.AASamples", json!(n));
    }
    if let Some(d) = &dir {
        // The output folder by way of the prefix: an absolute prefix keeps it.
        let (_, common, _) = aztek_scene::harold::settings::read(&mut s.graph, &mut Vec::new());
        let p = if common.prefix.is_empty() { "<Scene>".to_string() } else { common.prefix };
        set(&mut s, "defaultRenderGlobals.imageFilePrefix", json!(format!("{}/{p}", d.trim_end_matches('/'))));
    }
    let fps = aztek_scene::anim::fps();
    let frames: Vec<f64> = match (start, end) {
        (Some(a), Some(b)) => {
            let mut v = Vec::new();
            let mut x = a;
            while x <= b + 1e-9 {
                v.push(x);
                x += by.abs().max(1e-3);
            }
            v
        }
        (Some(a), None) => vec![a],
        _ => {
            let (_, common, _) = aztek_scene::harold::settings::read(&mut s.graph, &mut Vec::new());
            if common.animation {
                let mut v = Vec::new();
                let mut x = common.start;
                while x <= common.end + 1e-9 {
                    v.push(x);
                    x += common.by;
                }
                v
            } else {
                vec![s.graph.time() * fps]
            }
        }
    };
    match commands::render_frames(&mut s, &req, &frames) {
        Ok(files) => {
            for l in &s.extras.harold.log {
                eprintln!("Harold: {l}");
            }
            for f in files {
                println!("{f}");
            }
            0
        }
        Err(e) => {
            eprintln!("Harold: {e}");
            1
        }
    }
}
