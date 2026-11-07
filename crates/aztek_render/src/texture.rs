//! Image textures: loaded on first use by whichever render thread asks, kept as a mip pyramid, shared by every material
//! that names the same file and colour space. UDIM and UV tile tokens pick a file per tile. A memory limit drops the
//! finest levels of new images once it is reached, so a scene that doesn't fit renders with softer textures rather than
//! failing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use glam::{Vec2, Vec4};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ColorSpace {
    #[default]
    Srgb,
    /// Scene linear: values used as they are.
    Linear,
    /// Data, never converted: normal maps, roughness, masks.
    Raw,
}

impl ColorSpace {
    /// Maya's and OCIO's names, read tolerantly.
    pub fn parse(s: &str) -> ColorSpace {
        let l = s.to_ascii_lowercase();
        if l.contains("raw") || l == "utility - raw" || l == "data" || l == "non-color" {
            ColorSpace::Raw
        } else if l.contains("linear") || l.contains("acescg") || l.contains("aces2065") {
            ColorSpace::Linear
        } else {
            ColorSpace::Srgb
        }
    }
}

#[inline]
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_lut() -> &'static [f32; 256] {
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0.0; 256];
        for (i, v) in t.iter_mut().enumerate() {
            *v = srgb_to_linear(i as f32 / 255.0);
        }
        t
    })
}

#[derive(Clone, Debug)]
enum Texels {
    /// Eight bits, decoded through the sRGB curve when `srgb`.
    U8(Vec<[u8; 4]>),
    F32(Vec<[f32; 4]>),
}

#[derive(Clone, Debug)]
struct Level {
    w: u32,
    h: u32,
    texels: Texels,
}

#[derive(Debug)]
pub struct Image {
    levels: Vec<Level>,
    srgb: bool,
    pub has_alpha: bool,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
}

impl Level {
    #[inline]
    fn fetch(&self, x: i32, y: i32, srgb: bool) -> Vec4 {
        let x = x.rem_euclid(self.w as i32) as usize;
        let y = y.rem_euclid(self.h as i32) as usize;
        let i = y * self.w as usize + x;
        match &self.texels {
            Texels::U8(t) => {
                let p = t[i];
                if srgb {
                    let l = srgb_lut();
                    Vec4::new(l[p[0] as usize], l[p[1] as usize], l[p[2] as usize], p[3] as f32 / 255.0)
                } else {
                    Vec4::new(p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32) / 255.0
                }
            }
            Texels::F32(t) => Vec4::from_array(t[i]),
        }
    }

    fn bilinear(&self, uv: Vec2, srgb: bool) -> Vec4 {
        // Image rows go down, V goes up.
        let x = uv.x * self.w as f32 - 0.5;
        let y = (1.0 - uv.y) * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as i32, y0 as i32);
        let a = self.fetch(x0, y0, srgb);
        let b = self.fetch(x0 + 1, y0, srgb);
        let c = self.fetch(x0, y0 + 1, srgb);
        let d = self.fetch(x0 + 1, y0 + 1, srgb);
        a.lerp(b, fx).lerp(c.lerp(d, fx), fy)
    }

    fn half(&self, srgb: bool) -> Level {
        let w = (self.w / 2).max(1);
        let h = (self.h / 2).max(1);
        let mut acc = vec![Vec4::ZERO; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let (x2, y2) = (x as i32 * 2, y as i32 * 2);
                let s = self.fetch(x2, y2, srgb) + self.fetch((x2 + 1).min(self.w as i32 - 1), y2, srgb) + self.fetch(x2, (y2 + 1).min(self.h as i32 - 1), srgb) + self.fetch((x2 + 1).min(self.w as i32 - 1), (y2 + 1).min(self.h as i32 - 1), srgb);
                acc[(y * w + x) as usize] = s * 0.25;
            }
        }
        let texels = match &self.texels {
            Texels::U8(_) => Texels::U8(
                acc.iter()
                    .map(|v| {
                        let e = |c: f32| if srgb { linear_to_srgb(c) } else { c };
                        let q = |c: f32| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                        [q(e(v.x)), q(e(v.y)), q(e(v.z)), q(v.w)]
                    })
                    .collect(),
            ),
            Texels::F32(_) => Texels::F32(acc.iter().map(|v| v.to_array()).collect()),
        };
        Level { w, h, texels }
    }

    fn bytes(&self) -> usize {
        (self.w * self.h) as usize * if matches!(self.texels, Texels::U8(_)) { 4 } else { 16 }
    }
}

impl Image {
    fn from_level(base: Level, srgb: bool, has_alpha: bool, budget_left: Option<usize>) -> Image {
        let (width, height) = (base.w, base.h);
        let mut levels = vec![base];
        while levels.last().is_some_and(|l| l.w > 1 || l.h > 1) {
            let next = levels.last().unwrap().half(srgb);
            levels.push(next);
        }
        // Over the memory limit: drop finest levels while the image stays at least 256 across.
        if let Some(left) = budget_left {
            while levels.len() > 1 && levels.iter().map(Level::bytes).sum::<usize>() > left && levels[0].w.max(levels[0].h) > 256 {
                levels.remove(0);
            }
        }
        let bytes = levels.iter().map(Level::bytes).sum();
        Image { levels, srgb, has_alpha, width, height, bytes }
    }

    /// Filtered lookup: `width` is the footprint in UV units, picking the level.
    pub fn sample(&self, uv: Vec2, width: f32) -> Vec4 {
        let size = self.width.max(self.height) as f32;
        let lod = (width.max(1e-8) * size).log2().max(0.0);
        // Levels dropped for memory shift the index.
        let shift = (size / self.levels[0].w.max(self.levels[0].h) as f32).log2().max(0.0);
        let l = (lod - shift).max(0.0);
        let i = (l.floor() as usize).min(self.levels.len() - 1);
        let f = l - l.floor();
        let a = self.levels[i].bilinear(uv, self.srgb);
        if f < 1e-3 || i + 1 >= self.levels.len() {
            return a;
        }
        a.lerp(self.levels[i + 1].bilinear(uv, self.srgb), f)
    }
}

/// Reads an image file into its finest level. Eight bit images stay eight bit; others become floats.
fn read_level(path: &Path) -> Result<(Level, bool, bool), String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext == "exr" {
        let img = exr::prelude::read_first_rgba_layer_from_file(
            path,
            |res, _| {
                let (w, h) = (res.width(), res.height());
                (w, h, vec![[0.0f32, 0.0, 0.0, 1.0]; w * h])
            },
            |(w, _h, px), pos, (r, g, b, a): (f32, f32, f32, f32)| {
                px[pos.y() * *w + pos.x()] = [r, g, b, a];
            },
        )
        .map_err(|e| e.to_string())?;
        let (w, h, px) = img.layer_data.channel_data.pixels;
        let has_alpha = px.iter().any(|p| p[3] < 1.0);
        return Ok((Level { w: w as u32, h: h as u32, texels: Texels::F32(px) }, true, has_alpha));
    }
    let reader = image::ImageReader::open(path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    let img = reader.decode().map_err(|e| e.to_string())?;
    let declares_alpha = img.color().has_alpha();
    let float = !matches!(img, image::DynamicImage::ImageRgb8(_) | image::DynamicImage::ImageRgba8(_) | image::DynamicImage::ImageLuma8(_) | image::DynamicImage::ImageLumaA8(_));
    let (w, h) = (img.width(), img.height());
    if float {
        let px: Vec<[f32; 4]> = img.into_rgba32f().pixels().map(|p| p.0).collect();
        let has_alpha = declares_alpha && px.iter().any(|p| p[3] < 1.0);
        Ok((Level { w, h, texels: Texels::F32(px) }, true, has_alpha))
    } else {
        let px: Vec<[u8; 4]> = img.into_rgba8().pixels().map(|p| p.0).collect();
        let has_alpha = declares_alpha && px.iter().any(|p| p[3] != 255);
        Ok((Level { w, h, texels: Texels::U8(px) }, false, has_alpha))
    }
}

/// Which tokens a texture path carries.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TilePattern {
    None,
    /// `<udim>`: 1001 + u + 10 v.
    Udim,
    /// `<uvtile>`: `u1_v1` for the first tile.
    UvTile,
}

/// A texture as a material names it: a path, maybe with tile tokens, and a colour space.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextureKey {
    pub path: String,
    pub space: ColorSpace,
}

impl TextureKey {
    pub fn pattern(&self) -> TilePattern {
        let l = self.path.to_ascii_lowercase();
        if l.contains("<udim>") {
            TilePattern::Udim
        } else if l.contains("<uvtile>") {
            TilePattern::UvTile
        } else {
            TilePattern::None
        }
    }

    /// The file for the tile a UV falls in.
    pub fn tile_path(&self, uv: Vec2) -> String {
        let (tu, tv) = (uv.x.floor() as i32, uv.y.floor() as i32);
        let replace = |s: &str, token: &str, by: &str| {
            let low = s.to_ascii_lowercase();
            match low.find(token) {
                Some(i) => format!("{}{}{}", &s[..i], by, &s[i + token.len()..]),
                None => s.to_string(),
            }
        };
        match self.pattern() {
            TilePattern::None => self.path.clone(),
            TilePattern::Udim => replace(&self.path, "<udim>", &(1001 + tu.clamp(0, 9) + 10 * tv.max(0)).to_string()),
            TilePattern::UvTile => replace(&self.path, "<uvtile>", &format!("u{}_v{}", tu + 1, tv + 1)),
        }
    }
}

type Slot = Arc<OnceLock<Option<Arc<Image>>>>;

#[derive(Default)]
pub struct TextureCache {
    slots: RwLock<HashMap<(String, ColorSpace), Slot>>,
    pub bytes: AtomicUsize,
    /// Megabytes; 0 is no limit.
    pub budget_mb: AtomicUsize,
    pub warnings: Mutex<Vec<String>>,
    pub loaded: AtomicUsize,
}

impl std::fmt::Debug for TextureCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TextureCache({} bytes)", self.bytes.load(Ordering::Relaxed))
    }
}

impl TextureCache {
    pub fn new(budget_mb: usize) -> TextureCache {
        let c = TextureCache::default();
        c.budget_mb.store(budget_mb, Ordering::Relaxed);
        c
    }

    fn slot(&self, path: &str, space: ColorSpace) -> Slot {
        let key = (path.to_string(), space);
        if let Some(s) = self.slots.read().unwrap().get(&key) {
            return s.clone();
        }
        self.slots.write().unwrap().entry(key).or_default().clone()
    }

    /// The image of a file, loaded at the first ask. None for a file that won't read, warned once.
    pub fn image(&self, path: &str, space: ColorSpace) -> Option<Arc<Image>> {
        let slot = self.slot(path, space);
        slot.get_or_init(|| match read_level(Path::new(path)) {
            Ok((level, float, has_alpha)) => {
                let srgb = space == ColorSpace::Srgb && !float;
                // Float images in sRGB space are decoded at load.
                let level = if space == ColorSpace::Srgb && float {
                    match level.texels {
                        Texels::F32(px) => Level { w: level.w, h: level.h, texels: Texels::F32(px.into_iter().map(|p| [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2]), p[3]]).collect()) },
                        t => Level { w: level.w, h: level.h, texels: t },
                    }
                } else {
                    level
                };
                let budget = self.budget_mb.load(Ordering::Relaxed) * 1024 * 1024;
                let left = if budget == 0 { None } else { Some(budget.saturating_sub(self.bytes.load(Ordering::Relaxed))) };
                let img = Image::from_level(level, srgb, has_alpha, left);
                if left.is_some() && img.levels[0].w < img.width {
                    self.warnings.lock().unwrap().push(format!("{path}: texture memory limit reached, read at {} wide", img.levels[0].w));
                }
                self.bytes.fetch_add(img.bytes, Ordering::Relaxed);
                self.loaded.fetch_add(1, Ordering::Relaxed);
                Some(Arc::new(img))
            }
            Err(e) => {
                self.warnings.lock().unwrap().push(format!("{path}: {e}"));
                None
            }
        })
        .clone()
    }

    /// A filtered lookup through a texture key's tiles. None where no image reads.
    pub fn sample(&self, key: &TextureKey, uv: Vec2, width: f32) -> Option<Vec4> {
        let path = key.tile_path(uv);
        let img = self.image(&path, key.space)?;
        let local = if key.pattern() == TilePattern::None { uv } else { uv - uv.floor() };
        Some(img.sample(local, width))
    }

    pub fn take_warnings(&self) -> Vec<String> {
        std::mem::take(&mut *self.warnings.lock().unwrap())
    }

    /// Forget every image: files saved over are read again.
    pub fn clear(&self) {
        self.slots.write().unwrap().clear();
        self.bytes.store(0, Ordering::Relaxed);
    }
}

/// A texture as a material holds it: its image looked up in the cache once, and per tile for UDIMs, so sampling takes
/// no lock and makes no path.
#[derive(Debug)]
pub struct TexHandle {
    pub key: TextureKey,
    pattern: TilePattern,
    cache: Arc<TextureCache>,
    single: OnceLock<Option<Arc<Image>>>,
    tiles: RwLock<HashMap<(i32, i32), Option<Arc<Image>>>>,
}

impl TexHandle {
    pub fn new(key: TextureKey, cache: Arc<TextureCache>) -> TexHandle {
        TexHandle { pattern: key.pattern(), key, cache, single: OnceLock::new(), tiles: RwLock::new(HashMap::new()) }
    }

    pub fn tiled(&self) -> bool {
        self.pattern != TilePattern::None
    }

    pub fn sample(&self, uv: Vec2, width: f32) -> Option<Vec4> {
        if self.pattern == TilePattern::None {
            let img = self.single.get_or_init(|| self.cache.image(&self.key.path, self.key.space));
            return img.as_ref().map(|i| i.sample(uv, width));
        }
        let tile = (uv.x.floor() as i32, uv.y.floor() as i32);
        let local = uv - uv.floor();
        if let Some(img) = self.tiles.read().unwrap().get(&tile) {
            return img.as_ref().map(|i| i.sample(local, width));
        }
        let img = self.cache.image(&self.key.tile_path(uv), self.key.space);
        let out = img.as_ref().map(|i| i.sample(local, width));
        self.tiles.write().unwrap().insert(tile, img);
        out
    }
}

/// A path that exists for a tile pattern: the first tile found, for checking a texture is there at all.
pub fn any_tile(key: &TextureKey) -> Option<PathBuf> {
    match key.pattern() {
        TilePattern::None => Some(PathBuf::from(&key.path)).filter(|p| p.exists()),
        _ => (0..100).map(|k| key.tile_path(Vec2::new((k % 10) as f32 + 0.5, (k / 10) as f32 + 0.5))).map(PathBuf::from).find(|p| p.exists()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udim_tiles_name_files() {
        let k = TextureKey { path: "/t/skin.<UDIM>.png".into(), space: ColorSpace::Srgb };
        assert_eq!(k.tile_path(Vec2::new(0.5, 0.5)), "/t/skin.1001.png");
        assert_eq!(k.tile_path(Vec2::new(1.5, 2.5)), "/t/skin.1022.png");
        let k = TextureKey { path: "/t/a_<uvtile>.tif".into(), space: ColorSpace::Raw };
        assert_eq!(k.tile_path(Vec2::new(1.5, 0.2)), "/t/a_u2_v1.tif");
    }

    #[test]
    fn mips_keep_the_mean() {
        let px: Vec<[u8; 4]> = (0..64 * 64).map(|i| if (i % 64 + i / 64) % 2 == 0 { [255, 255, 255, 255] } else { [0, 0, 0, 255] }).collect();
        let img = Image::from_level(Level { w: 64, h: 64, texels: Texels::U8(px) }, false, false, None);
        let fine = img.sample(Vec2::new(0.5 / 64.0, 1.0 - 0.5 / 64.0), 0.0).x;
        assert!((fine - 1.0).abs() < 1e-3);
        let coarse = img.sample(Vec2::splat(0.5), 1.0).x;
        assert!((coarse - 0.5).abs() < 0.01, "{coarse}");
    }

    #[test]
    fn color_space_names() {
        assert_eq!(ColorSpace::parse("sRGB"), ColorSpace::Srgb);
        assert_eq!(ColorSpace::parse("Raw"), ColorSpace::Raw);
        assert_eq!(ColorSpace::parse("scene-linear Rec.709-sRGB"), ColorSpace::Linear);
        assert_eq!(ColorSpace::parse("Utility - Raw"), ColorSpace::Raw);
    }
}
