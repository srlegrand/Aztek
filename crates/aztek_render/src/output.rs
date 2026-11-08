//! Writing images: OpenEXR with every AOV as channels of one part (`diffuse_direct.R` and so on, the beauty as plain R,
//! G, B, A), through the pure Rust exr crate; PNG, TIFF, JPEG and TGA as 8 bits, sRGB encoded.

use std::path::Path;

use glam::Vec4;

use crate::film::Film;
use crate::texture::linear_to_srgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Exr,
    Png,
    Tiff,
    Jpeg,
    Tga,
}

impl Format {
    pub fn from_name(s: &str) -> Option<Format> {
        Some(match s.to_ascii_lowercase().trim_start_matches('.') {
            "exr" | "openexr" => Format::Exr,
            "png" => Format::Png,
            "tif" | "tiff" => Format::Tiff,
            "jpg" | "jpeg" => Format::Jpeg,
            "tga" | "targa" => Format::Tga,
            _ => return None,
        })
    }
    pub fn extension(self) -> &'static str {
        match self {
            Format::Exr => "exr",
            Format::Png => "png",
            Format::Tiff => "tif",
            Format::Jpeg => "jpg",
            Format::Tga => "tga",
        }
    }
    pub fn of_path(p: &Path) -> Option<Format> {
        Format::from_name(p.extension()?.to_str()?)
    }
}

/// 8 bit sRGB from linear, after exposure (stops) and a display gamma on top of sRGB (1 leaves it).
pub fn to_rgba8(px: &[Vec4], exposure: f32, gamma: f32) -> Vec<u8> {
    let k = 2f32.powf(exposure);
    let g = if gamma > 0.0 { 1.0 / gamma } else { 1.0 };
    let mut out = Vec::with_capacity(px.len() * 4);
    for p in px {
        for c in [p.x, p.y, p.z] {
            let v = linear_to_srgb((c * k).max(0.0)).min(1.0).powf(g);
            out.push((v * 255.0 + 0.5) as u8);
        }
        out.push((p.w.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
    }
    out
}

/// Writes one image of pixels in 8 bits.
pub fn write_ldr(path: &Path, w: u32, h: u32, px: &[Vec4], fmt: Format) -> Result<(), String> {
    let mut bytes = to_rgba8(px, 0.0, 1.0);
    // Eight bit images are seen as they are, not composited: opaque, the background already in them. EXR keeps alpha.
    for a in bytes.iter_mut().skip(3).step_by(4) {
        *a = 255;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match fmt {
        Format::Jpeg => {
            let rgb: Vec<u8> = bytes.chunks(4).flat_map(|c| [c[0], c[1], c[2]]).collect();
            image::save_buffer_with_format(path, &rgb, w, h, image::ColorType::Rgb8, image::ImageFormat::Jpeg).map_err(|e| e.to_string())
        }
        Format::Png => image::save_buffer_with_format(path, &bytes, w, h, image::ColorType::Rgba8, image::ImageFormat::Png).map_err(|e| e.to_string()),
        Format::Tiff => image::save_buffer_with_format(path, &bytes, w, h, image::ColorType::Rgba8, image::ImageFormat::Tiff).map_err(|e| e.to_string()),
        Format::Tga => image::save_buffer_with_format(path, &bytes, w, h, image::ColorType::Rgba8, image::ImageFormat::Tga).map_err(|e| e.to_string()),
        Format::Exr => write_exr(path, w, h, &[("RGBA".to_string(), px.to_vec())], false),
    }
}

/// Writes AOVs as channels of one EXR. `half` stores 16 bit floats.
pub fn write_exr(path: &Path, w: u32, h: u32, layers: &[(String, Vec<Vec4>)], half: bool) -> Result<(), String> {
    use exr::prelude::*;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut channels: Vec<AnyChannel<FlatSamples>> = Vec::new();
    for (name, px) in layers {
        let prefix = if name == "RGBA" || name == "beauty" { String::new() } else { format!("{name}.") };
        let names: &[&str] = if matches!(name.as_str(), "Z") { &["Z"] } else { &["R", "G", "B", "A"] };
        for (k, c) in names.iter().enumerate() {
            let data: Vec<f32> = px.iter().map(|p| p[k]).collect();
            let label = if name == "Z" { "Z".to_string() } else { format!("{prefix}{c}") };
            let samples = if half { FlatSamples::F16(data.iter().map(|&x| f16::from_f32(x)).collect()) } else { FlatSamples::F32(data) };
            channels.push(AnyChannel::new(label.as_str(), samples));
        }
    }
    let channels = AnyChannels::sort(channels.into());
    let layer = Layer::new((w as usize, h as usize), LayerAttributes::named("harold"), Encoding::FAST_LOSSLESS, channels);
    let image = Image::from_layer(layer);
    image.write().to_file(path).map_err(|e| e.to_string())
}

/// Writes a film: every AOV into one EXR, or for 8 bit formats the beauty to `path` and each AOV beside it with its
/// name before the extension.
pub fn write_film(film: &Film, path: &Path, fmt: Format, half: bool, extra: &[(String, Vec<Vec4>)]) -> Result<Vec<String>, String> {
    let mut layers: Vec<(String, Vec<Vec4>)> = film.aovs.iter().enumerate().map(|(i, (n, _))| (n.clone(), film.pixels(i))).collect();
    layers.extend(extra.iter().cloned());
    if fmt == Format::Exr {
        write_exr(path, film.width, film.height, &layers, half)?;
        return Ok(vec![path.display().to_string()]);
    }
    let mut written = Vec::new();
    for (name, px) in &layers {
        let p = if name == "RGBA" {
            path.to_path_buf()
        } else {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
            path.with_file_name(format!("{stem}_{name}.{}", fmt.extension()))
        };
        let px: Vec<Vec4> = if name == "Z" {
            // Depth as grey, nearest white, for a quick look.
            let far = px.iter().map(|p| p.x).filter(|z| z.is_finite()).fold(0.0f32, f32::max).max(1e-6);
            px.iter().map(|p| if p.x.is_finite() { Vec4::new(1.0 - p.x / far, 1.0 - p.x / far, 1.0 - p.x / far, 1.0) } else { Vec4::ZERO }).collect()
        } else {
            px.clone()
        };
        write_ldr(&p, film.width, film.height, &px, fmt)?;
        written.push(p.display().to_string());
    }
    Ok(written)
}
