//! Running a render: buckets over a thread pool of its own, so the caller's threads and the interface stay free.
//! Progressive: a coarse preview first, then one camera sample per pixel per pass, each pass refining the whole image,
//! until every pixel has its AA samples squared, or with adaptive sampling until its noise is under the threshold.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use glam::{Vec2, Vec4};
use rayon::prelude::*;

use crate::film::{Film, FilterTable};
use crate::first_hit::SampleOut;
use crate::integrator::Tracer;
use crate::math::Sampler;
use crate::scene::Scene;

#[derive(Clone, Debug, Default)]
pub struct Status {
    /// Passes finished, of how many.
    pub pass: u32,
    pub passes: u32,
    pub done: bool,
    pub cancelled: bool,
    pub rays: u64,
    pub samples: u64,
    pub elapsed: Duration,
    /// Rays per second over the render so far.
    pub rays_per_second: f64,
    pub memory_bytes: usize,
    /// A coarse preview is all there is yet.
    pub preview: bool,
    pub warnings: Vec<String>,
}

struct Shared {
    scene: Arc<Scene>,
    film: Mutex<Film>,
    /// The coarse image shown before the first full pass: beauty only.
    preview: Mutex<Option<Vec<Vec4>>>,
    cancel: AtomicBool,
    rays: AtomicU64,
    samples: AtomicU64,
    status: Mutex<Status>,
    started: Instant,
}

pub struct Render {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone, Copy, Debug)]
pub struct JobOptions {
    /// Show a coarse image before the first pass.
    pub preview: bool,
    /// Threads; 0 for all cores but one, so the interface keeps a core.
    pub threads: usize,
}

impl Default for JobOptions {
    fn default() -> Self {
        JobOptions { preview: true, threads: 0 }
    }
}

fn pixel_range(o: &crate::scene::Options) -> [u32; 4] {
    match o.region {
        Some([x0, y0, x1, y1]) => [x0.min(o.width), y0.min(o.height), x1.min(o.width).max(x0), y1.min(o.height).max(y0)],
        None => [0, 0, o.width, o.height],
    }
}

fn tiles(o: &crate::scene::Options) -> Vec<[u32; 4]> {
    let [x0, y0, x1, y1] = pixel_range(o);
    let b = o.bucket_size.clamp(8, 32);
    let mut out = Vec::new();
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            out.push([x, y, (x + b).min(x1), (y + b).min(y1)]);
            x += b;
        }
        y += b;
    }
    // Centre first, as buckets spiral out in other renderers: what is looked at arrives first.
    let (cx, cy) = ((x0 + x1) as f32 * 0.5, (y0 + y1) as f32 * 0.5);
    out.sort_by(|a, b| {
        let d = |t: &[u32; 4]| ((t[0] + t[2]) as f32 * 0.5 - cx).powi(2) + ((t[1] + t[3]) as f32 * 0.5 - cy).powi(2);
        d(a).total_cmp(&d(b))
    });
    out
}

/// The number of camera samples each pixel gets without adaptive sampling.
pub fn base_passes(o: &crate::scene::Options) -> u32 {
    (o.aa_samples * o.aa_samples).max(1)
}

fn max_passes(o: &crate::scene::Options) -> u32 {
    if o.adaptive {
        (o.aa_samples_max * o.aa_samples_max).max(base_passes(o))
    } else {
        base_passes(o)
    }
}

impl Shared {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn pixel_sample(&self, tr: &Tracer, filter: &FilterTable, x: u32, y: u32, index: u32) -> (SampleOut, f32, f32) {
        let s = &self.scene;
        let o = &s.options;
        let mut sampler = Sampler::new(x, y, index, o.seed);
        let u = sampler.next_2d();
        let (dx, sx) = filter.sample(u.x);
        let (dy, sy) = filter.sample(u.y);
        let ndc = Vec2::new((x as f32 + 0.5 + dx) / o.width as f32, (y as f32 + 0.5 + dy) / o.height as f32);
        let out = tr.camera_sample(ndc, &mut sampler);
        (out, sx * sy, (dx * dx + dy * dy).sqrt())
    }

    /// One pass over the image: every unfinished pixel in the region gets one more camera sample.
    fn pass(&self, filter: &FilterTable, list: &[[u32; 4]], index: u32, adaptive: bool) {
        self.passes(filter, list, index..index + 1, adaptive)
    }

    /// Several samples per pixel, each bucket taking them all before the next: no waiting between passes.
    fn passes(&self, filter: &FilterTable, list: &[[u32; 4]], indices: std::ops::Range<u32>, adaptive: bool) {
        let s = &self.scene;
        let w = s.options.width;
        list.par_iter().for_each(|t| {
            if self.cancelled() {
                return;
            }
            let tr = Tracer::new(s);
            let mut local: Vec<(usize, SampleOut, f32, f32)> = Vec::with_capacity(((t[2] - t[0]) * (t[3] - t[1])) as usize);
            {
                let film = self.film.lock().unwrap();
                for y in t[1]..t[3] {
                    for x in t[0]..t[2] {
                        let i = (y * w + x) as usize;
                        if adaptive && film.done[i] {
                            continue;
                        }
                        local.push((i, SampleOut::default(), 0.0, 0.0));
                    }
                }
            }
            for index in indices.clone() {
                for item in &mut local {
                    if self.cancelled() {
                        return;
                    }
                    let (x, y) = ((item.0 as u32) % w, (item.0 as u32) / w);
                    let (out, wt, d) = self.pixel_sample(&tr, filter, x, y, index);
                    item.1 = out;
                    item.2 = wt;
                    item.3 = d;
                }
                let mut film = self.film.lock().unwrap();
                for (i, out, wt, d) in &local {
                    film.add(*i, out, *wt, *d);
                }
                drop(film);
                self.samples.fetch_add(local.len() as u64, Ordering::Relaxed);
            }
            self.rays.fetch_add(tr.rays.get(), Ordering::Relaxed);
        });
    }

    /// A coarse image: one sample per block of pixels, filling the block.
    fn preview_pass(&self, block: u32) {
        let s = &self.scene;
        let o = &s.options;
        let [x0, y0, x1, y1] = pixel_range(o);
        let filter = FilterTable::new(crate::scene::FilterKind::Box, 1.0);
        let rows: Vec<u32> = (y0..y1).step_by(block as usize).collect();
        let w = o.width;
        let blocks: Vec<Vec<(u32, u32, Vec4)>> = rows
            .par_iter()
            .map(|&y| {
                let tr = Tracer::new(s);
                let mut v = Vec::new();
                for x in (x0..x1).step_by(block as usize) {
                    if self.cancelled() {
                        break;
                    }
                    let cx = (x + block / 2).min(x1 - 1);
                    let cy = (y + block / 2).min(y1 - 1);
                    let (out, _, _) = self.pixel_sample(&tr, &filter, cx, cy, 0);
                    v.push((x, y, out.color.extend(out.alpha)));
                }
                self.rays.fetch_add(tr.rays.get(), Ordering::Relaxed);
                v
            })
            .collect();
        let mut img = vec![Vec4::ZERO; (o.width * o.height) as usize];
        for row in blocks {
            for (x, y, c) in row {
                for yy in y..(y + block).min(y1) {
                    for xx in x..(x + block).min(x1) {
                        img[(yy * w + xx) as usize] = c;
                    }
                }
            }
        }
        *self.preview.lock().unwrap() = Some(img);
    }

    fn update_status(&self, pass: u32, passes: u32, done: bool, preview: bool) {
        let mut st = self.status.lock().unwrap();
        st.pass = pass;
        st.passes = passes;
        st.done = done;
        st.preview = preview;
        st.cancelled = self.cancelled();
        st.rays = self.rays.load(Ordering::Relaxed);
        st.samples = self.samples.load(Ordering::Relaxed);
        st.elapsed = self.started.elapsed();
        st.rays_per_second = st.rays as f64 / st.elapsed.as_secs_f64().max(1e-6);
        st.memory_bytes = self.scene.memory_bytes() + {
            let f = self.film.lock().unwrap();
            f.sums.len() * f.weights.len() * 16 + f.weights.len() * 24
        };
        let mut w = self.scene.textures.take_warnings();
        st.warnings.append(&mut w);
    }

    fn run(&self, job: JobOptions) {
        let o = &self.scene.options;
        let filter = FilterTable::new(o.filter, o.filter_width);
        let list = tiles(o);
        let base = base_passes(o);
        let max = max_passes(o);
        if job.preview {
            for block in [16u32, 4] {
                if self.cancelled() {
                    break;
                }
                self.preview_pass(block);
                self.update_status(0, base, false, true);
            }
        }
        let mut pass = 0;
        if !job.preview {
            // Batch: every bucket takes all its base samples in one go.
            self.passes(&filter, &list, 0..base, false);
            pass = base;
            if !self.cancelled() {
                self.update_status(pass, base, false, false);
            }
        }
        while pass < max && !self.cancelled() {
            let adaptive = o.adaptive && pass >= base;
            self.pass(&filter, &list, pass, adaptive);
            if self.cancelled() {
                break;
            }
            pass += 1;
            if pass == 1 {
                *self.preview.lock().unwrap() = None;
            }
            if o.adaptive && pass >= base {
                // Pixels whose neighbourhood is under the threshold stop.
                let mut film = self.film.lock().unwrap();
                let (w, h) = (film.width as i64, film.height as i64);
                let err: Vec<f32> = (0..film.counts.len()).map(|i| film.error(i)).collect();
                let mut remaining = 0;
                for i in 0..film.counts.len() {
                    let (x, y) = (i as i64 % w, i as i64 / w);
                    let mut e = 0.0f32;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let (xx, yy) = (x + dx, y + dy);
                            if xx >= 0 && yy >= 0 && xx < w && yy < h {
                                e = e.max(err[(yy * w + xx) as usize]);
                            }
                        }
                    }
                    film.done[i] = e < o.adaptive_threshold;
                    if !film.done[i] {
                        remaining += 1;
                    }
                }
                drop(film);
                if remaining == 0 {
                    self.update_status(pass, pass, false, false);
                    break;
                }
            }
            self.update_status(pass, if o.adaptive && pass >= base { max } else { base }, false, false);
        }
        let p = self.status.lock().unwrap().pass;
        self.update_status(p, p.max(base), true, false);
    }
}

impl Render {
    pub fn start(scene: Arc<Scene>, job: JobOptions) -> Render {
        let (w, h) = (scene.options.width, scene.options.height);
        let film = Film::new(w, h, &scene.options.aovs, &scene.options.light_groups);
        let shared = Arc::new(Shared { scene, film: Mutex::new(film), preview: Mutex::new(None), cancel: AtomicBool::new(false), rays: AtomicU64::new(0), samples: AtomicU64::new(0), status: Mutex::new(Status::default()), started: Instant::now() });
        let threads = if job.threads > 0 { job.threads } else { std::thread::available_parallelism().map_or(2, |n| n.get()).saturating_sub(1).max(1) };
        let sh = shared.clone();
        let thread = std::thread::Builder::new()
            .name("harold".into())
            .spawn(move || {
                let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).thread_name(|i| format!("harold-{i}")).build();
                match pool {
                    Ok(p) => p.install(|| sh.run(job)),
                    Err(_) => sh.run(job),
                }
            })
            .expect("render thread");
        Render { shared, thread: Some(thread) }
    }

    /// Stops at the next pixel and waits for the threads.
    pub fn cancel(&mut self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
        self.wait();
    }

    pub fn wait(&mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    pub fn status(&self) -> Status {
        self.shared.status.lock().unwrap().clone()
    }

    pub fn is_done(&self) -> bool {
        self.status().done
    }

    pub fn scene(&self) -> &Arc<Scene> {
        &self.shared.scene
    }

    /// An AOV as it stands, normalised; the coarse preview stands in for the beauty before the first pass.
    pub fn image(&self, aov: &str) -> Option<(u32, u32, Vec<Vec4>)> {
        if aov == "RGBA" || aov == "beauty" {
            if let Some(p) = self.shared.preview.lock().unwrap().as_ref() {
                let o = &self.shared.scene.options;
                return Some((o.width, o.height, p.clone()));
            }
        }
        let film = self.shared.film.lock().unwrap();
        let i = film.index_of(aov)?;
        Some((film.width, film.height, film.pixels(i)))
    }

    pub fn aov_names(&self) -> Vec<String> {
        self.shared.film.lock().unwrap().aovs.iter().map(|a| a.0.clone()).collect()
    }

    pub fn film(&self) -> Film {
        self.shared.film.lock().unwrap().clone()
    }
}

impl Drop for Render {
    fn drop(&mut self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
        self.wait();
    }
}

/// Renders to the end on the calling thread's behalf, for batch rendering and tests.
pub fn render(scene: Arc<Scene>, threads: usize) -> (Film, Status) {
    let mut r = Render::start(scene, JobOptions { preview: false, threads: if threads == 0 { std::thread::available_parallelism().map_or(2, |n| n.get()) } else { threads } });
    r.wait();
    (r.film(), r.status())
}
